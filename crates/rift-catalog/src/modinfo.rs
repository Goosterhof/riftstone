//! A mod's `modinfo.ini` — the packaging format Dragon's Dogma 2 mods ship
//! their names, menus and previews in — parsed leniently.
//!
//! **Decoding.** A UTF-8 BOM means UTF-8; a UTF-16 LE/BE BOM means UTF-16.
//! Without a BOM: bytes holding NULs are UTF-16 without a BOM (ASCII text in
//! UTF-16 has a zero in every other byte, and NUL never appears in UTF-8 or
//! Windows-1252 text); otherwise strict UTF-8, else Windows-1252 — the
//! encoding of a modinfo.ini written in Notepad on a Western Windows.
//!
//! **Garbled.** `garbled` is set when the decoded text still holds a
//! replacement character (U+FFFD: bytes the decoder could not map), a NUL, or
//! a C1 control character (U+0080–U+009F). Windows-1252 decodes every byte, but
//! the five bytes it leaves undefined come out as C1 controls — the
//! fingerprint of a Shift-JIS or other non-Western file read as Western.
//! Clean Windows-1252 (`Café`, `naïve`) is not garbled.
//!
//! **Lines.** Split on `\r\n`, `\n` or a lone `\r`; blank lines skipped; a line
//! whose first character is `;` or `#` is a comment (inline comments are NOT
//! stripped — descriptions and URLs carry `#` and `;`); `[section]` lines are
//! ignored. A line splits at its first `=`, or failing that its first `:`
//! (`key: value`). One refinement: when a `:` comes before the first `=` and
//! the text before it is a bare word, the colon is the separator, so
//! `homepage: https://example.com/?a=b` keeps its URL whole. Keys are trimmed
//! and lower-cased, values trimmed; a line with an empty key or value is
//! skipped (empty = absent, so it never erases an earlier value).
//!
//! **Keys.** `name description author version category screenshot homepage
//! addonfor nameasbundle` fill their fields; `requires` and `requirement` both
//! append to `requires` (case-insensitive duplicates dropped); `dummymod` is
//! true for `true`, `1` or `yes`; `menupriority` reads a leading integer
//! (`5`, `-2`, `10 (high)`), default 0. Every other key lands in `extra`. A
//! repeated key: the last one wins (except `requires`).

use std::borrow::Cow;

use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, WINDOWS_1252};
use rift_core::ModInfo;

/// The parse result. `garbled` is set when the text could not be decoded
/// cleanly — replacement characters, NULs or C1 control characters survived
/// decoding (see the module docs) — so the window can say the names may be
/// wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModinfo {
    pub info: ModInfo,
    pub garbled: bool,
}

/// Parse modinfo.ini bytes.
pub fn parse(bytes: &[u8]) -> ParsedModinfo {
    let (text, garbled) = decode(bytes);
    ParsedModinfo {
        info: parse_text(&text),
        garbled,
    }
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

fn decode(bytes: &[u8]) -> (Cow<'_, str>, bool) {
    let (text, had_errors) = if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        encoding.decode_without_bom_handling(&bytes[bom_len..])
    } else if let Some(encoding) = utf16_without_bom(bytes) {
        encoding.decode_without_bom_handling(bytes)
    } else {
        match std::str::from_utf8(bytes) {
            Ok(text) => (Cow::Borrowed(text), false),
            Err(_) => WINDOWS_1252.decode_without_bom_handling(bytes),
        }
    };
    let garbled = had_errors || text.chars().any(is_garbage_char);
    (text, garbled)
}

/// UTF-16 without a BOM: NUL bytes concentrated on one parity.
fn utf16_without_bom(bytes: &[u8]) -> Option<&'static Encoding> {
    if bytes.len() < 4 || !bytes.contains(&0) {
        return None;
    }
    let units = bytes.len() / 2;
    let (mut even_zeros, mut odd_zeros) = (0usize, 0usize);
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == 0 {
            if i % 2 == 0 {
                even_zeros += 1;
            } else {
                odd_zeros += 1;
            }
        }
    }
    if odd_zeros * 4 >= units && odd_zeros > even_zeros * 4 {
        Some(UTF_16LE)
    } else if even_zeros * 4 >= units && even_zeros > odd_zeros * 4 {
        Some(UTF_16BE)
    } else {
        None
    }
}

fn is_garbage_char(c: char) -> bool {
    c == '\u{FFFD}' || c == '\0' || ('\u{80}'..='\u{9F}').contains(&c)
}

// ---------------------------------------------------------------------------
// Lines and keys
// ---------------------------------------------------------------------------

fn parse_text(text: &str) -> ModInfo {
    let mut info = ModInfo::default();
    for raw in text.split(['\r', '\n']) {
        let line = raw.trim_start_matches('\u{FEFF}').trim();
        if line.is_empty()
            || line.starts_with([';', '#'])
            || (line.starts_with('[') && line.ends_with(']'))
        {
            continue;
        }
        let Some((key, value)) = split_key_value(line) else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            continue;
        }
        apply(&mut info, &key, value);
    }
    info
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let at = match (line.find('='), line.find(':')) {
        (Some(eq), Some(colon)) if colon < eq && is_bare_word(&line[..colon]) => colon,
        (Some(eq), _) => eq,
        (None, Some(colon)) => colon,
        (None, None) => return None,
    };
    Some((&line[..at], &line[at + 1..]))
}

fn is_bare_word(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn apply(info: &mut ModInfo, key: &str, value: &str) {
    let owned = || Some(value.to_string());
    match key {
        "name" => info.name = owned(),
        "description" => info.description = owned(),
        "author" => info.author = owned(),
        "version" => info.version = owned(),
        "category" => info.category = owned(),
        "screenshot" => info.screenshot = owned(),
        "homepage" => info.homepage = owned(),
        "addonfor" => info.addon_for = owned(),
        "nameasbundle" => info.name_as_bundle = owned(),
        "requires" | "requirement" => {
            let wanted = value.to_lowercase();
            if !info.requires.iter().any(|r| r.to_lowercase() == wanted) {
                info.requires.push(value.to_string());
            }
        }
        "dummymod" => {
            info.dummy = matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
        }
        "menupriority" => info.menu_priority = leading_integer(value),
        _ => {
            info.extra.insert(key.to_string(), value.to_string());
        }
    }
}

/// The integer a value starts with (`5`, `+3`, `-2`, `10 (high)`), clamped to
/// i32; 0 when it starts with none.
fn leading_integer(value: &str) -> i32 {
    let (negative, rest) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return 0;
    }
    let magnitude: i64 = rest[..digits].parse().unwrap_or(i64::MAX);
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed.clamp(i64::from(i32::MIN), i64::from(i32::MAX))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str, little_endian: bool, bom: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        let units = if bom { vec![0xFEFF_u16] } else { Vec::new() };
        for unit in units.into_iter().chain(text.encode_utf16()) {
            if little_endian {
                bytes.extend_from_slice(&unit.to_le_bytes());
            } else {
                bytes.extend_from_slice(&unit.to_be_bytes());
            }
        }
        bytes
    }

    #[test]
    fn reads_every_key_the_format_knows() {
        let text = "name=Ermine Cape - White\r\n\
                    description=A white ermine cape for the Arisen.\r\n\
                    author=Vernworth Tailors\r\n\
                    version=1.2\r\n\
                    category=Cloaks\r\n\
                    screenshot=preview.png\r\n\
                    homepage=https://www.nexusmods.com/dragonsdogma2/mods/123\r\n\
                    addonfor=Ermine Cape (Menu Option)\r\n\
                    NameAsBundle=Ermine Capes\r\n\
                    requires=RE Framework\r\n\
                    requirement=Lantern Glow Recolor\r\n\
                    DummyMod=True\r\n\
                    MenuPriority=7\r\n\
                    tags=fur, white\r\n";
        let parsed = parse(text.as_bytes());
        assert!(!parsed.garbled);
        let info = parsed.info;
        assert_eq!(info.name.as_deref(), Some("Ermine Cape - White"));
        assert_eq!(
            info.description.as_deref(),
            Some("A white ermine cape for the Arisen.")
        );
        assert_eq!(info.author.as_deref(), Some("Vernworth Tailors"));
        assert_eq!(info.version.as_deref(), Some("1.2"));
        assert_eq!(info.category.as_deref(), Some("Cloaks"));
        assert_eq!(info.screenshot.as_deref(), Some("preview.png"));
        assert_eq!(
            info.homepage.as_deref(),
            Some("https://www.nexusmods.com/dragonsdogma2/mods/123")
        );
        assert_eq!(info.addon_for.as_deref(), Some("Ermine Cape (Menu Option)"));
        assert_eq!(info.name_as_bundle.as_deref(), Some("Ermine Capes"));
        assert_eq!(info.requires, vec!["RE Framework", "Lantern Glow Recolor"]);
        assert!(info.dummy);
        assert_eq!(info.menu_priority, 7);
        assert_eq!(
            info.extra.get("tags").map(String::as_str),
            Some("fur, white")
        );
        assert_eq!(info.extra.len(), 1);
    }

    #[test]
    fn accepts_both_separators_and_any_key_case() {
        let info = parse(b"NAME: Lantern Glow Recolor\nAuthor = Someone\nAddonFor:Lanterns\n").info;
        assert_eq!(info.name.as_deref(), Some("Lantern Glow Recolor"));
        assert_eq!(info.author.as_deref(), Some("Someone"));
        assert_eq!(info.addon_for.as_deref(), Some("Lanterns"));
    }

    #[test]
    fn splits_at_the_first_equals_unless_a_bare_key_and_colon_come_first() {
        let info = parse(
            b"name=Lantern Glow: Ember Edition\n\
              homepage: https://example.com/mod?id=5\n\
              description=Ratio 16:9 = widescreen\n",
        )
        .info;
        assert_eq!(info.name.as_deref(), Some("Lantern Glow: Ember Edition"));
        assert_eq!(
            info.homepage.as_deref(),
            Some("https://example.com/mod?id=5")
        );
        assert_eq!(info.description.as_deref(), Some("Ratio 16:9 = widescreen"));
    }

    #[test]
    fn skips_comments_sections_blank_lines_and_junk() {
        let info = parse(
            b"; a comment\r\n# another = comment\r\n\r\n[ModInfo]\r\n   \r\nname=Lantern Glow # not a comment\r\njust some words\r\n=orphan value\r\n",
        )
        .info;
        assert_eq!(info.name.as_deref(), Some("Lantern Glow # not a comment"));
        assert!(info.extra.is_empty(), "{:?}", info.extra);
    }

    #[test]
    fn last_value_wins_empty_values_are_absent_requires_accumulate() {
        let info = parse(
            b"name=First\nname=Second\nname=\nauthor=   \nrequires=Lantern Glow\nrequires=\nrequirement=lantern glow\nrequires=RE Framework\n",
        )
        .info;
        assert_eq!(
            info.name.as_deref(),
            Some("Second"),
            "an empty line never erases a value"
        );
        assert_eq!(info.author, None);
        assert_eq!(
            info.requires,
            vec!["Lantern Glow", "RE Framework"],
            "duplicates dropped, case-blind"
        );
    }

    #[test]
    fn dummymod_and_menupriority_are_lenient() {
        for (value, expected) in [
            ("true", true),
            ("TRUE", true),
            ("1", true),
            ("Yes", true),
            ("false", false),
            ("0", false),
            ("nope", false),
        ] {
            let info = parse(format!("DummyMod={value}").as_bytes()).info;
            assert_eq!(info.dummy, expected, "DummyMod={value}");
        }
        for (value, expected) in [
            ("5", 5),
            ("+3", 3),
            ("-2", -2),
            ("10 (high)", 10),
            ("high", 0),
            ("99999999999", i32::MAX),
            ("-99999999999", i32::MIN),
        ] {
            let info = parse(format!("MenuPriority={value}").as_bytes()).info;
            assert_eq!(info.menu_priority, expected, "MenuPriority={value}");
        }
        assert_eq!(parse(b"name=x").info.menu_priority, 0, "default 0");
    }

    #[test]
    fn reads_utf8_with_and_without_a_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("name=Café Lantern\r\nauthor=Zoë".as_bytes());
        let parsed = parse(&bytes);
        assert_eq!(
            parsed.info.name.as_deref(),
            Some("Café Lantern"),
            "the BOM never sticks to the first key"
        );
        assert_eq!(parsed.info.author.as_deref(), Some("Zoë"));
        assert!(!parsed.garbled);
        assert_eq!(
            parse("name=Café".as_bytes()).info.name.as_deref(),
            Some("Café")
        );
    }

    #[test]
    fn reads_utf16_le_and_be_with_a_bom() {
        for little_endian in [true, false] {
            let parsed = parse(&utf16(
                "name=ドラゴン Cape\r\nversion=2\r\n",
                little_endian,
                true,
            ));
            assert_eq!(
                parsed.info.name.as_deref(),
                Some("ドラゴン Cape"),
                "LE={little_endian}"
            );
            assert_eq!(parsed.info.version.as_deref(), Some("2"));
            assert!(!parsed.garbled);
        }
    }

    #[test]
    fn reads_utf16_without_a_bom() {
        for little_endian in [true, false] {
            let parsed = parse(&utf16(
                "name=Lantern Glow\r\nauthor=Someone\r\n",
                little_endian,
                false,
            ));
            assert_eq!(
                parsed.info.name.as_deref(),
                Some("Lantern Glow"),
                "LE={little_endian}"
            );
            assert_eq!(parsed.info.author.as_deref(), Some("Someone"));
            assert!(!parsed.garbled);
        }
    }

    #[test]
    fn falls_back_to_windows_1252() {
        let parsed = parse(b"name=Caf\xE9 Lantern \x96 Ember\r\nauthor=Na\xEFve");
        assert_eq!(parsed.info.name.as_deref(), Some("Café Lantern – Ember"));
        assert_eq!(parsed.info.author.as_deref(), Some("Naïve"));
        assert!(!parsed.garbled, "clean Windows-1252 is not garbled");
    }

    #[test]
    fn flags_text_that_did_not_decode_cleanly() {
        // A UTF-8 BOM followed by bytes that are not UTF-8.
        let parsed = parse(b"\xEF\xBB\xBFname=Lantern \xFF\xFE Glow");
        assert!(parsed.garbled);
        assert_eq!(
            parsed.info.name.as_deref(),
            Some("Lantern \u{FFFD}\u{FFFD} Glow")
        );
        // Shift-JIS read as Windows-1252: 0x81 is undefined there and comes out as a C1 control.
        assert!(parse(b"name=\x83\x89\x83\x93\x83\x5E\x83\x93\x81\x40Glow").garbled);
    }

    #[test]
    fn lone_carriage_returns_split_lines_too() {
        let info = parse(b"name=Lantern Glow\rauthor=Someone\r").info;
        assert_eq!(info.name.as_deref(), Some("Lantern Glow"));
        assert_eq!(info.author.as_deref(), Some("Someone"));
    }
}
