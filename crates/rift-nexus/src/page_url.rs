//! Mod page URLs: parse what the user pastes, build what the window opens.

use crate::GAME_DOMAIN;

/// The largest id Nexus's GraphQL `Int` can carry.
const LARGEST_ID: u64 = i32::MAX as u64;

/// Parse a pasted mod reference into `(mod id, file id)`.
///
/// Accepts a bare mod id (`2207`) and Dragon's Dogma 2 page URLs in every
/// spelling seen in the wild — with or without scheme or `www.`, the
/// `/games/` prefix, any query order, a fragment, a trailing slash:
/// `https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=4410`
/// → `(2207, Some(4410))`. A mod-manager `nxm://dragonsdogma2/mods/2207/files/4410?key=…`
/// link works too (its key is ignored). Other games' URLs, other sites and
/// anything else → `None`.
pub fn parse_mod_url(text: &str) -> Option<(u64, Option<u64>)> {
    let text = text
        .trim()
        .trim_matches(|c: char| matches!(c, '<' | '>' | '"' | '\''))
        .trim();
    if text.is_empty() {
        return None;
    }
    if text.bytes().all(|b| b.is_ascii_digit()) {
        return id_in_range(text).map(|mod_id| (mod_id, None));
    }
    let absolute = if text.contains("://") {
        text.to_string()
    } else {
        format!("https://{text}")
    };
    let url = reqwest::Url::parse(&absolute).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let segments: Vec<String> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    match url.scheme() {
        // nxm://dragonsdogma2/mods/<mod>/files/<file>
        "nxm" if host == GAME_DOMAIN => mods_path(&segments),
        "http" | "https" if host == "nexusmods.com" || host.ends_with(".nexusmods.com") => {
            let rest = match segments.first().map(String::as_str) {
                Some("games") => &segments[1..],
                _ => &segments[..],
            };
            if rest.first().map(String::as_str) != Some(GAME_DOMAIN) {
                return None;
            }
            let (mod_id, file_id) = mods_path(&rest[1..])?;
            let file_id = file_id.or_else(|| {
                url.query_pairs()
                    .find(|(key, _)| key.eq_ignore_ascii_case("file_id"))
                    .and_then(|(_, value)| id_in_range(value.trim()))
            });
            Some((mod_id, file_id))
        }
        _ => None,
    }
}

/// `https://www.nexusmods.com/dragonsdogma2/mods/{id}`
pub fn mod_page_url(mod_id: u64) -> String {
    format!("https://www.nexusmods.com/{GAME_DOMAIN}/mods/{mod_id}")
}

/// The files tab of a mod page, scrolled to one file.
pub fn file_page_url(mod_id: u64, file_id: u64) -> String {
    format!("{}?tab=files&file_id={file_id}", mod_page_url(mod_id))
}

/// One file's page with its Mod Manager Download button (`&nmm=1`), where a
/// free account's download starts (v0.3): the button hands an `nxm://` link
/// back to the stone.
pub fn mod_manager_page_url(mod_id: u64, file_id: u64) -> String {
    format!("{}&nmm=1", file_page_url(mod_id, file_id))
}

/// `mods/<id>[/files/<file id>][/anything]`
fn mods_path(segments: &[String]) -> Option<(u64, Option<u64>)> {
    if segments.first().map(String::as_str) != Some("mods") {
        return None;
    }
    let mod_id = id_in_range(segments.get(1)?)?;
    let file_id = match segments.get(2).map(String::as_str) {
        Some("files") => segments.get(3).and_then(|id| id_in_range(id)),
        _ => None,
    };
    Some((mod_id, file_id))
}

fn id_in_range(text: &str) -> Option<u64> {
    text.parse::<u64>()
        .ok()
        .filter(|id| (1..=LARGEST_ID).contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a pasted text should parse to.
    type Case = (&'static str, Option<(u64, Option<u64>)>);

    #[test]
    fn parses_every_spelling_of_a_dd2_mod_page() {
        let cases: &[Case] = &[
            ("2207", Some((2207, None))),
            ("  2207\n", Some((2207, None))),
            (
                "https://www.nexusmods.com/dragonsdogma2/mods/2207",
                Some((2207, None)),
            ),
            (
                "https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=4410",
                Some((2207, Some(4410))),
            ),
            (
                "https://www.nexusmods.com/dragonsdogma2/mods/2207?file_id=4410&tab=files&nmm=1",
                Some((2207, Some(4410))),
            ),
            (
                "http://nexusmods.com/dragonsdogma2/mods/2207/",
                Some((2207, None)),
            ),
            (
                "www.nexusmods.com/dragonsdogma2/mods/2207?tab=description",
                Some((2207, None)),
            ),
            (
                "nexusmods.com/DragonsDogma2/mods/2207#comments",
                Some((2207, None)),
            ),
            (
                "https://www.nexusmods.com/games/dragonsdogma2/mods/2207",
                Some((2207, None)),
            ),
            (
                "<https://www.nexusmods.com/dragonsdogma2/mods/2207>",
                Some((2207, None)),
            ),
            (
                "nxm://dragonsdogma2/mods/2207/files/4410?key=abc&expires=1&user_id=2",
                Some((2207, Some(4410))),
            ),
            (
                "https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=nope",
                Some((2207, None)),
            ),
        ];
        for (text, expected) in cases {
            assert_eq!(parse_mod_url(text), *expected, "{text}");
        }
    }

    #[test]
    fn refuses_other_games_other_sites_and_nonsense() {
        for text in [
            "",
            "0",
            "99999999999",
            "-5",
            "https://www.nexusmods.com/skyrimspecialedition/mods/2207",
            "nxm://skyrimspecialedition/mods/2207/files/4410",
            "https://www.nexusmods.com/dragonsdogma2/collections/abc123",
            "https://www.nexusmods.com/dragonsdogma2/mods/",
            "https://evilnexusmods.com/dragonsdogma2/mods/2207",
            "https://nexusmods.com.example.org/dragonsdogma2/mods/2207",
            "ftp://www.nexusmods.com/dragonsdogma2/mods/2207",
            "Lantern Glow Recolor",
        ] {
            assert_eq!(parse_mod_url(text), None, "{text:?} should not parse");
        }
    }

    #[test]
    fn builds_page_urls_that_parse_back() {
        assert_eq!(
            mod_page_url(2207),
            "https://www.nexusmods.com/dragonsdogma2/mods/2207"
        );
        assert_eq!(parse_mod_url(&mod_page_url(4242)), Some((4242, None)));
        assert_eq!(
            parse_mod_url(&file_page_url(4242, 77)),
            Some((4242, Some(77)))
        );
        assert_eq!(
            mod_manager_page_url(2207, 9001),
            "https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=9001&nmm=1"
        );
        assert_eq!(
            parse_mod_url(&mod_manager_page_url(2207, 9001)),
            Some((2207, Some(9001)))
        );
    }
}
