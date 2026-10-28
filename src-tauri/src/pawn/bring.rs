//! Bring it through (RS-2's second exception, BUILD-SPEC § v0.7): at the
//! player's press, one built artifact from the workshop's `dist\` lands in
//! the mods folder, by the digest the window showed when the card opened.
//!
//! The press refers to the SHA-256 [`plan_bring_through`] froze. On the press
//! the stone copies the file to `<mods>\.riftstone-<id>.part`, hashes the
//! copy, and gives it its name only when that hash is the frozen digest, and
//! never over an existing file (the download landing's own rules,
//! [`crate::landing`]). Identical bytes already under that name answer
//! `alreadyHad` and add nothing. A different file under that name refuses
//! `nameTaken`. Changed bytes refuse `bytesChanged`. The workshop copy is
//! only ever read.
//!
//! Tamsin has no tool that reaches this: only the window's press does.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::landing;
use crate::pawn::folder::MailFolder;

/// The receipt's own size cap.
pub const MAX_RECEIPT_BYTES: u64 = 64 * 1024;

/// Why a bring-through was refused (WINDOW-CONTRACT § v0.7, `bring`).
pub use rift_core::BringReason;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BringRefusal {
    pub reason: BringReason,
    /// One sentence, in the stone's voice. Workshop names are the
    /// investor's own and may be shown.
    pub detail: String,
}

impl From<BringRefusal> for rift_core::RiftError {
    fn from(refusal: BringRefusal) -> Self {
        rift_core::RiftError::bring(refusal.reason, refusal.detail)
    }
}

fn refuse(reason: BringReason, detail: impl Into<String>) -> BringRefusal {
    BringRefusal {
        reason,
        detail: detail.into(),
    }
}

/// `<name>.receipt.json`, written by Tamsin, read by the stone and never
/// trusted: passed through for display under THE RECEIPT SAYS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub artifact: String,
    pub sha256: String,
    pub built_at: String,
    #[serde(default)]
    pub inputs: Vec<ReceiptInput>,
    #[serde(default)]
    pub checks: Vec<ReceiptCheck>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptInput {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptCheck {
    pub name: String,
    pub passed: u64,
    pub total: u64,
}

/// Is a name already in the mods folder, and is it these bytes?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Conflict {
    None,
    AlreadyHad,
    NameTaken,
}

/// What the card shows, with the digest frozen when it opened.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BringPlan {
    pub artifact: String,
    pub bytes: u64,
    pub sha256: String,
    pub lands_as: String,
    pub receipt: Receipt,
    pub conflict: Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BringOutcome {
    Landed,
    AlreadyHad,
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 and length of a file in the mods folder (the player's own), streamed.
pub fn digest_file(path: &Path) -> io::Result<(String, u64)> {
    digest(&mut File::open(path)?)
}

/// SHA-256 and length of what a reader holds, streamed.
fn digest(file: &mut impl Read) -> io::Result<(String, u64)> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok((hex(hash.finalize()), bytes))
}

/// The workshop's `dist\`, pinned by handle: a `dist` that is a link or
/// reparse point is refused, so nothing outside the workshop is ever read as
/// a build (the Heretic's WB3). The workshop itself is the stone's own
/// folder and the fence's mount point, which Tamsin cannot replace.
fn dist(workshop: &Path) -> Result<MailFolder, BringRefusal> {
    MailFolder::open(&workshop.join("dist")).map_err(|_| {
        refuse(
            BringReason::NotInDist,
            "The workshop's dist folder is missing or is not a plain folder.",
        )
    })
}

/// The artifact in `dist\`, opened by the pinned handle: a plain `.zip`
/// name, a regular file, not a link. Every read of it goes through this one
/// open file.
fn open_artifact(dist: &MailFolder, artifact: &str) -> Result<File, BringRefusal> {
    let plain = !artifact.is_empty()
        && !artifact.contains(['/', '\\', ':'])
        && artifact != "."
        && artifact != ".."
        && !artifact.starts_with('.')
        && artifact.to_ascii_lowercase().ends_with(".zip")
        && !artifact.chars().any(char::is_control);
    let not_in_dist = || {
        refuse(
            BringReason::NotInDist,
            format!("{artifact} is not a built .zip in the workshop's dist folder."),
        )
    };
    if !plain {
        return Err(not_in_dist());
    }
    dist.open_file(artifact).map_err(|_| not_in_dist())
}

fn receipt_name(artifact: &str) -> String {
    let stem = &artifact[..artifact.len() - ".zip".len()];
    format!("{stem}.receipt.json")
}

/// Read the receipt beside the artifact, by the pinned handle, stopping at
/// 64 KiB + 1 byte: missing, linked, unreadable or too large is `noReceipt`.
fn read_receipt(dist: &MailFolder, artifact: &str) -> Result<Receipt, BringRefusal> {
    let missing = || {
        refuse(
            BringReason::NoReceipt,
            format!(
                "{artifact} has no readable receipt beside it ({}). Ask Tamsin to write one.",
                receipt_name(artifact)
            ),
        )
    };
    let file = dist
        .open_file(&receipt_name(artifact))
        .map_err(|_| missing())?;
    let mut bytes = Vec::new();
    file.take(MAX_RECEIPT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| missing())?;
    if bytes.len() as u64 > MAX_RECEIPT_BYTES {
        return Err(missing());
    }
    let receipt: Receipt = serde_json::from_slice(&bytes).map_err(|_| missing())?;
    if receipt.artifact != artifact {
        return Err(refuse(
            BringReason::ReceiptMismatch,
            format!("The receipt beside {artifact} names another artifact."),
        ));
    }
    Ok(receipt)
}

/// Is `name` in the mods folder (compared as Windows compares names), and
/// does it hold `sha256`?
fn conflict(mods: &Path, name: &str, sha256: &str) -> io::Result<Conflict> {
    match landing::occupant(mods, name) {
        None => Ok(Conflict::None),
        Some((_, path)) => {
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_file() && digest_file(&path)?.0 == sha256 {
                Ok(Conflict::AlreadyHad)
            } else {
                Ok(Conflict::NameTaken)
            }
        }
    }
}

fn io_refusal(err: io::Error) -> BringRefusal {
    refuse(
        BringReason::NotInDist,
        format!("The stone could not read the workshop's build: {err}."),
    )
}

/// Open the card: hash the artifact now, check its receipt names these
/// bytes, and say what a press would do. The digest returned is the one the
/// press must name.
pub fn plan_bring_through(
    workshop: &Path,
    mods: &Path,
    artifact: &str,
) -> Result<BringPlan, BringRefusal> {
    let dist = dist(workshop)?;
    let mut file = open_artifact(&dist, artifact)?;
    let receipt = read_receipt(&dist, artifact)?;
    let (sha256, bytes) = digest(&mut file).map_err(io_refusal)?;
    if receipt.sha256 != sha256 {
        return Err(refuse(
            BringReason::ReceiptMismatch,
            format!("The receipt beside {artifact} names other bytes than the file holds."),
        ));
    }
    let conflict = conflict(mods, artifact, &sha256).map_err(io_refusal)?;
    Ok(BringPlan {
        artifact: artifact.to_string(),
        bytes,
        sha256,
        lands_as: artifact.to_string(),
        receipt,
        conflict,
    })
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(16)]
}

/// The press: land the artifact only as the bytes `sha256` names. `part_id`
/// makes the partial's name (`.riftstone-<id>.part`, which the scan and the
/// watcher already skip).
pub fn bring_through(
    workshop: &Path,
    mods: &Path,
    artifact: &str,
    sha256: &str,
    part_id: &str,
) -> Result<BringOutcome, BringRefusal> {
    let dist = dist(workshop)?;
    let mut source = open_artifact(&dist, artifact)?;
    let taken = || {
        refuse(
            BringReason::NameTaken,
            format!("A different {artifact} is already in the mods folder. Rename the build in the workshop and bring it through again."),
        )
    };
    match conflict(mods, artifact, sha256).map_err(io_refusal)? {
        Conflict::AlreadyHad => return Ok(BringOutcome::AlreadyHad),
        Conflict::NameTaken => return Err(taken()),
        Conflict::None => {}
    }
    let part = mods.join(crate::downloads::part_name(part_id));
    let copied = (|| -> io::Result<()> {
        let mut target = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part)?;
        io::copy(&mut source, &mut target)?;
        target.flush()?;
        target.sync_all()
    })();
    if let Err(err) = copied {
        let _ = fs::remove_file(&part);
        return Err(io_refusal(err));
    }
    // The copy is what lands, so the copy is what is checked.
    let landed_sha = match digest_file(&part) {
        Ok((sha, _)) => sha,
        Err(err) => {
            let _ = fs::remove_file(&part);
            return Err(io_refusal(err));
        }
    };
    if landed_sha != sha256 {
        let _ = fs::remove_file(&part);
        return Err(refuse(
            BringReason::BytesChanged,
            format!(
                "{artifact} changed since its card opened (shown {}, now {}). Open it again to see the new build.",
                short(sha256),
                short(&landed_sha)
            ),
        ));
    }
    match landing::rename_new(&part, &mods.join(artifact)) {
        Ok(()) => Ok(BringOutcome::Landed),
        Err(err) => {
            let _ = fs::remove_file(&part);
            if err.kind() == io::ErrorKind::AlreadyExists {
                // Something took the name between the check and the landing.
                return match conflict(mods, artifact, sha256).map_err(io_refusal)? {
                    Conflict::AlreadyHad => Ok(BringOutcome::AlreadyHad),
                    _ => Err(taken()),
                };
            }
            Err(io_refusal(err))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Bench {
        _root: tempfile::TempDir,
        workshop: PathBuf,
        mods: PathBuf,
    }

    fn bench() -> Bench {
        let root = tempfile::tempdir().unwrap();
        let mods = root.path().join("mods");
        let workshop = root.path().join("mods (workshop)");
        fs::create_dir_all(workshop.join("dist")).unwrap();
        fs::create_dir_all(&mods).unwrap();
        Bench {
            _root: root,
            workshop,
            mods,
        }
    }

    fn sha(bytes: &[u8]) -> String {
        hex(Sha256::digest(bytes))
    }

    fn build(bench: &Bench, name: &str, bytes: &[u8]) {
        fs::write(bench.workshop.join("dist").join(name), bytes).unwrap();
        let receipt = serde_json::json!({
            "artifact": name,
            "sha256": sha(bytes),
            "builtAt": "2026-10-28T12:00:00Z",
            "inputs": [{ "path": "/inputs/0/natives/x.mesh", "sha256": "00".repeat(32) }],
            "checks": [{ "name": "lua", "passed": 12, "total": 12 }]
        });
        fs::write(
            bench.workshop.join("dist").join(receipt_name(name)),
            serde_json::to_vec(&receipt).unwrap(),
        )
        .unwrap();
    }

    fn mods_names(bench: &Bench) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&bench.mods)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn it_lands_only_the_frozen_digest_and_leaves_the_workshop_copy() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        let plan = plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").unwrap();
        assert_eq!(plan.sha256, sha(b"stature build"));
        assert_eq!(plan.conflict, Conflict::None);
        assert_eq!(plan.receipt.checks[0].passed, 12);
        let outcome = bring_through(
            &bench.workshop,
            &bench.mods,
            "Stature-0.2.0.zip",
            &plan.sha256,
            "b1",
        )
        .unwrap();
        assert_eq!(outcome, BringOutcome::Landed);
        assert_eq!(
            mods_names(&bench),
            ["Stature-0.2.0.zip"],
            "no partial is left"
        );
        assert_eq!(
            fs::read(bench.mods.join("Stature-0.2.0.zip")).unwrap(),
            b"stature build"
        );
        assert_eq!(
            fs::read(bench.workshop.join("dist/Stature-0.2.0.zip")).unwrap(),
            b"stature build",
            "the workshop copy is never moved or changed"
        );
    }

    #[test]
    fn identical_bytes_already_there_add_nothing() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        fs::write(bench.mods.join("stature-0.2.0.ZIP"), b"stature build").unwrap();
        let plan = plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").unwrap();
        assert_eq!(plan.conflict, Conflict::AlreadyHad);
        let outcome = bring_through(
            &bench.workshop,
            &bench.mods,
            "Stature-0.2.0.zip",
            &plan.sha256,
            "b2",
        )
        .unwrap();
        assert_eq!(outcome, BringOutcome::AlreadyHad);
        assert_eq!(mods_names(&bench), ["stature-0.2.0.ZIP"]);
    }

    #[test]
    fn a_different_file_under_the_name_refuses_and_changes_nothing() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        fs::write(
            bench.mods.join("Stature-0.2.0.zip"),
            b"the player's own file",
        )
        .unwrap();
        let plan = plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").unwrap();
        assert_eq!(plan.conflict, Conflict::NameTaken);
        let refusal = bring_through(
            &bench.workshop,
            &bench.mods,
            "Stature-0.2.0.zip",
            &plan.sha256,
            "b3",
        )
        .unwrap_err();
        assert_eq!(refusal.reason, BringReason::NameTaken);
        assert_eq!(mods_names(&bench), ["Stature-0.2.0.zip"]);
        assert_eq!(
            fs::read(bench.mods.join("Stature-0.2.0.zip")).unwrap(),
            b"the player's own file"
        );
    }

    #[test]
    fn bytes_changed_since_the_card_opened_refuse_and_change_nothing() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        let plan = plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").unwrap();
        // Tamsin rebuilds between the card and the press.
        fs::write(
            bench.workshop.join("dist/Stature-0.2.0.zip"),
            b"stature rebuilt",
        )
        .unwrap();
        let refusal = bring_through(
            &bench.workshop,
            &bench.mods,
            "Stature-0.2.0.zip",
            &plan.sha256,
            "b4",
        )
        .unwrap_err();
        assert_eq!(refusal.reason, BringReason::BytesChanged);
        assert!(refusal.detail.contains(&plan.sha256[..16]));
        assert!(refusal.detail.contains(&sha(b"stature rebuilt")[..16]));
        assert!(
            mods_names(&bench).is_empty(),
            "nothing landed, no partial left"
        );
    }

    #[test]
    fn a_receipt_that_names_other_bytes_or_is_missing_closes_the_card() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        fs::write(
            bench.workshop.join("dist/Stature-0.2.0.zip"),
            b"other bytes",
        )
        .unwrap();
        let refusal =
            plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").unwrap_err();
        assert_eq!(refusal.reason, BringReason::ReceiptMismatch);

        fs::write(bench.workshop.join("dist/Bare.zip"), b"bare").unwrap();
        let refusal = plan_bring_through(&bench.workshop, &bench.mods, "Bare.zip").unwrap_err();
        assert_eq!(refusal.reason, BringReason::NoReceipt);

        fs::write(bench.workshop.join("dist/Odd.zip"), b"odd").unwrap();
        fs::write(
            bench.workshop.join("dist/Odd.receipt.json"),
            br#"{"artifact":"Odd.zip","sha256":"x","builtAt":"t","extra":1}"#,
        )
        .unwrap();
        let refusal = plan_bring_through(&bench.workshop, &bench.mods, "Odd.zip").unwrap_err();
        assert_eq!(
            refusal.reason,
            BringReason::NoReceipt,
            "an extra key is not a receipt"
        );
    }

    #[test]
    fn only_a_plain_zip_in_dist_can_be_brought() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        fs::write(bench.workshop.join("loose.zip"), b"x").unwrap();
        for bad in [
            "../loose.zip",
            "..\\loose.zip",
            "dist/Stature-0.2.0.zip",
            "C:Stature-0.2.0.zip",
            "Stature-0.2.0.receipt.json",
            ".riftstone-x.part",
            "",
            "Missing.zip",
        ] {
            let refusal = plan_bring_through(&bench.workshop, &bench.mods, bad).unwrap_err();
            assert_eq!(refusal.reason, BringReason::NotInDist, "{bad}");
        }
        assert!(mods_names(&bench).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_linked_dist_folder_is_refused_and_outside_bytes_never_land() {
        // The Heretic's WB3: dist\ itself a link to a folder outside the
        // workshop, holding a build and a receipt that match each other.
        let bench = bench();
        let outside = tempfile::tempdir().unwrap();
        let elsewhere = Bench {
            _root: tempfile::tempdir().unwrap(),
            workshop: outside.path().to_path_buf(),
            mods: bench.mods.clone(),
        };
        fs::create_dir_all(outside.path().join("dist")).unwrap();
        build(&elsewhere, "Outside.zip", b"outside bytes");
        fs::remove_dir(bench.workshop.join("dist")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("dist"), bench.workshop.join("dist"))
            .unwrap();
        let refusal = plan_bring_through(&bench.workshop, &bench.mods, "Outside.zip").unwrap_err();
        assert_eq!(refusal.reason, BringReason::NotInDist);
        let refusal = bring_through(
            &bench.workshop,
            &bench.mods,
            "Outside.zip",
            &sha(b"outside bytes"),
            "b9",
        )
        .unwrap_err();
        assert_eq!(refusal.reason, BringReason::NotInDist);
        assert!(mods_names(&bench).is_empty(), "nothing landed");
    }

    #[cfg(windows)]
    #[test]
    fn a_junctioned_dist_folder_is_refused_and_outside_bytes_never_land() {
        let bench = bench();
        let outside = tempfile::tempdir().unwrap();
        let elsewhere = Bench {
            _root: tempfile::tempdir().unwrap(),
            workshop: outside.path().to_path_buf(),
            mods: bench.mods.clone(),
        };
        fs::create_dir_all(outside.path().join("dist")).unwrap();
        build(&elsewhere, "Outside.zip", b"outside bytes");
        fs::remove_dir(bench.workshop.join("dist")).unwrap();
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(bench.workshop.join("dist"))
            .arg(outside.path().join("dist"))
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink /J: {made:?}");
        let refusal = plan_bring_through(&bench.workshop, &bench.mods, "Outside.zip").unwrap_err();
        assert_eq!(refusal.reason, BringReason::NotInDist);
        let refusal = bring_through(
            &bench.workshop,
            &bench.mods,
            "Outside.zip",
            &sha(b"outside bytes"),
            "b9",
        )
        .unwrap_err();
        assert_eq!(refusal.reason, BringReason::NotInDist);
        assert!(mods_names(&bench).is_empty(), "nothing landed");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_linked_receipt_is_no_receipt() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        let outside = tempfile::tempdir().unwrap();
        let real = bench.workshop.join("dist/Stature-0.2.0.receipt.json");
        fs::rename(&real, outside.path().join("r.json")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("r.json"), &real).unwrap();
        let refusal =
            plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").unwrap_err();
        assert_eq!(refusal.reason, BringReason::NoReceipt);
    }

    #[test]
    fn a_receipt_past_64_kib_is_refused_by_the_read_itself() {
        let bench = bench();
        build(&bench, "Stature-0.2.0.zip", b"stature build");
        let path = bench.workshop.join("dist/Stature-0.2.0.receipt.json");
        let mut padded = fs::read(&path).unwrap();
        padded.resize(MAX_RECEIPT_BYTES as usize + 1, b' ');
        fs::write(&path, &padded).unwrap();
        let refusal =
            plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").unwrap_err();
        assert_eq!(refusal.reason, BringReason::NoReceipt);
        padded.truncate(MAX_RECEIPT_BYTES as usize);
        fs::write(&path, &padded).unwrap();
        assert!(
            plan_bring_through(&bench.workshop, &bench.mods, "Stature-0.2.0.zip").is_ok(),
            "64 KiB exactly reads"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_linked_artifact_is_not_in_dist() {
        let bench = bench();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.zip"), b"secret").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.zip"),
            bench.workshop.join("dist/Linked.zip"),
        )
        .unwrap();
        let refusal = plan_bring_through(&bench.workshop, &bench.mods, "Linked.zip").unwrap_err();
        assert_eq!(refusal.reason, BringReason::NotInDist);
    }
}
