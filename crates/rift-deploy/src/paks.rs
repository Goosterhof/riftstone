//! How the Riftstone names the paks it puts in `pak_mods/`.
//!
//! REFramework loads `pak_mods/` in file-name order and the last name wins
//! (see [`rift_core::dd2::PAK_MODS_DIR`]), so the name IS the load order. The
//! Riftstone writes `zrift-NNNN-<stem>.pak`: [`rift_core::dd2::PAK_PREFIX`],
//! a four-digit number, and the original file's stem with any earlier
//! priority prefix (`x0003-` from Fluffy, `zrift-0002-` from a previous
//! Riftstone install) stripped. The extension is always a lowercase `.pak` —
//! REFramework's extension check is case-sensitive.
//!
//! The number of a NEW pak sits above every `zrift-` pak already in
//! `pak_mods/` or the journal (and is at least the pak order's length), so a
//! new install always loads last and wins — over a stray `zrift-` name too —
//! and file-name order always equals the journal's pak order. It is not the
//! pak's position: after an uninstall leaves a gap, the position is already
//! worn by the next pak, and a stem that sorts first would load before it.
//! Closing the gap would rename other mods' paks, which an install may not
//! do while the game holds them open. [`crate::reorder_paks`] renumbers every
//! managed pak from `0000` to its position.

use std::collections::HashSet;

use rift_core::dd2::PAK_PREFIX;
use rift_core::lower_key;

/// The only extension REFramework loads from `pak_mods/` (case-sensitive).
pub const PAK_EXTENSION: &str = ".pak";

/// The highest position four digits can write.
pub const MAX_PAK_POSITION: u32 = 9_999;

/// Stems are capped so the full name stays well inside Windows path limits.
pub const MAX_STEM_CHARS: usize = 120;

/// The stem the Riftstone keeps from a pak's original file name: extension
/// and priority prefixes stripped, Windows-illegal characters (`<>:"/\|?*`
/// and controls) replaced by `_`, trailing dots and spaces trimmed, capped at
/// [`MAX_STEM_CHARS`]. Never empty.
pub fn pak_stem(file_name: &str) -> String {
    let unified = file_name.replace('\\', "/");
    let base = rift_core::path::file_name(&unified);
    let mut stem = strip_suffix_ignore_case(base, PAK_EXTENSION);
    while let Some(rest) = strip_priority_prefix(stem) {
        stem = rest;
    }
    let cleaned: String = stem
        .chars()
        .map(|c| if is_illegal(c) { '_' } else { c })
        .collect();
    let capped: String = trim_end_dots_spaces(&cleaned)
        .chars()
        .take(MAX_STEM_CHARS)
        .collect();
    let finished = trim_end_dots_spaces(&capped);
    if finished.is_empty() {
        "mod".to_string()
    } else {
        finished.to_string()
    }
}

/// `zrift-0007-<stem>.pak`.
pub fn managed_pak_name(position: u32, stem: &str) -> String {
    format!("{PAK_PREFIX}{position:04}-{stem}{PAK_EXTENSION}")
}

/// The position in a `zrift-NNNN-…` name (any case), if it is one.
pub fn managed_number(file_name: &str) -> Option<u32> {
    let prefix_len = PAK_PREFIX.len();
    let head = file_name.get(..prefix_len)?;
    if !head.eq_ignore_ascii_case(PAK_PREFIX) {
        return None;
    }
    let digits = file_name.get(prefix_len..prefix_len + 4)?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if file_name.as_bytes().get(prefix_len + 4) != Some(&b'-') {
        return None;
    }
    digits.parse().ok()
}

/// The order NTFS lists names in: compared as upcased UTF-16 code units
/// (NTFS's upcase table maps one unit to one unit, so a character whose
/// uppercase is longer than one character keeps its own spelling). This is
/// the order REFramework's directory iterator hands `pak_mods/` over in.
pub fn ntfs_order_key(name: &str) -> Vec<u16> {
    let mut key = Vec::with_capacity(name.len());
    let mut units = [0u16; 2];
    for c in name.chars() {
        let mut upper = c.to_uppercase();
        let mapped = match (upper.next(), upper.next()) {
            (Some(u), None) => u,
            _ => c,
        };
        key.extend_from_slice(mapped.encode_utf16(&mut units));
    }
    key
}

/// `stem`, or `stem-2`, `stem-3`… — the first one not yet in `used`
/// (compared case-insensitively). Two paks of one option may share a name.
pub fn unique_stem(stem: String, used: &mut HashSet<String>) -> String {
    if used.insert(lower_key(&stem)) {
        return stem;
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{stem}-{n}");
        if used.insert(lower_key(&candidate)) {
            return candidate;
        }
        n += 1;
    }
}

fn strip_priority_prefix(stem: &str) -> Option<&str> {
    let bytes = stem.as_bytes();
    let p = PAK_PREFIX.len();
    if let Some(head) = stem.get(..p) {
        if head.eq_ignore_ascii_case(PAK_PREFIX)
            && bytes.len() >= p + 5
            && bytes[p..p + 4].iter().all(u8::is_ascii_digit)
            && bytes[p + 4] == b'-'
        {
            return Some(&stem[p + 5..]);
        }
    }
    if bytes.len() >= 6
        && (bytes[0] == b'x' || bytes[0] == b'X')
        && bytes[1..5].iter().all(u8::is_ascii_digit)
        && bytes[5] == b'-'
    {
        return Some(&stem[6..]);
    }
    None
}

fn strip_suffix_ignore_case<'a>(name: &'a str, suffix: &str) -> &'a str {
    if name.len() >= suffix.len() {
        let cut = name.len() - suffix.len();
        if let Some(tail) = name.get(cut..) {
            if tail.eq_ignore_ascii_case(suffix) {
                return &name[..cut];
            }
        }
    }
    name
}

fn is_illegal(c: char) -> bool {
    matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control()
}

fn trim_end_dots_spaces(s: &str) -> &str {
    s.trim_end_matches(['.', ' '])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_lose_their_priority_prefixes_and_extension() {
        assert_eq!(
            pak_stem("x0003-Lantern Glow Recolor.pak"),
            "Lantern Glow Recolor"
        );
        assert_eq!(
            pak_stem("X0003-Lantern Glow Recolor.PAK"),
            "Lantern Glow Recolor"
        );
        assert_eq!(
            pak_stem("zrift-0002-Vernworth Cloaks.pak"),
            "Vernworth Cloaks"
        );
        assert_eq!(
            pak_stem("ZRIFT-0002-x0001-Vernworth Cloaks.pak"),
            "Vernworth Cloaks"
        );
        assert_eq!(pak_stem("Mod/Sub/Plain Mod.pak"), "Plain Mod");
        assert_eq!(pak_stem(r"Mod\Sub\Plain Mod.pak"), "Plain Mod");
        // A name that only looks like a prefix keeps its digits.
        assert_eq!(pak_stem("x12-Short.pak"), "x12-Short");
    }

    #[test]
    fn stems_are_safe_windows_names() {
        assert_eq!(pak_stem("a<b>c:d\"e|f?g*h.pak"), "a_b_c_d_e_f_g_h");
        assert_eq!(pak_stem("trailing. . .pak"), "trailing");
        assert_eq!(pak_stem("tab\there.pak"), "tab_here");
        assert_eq!(pak_stem(".pak"), "mod");
        assert_eq!(pak_stem("x0001-.pak"), "mod");
        let long = format!("{}.pak", "a".repeat(300));
        assert_eq!(pak_stem(&long).chars().count(), MAX_STEM_CHARS);
        let long_dots = format!("{}. .x.pak", "b".repeat(118));
        assert!(!pak_stem(&long_dots).ends_with('.'));
        // Multi-byte characters near the prefix never split a character.
        assert_eq!(pak_stem("zrifé-0001-x.pak"), "zrifé-0001-x");
    }

    #[test]
    fn managed_names_round_trip_their_position() {
        let name = managed_pak_name(7, "Lantern Glow Recolor");
        assert_eq!(name, "zrift-0007-Lantern Glow Recolor.pak");
        assert_eq!(managed_number(&name), Some(7));
        assert_eq!(managed_number("ZRIFT-0123-x.pak"), Some(123));
        assert_eq!(managed_number("x0001-x.pak"), None);
        assert_eq!(managed_number("zrift-12-x.pak"), None);
        assert_eq!(managed_number("zrift-0012x.pak"), None);
    }

    #[test]
    fn ntfs_order_puts_ours_after_fluffys() {
        let mut names = [
            "zrift-0001-b.pak",
            "x0001-z.pak",
            "zrift-0000-c.pak",
            "X0000-a.pak",
        ];
        names.sort_by_key(|n| ntfs_order_key(n));
        assert_eq!(
            names,
            [
                "X0000-a.pak",
                "x0001-z.pak",
                "zrift-0000-c.pak",
                "zrift-0001-b.pak"
            ]
        );
    }

    #[test]
    fn unique_stems_count_up() {
        let mut used = HashSet::new();
        assert_eq!(unique_stem("Mod".into(), &mut used), "Mod");
        assert_eq!(unique_stem("mod".into(), &mut used), "mod-2");
        assert_eq!(unique_stem("MOD".into(), &mut used), "MOD-3");
    }
}
