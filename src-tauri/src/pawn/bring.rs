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
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::landing;

/// The receipt's own size cap.
pub const MAX_RECEIPT_BYTES: u64 = 64 * 1024;

/// Why a bring-through was refused (WINDOW-CONTRACT § v0.7, `bring`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BringReason {
    NameTaken,
    BytesChanged,
    NoReceipt,
    ReceiptMismatch,
    NotInDist,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BringRefusal {
    pub reason: BringReason,
    /// One sentence, in the stone's voice. Workshop names are the
    /// investor's own and may be shown.
    pub detail: String,
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

/// SHA-256 and length of a file, streamed.
pub fn digest_file(path: &Path) -> io::Result<(String, u64)> {
    let mut file = File::open(path)?;
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

/// The artifact's path in `<workshop>\dist\`: a plain `.zip` name, a regular
/// file, not a link.
fn artifact_path(workshop: &Path, artifact: &str) -> Result<PathBuf, BringRefusal> {
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
    let path = workshop.join("dist").join(artifact);
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_file() => Ok(path),
        _ => Err(not_in_dist()),
    }
}

fn receipt_name(artifact: &str) -> String {
    let stem = &artifact[..artifact.len() - ".zip".len()];
    format!("{stem}.receipt.json")
}

/// Read the receipt beside the artifact: missing, unreadable or too large is
/// `noReceipt`.
pub fn read_receipt(workshop: &Path, artifact: &str) -> Result<Receipt, BringRefusal> {
    let path = workshop.join("dist").join(receipt_name(artifact));
    let missing = || {
        refuse(
            BringReason::NoReceipt,
            format!(
                "{artifact} has no readable receipt beside it ({}). Ask Tamsin to write one.",
                receipt_name(artifact)
            ),
        )
    };
    let meta = fs::symlink_metadata(&path).map_err(|_| missing())?;
    if !meta.file_type().is_file() || meta.len() > MAX_RECEIPT_BYTES {
        return Err(missing());
    }
    let bytes = fs::read(&path).map_err(|_| missing())?;
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
    let path = artifact_path(workshop, artifact)?;
    let receipt = read_receipt(workshop, artifact)?;
    let (sha256, bytes) = digest_file(&path).map_err(io_refusal)?;
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
    let source = artifact_path(workshop, artifact)?;
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
        io::copy(&mut File::open(&source)?, &mut target)?;
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
