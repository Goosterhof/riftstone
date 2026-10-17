//! Path discipline shared by every crate.
//!
//! Archive entry paths arrive in every dialect (`Mod\natives\STM\...`,
//! `./Mod/natives/...`, `/Mod//natives/`). Inside the Riftstone an entry path
//! is always **normalized**: forward slashes, no leading `./` or `/`, no empty
//! or `.` segments, original case preserved. Comparisons that must behave like
//! the Windows filesystem go through [`lower_key`].
//!
//! Normalization is also the zip-slip guard: a path with a `..` segment, a
//! drive letter or any `:` (an NTFS alternate data stream) never becomes an
//! entry path, so it can never become a write target.

use crate::error::RiftError;

/// Normalize an archive entry path. Returns `Ok("")` for the package root
/// itself (a bare `./` or `/` directory entry), which callers skip.
pub fn normalize_entry_path(raw: &str) -> Result<String, RiftError> {
    let unsafe_path = || RiftError::UnsafeEntryPath {
        entry: raw.to_string(),
    };
    let unified = raw.replace('\\', "/");
    let mut segments: Vec<&str> = Vec::new();
    for segment in unified.split('/') {
        match segment {
            "" | "." => continue,
            ".." => return Err(unsafe_path()),
            s if s.contains(':') => return Err(unsafe_path()),
            s if s.chars().any(|c| c.is_control()) => return Err(unsafe_path()),
            s => segments.push(s),
        }
    }
    Ok(segments.join("/"))
}

/// The case-folded comparison key for a normalized path or a name — how the
/// Windows filesystem treats `natives/STM` and `natives/stm` as one place.
pub fn lower_key(path: &str) -> String {
    path.to_lowercase()
}

/// FNV-1a, 64-bit. Stable across runs and platforms; used for package ids.
/// Not a security hash — only an identity.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The last segment of a normalized path (`a/b/c.pak` → `c.pak`).
pub fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The parent directory of a normalized path (`a/b/c.pak` → `a/b`, `c` → ``).
pub fn parent(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Is `path` equal to `dir` or inside it? Both normalized; case-insensitive.
/// The empty `dir` (package root) contains everything.
pub fn is_within(path: &str, dir: &str) -> bool {
    if dir.is_empty() {
        return true;
    }
    let p = lower_key(path);
    let d = lower_key(dir);
    p == d || (p.len() > d.len() && p.starts_with(&d) && p.as_bytes()[d.len()] == b'/')
}

/// `path` relative to `dir` (both normalized, `dir` must contain `path`).
pub fn relative_to<'a>(path: &'a str, dir: &str) -> &'a str {
    if dir.is_empty() {
        return path;
    }
    if path.len() > dir.len() {
        &path[dir.len() + 1..]
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_every_dialect_to_one_shape() {
        assert_eq!(
            normalize_entry_path(r"Mod\natives\STM\a.mesh").unwrap(),
            "Mod/natives/STM/a.mesh"
        );
        assert_eq!(
            normalize_entry_path("./Mod//natives/./a.mesh").unwrap(),
            "Mod/natives/a.mesh"
        );
        assert_eq!(normalize_entry_path("/Mod/").unwrap(), "Mod");
        assert_eq!(normalize_entry_path("./").unwrap(), "");
    }

    #[test]
    fn refuses_paths_that_climb_out() {
        for bad in [
            "../evil.dll",
            "Mod/../../evil.dll",
            r"..\evil.dll",
            "C:/Windows/evil.dll",
            "Mod/file.txt:stream",
            "Mod/\u{0007}bell",
        ] {
            assert!(
                matches!(
                    normalize_entry_path(bad),
                    Err(RiftError::UnsafeEntryPath { .. })
                ),
                "{bad} should be refused"
            );
        }
    }

    #[test]
    fn fnv_is_stable() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn within_is_segment_aware_and_case_blind() {
        assert!(is_within("Mod/natives/a", "mod"));
        assert!(is_within("Mod/natives/a", "Mod/natives"));
        assert!(!is_within("Modded/natives/a", "Mod"));
        assert!(is_within("anything", ""));
        assert_eq!(relative_to("Mod/natives/a", "Mod"), "natives/a");
        assert_eq!(relative_to("natives/a", ""), "natives/a");
        assert_eq!(parent("a/b/c.pak"), "a/b");
        assert_eq!(parent("c.pak"), "");
        assert_eq!(file_name("a/b/c.pak"), "c.pak");
    }
}
