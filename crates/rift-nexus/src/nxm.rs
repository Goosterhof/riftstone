//! `nxm://` links: what Nexus's Mod Manager Download button hands a mod
//! manager (v0.3).
//!
//! `nxm://dragonsdogma2/mods/<mod>/files/<file>?key=…&expires=…&user_id=…`
//! asks for one file. Its `key` and `expires` are a grant: they let a free
//! account fetch that file's download link through v1 (`download_link.json`
//! with `?key=…&expires=…`), which Nexus otherwise serves to premium accounts
//! only. `nxm://premium` tells a mod manager the account just became
//! premium. Collections (`nxm://<game>/collections/…`) and other games' links
//! are refused with a sentence.
//!
//! An arriving link is untrusted input: it is parsed strictly, its ids must
//! be ones Nexus can hold, its `key` must be printable ASCII of a sane
//! length, and a key without its expiry (or the other way round), or a
//! `user_id` that is not a number, makes the link unreadable rather than
//! quietly weaker. Unknown parameters are ignored.
//!
//! The grant's `key` is a secret. It is never logged, never in an error,
//! never sent to the window: [`NxmGrant`]'s `Debug` shows `<redacted>`, and
//! neither it nor [`NxmLink`] serializes.

use crate::GAME_DOMAIN;

/// The largest id Nexus's GraphQL `Int` can carry.
const LARGEST_ID: u64 = i32::MAX as u64;
/// The longest `key` an nxm link may carry.
const LARGEST_KEY: usize = 512;

/// What an `nxm://` link asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NxmMessage {
    /// One file of a Dragon's Dogma 2 mod.
    File(NxmLink),
    /// `nxm://premium`: the account became premium; validate it again.
    Premium,
}

/// One file of a Dragon's Dogma 2 mod, as the site's Mod Manager Download
/// button names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NxmLink {
    pub mod_id: u64,
    pub file_id: u64,
    /// The site's permission to fetch this file's download link without
    /// premium (`key` + `expires`). `None` when the link carried neither.
    pub grant: Option<NxmGrant>,
    /// The account the site made the link for (`user_id`), when it says.
    pub user_id: Option<u64>,
}

impl NxmLink {
    /// Was this link made for the account `account_user_id`? True unless both
    /// ids are known and differ: a link for another account is refused
    /// (rift-core's `LinkNotYours`), while a side that does not say cannot be
    /// held against the link.
    pub fn made_for(&self, account_user_id: Option<u64>) -> bool {
        match (self.user_id, account_user_id) {
            (Some(link), Some(account)) => link == account,
            _ => true,
        }
    }
}

/// An nxm link's `key` and `expires`: what lets a free account download one
/// file through v1. The `key` is a secret: `Debug` shows `<redacted>`.
#[derive(Clone, PartialEq, Eq)]
pub struct NxmGrant {
    pub key: String,
    /// When the grant runs out, unix seconds.
    pub expires_s: u64,
}

impl NxmGrant {
    /// Has the grant run out at `now_s` (unix seconds)? A grant is good up to,
    /// not including, its `expires` second.
    pub fn expired(&self, now_s: u64) -> bool {
        now_s >= self.expires_s
    }
}

impl std::fmt::Debug for NxmGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NxmGrant")
            .field("key", &"<redacted>")
            .field("expires_s", &self.expires_s)
            .finish()
    }
}

/// Why an `nxm://` link is not one the Riftstone acts on. Each is a sentence
/// the window can show; none repeats anything from the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum NxmRefusal {
    /// A link for another game on Nexus.
    #[error(
        "That Nexus Mods link is for another game. The Riftstone handles Dragon's Dogma 2 \
         mods only."
    )]
    OtherGame,
    /// A collection: the Riftstone downloads single mod files.
    #[error(
        "That Nexus Mods link opens a collection. The Riftstone downloads single mod files: \
         download the collection's mods from their own pages."
    )]
    Collection,
    /// Not an nxm link the Riftstone can read.
    #[error(
        "That nxm link could not be read. Press Mod Manager Download on the file's Nexus Mods \
         page again."
    )]
    Unreadable,
}

/// Read an `nxm://` link.
///
/// - `nxm://dragonsdogma2/mods/<mod>/files/<file>[?key=…&expires=…&user_id=…]`
///   → [`NxmMessage::File`] (the game and the words any case; a trailing `/`
///   is fine; ids 1..=`i32::MAX`; unknown parameters ignored).
/// - `nxm://premium` → [`NxmMessage::Premium`].
/// - another game → [`NxmRefusal::OtherGame`]; a collection of this game →
///   [`NxmRefusal::Collection`]; anything else (another scheme, another path,
///   a key without its expiry or the other way round, an expiry or `user_id`
///   that is no number, a key that is blank, too long or not printable ASCII)
///   → [`NxmRefusal::Unreadable`].
///
/// Surrounding whitespace, quotes and angle brackets are trimmed (Windows
/// hands the link over as `"%1"`).
pub fn parse_nxm(text: &str) -> Result<NxmMessage, NxmRefusal> {
    let text = text
        .trim()
        .trim_matches(|c: char| matches!(c, '<' | '>' | '"' | '\''))
        .trim();
    let url = reqwest::Url::parse(text).map_err(|_| NxmRefusal::Unreadable)?;
    if url.scheme() != "nxm" {
        return Err(NxmRefusal::Unreadable);
    }
    let host = url
        .host_str()
        .map(str::to_ascii_lowercase)
        .filter(|host| {
            !host.is_empty()
                && host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        .ok_or(NxmRefusal::Unreadable)?;
    if host == "premium" {
        return Ok(NxmMessage::Premium);
    }
    if host != GAME_DOMAIN {
        return Err(NxmRefusal::OtherGame);
    }
    let segments: Vec<String> = url
        .path_segments()
        .ok_or(NxmRefusal::Unreadable)?
        .filter(|segment| !segment.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
    let (mod_id, file_id) = match segments.as_slice() {
        ["collections", ..] => return Err(NxmRefusal::Collection),
        ["mods", mod_id, "files", file_id] => (id_in_range(mod_id)?, id_in_range(file_id)?),
        _ => return Err(NxmRefusal::Unreadable),
    };
    let params = Params::read(url.query().unwrap_or(""))?;
    let grant = match (params.key, params.expires) {
        (None, None) => None,
        (Some(key), Some(expires)) => Some(NxmGrant {
            key: usable_key(key)?,
            expires_s: number(&expires)?,
        }),
        _ => return Err(NxmRefusal::Unreadable),
    };
    let user_id = params
        .user_id
        .map(|id| {
            number(&id)
                .ok()
                .filter(|id| *id > 0)
                .ok_or(NxmRefusal::Unreadable)
        })
        .transpose()?;
    Ok(NxmMessage::File(NxmLink {
        mod_id,
        file_id,
        grant,
        user_id,
    }))
}

/// The three parameters the Riftstone reads; the first of each counts.
#[derive(Default)]
struct Params {
    key: Option<String>,
    expires: Option<String>,
    user_id: Option<String>,
}

impl Params {
    /// Read `a=b&c=d`, percent-decoding each side. A `+` stays a `+`: a key
    /// is not form text, and reading `+` as a space would break one.
    fn read(query: &str) -> Result<Self, NxmRefusal> {
        let mut params = Params::default();
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let slot = match percent_decode(name)?.to_ascii_lowercase().as_str() {
                "key" => &mut params.key,
                "expires" => &mut params.expires,
                "user_id" => &mut params.user_id,
                _ => continue,
            };
            if slot.is_none() {
                let value = percent_decode(value)?;
                if !value.trim().is_empty() {
                    *slot = Some(value.trim().to_string());
                }
            }
        }
        Ok(params)
    }
}

/// `%XX` → its byte; the result must be UTF-8.
fn percent_decode(text: &str) -> Result<String, NxmRefusal> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = text.get(at + 1..at + 3).ok_or(NxmRefusal::Unreadable)?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| NxmRefusal::Unreadable)?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).map_err(|_| NxmRefusal::Unreadable)
}

/// A key worth sending: 1..=[`LARGEST_KEY`] printable ASCII characters.
fn usable_key(key: String) -> Result<String, NxmRefusal> {
    let usable =
        (1..=LARGEST_KEY).contains(&key.len()) && key.bytes().all(|b| b.is_ascii_graphic());
    if usable {
        Ok(key)
    } else {
        Err(NxmRefusal::Unreadable)
    }
}

fn number(text: &str) -> Result<u64, NxmRefusal> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(NxmRefusal::Unreadable);
    }
    text.parse().map_err(|_| NxmRefusal::Unreadable)
}

fn id_in_range(text: &str) -> Result<u64, NxmRefusal> {
    number(text)
        .ok()
        .filter(|id| (1..=LARGEST_ID).contains(id))
        .ok_or(NxmRefusal::Unreadable)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An invented key: no real account's key ever enters this repo.
    const KEY: &str = "Invented-Grant_Key.0123";

    fn file(text: &str) -> NxmLink {
        match parse_nxm(text) {
            Ok(NxmMessage::File(link)) => link,
            other => panic!("{text}: expected a file link, got {other:?}"),
        }
    }

    #[test]
    fn a_file_link_reads_its_ids_grant_and_account() {
        let link = file(&format!(
            "nxm://dragonsdogma2/mods/2207/files/9001?key={KEY}&expires=1791987830&user_id=4242"
        ));
        assert_eq!(
            link,
            NxmLink {
                mod_id: 2207,
                file_id: 9001,
                grant: Some(NxmGrant {
                    key: KEY.into(),
                    expires_s: 1_791_987_830
                }),
                user_id: Some(4242),
            }
        );
    }

    #[test]
    fn spellings_seen_in_the_wild_read_the_same() {
        let expected = file("nxm://dragonsdogma2/mods/2207/files/9001");
        for text in [
            "nxm://DragonsDogma2/Mods/2207/Files/9001",
            "NXM://dragonsdogma2/mods/2207/files/9001/",
            "  \"nxm://dragonsdogma2/mods/2207/files/9001\"  ",
            "<nxm://dragonsdogma2/mods/2207/files/9001>",
            "nxm://dragonsdogma2/mods/2207/files/9001?campaign=x&nmm=1",
            "nxm://dragonsdogma2/mods/2207/files/9001#fragment",
        ] {
            assert_eq!(file(text), expected, "{text}");
        }
        assert_eq!(expected.grant, None, "no key, no grant");
        assert_eq!(expected.user_id, None);
    }

    #[test]
    fn parameters_are_read_in_any_order_case_and_encoding() {
        let link = file(
            "nxm://dragonsdogma2/mods/2207/files/9001?USER_ID=4242&Expires=1791987830&key=a%2Bb%2Fc+d",
        );
        let grant = link.grant.unwrap();
        assert_eq!(grant.key, "a+b/c+d", "a + is a + and %XX decodes");
        assert_eq!(grant.expires_s, 1_791_987_830);
        assert_eq!(link.user_id, Some(4242));
        // The first of a repeated parameter counts.
        let twice = file(&format!(
            "nxm://dragonsdogma2/mods/2207/files/9001?key={KEY}&key=other&expires=5&expires=6"
        ));
        assert_eq!(
            twice.grant,
            Some(NxmGrant {
                key: KEY.into(),
                expires_s: 5
            })
        );
    }

    #[test]
    fn premium_and_refusals() {
        assert_eq!(parse_nxm("nxm://premium"), Ok(NxmMessage::Premium));
        assert_eq!(parse_nxm("nxm://Premium/"), Ok(NxmMessage::Premium));
        for other in [
            "nxm://skyrimspecialedition/mods/2207/files/9001?key=a&expires=1",
            "nxm://dragonsdogma/mods/2207/files/9001",
            "nxm://skyrimspecialedition/collections/abcd12/revisions/3",
        ] {
            assert_eq!(parse_nxm(other), Err(NxmRefusal::OtherGame), "{other}");
        }
        for collection in [
            "nxm://dragonsdogma2/collections/vernworth-set/revisions/3",
            "nxm://DragonsDogma2/Collections/vernworth-set",
        ] {
            assert_eq!(
                parse_nxm(collection),
                Err(NxmRefusal::Collection),
                "{collection}"
            );
        }
    }

    #[test]
    fn anything_else_is_unreadable() {
        for text in [
            "",
            "   ",
            "Lantern Glow Recolor",
            "https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=9001",
            "nxm:dragonsdogma2/mods/2207/files/9001",
            "nxm://",
            "nxm://dragonsdogma2",
            "nxm://dragonsdogma2/mods/2207",
            "nxm://dragonsdogma2/mods/2207/files",
            "nxm://dragonsdogma2/mods/2207/files/9001/extra",
            "nxm://dragonsdogma2/mods/0/files/9001",
            "nxm://dragonsdogma2/mods/2207/files/0",
            "nxm://dragonsdogma2/mods/99999999999/files/9001",
            "nxm://dragonsdogma2/mods/-5/files/9001",
            "nxm://dragonsdogma2/mods/22o7/files/9001",
            "nxm://dragonsdogma2/files/9001/mods/2207",
            // A grant must be whole, and its parts must be what they claim.
            "nxm://dragonsdogma2/mods/2207/files/9001?key=abc",
            "nxm://dragonsdogma2/mods/2207/files/9001?expires=1791987830",
            "nxm://dragonsdogma2/mods/2207/files/9001?key=abc&expires=soon",
            "nxm://dragonsdogma2/mods/2207/files/9001?key=abc&expires=-1",
            "nxm://dragonsdogma2/mods/2207/files/9001?key=a%20b&expires=1",
            "nxm://dragonsdogma2/mods/2207/files/9001?key=%ZZ&expires=1",
            "nxm://dragonsdogma2/mods/2207/files/9001?key=%FF&expires=1",
            "nxm://dragonsdogma2/mods/2207/files/9001?user_id=me",
            "nxm://dragonsdogma2/mods/2207/files/9001?user_id=0",
            "nxm://drag%20onsdogma2/mods/2207/files/9001",
        ] {
            assert_eq!(parse_nxm(text), Err(NxmRefusal::Unreadable), "{text:?}");
        }
        let long_key = "k".repeat(LARGEST_KEY + 1);
        assert_eq!(
            parse_nxm(&format!(
                "nxm://dragonsdogma2/mods/2207/files/9001?key={long_key}&expires=1"
            )),
            Err(NxmRefusal::Unreadable)
        );
        // Blank values count as absent: no key and no expiry is no grant.
        assert_eq!(
            file("nxm://dragonsdogma2/mods/2207/files/9001?key=&expires=").grant,
            None
        );
    }

    #[test]
    fn a_grant_never_shows_its_key_and_knows_when_it_runs_out() {
        let link = file(&format!(
            "nxm://dragonsdogma2/mods/2207/files/9001?key={KEY}&expires=1000"
        ));
        for shown in [
            format!("{link:?}"),
            format!("{:?}", NxmMessage::File(link.clone())),
        ] {
            assert!(!shown.contains(KEY), "{shown}");
            assert!(
                shown.contains("<redacted>") && shown.contains("1000"),
                "{shown}"
            );
        }
        let grant = link.grant.unwrap();
        assert!(!grant.expired(999));
        assert!(grant.expired(1000), "good up to, not including, its second");
        assert!(grant.expired(5000));
    }

    #[test]
    fn refusals_are_sentences_that_repeat_nothing_from_the_link() {
        let link = format!("nxm://skyrimspecialedition/mods/2207/files/9001?key={KEY}&expires=1");
        for refusal in [
            parse_nxm(&link).unwrap_err(),
            NxmRefusal::Collection,
            NxmRefusal::Unreadable,
        ] {
            let sentence = refusal.to_string();
            assert!(sentence.ends_with('.'), "{sentence}");
            assert!(
                !sentence.contains(KEY) && !sentence.contains("skyrim"),
                "{sentence}"
            );
        }
    }

    #[test]
    fn a_link_for_another_account_is_told_apart() {
        let mine = file("nxm://dragonsdogma2/mods/2207/files/9001?user_id=4242");
        assert!(mine.made_for(Some(4242)));
        assert!(!mine.made_for(Some(5150)));
        assert!(mine.made_for(None), "an account that does not say");
        let silent = file("nxm://dragonsdogma2/mods/2207/files/9001");
        assert!(silent.made_for(Some(4242)), "a link that does not say");
    }
}
