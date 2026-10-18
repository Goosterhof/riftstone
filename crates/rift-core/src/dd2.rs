//! What the Riftstone knows about Dragon's Dogma 2 itself — measured on the
//! investor's install and in REFramework's source, October 2026. Every
//! constant carries where it came from, because the next game update may
//! change it and the person fixing it needs to know what to re-measure.

/// Steam app id (appmanifest_2054970.acf).
pub const STEAM_APP_ID: &str = "2054970";

/// Nexus Mods game domain and numeric game id (live API, 2026-10-16).
pub const NEXUS_GAME_DOMAIN: &str = "dragonsdogma2";
pub const NEXUS_GAME_ID: u64 = 6234;

/// The folder REFramework's `IntegrityCheckBypass_LoadPakDirectory` reads
/// (REFramework `IntegrityCheckBypass.hpp`: `CUSTOM_PAK_DIRECTORY_PATH`).
/// It iterates it with `recursive_directory_iterator` — NTFS returns names in
/// alphabetical order, so **file-name order is load order** — and injects
/// each pak into the next free `re_chunk_000.pak.sub_000.pak.patch_NNN` slot;
/// a later slot overrides an earlier one. Observed in the investor's
/// `re2_framework_log.txt`: `x0000-…` → `patch_001`, `x0001-…` → `patch_002`.
/// The extension check is CASE-SENSITIVE: a `.PAK` never loads.
pub const PAK_MODS_DIR: &str = "pak_mods";

/// The prefix the Riftstone gives the paks it owns: `zrift-0007-<stem>.pak`.
/// `z` sorts after the numbered prefixes other tools give paks (`x0000-` and
/// kin; NTFS compares upcased names: `X` < `Z`), so a pak the Riftstone
/// installs loads after — and wins over — anything already in `pak_mods/`, the
/// way "newest install wins" should behave.
pub const PAK_PREFIX: &str = "zrift-";

/// REFramework hands out patch slots with no bounds check
/// (`s_pak_array_len` is computed but unused); Monster Hunter Wilds crashed
/// at 130 pak_mods files. Above this count the Riftstone warns.
pub const PAK_COUNT_WARNING: usize = 100;

/// One asset type whose file-format version suffix changed when TU 3.2
/// (2026-09-01/02) moved DD2 to the new RE Engine (REFramework commit
/// 919982f4: TDB 73 → 83). Measured in Ekey's DD2_STM_Release.list, Mar 2025
/// vs Oct 2026. A loose file still carrying an `old` suffix was built for the
/// old engine and will not load — the mod needs an update (or DD2 Pak Bridge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuffixChange {
    pub asset: &'static str,
    pub old: &'static [&'static str],
    pub new: &'static str,
}

pub const PRE_TU32_SUFFIXES: &[SuffixChange] = &[
    SuffixChange {
        asset: "mesh",
        old: &["231011879", "240423143"],
        new: "260421070",
    },
    SuffixChange {
        asset: "tex",
        old: &["760230703"],
        new: "251211553",
    },
    SuffixChange {
        asset: "mdf2",
        old: &["40"],
        new: "51",
    },
    SuffixChange {
        asset: "motlist",
        old: &["751"],
        new: "1057",
    },
];

/// Does this file name end in a pre-TU 3.2 asset suffix (`foo.mesh.240423143`)?
pub fn outdated_suffix(path: &str) -> Option<&'static SuffixChange> {
    let name = crate::path::file_name(path).to_lowercase();
    let mut parts = name.rsplitn(3, '.');
    let version = parts.next()?;
    let asset = parts.next()?;
    parts.next()?; // a stem must exist
    PRE_TU32_SUFFIXES
        .iter()
        .find(|change| change.asset == asset && change.old.contains(&version))
}

/// Game-folder paths the deploy engine must never write, whatever a mod says.
/// Lower-cased, forward slashes; a trailing `/` means "everything under".
pub const NEVER_WRITE: &[&str] = &[
    "dd2.exe",
    "re_chunk_000.pak",
    "dlc/",
    "_storage_/", // REFramework's own DLL cache, regenerated every launch
    "steam_api64.dll",
    "steam_appid.txt",
    "shader.cache2",
    "config.ini",
    "config_default.ini",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_engine_suffixes_are_recognised() {
        assert_eq!(
            outdated_suffix("natives/stm/a/top_wb_f.mesh.240423143").map(|c| c.asset),
            Some("mesh")
        );
        assert_eq!(
            outdated_suffix("natives/STM/a/x.TEX.760230703").map(|c| c.new),
            Some("251211553")
        );
        assert!(outdated_suffix("natives/stm/a/top_314_wb_f.mesh.260421070").is_none());
        assert!(outdated_suffix("natives/stm/a/x.pfb.18").is_none());
        assert!(
            outdated_suffix("mesh.240423143").is_none(),
            "no stem, no verdict"
        );
    }

    #[test]
    fn our_pak_prefix_sorts_after_numbered_prefixes() {
        assert!("X0000-A.PAK" < "ZRIFT-0000-A.PAK");
        assert!(PAK_PREFIX.to_uppercase().as_str() > "X0000-");
    }
}
