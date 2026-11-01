//! A picture the player pastes to Tamsin (0.7.5, investor report 2026-10-31:
//! *"i can not paste to Tamsin, when i want to show a screenshot of something
//! i can not paste it in"*).
//!
//! Claude Code takes a pasted image by reading the system clipboard itself.
//! Tamsin's Claude Code runs in WSL inside the fence, which closes every way
//! to Windows (RS-9: no interop, no Windows process), so her Ctrl+V can never
//! reach the player's clipboard, and the hollow's paste carries text only.
//! So the window hands the stone the picture the player pasted, and the stone
//! writes it into the workshop, where she can read it, and types its path.
//!
//! - Only a PNG, JPEG, GIF or WebP, told by its own first bytes, never by a
//!   name or a type the window claimed; at most [`MAX_PICTURE_BYTES`].
//! - The name is the stone's own (`paste-<time>.<ext>`), in `pasted\`, which
//!   is opened by handle and refused if it is a link or reparse point: Tamsin
//!   can make, replace and link anything in the workshop, so the stone never
//!   follows a path she could have planted. The file is created exclusively
//!   relative to that handle and renamed into place without replacing
//!   anything ([`MailFolder::write_new`]).
//! - The path typed for her is relative to the workshop, her working folder.

use std::path::Path;

use crate::pawn::folder::{MailFolder, Written};

/// The folder in the workshop that pasted pictures land in.
pub const PASTED: &str = "pasted";

/// 20 MiB: far above a 4K screenshot as PNG, far below what would strain the
/// bridge or her context.
pub const MAX_PICTURE_BYTES: usize = 20 * 1024 * 1024;

/// The picture's kind, by its own first bytes: the extension it lands with.
pub fn kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// Write `bytes` into `<workshop>\pasted\` as `paste-<stamp>.<ext>` (then
/// `-2`, `-3` … when the name holds other bytes) and return the path relative
/// to the workshop, with forward slashes, as she reads it. `Err` is the
/// stone's sentence.
pub fn paste_picture(workshop: &Path, bytes: &[u8], stamp: &str) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("The clipboard held an empty picture, so nothing was pasted.".into());
    }
    if bytes.len() > MAX_PICTURE_BYTES {
        return Err(format!(
            "That picture is {} MiB; the stone pastes pictures up to 20 MiB. Save a smaller one, or crop it.",
            bytes.len().div_ceil(1024 * 1024)
        ));
    }
    let ext = kind(bytes).ok_or_else(|| {
        "The stone pastes PNG, JPEG, GIF and WebP pictures only, and this was none of them."
            .to_string()
    })?;
    if !workshop.is_dir() {
        return Err("The workshop folder is not there, so the picture had nowhere to go.".into());
    }
    let folder = workshop.join(PASTED);
    // Made by path; opened by handle, which refuses a link she put in its place.
    let _ = std::fs::create_dir(&folder);
    let pasted = MailFolder::open(&folder).map_err(|_| {
        format!("The workshop's {PASTED} folder is not a plain folder, so the stone pasted nothing there. Remove it, and paste again.")
    })?;
    // A name the folder already lists is passed over, whatever it is (a link
    // she planted under it included); a race for a name is caught by the
    // exclusive create below.
    let taken: std::collections::HashSet<String> = pasted
        .names()
        .unwrap_or_default()
        .into_iter()
        .map(|name| name.to_lowercase())
        .collect();
    for n in 1..=99u32 {
        let name = if n == 1 {
            format!("paste-{stamp}.{ext}")
        } else {
            format!("paste-{stamp}-{n}.{ext}")
        };
        if taken.contains(&name.to_lowercase()) {
            continue;
        }
        match pasted.write_new(&name, bytes) {
            Ok(Written::New | Written::AlreadyThere) => return Ok(format!("{PASTED}/{name}")),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(format!("The stone could not write the picture: {err}.")),
        }
    }
    Err("Ninety-nine pictures were pasted this second; wait a moment and paste again.".into())
}

/// The name's time: local, to the second, sortable (`20261031-231405`).
pub fn stamp_now() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n invented pixels";

    #[test]
    fn a_picture_lands_in_pasted_under_the_stones_own_name() {
        let workshop = tempfile::tempdir().unwrap();
        let path = paste_picture(workshop.path(), PNG, "20261031-231405").unwrap();
        assert_eq!(path, "pasted/paste-20261031-231405.png");
        assert_eq!(
            std::fs::read(workshop.path().join("pasted/paste-20261031-231405.png")).unwrap(),
            PNG
        );
    }

    #[test]
    fn every_paste_in_the_same_second_takes_the_next_free_name() {
        let workshop = tempfile::tempdir().unwrap();
        let other = b"\x89PNG\r\n\x1a\n other pixels";
        assert_eq!(
            paste_picture(workshop.path(), PNG, "s").unwrap(),
            "pasted/paste-s.png"
        );
        assert_eq!(
            paste_picture(workshop.path(), other, "s").unwrap(),
            "pasted/paste-s-2.png"
        );
        assert_eq!(
            paste_picture(workshop.path(), PNG, "s").unwrap(),
            "pasted/paste-s-3.png",
            "each paste is its own file"
        );
        assert_eq!(
            std::fs::read(workshop.path().join("pasted/paste-s.png")).unwrap(),
            PNG,
            "nothing is written over"
        );
    }

    #[test]
    fn only_the_four_kinds_by_their_own_bytes() {
        assert_eq!(kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(kind(b"GIF89a.."), Some("gif"));
        assert_eq!(kind(b"RIFF\x10\x00\x00\x00WEBPVP8 "), Some("webp"));
        assert_eq!(kind(b"<svg xmlns=..."), None);
        assert_eq!(kind(b"MZ\x90\x00"), None);
        let workshop = tempfile::tempdir().unwrap();
        let refused = paste_picture(workshop.path(), b"MZ\x90\x00 an exe", "s").unwrap_err();
        assert!(refused.contains("PNG, JPEG, GIF and WebP"), "{refused}");
        assert!(
            !workshop.path().join("pasted").exists()
                || std::fs::read_dir(workshop.path().join("pasted"))
                    .unwrap()
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn too_large_and_empty_are_refused() {
        let workshop = tempfile::tempdir().unwrap();
        let mut big = PNG.to_vec();
        big.resize(MAX_PICTURE_BYTES + 1, 0);
        assert!(paste_picture(workshop.path(), &big, "s")
            .unwrap_err()
            .contains("up to 20 MiB"));
        assert!(paste_picture(workshop.path(), b"", "s")
            .unwrap_err()
            .contains("empty"));
    }

    #[cfg(unix)]
    #[test]
    fn a_pasted_folder_she_linked_elsewhere_gets_nothing() {
        let workshop = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), workshop.path().join("pasted")).unwrap();
        let refused = paste_picture(workshop.path(), PNG, "s").unwrap_err();
        assert!(refused.contains("not a plain folder"), "{refused}");
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_name_she_planted_as_a_link_is_never_written_through() {
        let workshop = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir(workshop.path().join("pasted")).unwrap();
        let target = outside.path().join("victim.png");
        std::os::unix::fs::symlink(&target, workshop.path().join("pasted/paste-s.png")).unwrap();
        let path = paste_picture(workshop.path(), PNG, "s").unwrap();
        assert_eq!(path, "pasted/paste-s-2.png");
        assert!(!target.exists(), "the link's target was never written");
    }
}
