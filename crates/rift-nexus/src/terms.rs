//! Search terms from a package's name (v0.2, Find on Nexus).
//!
//! MD5 cannot identify an unpacked folder (there is no archive to hash) or a
//! repack (its bytes are new), so the Riftstone searches Nexus by NAME. A name
//! on disk carries a lot Nexus's search would trip over: an archive
//! extension, the 2026 download suffix, underscores, a camelCase run, a
//! version, a `(1)` from the browser, the game's own name, the mod manager's.
//! [`search_terms`] keeps only the words Nexus can match against a mod's name.
//!
//! Nexus wants EVERY term in a mod's name (`MATCHES` and `WILDCARD` alike), so
//! the two ways to get a word wrong do not cost the same. A stray word kept
//! (`2b` of `v1.2b`, a sqid) narrows the search to nothing; a real word
//! dropped only widens it, since the mod still holds every word left. So a
//! word that is mostly noise in file names goes even where it was meant as a
//! name (`V8`, `Version`).
//!
//! Pure: no I/O, no network. The words leave the machine only when the player
//! presses Find (see [`crate::NexusClient::find_mods`]).

/// At most this many words go to Nexus.
const MAX_WORDS: usize = 8;
/// At most this many characters go to Nexus.
const MAX_CHARS: usize = 100;
/// The archive extensions a name may still carry.
const ARCHIVE_EXTENSIONS: [&str; 3] = ["zip", "7z", "rar"];
/// Words that only say a file has a version.
const VERSION_WORDS: [&str; 3] = ["v", "ver", "version"];
/// Words that name the game or the mod manager, never the mod. (`Fluffy` on
/// its own is a word as well, see [`keep_words`].)
const GAME_AND_MANAGER_WORDS: [&str; 2] = ["dd2", "fmm"];
/// Names that are camelCase themselves: their two humps stay one word.
const WHOLE_WORDS: [&str; 2] = ["REFramework", "ReShade"];
/// Apostrophes as people (and keyboards) type them; read as `'`.
const APOSTROPHES: [char; 7] = [
    '\'', '\u{2019}', '\u{2018}', '`', '\u{b4}', '\u{2bc}', '\u{ff07}',
];

/// The words to search Nexus with for a package called `name` (its display
/// name, or a file or folder name). `""` when nothing in it is searchable.
///
/// The rules, in order:
///
/// 1. A trailing `.zip`, `.7z` or `.rar` is dropped (any case).
/// 2. A Nexus download name, in the 2026 shape
///    (`Vernworth Cloaks 2207 1.2 2026-10-16T09-30Z Rk4vLmN2Q`) or a classic one
///    (`Vernworth Cloaks-2207-1-2-1700000000`), keeps only its name part, read
///    by rift-core's own parser ([`rift_core::nexus_name::parse`]).
/// 3. `(…)` and `[…]` segments are dropped, nested ones too; a bracket
///    without its partner is read as a space. A name that is ALL brackets
///    (`[Lantern Glow Recolor]`) would keep nothing, so its brackets' words are
///    read after all.
/// 4. A 2026 upload stamp (`2026-10-16T09-30Z`, seconds optional, any case)
///    is dropped wherever it sits, joined to a word or not
///    (`Lantern Glow-2026-10-16T09-30Z`). When one token of `[A-Za-z0-9_-]`
///    follows it after a space or `_` and ends the name, that token is the
///    sqid of a 2026 name the parser could not read, and goes too
///    (`Lantern Glow 2026-10-16T09-30Z Rk4vLmN2Q`).
/// 5. `_`, `.`, `+`, `-` and any other punctuation (`&`, `,`, `!`, `:` …)
///    separate words. Letters, digits and apostrophes inside a word stay
///    (`Arisen's`); typographic apostrophes are written `'`.
/// 6. camelCase splits: `LanternGlowRecolor` → `Lantern Glow Recolor`, while
///    acronyms stay whole (`ErmineCapeHD` → `Ermine Cape HD`,
///    `UIOverhaul` → `UI Overhaul`, `BetterNPCs` → `Better NPCs`), and so do
///    `REFramework` and `ReShade`.
/// 7. Versions go: `v1`, `v1.2.3`, `1.0`, `1.2b`, `ver`, `version`, a lone
///    `v`, any standalone number, and a version that dots or hyphens hold
///    together inside a longer name (`Lantern-Glow-v1.2b`,
///    `Lantern.Glow.1.2.3a`) goes whole. Words that merely hold digits stay
///    (`4K`, `2B`, `1080p`), and so does a multiplier (`2x`, `1.5x`, `x1.5`).
/// 8. The game's name (`Dragon's Dogma 2` or `Dragons Dogma 2` with any
///    apostrophe, `DD2`) and the mod manager's (`Fluffy Mod Manager`,
///    `Fluffy Manager`, `FMM`, and a `Fluffy` that no kept word follows) go,
///    each taking a `for` right before it along (`Lantern Glow for Fluffy`).
///    A `Fluffy` that a word follows is a word (`Fluffy Ermine Cape`).
/// 9. At most 8 words and 100 characters, cut between words (a single word
///    longer than that is cut on a character boundary).
/// 10. Nothing searchable (no word with two or more letters) → `""`.
pub fn search_terms(name: &str) -> String {
    let stem = strip_archive_extension(name.trim());
    let base = match rift_core::nexus_name::parse(stem) {
        Some(parsed) => parsed.name,
        None => stem.to_string(),
    };
    let mut kept = keep_words(&outside_brackets(&base));
    if !searchable(&kept) {
        kept = keep_words(&base.replace(['(', ')', '[', ']'], " "));
    }
    let terms = within_limits(&kept);
    if searchable(&terms) {
        terms.join(" ")
    } else {
        String::new()
    }
}

/// `Lantern Glow.zip` → `Lantern Glow`; a name that is only an extension
/// (`.zip`) keeps nothing.
fn strip_archive_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext))
            if ARCHIVE_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known)) =>
        {
            stem.trim_end()
        }
        _ => name,
    }
}

/// The text with every matched `(…)` and `[…]` (brackets included) blanked
/// out, and every unmatched bracket read as a space.
fn outside_brackets(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut dropped = vec![false; chars.len()];
    // The closer each open bracket waits for, and where it opened.
    let mut open: Vec<(char, usize)> = Vec::new();
    for (at, &c) in chars.iter().enumerate() {
        match c {
            '(' => open.push((')', at)),
            '[' => open.push((']', at)),
            ')' | ']' => {
                // The nearest opener this closes; brackets opened inside it
                // and never closed go with it.
                if let Some(pos) = open.iter().rposition(|&(closer, _)| closer == c) {
                    let start = open[pos].1;
                    open.truncate(pos);
                    dropped[start..=at].fill(true);
                }
            }
            _ => {}
        }
    }
    chars
        .iter()
        .zip(&dropped)
        .map(|(&c, &gone)| {
            if gone || matches!(c, '(' | ')' | '[' | ']') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// A word [`keep_words`] keeps so far.
enum Kept {
    Word(String),
    /// A `Fluffy`, kept only when a kept word follows it: at the end of a
    /// name it is the mod manager's (`Lantern Glow Fluffy`), before a word
    /// it is a word (`Fluffy Ermine Cape`).
    Fluffy(String),
}

/// The words of `text` that are worth searching for, in order.
fn keep_words(text: &str) -> Vec<String> {
    let words = split_words(text);
    let mut kept: Vec<Kept> = Vec::with_capacity(words.len());
    let mut at = 0;
    while at < words.len() {
        if let Some(len) = game_or_manager_name(&words[at..]) {
            drop_trailing_for(&mut kept);
            at += len;
            continue;
        }
        let word = &words[at];
        if word.eq_ignore_ascii_case("fluffy") {
            kept.push(Kept::Fluffy(word.clone()));
        } else if !is_version_word(word) && !is_version(word) {
            kept.push(Kept::Word(word.clone()));
        }
        at += 1;
    }
    // No kept word follows these: they name the manager.
    while matches!(kept.last(), Some(Kept::Fluffy(_))) {
        kept.pop();
        drop_trailing_for(&mut kept);
    }
    kept.into_iter()
        .map(|kept| match kept {
            Kept::Word(word) | Kept::Fluffy(word) => word,
        })
        .collect()
}

/// A `for` right before the game's or the manager's name goes with it
/// (`Lantern Glow for Fluffy`, `Lantern Glow for DD2`).
fn drop_trailing_for(kept: &mut Vec<Kept>) {
    if matches!(kept.last(), Some(Kept::Word(word)) if word.eq_ignore_ascii_case("for")) {
        kept.pop();
    }
}

/// How many of the words at the start of `words` name the game or the mod
/// manager: `Dragon's Dogma 2`, `Fluffy Mod Manager` (3), `Fluffy Manager`
/// (2), `DD2`, `FMM` (1). A lone `Fluffy` is judged by [`keep_words`].
fn game_or_manager_name(words: &[String]) -> Option<usize> {
    let is = |at: usize, wanted: &str| {
        words
            .get(at)
            .is_some_and(|word| word.eq_ignore_ascii_case(wanted))
    };
    if is_game_title(words) || (is(0, "fluffy") && is(1, "mod") && is(2, "manager")) {
        Some(3)
    } else if is(0, "fluffy") && is(1, "manager") {
        Some(2)
    } else {
        words
            .first()
            .is_some_and(|word| {
                GAME_AND_MANAGER_WORDS
                    .iter()
                    .any(|noise| word.eq_ignore_ascii_case(noise))
            })
            .then_some(1)
    }
}

/// Every word of `text`: upload stamps (and a sqid after one) gone,
/// punctuation separating, a version or a multiplier that dots or hyphens
/// hold together kept as one word to be judged whole, camelCase split.
fn split_words(text: &str) -> Vec<String> {
    let text: String = text
        .chars()
        .map(|c| if APOSTROPHES.contains(&c) { '\'' } else { c })
        .collect();
    let text = without_upload_stamps(&text);
    // Coarse tokens keep `.` and `-`, so a dotted version is judged whole
    // before its dots become separators too.
    text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '\'' | '.' | '-')))
        .filter(|token| !token.is_empty())
        .flat_map(token_words)
        .collect()
}

/// The words of one run of letters, digits, apostrophes, dots and hyphens.
/// Dots, hyphens and camelCase humps divide it, except that the longest
/// version or multiplier the dots and hyphens tie together stays one word
/// (`Lantern-Glow-v1.2b` → `Lantern`, `Glow`, `v1.2b`; `1.5xSpeed` → `1.5x`,
/// `Speed`).
fn token_words(token: &str) -> Vec<String> {
    // Every hump, with the dot or hyphen that ties it to the hump before:
    // none at the start, across a camelCase boundary, or after a piece with
    // no word in it (`1..2`).
    let mut humps: Vec<(Option<char>, String)> = Vec::new();
    let mut tie = None;
    let mut start = 0;
    let piece_ends = token
        .char_indices()
        .filter(|&(_, c)| matches!(c, '.' | '-'))
        .chain([(token.len(), '.')]);
    for (end, separator) in piece_ends {
        let before = humps.len();
        for hump in camel_words(&token[start..end]) {
            let hump = hump.trim_matches('\'');
            if hump.chars().any(char::is_alphanumeric) {
                let tied = if humps.len() == before { tie } else { None };
                humps.push((tied, hump.to_string()));
            }
        }
        tie = (humps.len() > before).then_some(separator);
        start = end + separator.len_utf8();
    }

    let mut words = Vec::with_capacity(humps.len());
    let mut at = 0;
    while at < humps.len() {
        // The longest run from here that the ties make a version or a
        // multiplier, spelled as it was written.
        let mut joined = humps[at].1.clone();
        let mut run: Option<(usize, String)> = None;
        let mut next = at + 1;
        while let Some((Some(tie), hump)) = humps.get(next) {
            joined.push(*tie);
            joined.push_str(hump);
            if is_version(&joined) || is_multiplier(&joined) {
                run = Some((next, joined.clone()));
            }
            next += 1;
        }
        match run {
            Some((last, word)) => {
                words.push(word);
                at = last + 1;
            }
            None => {
                words.push(humps[at].1.clone());
                at += 1;
            }
        }
    }
    words
}

/// `LanternGlowRecolor` → `Lantern`, `Glow`, `Recolor`. An acronym stays
/// whole (`HDErmine` → `HD`, `Ermine`; `NPCs` stays), and so does a run of
/// digits and capitals (`4K`, `DD2`) and a name in [`WHOLE_WORDS`]
/// (`REFramework`, not `RE Framework`).
fn camel_words(piece: &str) -> Vec<String> {
    let chars: Vec<char> = piece.chars().collect();
    // Lowercase letters from `at` on (`Overhaul` after `UI` → 7).
    let lower_run = |at: usize| {
        chars[at.min(chars.len())..]
            .iter()
            .take_while(|c| c.is_lowercase())
            .count()
    };
    let mut humps: Vec<String> = Vec::new();
    let mut start = 0;
    for at in 1..chars.len() {
        let (before, here) = (chars[at - 1], chars[at]);
        // A capital that opens a word of two or more lowercase letters ends
        // the acronym or number before it (`UIOverhaul`, `4KTextures`), but
        // not a plural's `s` (`NPCs`).
        let opens_word = here.is_uppercase() && lower_run(at + 1) >= 2;
        let boundary = (before.is_lowercase() && here.is_uppercase())
            || ((before.is_uppercase() || before.is_ascii_digit()) && opens_word);
        if boundary {
            humps.push(chars[start..at].iter().collect());
            start = at;
        }
    }
    if start < chars.len() {
        humps.push(chars[start..].iter().collect());
    }
    let mut words: Vec<String> = Vec::with_capacity(humps.len());
    for hump in humps {
        match words.last_mut() {
            Some(last)
                if WHOLE_WORDS
                    .iter()
                    .any(|whole| whole.eq_ignore_ascii_case(&format!("{last}{hump}"))) =>
            {
                last.push_str(&hump);
            }
            _ => words.push(hump),
        }
    }
    words
}

/// `Dragon's Dogma 2` / `Dragons Dogma 2` at the start of `words`.
fn is_game_title(words: &[String]) -> bool {
    match words {
        [first, second, third, ..] => {
            matches!(first.to_lowercase().as_str(), "dragon's" | "dragons")
                && second.to_lowercase() == "dogma"
                && third == "2"
        }
        _ => false,
    }
}

fn is_version_word(word: &str) -> bool {
    VERSION_WORDS
        .iter()
        .any(|version| word.eq_ignore_ascii_case(version))
}

/// `v1`, `V2b`, `v1.2.3`, `v1-2`, `ver2`, `version.3`, `1.0`, `1.2b`,
/// `2026-10-16` and any plain number. A word that merely holds digits (`4K`,
/// `2B`, `1080p`) is no version, and neither is a multiplier (`1.5x`).
fn is_version(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    let prefixed = ["version", "ver", "v"]
        .iter()
        .find_map(|prefix| lower.strip_prefix(prefix))
        .map(|rest| rest.strip_prefix(['.', '-']).unwrap_or(rest));
    match prefixed {
        Some(rest) => digit_runs(rest.strip_suffix(is_ascii_letter).unwrap_or(rest)) >= 1,
        None => match lower.strip_suffix(is_ascii_letter) {
            // `1.2b`: a letter ends only a version of two parts or more, and
            // an `x` makes it a multiplier (`1.5x`).
            Some(body) => !lower.ends_with('x') && digit_runs(body) >= 2,
            None => digit_runs(&lower) >= 1,
        },
    }
}

/// `2x`, `1.5x`, `x1.5`: a number with an `x` at one end, a word of the
/// name (`1.5x Lantern Glow`), never a version.
fn is_multiplier(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    lower
        .strip_suffix('x')
        .or_else(|| lower.strip_prefix('x'))
        .is_some_and(|number| digit_runs(number) >= 1)
}

fn is_ascii_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
}

/// How many runs of ASCII digits `text` is, joined by single dots or
/// hyphens (`1.2-3` → 3); 0 when it is anything else (`""`, `1..2`, `1.`).
fn digit_runs(text: &str) -> usize {
    let runs: Vec<&str> = text.split(['.', '-']).collect();
    if runs
        .iter()
        .all(|run| !run.is_empty() && run.bytes().all(|b| b.is_ascii_digit()))
    {
        runs.len()
    } else {
        0
    }
}

/// `text` with every 2026 upload stamp read as a space, wherever it sits,
/// and without the sqid that ends a 2026 download name after its stamp (see
/// rule 4 of [`search_terms`]). A stamp is never the tail of a longer
/// number, and no letter or digit follows its `Z`.
fn without_upload_stamps(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let mut before: Option<char> = None;
    while let Some(c) = rest.chars().next() {
        let stamp = [20, 17].into_iter().find(|&len| {
            !before.is_some_and(|b| b.is_ascii_digit())
                && rest.get(..len).is_some_and(is_upload_stamp)
                && !rest[len..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric)
        });
        let Some(len) = stamp else {
            out.push(c);
            before = Some(c);
            rest = &rest[c.len_utf8()..];
            continue;
        };
        out.push(' ');
        before = None;
        rest = &rest[len..];
        let after = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '_');
        if after.len() < rest.len() && is_sqid(after.trim_end()) {
            break;
        }
    }
    out
}

/// A Nexus sqid as rift-core's parser reads one: 1–32 characters of
/// `[A-Za-z0-9_-]`.
fn is_sqid(token: &str) -> bool {
    (1..=32).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `2026-10-16T09-30Z` (seconds optional): the upload stamp of a 2026
/// download name, left behind when the rest of the name was not in that shape.
fn is_upload_stamp(token: &str) -> bool {
    let bytes = token.as_bytes();
    let digit_at = |at: usize| bytes.get(at).is_some_and(u8::is_ascii_digit);
    let digits = |from: usize, count: usize| (from..from + count).all(digit_at);
    let byte_at = |at: usize, wanted: &[u8]| bytes.get(at).is_some_and(|b| wanted.contains(b));
    let stamp = |len: usize| {
        bytes.len() == len
            && digits(0, 4)
            && byte_at(4, b"-")
            && digits(5, 2)
            && byte_at(7, b"-")
            && digits(8, 2)
            && byte_at(10, b"Tt")
            && digits(11, 2)
            && byte_at(13, b"-")
            && digits(14, 2)
            && byte_at(len - 1, b"Zz")
    };
    stamp(17) || (stamp(20) && byte_at(16, b"-") && digits(17, 2))
}

/// Does any word hold two or more letters?
fn searchable<S: AsRef<str>>(words: &[S]) -> bool {
    words
        .iter()
        .any(|word| word.as_ref().chars().filter(|c| c.is_alphabetic()).count() >= 2)
}

/// The first [`MAX_WORDS`] words that fit in [`MAX_CHARS`] characters,
/// joined by single spaces. A first word that is too long on its own is cut.
fn within_limits(words: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    let mut length = 0;
    for word in words.iter().take(MAX_WORDS) {
        let width = word.chars().count();
        let needed = if kept.is_empty() { width } else { width + 1 };
        if length + needed > MAX_CHARS {
            if kept.is_empty() {
                kept.push(word.chars().take(MAX_CHARS).collect());
            }
            break;
        }
        length += needed;
        kept.push(word.clone());
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name in `cases` must give its terms; a failure lists each name
    /// that did not, not just the first.
    fn assert_terms(cases: &[(&str, &str)]) {
        let wrong: Vec<(&str, String, &str)> = cases
            .iter()
            .map(|(name, expected)| (*name, search_terms(name), *expected))
            .filter(|(_, got, expected)| got != expected)
            .collect();
        assert!(wrong.is_empty(), "(name, got, expected): {wrong:#?}");
    }

    #[test]
    fn archive_extensions_are_dropped_in_any_case() {
        assert_terms(&[
            ("Lantern Glow Recolor.zip", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor.7z", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor.RAR", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor.Zip", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor", "Lantern Glow Recolor"),
            // Another extension is a word of the name, read like one.
            ("Lantern Glow Recolor.pak", "Lantern Glow Recolor pak"),
        ]);
    }

    #[test]
    fn a_nexus_download_name_keeps_only_its_name_part() {
        assert_terms(&[
            // The 2026 shape: name, mod id, version, upload stamp, sqid.
            (
                "Vernworth Cloaks Collection 2207 1.2 2026-10-16T09-30Z Rk4vLmN2Q.zip",
                "Vernworth Cloaks Collection",
            ),
            (
                "Vernworth Cloaks Collection 2207 1.2 2026-10-16T09-30Z Rk4vLmN2Q",
                "Vernworth Cloaks Collection",
            ),
            (
                "Vernworth Cloaks Collection 2207 2026-10-16T09-30Z Rk4vLmN2Q.7z",
                "Vernworth Cloaks Collection",
            ),
            // The classic shape, timestamped and very old.
            (
                "Lantern Glow Recolor-2207-1-2-1700000000.zip",
                "Lantern Glow Recolor",
            ),
            (
                "Lantern Glow Recolor-2207-1-2-1700000000 (1).zip",
                "Lantern Glow Recolor",
            ),
            ("Lantern Glow Recolor-2207-1-2", "Lantern Glow Recolor"),
            // The name part then goes through every other rule.
            (
                "Lantern_Glow_Recolor 2207 1.2 2026-10-16T09-30Z Rk4vLmN2Q.zip",
                "Lantern Glow Recolor",
            ),
        ]);
    }

    #[test]
    fn an_upload_stamp_outside_the_2026_shape_is_dropped_too() {
        // No sqid after it: not a 2026 name, but the stamp is still no word.
        assert_terms(&[
            ("Lantern Glow 2026-10-16T09-30Z", "Lantern Glow"),
            ("Lantern Glow 2026-10-16T09-30-15Z", "Lantern Glow"),
            ("Lantern Glow_2026-10-16t09-30z", "Lantern Glow"),
        ]);
    }

    #[test]
    fn underscores_dots_pluses_and_hyphens_between_words_become_spaces() {
        assert_terms(&[
            ("Lantern_Glow_Recolor", "Lantern Glow Recolor"),
            ("Lantern.Glow.Recolor", "Lantern Glow Recolor"),
            ("Lantern+Glow+Recolor", "Lantern Glow Recolor"),
            ("Lantern-Glow-Recolor", "Lantern Glow Recolor"),
            (
                "Vernworth Cloaks - Ermine Cape",
                "Vernworth Cloaks Ermine Cape",
            ),
            ("Ermine Cape -", "Ermine Cape"),
            ("-Ermine-Cape-", "Ermine Cape"),
        ]);
    }

    #[test]
    fn other_punctuation_separates_words_and_apostrophes_stay_inside_them() {
        assert_terms(&[
            ("Cloaks & Hoods, Vol. 2!", "Cloaks Hoods Vol"),
            ("Ermine Cape: Winter/Summer", "Ermine Cape Winter Summer"),
            ("Arisen's Ermine Cape", "Arisen's Ermine Cape"),
            ("Arisen\u{2019}s Ermine Cape", "Arisen's Ermine Cape"),
            ("'Ermine' Cape", "Ermine Cape"),
            ("\u{2018}Ermine\u{2019} Cape", "Ermine Cape"),
            ("Ermine Cape {Winter}", "Ermine Cape Winter"),
        ]);
    }

    #[test]
    fn camel_case_splits_and_acronyms_stay_whole() {
        assert_terms(&[
            ("LanternGlowRecolor", "Lantern Glow Recolor"),
            ("lanternGlowRecolor", "lantern Glow Recolor"),
            ("ErmineCapeHD", "Ermine Cape HD"),
            ("HDErmineCape", "HD Ermine Cape"),
            ("BetterUI", "Better UI"),
            ("UIOverhaul", "UI Overhaul"),
            ("BetterNPCs", "Better NPCs"),
            ("4KErmineCape", "4K Ermine Cape"),
            ("Arisen'sErmineCape", "Arisen's Ermine Cape"),
            ("LANTERN GLOW RECOLOR", "LANTERN GLOW RECOLOR"),
            ("LANTERNGlow", "LANTERN Glow"),
            ("\u{c9}peeDuSoleil", "\u{c9}pee Du Soleil"),
        ]);
    }

    #[test]
    fn round_and_square_brackets_are_dropped_with_what_they_hold() {
        assert_terms(&[
            ("Lantern Glow Recolor (Ember Red)", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor [1080p]", "Lantern Glow Recolor"),
            (
                "[DD2 Mods] Lantern Glow Recolor (1)",
                "Lantern Glow Recolor",
            ),
            ("Lantern Glow ((alt) red) Recolor", "Lantern Glow Recolor"),
            ("Lantern Glow ([red)] Recolor", "Lantern Glow Recolor"),
            ("Lantern(Ember)Glow", "Lantern Glow"),
            // A bracket without its partner is a space.
            ("Lantern Glow (Ember", "Lantern Glow Ember"),
            ("Lantern Glow] Ember", "Lantern Glow Ember"),
        ]);
    }

    #[test]
    fn a_name_that_is_all_brackets_reads_its_brackets_after_all() {
        assert_terms(&[
            ("[Lantern Glow Recolor]", "Lantern Glow Recolor"),
            ("(Lantern Glow Recolor) v1.2", "Lantern Glow Recolor"),
            ("[DD2] (Ermine Cape)", "Ermine Cape"),
            ("(1)", ""),
        ]);
    }

    #[test]
    fn version_tokens_and_standalone_numbers_are_dropped() {
        assert_terms(&[
            ("Lantern Glow Recolor v1", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor V2", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor v1.2.3", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor v1.2b", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor 1.0", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor 1.2a", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor ver 2", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor Ver.2", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor version 3", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor Version3", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor v 1.2", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor 2", "Lantern Glow Recolor"),
            ("2 Lantern 2207 Glow 100", "Lantern Glow"),
            ("Lantern_Glow_v1-2", "Lantern Glow"),
            ("LanternGlowV2", "Lantern Glow"),
            ("Lantern Glow 2026-10-16", "Lantern Glow"),
        ]);
    }

    #[test]
    fn a_version_held_together_by_dots_or_hyphens_goes_whole() {
        // The probe of 2026-10-16: a letter-ended version split at its dot
        // or hyphen left its tail behind as a word (`Lantern Glow 2b`).
        assert_terms(&[
            ("Lantern.Glow.v1.2b", "Lantern Glow"),
            ("Lantern-Glow-v1.2b", "Lantern Glow"),
            ("Lantern-Glow-1.2.3a", "Lantern Glow"),
            ("Lantern-Glow-v1-2b", "Lantern Glow"),
            ("Lantern.Glow.1.2b", "Lantern Glow"),
            ("Lantern.Glow.v.1.2b", "Lantern Glow"),
            ("LanternGlowV1.2b", "Lantern Glow"),
            ("Lantern Glow-ver.2b", "Lantern Glow"),
            // A word that merely holds digits is still no version.
            ("Ermine-Cape-2B", "Ermine Cape 2B"),
            ("2B-Ermine-Cape", "2B Ermine Cape"),
            ("Ermine.Cape.4K.v2", "Ermine Cape 4K"),
        ]);
    }

    #[test]
    fn an_upload_stamp_is_dropped_wherever_it_sits() {
        // Joined to the name by a hyphen, a dot or nothing at all.
        assert_terms(&[
            ("Lantern Glow-2026-10-16T09-30Z", "Lantern Glow"),
            ("Lantern-Glow-2026-10-16T09-30-15Z", "Lantern Glow"),
            ("Lantern.Glow.2026-10-16t09-30z", "Lantern Glow"),
            ("LanternGlow2026-10-16T09-30Z", "Lantern Glow"),
            ("Lantern Glow-2026-10-16T09-30Z-Ember", "Lantern Glow Ember"),
        ]);
    }

    #[test]
    fn the_sqid_that_ends_a_name_after_its_stamp_goes_too() {
        // A 2026 download name the parser refused (no mod id, say) still ends
        // in its upload stamp and its sqid; neither is a word.
        assert_terms(&[
            ("Lantern Glow 2026-10-16T09-30Z Rk4vLmN2Q", "Lantern Glow"),
            (
                "Lantern Glow 1.2 2026-10-16T09-30Z Rk4vLmN2Q.zip",
                "Lantern Glow",
            ),
            (
                "Lantern Glow 2026-10-16T09-30Z Rk4vLmN2Q (1)",
                "Lantern Glow",
            ),
            (
                "Lantern_Glow_2207_1.2_2026-10-16T09-30Z_Rk4vLmN2Q",
                "Lantern Glow",
            ),
            ("Lantern Glow 2026-10-16T09-30Z Rk4v-LmN2Q", "Lantern Glow"),
            // Only a single token that ends the name: words further on stay.
            (
                "Lantern 2026-10-16T09-30Z Glow Recolor",
                "Lantern Glow Recolor",
            ),
        ]);
    }

    #[test]
    fn a_multiplier_is_a_word_of_the_name() {
        assert_terms(&[
            ("1.5x Lantern Glow", "1.5x Lantern Glow"),
            ("Lantern Glow 2.5X", "Lantern Glow 2.5X"),
            ("Lantern Glow x1.5", "Lantern Glow x1.5"),
            ("Lantern_Glow_1.5x_v2", "Lantern Glow 1.5x"),
            ("2x Ermine Cape", "2x Ermine Cape"),
            ("1.5xLanternGlow", "1.5x Lantern Glow"),
        ]);
    }

    #[test]
    fn fluffy_is_noise_only_as_the_managers_name() {
        assert_terms(&[
            // The manager's name, wherever it stands, takes a `for` before
            // it along.
            ("Lantern Glow FMM", "Lantern Glow"),
            ("Lantern Glow (Fluffy Mod Manager)", "Lantern Glow"),
            (
                "Lantern Glow Fluffy Mod Manager Edition",
                "Lantern Glow Edition",
            ),
            ("Lantern Glow for Fluffy Manager", "Lantern Glow"),
            ("LanternGlow_FluffyModManager", "Lantern Glow"),
            ("Lantern Glow for FMM", "Lantern Glow"),
            // A Fluffy that ends the name is the manager's name too…
            ("Lantern Glow Fluffy", "Lantern Glow"),
            ("Lantern Glow - Fluffy Version", "Lantern Glow"),
            ("Lantern Glow for FLUFFY", "Lantern Glow"),
            ("Lantern Glow Fluffy v2 (1)", "Lantern Glow"),
            // …and one a word follows is a word.
            ("Fluffy Ermine Cape", "Fluffy Ermine Cape"),
            ("Vernworth Fluffy Cloaks", "Vernworth Fluffy Cloaks"),
            ("Fluffy Ermine Cape FMM", "Fluffy Ermine Cape"),
            // `for` stays where no manager or game follows it.
            ("Vernworth Cloaks for Pawns", "Vernworth Cloaks for Pawns"),
        ]);
    }

    #[test]
    fn names_that_are_camel_case_themselves_stay_whole() {
        assert_terms(&[
            ("ReShade Preset Lantern Glow", "ReShade Preset Lantern Glow"),
            ("LanternGlowReShade", "Lantern Glow ReShade"),
            ("REFramework Lantern Glow", "REFramework Lantern Glow"),
            ("Lantern_Glow_REFramework_v2", "Lantern Glow REFramework"),
            ("reframework", "reframework"),
            ("RESHADE", "RESHADE"),
        ]);
    }

    #[test]
    fn a_word_shaped_like_a_version_goes_even_in_a_name() {
        // `V8` and `Version` can be words of a mod's name, but in a file
        // name they are far more often its version. Dropping a word only
        // widens the search (the right mod still holds every word left);
        // keeping a version narrows it to nothing.
        assert_terms(&[
            ("V8 Lantern Glow", "Lantern Glow"),
            ("Lantern Glow Lore Version", "Lantern Glow Lore"),
        ]);
    }

    #[test]
    fn words_that_merely_hold_digits_stay() {
        assert_terms(&[
            ("Lantern Glow 4K", "Lantern Glow 4K"),
            ("2B Ermine Cape", "2B Ermine Cape"),
            ("Ermine Cape 1080p", "Ermine Cape 1080p"),
            ("Ermine Cape x64", "Ermine Cape x64"),
            ("Vernworth2 Cloaks", "Vernworth2 Cloaks"),
        ]);
    }

    #[test]
    fn noise_words_and_the_games_own_name_are_dropped() {
        assert_terms(&[
            ("Lantern Glow Fluffy FMM", "Lantern Glow"),
            ("DD2 Lantern Glow", "Lantern Glow"),
            ("dd2_lantern_glow", "lantern glow"),
            ("Lantern Glow for DD2", "Lantern Glow"),
            ("Lantern Glow for Dragon's Dogma 2", "Lantern Glow"),
            ("Dragon's Dogma 2 Lantern Glow", "Lantern Glow"),
            ("Dragons Dogma 2 - Lantern Glow", "Lantern Glow"),
            ("Dragon\u{2019}s Dogma 2 Lantern Glow", "Lantern Glow"),
            ("Dragon`s Dogma 2 Lantern Glow", "Lantern Glow"),
            ("dragon's dogma 2_lantern_glow", "lantern glow"),
            ("Lantern Glow (Dragon's Dogma 2)", "Lantern Glow"),
            ("Lantern Glow - DRAGONS DOGMA 2", "Lantern Glow"),
            // Only the whole title: the words on their own are names.
            ("Dragon's Lantern", "Dragon's Lantern"),
            ("Dogma of the Lantern", "Dogma of the Lantern"),
            ("Dragons Dogma Lantern", "Dragons Dogma Lantern"),
        ]);
    }

    #[test]
    fn whitespace_collapses_to_single_spaces() {
        assert_terms(&[
            ("  Lantern   Glow\tRecolor  ", "Lantern Glow Recolor"),
            ("Lantern\nGlow\r\nRecolor", "Lantern Glow Recolor"),
            ("Lantern _ . + Glow", "Lantern Glow"),
        ]);
    }

    #[test]
    fn at_most_eight_words_and_a_hundred_characters_go_out() {
        assert_eq!(
            search_terms("one two three four five six seven eight nine ten"),
            "one two three four five six seven eight"
        );
        // Dropped words do not count against the eight.
        assert_eq!(
            search_terms("DD2 one v1 two 3 three FMM four five six seven eight nine"),
            "one two three four five six seven eight"
        );
        let long = "vernworthian".repeat(3); // 36 letters, no camelCase inside
        let terms = search_terms(&format!("{long} {long} {long} {long}"));
        assert_eq!(terms, format!("{long} {long}"), "cut between words");
        assert!(terms.chars().count() <= 100);
        let huge = "\u{e9}".repeat(150);
        let cut = search_terms(&huge);
        assert_eq!(cut.chars().count(), 100, "one long word is cut");
        assert!(
            cut.chars().all(|c| c == '\u{e9}'),
            "on a character boundary"
        );
    }

    #[test]
    fn nothing_searchable_is_the_empty_string() {
        assert_terms(&[
            ("", ""),
            ("   ", ""),
            (".zip", ""),
            ("v1.2", ""),
            ("DD2 1.0", ""),
            ("Fluffy FMM dd2", ""),
            ("Dragon's Dogma 2", ""),
            ("(1)", ""),
            ("2207", ""),
            ("4K", ""),
            ("A B", ""),
            ("- _ + .", ""),
            ("2207 1.2 2026-10-16T09-30Z", ""),
        ]);
        // One letter-pair is enough.
        assert_eq!(search_terms("UI"), "UI");
        assert_eq!(search_terms("4K UI"), "4K UI");
    }

    #[test]
    fn letters_of_every_script_are_words() {
        assert_terms(&[
            ("\u{c9}p\u{e9}e du Soleil", "\u{c9}p\u{e9}e du Soleil"),
            ("\u{706f}\u{7b3c}", "\u{706f}\u{7b3c}"),
            ("Laterne_Gl\u{fc}hen", "Laterne Gl\u{fc}hen"),
        ]);
    }

    #[test]
    fn the_version_and_stamp_shapes_are_exact() {
        for version in [
            "1",
            "12",
            "1.2",
            "1.2.3",
            "1.2b",
            "v1",
            "V1.2",
            "v1.2b",
            "ver2",
            "version.3",
            "v1-2b",
            "1-2",
            "1.2-3a",
            "2026-10-16",
        ] {
            assert!(is_version(version), "{version} is a version");
        }
        for word in [
            "4K", "2B", "1080p", "v", "ver", "va", "v1a2", "1..2", "1.", ".1", "-1", "1-", "x64",
            "1.5x", "2.5X", "x1.5", "2x", "",
        ] {
            assert!(!is_version(word), "{word:?} is no version");
        }
        for multiplier in ["2x", "1.5x", "2.5X", "x1.5", "X2", "1-5x"] {
            assert!(is_multiplier(multiplier), "{multiplier} is a multiplier");
        }
        for word in ["x", "xx", "1.2b", "x1.5x", "1..5x", "Lux"] {
            assert!(!is_multiplier(word), "{word:?} is no multiplier");
        }
        assert_eq!(digit_runs("1.2-3"), 3);
        for not_runs in ["", "1..2", "1.", "-1", "1a"] {
            assert_eq!(digit_runs(not_runs), 0, "{not_runs:?}");
        }
        assert!(is_upload_stamp("2026-10-16T09-30Z"));
        assert!(is_upload_stamp("2026-10-16T09-30-15Z"));
        for token in [
            "2026-10-16",
            "2026-10-16T09-30",
            "2026-10-16T09:30Z",
            "2026-1-16T09-30Z",
        ] {
            assert!(!is_upload_stamp(token), "{token} is no stamp");
        }
    }
}
