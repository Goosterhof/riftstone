//! Nexus Mods download file names: the mod name, mod id, version, upload time
//! and file id a downloaded archive carries in its own file name.
//!
//! The shapes in the wild (decoded live, October 2026 — § rift-nexus):
//!
//! - **2026** — `<Name> <modid> <version> <YYYY-MM-DDTHH-MMZ> <sqid>.<ext>`,
//!   space separated; the stamp is the file's UPLOAD time (UTC, to the minute)
//!   and the last token is the file's `sqid`, its short unique id on Nexus:
//!   `Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip` →
//!   `Vernworth Cloaks`, mod 2207, `8`, 2026-10-14 14:23 UTC, sqid `Rk4vLmN2Q`.
//! - **classic** — `<Name>-<modid>-<version>-<unix seconds>.<ext>`, the dots of
//!   the version written as hyphens:
//!   `Lantern Glow-123-1-2-1711234567.zip` → `Lantern Glow`, mod 123, `1.2`,
//!   uploaded 2024-03-23 22:56:07 UTC.
//! - **very old classic** — the same without the timestamp:
//!   `Lantern Glow-123-1-2.zip` → `Lantern Glow`, mod 123, `1.2`.
//!
//! Any shape may carry a browser duplicate marker before the extension
//! (` (1)`, or Firefox's `(1)`). Anything else is not a Nexus download name
//! and parses to `None`: an archive the user renamed is simply a package
//! without a filename link (the MD5 lookup can still find it).
//!
//! ## How the ambiguity is settled
//!
//! A mod's own name may hold hyphens and numbers (`Pack-2`, `DD2 Lantern Glow
//! 4K`). The 2026 shape is read from the END — sqid, stamp, version, mod id —
//! so everything before the mod id is the name, spaces, hyphens and digits
//! included. The classic shapes join every field with `-`, so the parser peels
//! the timestamp off the end and then chooses the mod id as the leftmost
//! all-digit field whose remaining fields all look like version parts,
//! preferring (in order) an id of two or more digits with a version, any id
//! with a version, and finally any id at all. `Pack-2-123-1-0-…` is therefore
//! `Pack-2`, mod 123, `1.0`: the single digit `2` loses to `123`, the same
//! tie-break Mod Organizer 2 uses for these names.
//!
//! Without a timestamp nothing anchors the name, so the very old shape is held
//! to a stricter rule — an id of two or more digits AND a version that starts
//! with a digit (or `v` + digit) — because `Lantern Glow-1-0.zip` is far more
//! often a person's own name for an archive than mod 1, version 0.
//!
//! ## What is never a mod id
//!
//! People name archives after the day they made them, and split on `-` a date
//! looks exactly like a mod id and a version (`DD2 Mods Backup 2026-10-14`
//! would read mod 10, version 14). So the field the tie-break picks is then
//! judged, and a failed judgement refuses the whole name. It never steps on to
//! the next candidate, which is the date's other half or the time written
//! after it (`Pawn Backup 2026-10-14-12-30` would read mod 12, version 30).
//!
//! - **A calendar date.** The picked field is one of a date's pieces, or the
//!   name before it ends in a date (the field is then the time written after
//!   it). Dates are year first, day first (the Dutch order) or month first,
//!   with a 1900–2099 year and a one- or two-digit day and month — except
//!   that in a timestamped name a date the picked field would open needs
//!   ISO's two-digit month and day: `Lantern Glow-2010-1-2-1711234567.zip`
//!   is mod 2010 at version 1.2, not the 2nd of January. Without a timestamp
//!   nothing vouches for that reading, so `Lantern Glow-2010-1-2.zip` is a
//!   date. A date inside the version (`Lantern Glow-123-2024-10-15-…`, a file
//!   versioned by its date) or earlier in the name is fine.
//! - **More than seven digits.** Ids count up per game from 1 and the largest
//!   on the whole site are six digits; a longer number is a compact date
//!   (`20241001`) or a counter.
//! - **A 2026 upload stamp outside the 2026 shape.** A name holding a
//!   `YYYY-MM-DDTHH-MMZ` stamp that does not parse as the 2026 shape (its sqid
//!   missing, a field broken) is refused before the classic reading is tried.
//!   The 2026 shape itself refuses an over-wide id rather than re-reading the
//!   version as the id.
//!
//! The price is deliberate: a real file whose mod id sits beside a date-shaped
//! field (mod 2010 at version `10.14`; a mod whose own name ends in a date)
//! loses its filename link. A missing link is recoverable — the MD5 lookup or
//! a pasted URL still finds the file — while a wrong one points update checks
//! at an unrelated mod.

use serde::{Deserialize, Serialize};

use crate::model::{LinkSource, NexusLink};

/// What a Nexus download file name says about the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFileName {
    /// The mod's name as the file carries it (`Lantern Glow`).
    pub name: String,
    pub mod_id: u64,
    /// The file's version, dots restored in the classic shapes (`1.2`).
    pub version: Option<String>,
    /// When the file was uploaded to Nexus, ms since the epoch (2026 and
    /// classic shapes; the very old shape carries no time).
    pub uploaded_ms: Option<u64>,
    /// The file's short unique id on Nexus — the last token of a 2026 name.
    #[serde(default)]
    pub sqid: Option<String>,
}

impl NexusFileName {
    /// The package link this file name establishes (source `Filename`). The
    /// file id and group id are unknown until `pin_link` matches the name
    /// against the mod's files.
    pub fn link(&self) -> NexusLink {
        NexusLink {
            mod_id: self.mod_id,
            file_id: None,
            group_id: None,
            sqid: self.sqid.clone(),
            version: self.version.clone(),
            uploaded_ms: self.uploaded_ms,
            source: LinkSource::Filename,
        }
    }
}

/// Parse a Nexus download file name. Accepts a bare file name, a stem without
/// its extension (a folder unpacked under the download's name), or a full
/// path (only the last component is read).
pub fn parse(file_name: &str) -> Option<NexusFileName> {
    let base = file_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(file_name)
        .trim();
    let stem = strip_duplicate_marker(strip_extension(base));
    if let Some(parsed) = parse_2026(stem) {
        return Some(parsed);
    }
    // A 2026 upload stamp the 2026 shape could not account for (no sqid, a
    // broken field) marks a name that is neither shape. Read as classic, the
    // stamp's own digits would pose as a mod id and a version.
    if holds_upload_stamp(stem) {
        return None;
    }
    parse_classic(stem)
}

/// The Dragon's Dogma 2 mod id in a Nexus Mods page URL
/// (`https://www.nexusmods.com/dragonsdogma2/mods/123?tab=files` → 123), as
/// modinfo `homepage=` lines carry it. Other games' pages, other hosts and
/// anything that is not a mod page → `None`.
pub fn mod_id_from_url(url: &str) -> Option<u64> {
    const MARKER: &str = "nexusmods.com/dragonsdogma2/mods/";
    let lower = url.trim().to_ascii_lowercase();
    let at = lower.find(MARKER)?;
    // The host must be nexusmods.com itself or a subdomain of it.
    let host_start = lower[..at].rfind(['/', '@']).map_or(0, |i| i + 1);
    let host_prefix = &lower[host_start..at];
    if !(host_prefix.is_empty() || host_prefix.ends_with('.')) {
        return None;
    }
    let rest = &lower[at + MARKER.len()..];
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let tail = &rest[digits_end..];
    if !(tail.is_empty() || tail.starts_with(['/', '?', '#'])) {
        return None;
    }
    parse_mod_id(&rest[..digits_end])
}

// ---------------------------------------------------------------------------
// The shapes
// ---------------------------------------------------------------------------

/// `<Name> <modid> <version> <YYYY-MM-DDTHH-MMZ> <sqid>`.
fn parse_2026(stem: &str) -> Option<NexusFileName> {
    let tokens: Vec<&str> = stem.split(' ').collect();
    let n = tokens.len();
    if n < 4 || !is_sqid(tokens[n - 1]) {
        return None;
    }
    let uploaded_ms = Some(parse_upload_stamp(tokens[n - 2])?);
    let sqid = Some(tokens[n - 1].to_string());

    // With a version (an empty token — a double space — means "no version").
    // An id-shaped field in the id slot settles the reading: when it is too
    // wide to be a mod id the name is refused, never re-read with its
    // version standing in as the id.
    if n >= 5 && is_version_2026(tokens[n - 3]) && is_id_field(tokens[n - 4]) {
        if let Some(name) = join_name(&tokens[..n - 4], " ") {
            return Some(NexusFileName {
                name,
                mod_id: parse_mod_id(tokens[n - 4])?,
                version: non_empty(tokens[n - 3]),
                uploaded_ms,
                sqid,
            });
        }
    }

    // Without one: <Name> <modid> <stamp> <sqid>.
    Some(NexusFileName {
        name: join_name(&tokens[..n - 3], " ")?,
        mod_id: parse_mod_id(tokens[n - 3])?,
        version: None,
        uploaded_ms,
        sqid,
    })
}

/// `<Name>-<modid>-<version, dots as hyphens>-<unix seconds>`, or the very old
/// `<Name>-<modid>-<version>` with no timestamp.
fn parse_classic(stem: &str) -> Option<NexusFileName> {
    let tokens: Vec<&str> = stem.split('-').collect();
    if let Some((last, fields)) = tokens.split_last() {
        if let Some(seconds) = parse_unix_seconds(last) {
            if let Some(id_at) = pick_mod_id(fields, false) {
                return read_classic(fields, id_at, Some(seconds * 1000));
            }
        }
    }
    let id_at = pick_mod_id(&tokens, true)?;
    read_classic(&tokens, id_at, None)
}

/// Choose which field of `<name…>-<modid>-<version parts…>` (timestamp
/// already peeled off) is the mod id, by the ranking in the module doc.
/// `strict` is the very old shape's stricter rule.
fn pick_mod_id(tokens: &[&str], strict: bool) -> Option<usize> {
    // (rank, index): lower rank is better, leftmost wins within a rank.
    let mut best: Option<(u8, usize)> = None;
    for i in 1..tokens.len() {
        if !is_id_field(tokens[i]) || join_name(&tokens[..i], "-").is_none() {
            continue;
        }
        let parts = &tokens[i + 1..];
        let Some(has_version) = classic_version_shape(parts) else {
            continue;
        };
        let multi_digit = tokens[i].len() >= 2;
        if strict && !(multi_digit && has_version && starts_like_version(parts[0])) {
            continue;
        }
        let rank = match (multi_digit, has_version) {
            (true, true) => 0,
            (false, true) => 1,
            _ => 2,
        };
        if best.is_none_or(|(r, _)| rank < r) {
            best = Some((rank, i));
        }
    }
    best.map(|(_, i)| i)
}

/// Read the classic name with `tokens[id_at]` as its mod id — or refuse the
/// whole name when that field is a piece of a calendar date or too wide to
/// be a mod id. Refusing, not stepping to the next candidate, is the point:
/// the next one is the date's other half or the time written after it.
fn read_classic(tokens: &[&str], id_at: usize, uploaded_ms: Option<u64>) -> Option<NexusFileName> {
    if claimed_by_date(tokens, id_at, uploaded_ms.is_some()) {
        return None;
    }
    let parts = &tokens[id_at + 1..];
    Some(NexusFileName {
        name: join_name(&tokens[..id_at], "-")?,
        mod_id: parse_mod_id(tokens[id_at])?,
        version: parts.iter().any(|p| !p.is_empty()).then(|| parts.join(".")),
        uploaded_ms,
        sqid: None,
    })
}

// ---------------------------------------------------------------------------
// Field rules
// ---------------------------------------------------------------------------

/// Drop a trailing `.zip` / `.7z` / `.rar` (any short alphanumeric extension
/// with a letter in it). A "dot part" holding spaces or hyphens is part of the
/// name or version, not an extension (`Lantern Glow v1.2-123-…`).
fn strip_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && (1..=5).contains(&ext.len())
                && ext.bytes().all(|b| b.is_ascii_alphanumeric())
                && ext.bytes().any(|b| b.is_ascii_alphabetic()) =>
        {
            stem
        }
        _ => name,
    }
}

/// Drop a browser's duplicate-download marker: ` (1)` (Chromium) or `(1)`
/// (Firefox) at the end of the stem.
fn strip_duplicate_marker(stem: &str) -> &str {
    let Some(body) = stem.strip_suffix(')') else {
        return stem;
    };
    match body.rsplit_once('(') {
        Some((before, digits))
            if (1..=3).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            let before = before.strip_suffix(' ').unwrap_or(before);
            if before.is_empty() {
                stem
            } else {
                before
            }
        }
        _ => stem,
    }
}

/// The widest plausible Nexus mod id. Ids count up per game from 1 and the
/// largest on the whole site are six digits, so seven leaves decades of room
/// while refusing what is really a compact date (`20241001`) or a counter.
const MAX_MOD_ID_DIGITS: usize = 7;

fn is_all_digits(token: &str) -> bool {
    !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit())
}

/// A field shaped like a mod id: digits only, no leading zero. Shape picks
/// the field; [`parse_mod_id`] decides whether it is a plausible id.
fn is_id_field(token: &str) -> bool {
    is_all_digits(token) && !token.starts_with('0')
}

/// A Nexus mod id: 1–7 digits, no leading zero.
fn parse_mod_id(token: &str) -> Option<u64> {
    if !is_id_field(token) || token.len() > MAX_MOD_ID_DIGITS {
        return None;
    }
    token.parse().ok()
}

/// Does a calendar date claim `tokens[i]`? It does when the field is one of
/// the date's pieces, or when the name before it ends in a date — the field
/// is then the time written after it (`Backup 2024-09-08-19-47`: the
/// zero-padded month and day are no id-shaped fields, so the tie-break lands
/// on the hour).
///
/// A date the field would open makes the field its year. In a timestamped
/// name a year-shaped mod id followed by `1-2` is mod 2010 at version 1.2,
/// not the 2nd of January, so `timestamped` admits only ISO's two-digit
/// month and day there. Without a timestamp nothing vouches for that
/// reading, and `2010-1-2` is a date.
fn claimed_by_date(tokens: &[&str], i: usize, timestamped: bool) -> bool {
    let reads = |start: usize, iso_padded: bool| match tokens.get(start..start + 3) {
        Some(&[first, middle, last]) => reads_as_date(first, middle, last, iso_padded),
        _ => false,
    };
    reads(i, timestamped)
        || (1..=2).any(|back| i >= back && reads(i - back, false))
        || (i >= 3 && is_all_digits(tokens[i - 1]) && reads(i - 3, false))
}

/// Three consecutive fields read as a date: the digits the first field ends
/// with (`Backup 2026` → 2026), the whole middle field, and the digits the
/// last field starts with (`14T14` → 14). Year first, day first (the Dutch
/// order) or month first; the year four digits, the day and month one or two
/// — two each for year first when `iso_padded`.
fn reads_as_date(first: &str, middle: &str, last: &str, iso_padded: bool) -> bool {
    let (a, b, c) = (trailing_digits(first), middle, leading_digits(last));
    if a.is_empty() || c.is_empty() || !is_all_digits(b) {
        return false;
    }
    let short = |field: &str| field.len() <= 2;
    let iso_fields = if iso_padded {
        b.len() == 2 && c.len() == 2
    } else {
        short(b) && short(c)
    };
    let year_first = a.len() == 4 && iso_fields && is_calendar_date(a, b, c);
    let day_first = c.len() == 4 && short(a) && short(b) && is_calendar_date(c, b, a);
    let month_first = c.len() == 4 && short(a) && short(b) && is_calendar_date(c, a, b);
    year_first || day_first || month_first
}

/// A real day between 1900 and 2099 (the fields are already digits only).
fn is_calendar_date(year: &str, month: &str, day: &str) -> bool {
    let (Ok(year), Ok(month), Ok(day)) = (year.parse(), month.parse(), day.parse()) else {
        return false;
    };
    (1900..=2099).contains(&year)
        && (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
}

/// The run of ASCII digits a field ends with (`Backup 2026` → `2026`).
fn trailing_digits(field: &str) -> &str {
    let run = field.bytes().rev().take_while(u8::is_ascii_digit).count();
    &field[field.len() - run..]
}

/// The run of ASCII digits a field starts with (`14T14` → `14`).
fn leading_digits(field: &str) -> &str {
    let run = field.bytes().take_while(u8::is_ascii_digit).count();
    &field[..run]
}

/// Does a 2026 upload stamp (`2026-10-14T14-23Z`, seconds optional) appear
/// anywhere in the stem, separated or not?
fn holds_upload_stamp(stem: &str) -> bool {
    // `YYYY-MM-DDTHH-MMZ` and `YYYY-MM-DDTHH-MM-SSZ`.
    const STAMP_LENGTHS: [usize; 2] = [17, 20];
    (0..stem.len()).any(|start| {
        STAMP_LENGTHS.iter().any(|len| {
            stem.get(start..start + len)
                .is_some_and(|window| parse_upload_stamp(window).is_some())
        })
    })
}

/// The name: the leading tokens joined back, trimmed, holding at least one
/// letter (a bare number is a field, never a name).
fn join_name(tokens: &[&str], separator: &str) -> Option<String> {
    let joined = tokens.join(separator);
    let name = joined.trim();
    name.chars()
        .any(char::is_alphabetic)
        .then(|| name.to_string())
}

fn non_empty(token: &str) -> Option<String> {
    (!token.is_empty()).then(|| token.to_string())
}

/// The version slot of the 2026 shape: one token (no spaces), printable, at
/// most 32 characters. Empty means "no version".
fn is_version_2026(token: &str) -> bool {
    token.chars().count() <= 32 && !token.chars().any(char::is_control)
}

/// The classic version parts between the mod id and the timestamp. `None` when
/// they do not look like a version; otherwise whether a version is present.
/// Accepted: no parts, one empty part (`Name-123--1711234567`), or one or more
/// short alphanumeric parts (`1`, `2`, `v1`, `0a`, `2SE`).
fn classic_version_shape(parts: &[&str]) -> Option<bool> {
    match parts {
        [] | [""] => Some(false),
        _ => parts
            .iter()
            .all(|p| {
                !p.is_empty()
                    && p.len() <= 16
                    && p.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'+')
            })
            .then_some(true),
    }
}

/// `1…`, `v1…`, `V1…`.
fn starts_like_version(part: &str) -> bool {
    match part.as_bytes() {
        [first, ..] if first.is_ascii_digit() => true,
        [b'v' | b'V', second, ..] => second.is_ascii_digit(),
        _ => false,
    }
}

/// The 2026 sqid: 1–32 characters of `[A-Za-z0-9_-]`.
fn is_sqid(token: &str) -> bool {
    (1..=32).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Classic timestamps: unix seconds between 2000-01-01 and 2100-01-01.
fn parse_unix_seconds(token: &str) -> Option<u64> {
    if !(9..=10).contains(&token.len()) || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let seconds: u64 = token.parse().ok()?;
    (946_684_800..4_102_444_800)
        .contains(&seconds)
        .then_some(seconds)
}

/// `YYYY-MM-DDTHH-MMZ` (seconds `THH-MM-SSZ` tolerated) → ms since the epoch.
fn parse_upload_stamp(token: &str) -> Option<u64> {
    let body = token.strip_suffix(['Z', 'z'])?;
    let (date, time) = body.split_once(['T', 't'])?;

    let mut date_parts = date.split('-');
    let year = fixed_digits(date_parts.next()?, 4)?;
    let month = fixed_digits(date_parts.next()?, 2)?;
    let day = fixed_digits(date_parts.next()?, 2)?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut time_parts = time.split('-');
    let hour = fixed_digits(time_parts.next()?, 2)?;
    let minute = fixed_digits(time_parts.next()?, 2)?;
    let second = match time_parts.next() {
        Some(s) => fixed_digits(s, 2)?,
        None => 0,
    };
    if time_parts.next().is_some() {
        return None;
    }

    let valid = (1970..=9999).contains(&year)
        && (1..=12).contains(&month)
        && (1..=days_in_month(year, month)).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60;
    if !valid {
        return None;
    }
    let days = u64::try_from(days_from_civil(i64::from(year), month, day)).ok()?;
    let seconds =
        days * 86_400 + u64::from(hour) * 3_600 + u64::from(minute) * 60 + u64::from(second);
    Some(seconds * 1000)
}

fn fixed_digits(token: &str, width: usize) -> Option<u32> {
    if token.len() != width || !token.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    token.parse().ok()
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let year_of_era = y - era * 400;
    let m = i64::from(month);
    let day_of_year = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    const LANTERN_UPLOAD_MS: u64 = 1_711_234_567_000;
    const VERNWORTH_UPLOAD_MS: u64 = 1_791_987_780_000; // 2026-10-14T14:23Z

    fn classic(
        name: &str,
        mod_id: u64,
        version: Option<&str>,
        uploaded_ms: Option<u64>,
    ) -> NexusFileName {
        NexusFileName {
            name: name.to_string(),
            mod_id,
            version: version.map(str::to_string),
            uploaded_ms,
            sqid: None,
        }
    }

    fn shape_2026(
        name: &str,
        mod_id: u64,
        version: Option<&str>,
        uploaded_ms: u64,
        sqid: &str,
    ) -> NexusFileName {
        NexusFileName {
            name: name.to_string(),
            mod_id,
            version: version.map(str::to_string),
            uploaded_ms: Some(uploaded_ms),
            sqid: Some(sqid.to_string()),
        }
    }

    #[test]
    fn reads_the_classic_shape() {
        assert_eq!(
            parse("Lantern Glow-123-1-2-1711234567.zip"),
            Some(classic(
                "Lantern Glow",
                123,
                Some("1.2"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
        assert_eq!(
            parse("Lantern Glow-123-v1-2b-1711234567.7z"),
            Some(classic(
                "Lantern Glow",
                123,
                Some("v1.2b"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
        assert_eq!(
            parse("Lantern Glow-123-1711234567.rar"),
            Some(classic("Lantern Glow", 123, None, Some(LANTERN_UPLOAD_MS))),
            "a file uploaded without a version"
        );
    }

    #[test]
    fn classic_names_keep_their_own_hyphens_and_digits() {
        assert_eq!(
            parse("Vernworth-Cloaks-Collection-4512-2-0-1-1711234567.7z"),
            Some(classic(
                "Vernworth-Cloaks-Collection",
                4512,
                Some("2.0.1"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
        assert_eq!(
            parse("DD2 Lantern Glow 4K-321-1-0-1711234567.zip"),
            Some(classic(
                "DD2 Lantern Glow 4K",
                321,
                Some("1.0"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
        assert_eq!(
            parse("Pack-2-123-1-0-1711234567.zip"),
            Some(classic("Pack-2", 123, Some("1.0"), Some(LANTERN_UPLOAD_MS))),
            "a single-digit name part loses to a multi-digit mod id"
        );
        assert_eq!(
            parse("Lantern Glow - Ember Red-88-3-1711234567.zip"),
            Some(classic(
                "Lantern Glow - Ember Red",
                88,
                Some("3"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
    }

    #[test]
    fn single_digit_mod_ids_still_parse_when_timestamped() {
        assert_eq!(
            parse("Lantern Glow-7-1-0-1711234567.zip"),
            Some(classic(
                "Lantern Glow",
                7,
                Some("1.0"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
    }

    #[test]
    fn reads_the_very_old_shape_without_a_timestamp() {
        assert_eq!(
            parse("Lantern Glow-123-1-2.zip"),
            Some(classic("Lantern Glow", 123, Some("1.2"), None))
        );
        assert_eq!(
            parse("Pack-2-123-1-0.zip"),
            Some(classic("Pack-2", 123, Some("1.0"), None))
        );
        assert_eq!(
            parse("Lantern Glow 2-4512-v3.zip"),
            Some(classic("Lantern Glow 2", 4512, Some("v3"), None))
        );
        for too_loose in [
            "Lantern Glow-1-0.zip",
            "Lantern Glow-123.zip",
            "Lantern Glow-123-final.zip",
        ] {
            assert_eq!(
                parse(too_loose),
                None,
                "{too_loose:?} is too loose to trust without a timestamp"
            );
        }
    }

    #[test]
    fn reads_the_2026_shape() {
        assert_eq!(
            parse("Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip"),
            Some(shape_2026(
                "Vernworth Cloaks",
                2207,
                Some("8"),
                VERNWORTH_UPLOAD_MS,
                "Rk4vLmN2Q"
            ))
        );
    }

    #[test]
    fn names_in_the_2026_shape_keep_hyphens_and_digits() {
        assert_eq!(
            parse("Lantern-Glow 2 Remastered 3141 1.2.0 2026-01-05T09-07Z Ab12Cd34E.7z"),
            Some(shape_2026(
                "Lantern-Glow 2 Remastered",
                3141,
                Some("1.2.0"),
                1_767_604_020_000,
                "Ab12Cd34E"
            ))
        );
        assert_eq!(
            parse("4K Vernworth Banners 90 v2.1b 2024-02-29T23-59-30Z k_9-x.rar"),
            Some(shape_2026(
                "4K Vernworth Banners",
                90,
                Some("v2.1b"),
                1_709_251_170_000,
                "k_9-x"
            )),
            "a leap day, a seconds field and a sqid with _ and -"
        );
    }

    #[test]
    fn the_2026_shape_without_a_version() {
        let expected = Some(shape_2026(
            "Vernworth Cloaks",
            2207,
            None,
            VERNWORTH_UPLOAD_MS,
            "Rk4vLmN2Q",
        ));
        assert_eq!(
            parse("Vernworth Cloaks 2207 2026-10-14T14-23Z Rk4vLmN2Q.zip"),
            expected
        );
        assert_eq!(
            parse("Vernworth Cloaks 2207  2026-10-14T14-23Z Rk4vLmN2Q.zip"),
            expected,
            "a double space where the version would be"
        );
    }

    #[test]
    fn browser_duplicate_markers_are_ignored() {
        let shape_2026_expected = Some(shape_2026(
            "Vernworth Cloaks",
            2207,
            Some("8"),
            VERNWORTH_UPLOAD_MS,
            "Rk4vLmN2Q",
        ));
        assert_eq!(
            parse("Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q (1).zip"),
            shape_2026_expected
        );
        assert_eq!(
            parse("Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q(12).zip"),
            shape_2026_expected,
            "Firefox writes no space"
        );
        assert_eq!(
            parse("Lantern Glow-123-1-2-1711234567 (2).7z"),
            Some(classic(
                "Lantern Glow",
                123,
                Some("1.2"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
        assert_eq!(
            parse("Lantern Glow-123-1-2 (1).zip"),
            Some(classic("Lantern Glow", 123, Some("1.2"), None))
        );
    }

    #[test]
    fn stems_folders_and_full_paths_parse_too() {
        let expected = Some(classic(
            "Lantern Glow",
            123,
            Some("1.2"),
            Some(LANTERN_UPLOAD_MS),
        ));
        assert_eq!(parse("Lantern Glow-123-1-2-1711234567"), expected);
        assert_eq!(
            parse(r"C:\Users\someone\Downloads\Lantern Glow-123-1-2-1711234567.zip"),
            expected
        );
        assert_eq!(
            parse("/mnt/library/Lantern Glow-123-1-2-1711234567.zip"),
            expected
        );
        assert_eq!(
            parse("Vernworth Cloaks 2207 1.4 2026-10-14T14-23Z Rk4vLmN2Q"),
            Some(shape_2026(
                "Vernworth Cloaks",
                2207,
                Some("1.4"),
                VERNWORTH_UPLOAD_MS,
                "Rk4vLmN2Q"
            )),
            "a dotted version is not mistaken for an extension"
        );
    }

    /// Every name in `names` must parse to `None`; a failure lists each name
    /// that parsed and what it parsed to, not just the first.
    fn assert_none_parse(names: &[&str]) {
        let parsed: Vec<(&str, NexusFileName)> = names
            .iter()
            .filter_map(|name| parse(name).map(|found| (*name, found)))
            .collect();
        assert!(parsed.is_empty(), "these should not parse: {parsed:#?}");
    }

    #[test]
    fn anything_else_is_not_a_nexus_name() {
        assert_none_parse(&[
            "",
            "Lantern Glow.zip",
            "Lantern Glow v1.2.zip",
            "Lantern Glow (1).zip",
            "Lantern Glow-2.zip",
            "Lantern-Glow-1-0-0.zip",
            "Lantern Glow-1-2-99.zip",
            "-123-1-2-1711234567.zip",
            "Vernworth Cloaks 2207 8 2026-13-14T14-23Z Rk4vLmN2Q.zip",
            "Vernworth Cloaks 2207 8 2026-02-30T14-23Z Rk4vLmN2Q.zip",
            "Vernworth Cloaks 2207 8 2026-10-14T24-00Z Rk4vLmN2Q.zip",
            "Vernworth Cloaks 2207 8 2026-10-14 14-23Z Rk4vLmN2Q.zip",
            "Vernworth Cloaks 2207 8 2026-10-14T14-23Z bad*sqid.zip",
            "2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip",
            // Archives a person named after a calendar date (verifier finding).
            "DD2 Mods Backup 2026-10-14.zip",
            "Lantern Glow 2025-11-03.zip",
            "Lantern Glow 2025-12-31.7z",
            "Backup-2026-10-14.zip",
            "Screenshots 2026-10-14T14-23Z.zip",
            "Vernworth Cloaks 2207 8 2026-10-14T14-23Z.zip",
            "Backup-20241001-1711234567.zip",
        ]);
    }

    #[test]
    fn calendar_dates_never_become_mod_ids() {
        assert_none_parse(&[
            // Year first (ISO), with a suffix, a duplicate marker or a time.
            "Lantern Glow 2025-11-03_final.zip",
            "DD2 Mods Backup 2026-10-14 (1).zip",
            "Pawn Backup 2026-10-14-12-30.zip",
            "Pawn Backup-2026-10-14-12-30.zip",
            "Pawn Backup-2026-10-14-1711234567.zip",
            // Zero-padded month and day are no id-shaped fields, so the
            // tie-break skips them and lands on the hour of the time after.
            "Pawn Backup 2024-09-08-19-47.zip",
            // Unpadded, with the year in the name.
            "Lantern Glow 2022-10-1.7z",
            "Arisen Saves 2018-2-4-11-43.zip",
            // Unpadded after a hyphen, no timestamp: the year would be the
            // mod id, and nothing vouches for reading `8-9` as its version.
            "Screenshots-2016-9-24.zip",
            "Lantern Glow-2026-1-14.zip",
            "Pawn Backup-2025-7-10T12-08Z.zip",
            // Day first (the Dutch order) and month first, padded or not.
            "Arisen Saves 14-10-2026.zip",
            "Arisen Saves 4-10-2026.zip",
            "Arisen Saves 10-14-2026.zip",
            "Arisen Saves-14-10-2026.zip",
            "Arisen Saves-10-14-2026.zip",
            // A 2026 upload stamp with nothing in front of it to split on.
            "Screenshots_2026-10-14T14-23Z.zip",
        ]);
    }

    #[test]
    fn a_refused_mod_id_refuses_the_name_instead_of_steering_to_another_field() {
        // Skipping only the date's own fields would promote the time that
        // follows it: `Pawn Backup 2026-10-14` + mod 12 + version `30`.
        assert_eq!(parse("Pawn Backup 2026-10-14-12-30.zip"), None);
        // A number wider than seven digits is refused the same way, never
        // stepped over to the single digit after it (mod 1, version `0`).
        assert_eq!(parse("Lantern Glow-12345678-1-0-1711234567.zip"), None);
        // In the 2026 shape it must not fall back to reading the version as
        // the mod id (mod 8, named `Vernworth Cloaks 12345678`).
        assert_eq!(
            parse("Vernworth Cloaks 12345678 8 2026-10-14T14-23Z Rk4vLmN2Q.zip"),
            None
        );
    }

    #[test]
    fn mod_ids_stop_at_seven_digits() {
        assert_eq!(
            parse("Lantern Glow-1234567-1-0-1711234567.zip"),
            Some(classic(
                "Lantern Glow",
                1_234_567,
                Some("1.0"),
                Some(LANTERN_UPLOAD_MS)
            )),
            "seven digits is still a mod id"
        );
        assert_none_parse(&[
            "Pawn Backup-20241001-12-30.zip",
            "Lantern Glow-123456789-1-2.zip",
        ]);
        assert_eq!(
            mod_id_from_url("https://www.nexusmods.com/dragonsdogma2/mods/1234567"),
            Some(1_234_567)
        );
        assert_eq!(
            mod_id_from_url("https://www.nexusmods.com/dragonsdogma2/mods/12345678"),
            None
        );
    }

    #[test]
    fn dates_in_a_version_and_year_shaped_ids_still_parse() {
        assert_eq!(
            parse("Lantern Glow-123-2024-10-15-1711234567.zip"),
            Some(classic(
                "Lantern Glow",
                123,
                Some("2024.10.15"),
                Some(LANTERN_UPLOAD_MS)
            )),
            "a file versioned by its date"
        );
        assert_eq!(
            parse("Lantern Glow-123-2024-10-15.zip"),
            Some(classic("Lantern Glow", 123, Some("2024.10.15"), None))
        );
        assert_eq!(
            parse("Lantern Glow-2010-1-2-1711234567.zip"),
            Some(classic(
                "Lantern Glow",
                2010,
                Some("1.2"),
                Some(LANTERN_UPLOAD_MS)
            )),
            "a timestamped year-shaped mod id with an ordinary version is not a date"
        );
        assert_eq!(
            parse("Lantern Glow-2026-1-14-1711234567.zip"),
            Some(classic(
                "Lantern Glow",
                2026,
                Some("1.14"),
                Some(LANTERN_UPLOAD_MS)
            ))
        );
        assert_eq!(
            parse("Lantern Glow-2026-1-0.zip"),
            Some(classic("Lantern Glow", 2026, Some("1.0"), None)),
            "day 0 is no date: version 1.0 survives without a timestamp too"
        );
        assert_eq!(
            parse("Lantern Glow 2077-4512-1-0-1711234567.zip"),
            Some(classic(
                "Lantern Glow 2077",
                4512,
                Some("1.0"),
                Some(LANTERN_UPLOAD_MS)
            )),
            "a year-like number inside the name"
        );
        assert_eq!(
            parse("Lantern Glow 2024-10-15 Edition-123-1-0-1711234567.zip"),
            Some(classic(
                "Lantern Glow 2024-10-15 Edition",
                123,
                Some("1.0"),
                Some(LANTERN_UPLOAD_MS)
            )),
            "a date earlier in the name, not where it ends"
        );
        assert_eq!(
            parse("Vernworth Cloaks 2207 2026-10-14 2026-10-14T14-23Z Rk4vLmN2Q.zip"),
            Some(shape_2026(
                "Vernworth Cloaks",
                2207,
                Some("2026-10-14"),
                VERNWORTH_UPLOAD_MS,
                "Rk4vLmN2Q"
            )),
            "a date as the version of a 2026 name"
        );
    }

    #[test]
    fn the_link_carries_the_filename_source_and_the_sqid() {
        let link = parse("Lantern Glow-123-1-2-1711234567.zip").unwrap().link();
        assert_eq!(link.mod_id, 123);
        assert_eq!(link.version.as_deref(), Some("1.2"));
        assert_eq!(link.uploaded_ms, Some(LANTERN_UPLOAD_MS));
        assert_eq!((link.file_id, link.group_id, link.sqid), (None, None, None));
        assert_eq!(link.source, LinkSource::Filename);

        let link = parse("Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip")
            .unwrap()
            .link();
        assert_eq!(link.sqid.as_deref(), Some("Rk4vLmN2Q"));
        assert_eq!(link.uploaded_ms, Some(VERNWORTH_UPLOAD_MS));
    }

    #[test]
    fn serialises_camel_case() {
        let json = serde_json::to_value(
            parse("Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip").unwrap(),
        )
        .unwrap();
        assert_eq!(json["modId"], 2207);
        assert_eq!(json["uploadedMs"], VERNWORTH_UPLOAD_MS);
        assert_eq!(json["sqid"], "Rk4vLmN2Q");
    }

    #[test]
    fn civil_dates_count_days_like_the_calendar() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1) * 86_400, 951_868_800);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(parse_upload_stamp("1970-01-01T00-00Z"), Some(0));
        assert_eq!(
            parse_upload_stamp("2026-10-14T14-23Z"),
            Some(VERNWORTH_UPLOAD_MS)
        );
        assert_eq!(
            parse_upload_stamp("2026-10-14T14-23"),
            None,
            "the Z is required"
        );
    }

    #[test]
    fn homepage_urls_yield_dd2_mod_ids() {
        for (url, expected) in [
            (
                "https://www.nexusmods.com/dragonsdogma2/mods/123",
                Some(123),
            ),
            (
                "https://www.nexusmods.com/dragonsdogma2/mods/123?tab=files",
                Some(123),
            ),
            ("http://nexusmods.com/DragonsDogma2/mods/45/", Some(45)),
            (
                "www.nexusmods.com/dragonsdogma2/mods/9#description",
                Some(9),
            ),
            (
                "https://next.nexusmods.com/dragonsdogma2/mods/321",
                Some(321),
            ),
            (
                "https://www.nexusmods.com/skyrimspecialedition/mods/123",
                None,
            ),
            ("https://www.nexusmods.com/dragonsdogma2/mods/", None),
            ("https://www.nexusmods.com/dragonsdogma2/mods/12abc", None),
            ("https://evilnexusmods.com/dragonsdogma2/mods/123", None),
            ("not a url", None),
        ] {
            assert_eq!(mod_id_from_url(url), expected, "{url}");
        }
    }
}
