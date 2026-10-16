//! The domain model. Every struct here serializes camelCase for the window.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::path::{fnv1a64, lower_key};

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// A package's identity: 16 hex chars. Archives are identified by file name +
/// size (moving a zip between roots keeps its id; a new version of the same
/// file is a new package), folders by their lower-cased absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageId(pub String);

impl PackageId {
    pub fn for_archive(file_name: &str, size: u64) -> Self {
        let key = format!("archive|{}|{size}", lower_key(file_name));
        PackageId(format!("{:016x}", fnv1a64(key.as_bytes())))
    }

    pub fn for_folder(absolute_path: &std::path::Path) -> Self {
        let key = format!("folder|{}", lower_key(&absolute_path.to_string_lossy()));
        PackageId(format!("{:016x}", fnv1a64(key.as_bytes())))
    }
}

impl std::fmt::Display for PackageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An option's identity: `<package id>#<option root>` — readable, stable for
/// as long as the package is. The root is the normalized folder inside the
/// package (`""` when the option is the package root itself).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OptionId(pub String);

impl OptionId {
    pub fn new(package: &PackageId, root: &str) -> Self {
        OptionId(format!("{}#{}", package.0, root))
    }

    /// The package half of the id.
    pub fn package(&self) -> PackageId {
        PackageId(
            self.0
                .split_once('#')
                .map(|(p, _)| p)
                .unwrap_or(&self.0)
                .to_string(),
        )
    }

    /// The option-root half of the id.
    pub fn root(&self) -> &str {
        self.0.split_once('#').map(|(_, r)| r).unwrap_or("")
    }
}

impl std::fmt::Display for OptionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RootKind {
    /// Fluffy Mod Manager's `Games/DragonsDogma2/Mods` folder.
    Fluffy,
    /// The user's Downloads folder — only DD2-shaped archives are listed.
    Downloads,
    /// Any folder the user added.
    Custom,
}

/// A folder the Riftstone reads packages from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRoot {
    /// Stable id (`fluffy`, `downloads`, or `custom-<hash>`).
    pub id: String,
    pub path: PathBuf,
    pub label: String,
    pub kind: RootKind,
    /// Downloads-style roots list only archives that look like DD2 mods.
    pub dd2_shaped_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageFormat {
    Zip,
    SevenZip,
    Rar,
    Folder,
}

/// One file inside a package. Directories are not listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    /// Normalized path inside the package (see [`crate::path`]).
    pub path: String,
    /// Uncompressed size in bytes.
    pub size: u64,
}

/// Something the reader or the catalog noticed and the user should see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub kind: ProblemKind,
    /// One plain sentence.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProblemKind {
    /// The package could not be opened at all.
    Unreadable,
    /// The package opened but holds nothing the game can use.
    NothingToInstall,
    /// Some files will not be installed (outside natives/, reframework/, paks, root dlls).
    FilesSkipped,
    /// An `addonfor` names a mod that is not in the library.
    MissingParent,
    /// A `requires` names a mod that is not installed.
    MissingRequirement,
    /// modinfo.ini could not be decoded cleanly.
    ModinfoGarbled,
    /// Loose files carry asset suffixes from before the September 2026 engine
    /// update (TU 3.2) — see [`crate::dd2::PRE_TU32_SUFFIXES`].
    OutdatedFormats,
}

/// One thing you downloaded: an archive or a plain folder in a library root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: PackageId,
    pub root_id: String,
    pub path: PathBuf,
    /// File or folder name as it sits in the root.
    pub file_name: String,
    /// A human name: the file name without extension and without a Nexus
    /// download suffix (`Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip`
    /// → `Vernworth Cloaks`).
    pub display_name: String,
    pub format: PackageFormat,
    /// Bytes on disk (sum of files for a folder).
    pub size: u64,
    /// Last-modified time, ms since the epoch.
    pub modified_ms: u64,
    pub entry_count: usize,
    pub options: Vec<ModOption>,
    pub nexus: Option<NexusLink>,
    pub problems: Vec<Problem>,
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Fluffy's `modinfo.ini`, parsed. Keys are case-insensitive in the file;
/// `key=value` and `key: value` are both accepted. Unknown keys land in `extra`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    /// File name of the preview image, relative to the option root.
    pub screenshot: Option<String>,
    pub homepage: Option<String>,
    /// `addonfor=` — the NAME of the option this one hangs under in Fluffy's menus.
    pub addon_for: Option<String>,
    /// `NameAsBundle=` — options sharing this value collapse into one bundle.
    pub name_as_bundle: Option<String>,
    /// `requires=` / `requirement=` — names that must be installed too.
    pub requires: Vec<String>,
    /// `DummyMod=true` — a pure menu entry, nothing to install.
    pub dummy: bool,
    /// `MenuPriority=` — higher sorts first among siblings (Fluffy default 0).
    pub menu_priority: i32,
    pub extra: BTreeMap<String, String>,
}

/// Where a payload file lands, relative to the game root.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DeployTarget {
    /// An RE Engine .pak. Lands in `pak_mods/` under a priority-prefixed name
    /// chosen at install time (`x0003-<file_name>`); later = wins.
    #[serde(rename_all = "camelCase")]
    Pak { file_name: String },
    /// A loose file under `natives/` — loaded by REFramework's LooseFileLoader.
    /// `game_path` keeps the archive's own spelling (`natives/stm/...`).
    #[serde(rename_all = "camelCase")]
    Loose { game_path: String },
    /// A REFramework file (`reframework/autorun/*.lua`, `reframework/data/...`,
    /// `reframework/plugins/*.dll`).
    #[serde(rename_all = "camelCase")]
    Reframework { game_path: String },
    /// A file in the game root (`dinput8.dll` — REFramework itself — or a root
    /// dll/ini a mod needs).
    #[serde(rename_all = "camelCase")]
    Root { game_path: String },
}

impl DeployTarget {
    /// The path relative to the game root for every target except paks, whose
    /// final name is decided at install time.
    pub fn game_path(&self) -> Option<&str> {
        match self {
            DeployTarget::Pak { .. } => None,
            DeployTarget::Loose { game_path }
            | DeployTarget::Reframework { game_path }
            | DeployTarget::Root { game_path } => Some(game_path),
        }
    }
}

/// One file an option installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadFile {
    /// Normalized entry path inside the package.
    pub entry: String,
    pub target: DeployTarget,
    pub size: u64,
}

/// A file inside an option that will NOT be installed, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub entry: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// modinfo.ini itself.
    Modinfo,
    /// The preview image or another picture at the option root.
    Picture,
    /// readme / txt / md / pdf / url at the option root.
    Document,
    /// In a folder the game never reads (not natives/, reframework/, a pak, or
    /// a root dll). Shown so nothing is lost silently.
    UnknownPlace,
}

/// Counts per deploy kind — the badge row on a result card.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadKinds {
    pub pak: u32,
    pub loose: u32,
    pub reframework: u32,
    pub root: u32,
}

impl PayloadKinds {
    pub fn tally(payload: &[PayloadFile]) -> Self {
        let mut kinds = PayloadKinds::default();
        for file in payload {
            match file.target {
                DeployTarget::Pak { .. } => kinds.pak += 1,
                DeployTarget::Loose { .. } => kinds.loose += 1,
                DeployTarget::Reframework { .. } => kinds.reframework += 1,
                DeployTarget::Root { .. } => kinds.root += 1,
            }
        }
        kinds
    }

    pub fn total(&self) -> u32 {
        self.pak + self.loose + self.reframework + self.root
    }
}

/// One installable unit inside a package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOption {
    pub id: OptionId,
    pub package_id: PackageId,
    /// Normalized folder inside the package that this option installs from
    /// (`""` = the package root). Payload paths are relative to it.
    pub root: String,
    /// Display name: modinfo `name`, else the root folder's name, else the
    /// package's display name.
    pub name: String,
    /// The parsed modinfo.ini, when the option has one.
    pub info: Option<ModInfo>,
    /// Entry path of the preview image, when one exists.
    pub screenshot: Option<String>,
    pub payload: Vec<PayloadFile>,
    pub skipped: Vec<SkippedFile>,
    pub kinds: PayloadKinds,
    /// The option this one hangs under (resolved from `addonfor`).
    pub parent: Option<OptionId>,
    /// The raw `addonfor` value, kept even when it resolves to nothing.
    pub addon_for: Option<String>,
    /// The `NameAsBundle` group, when any.
    pub bundle: Option<String>,
    /// A menu entry with nothing to install (`DummyMod=true`, or an empty
    /// payload with addon children — "Menu Option. Enabling/disabling does nothing.").
    pub menu_only: bool,
    /// Breadcrumb from the package down to this option's parent:
    /// `["Vernworth Wardrobe Collection", "Cloaks", "Ermine Cape (Menu Option)"]`.
    pub trail: Vec<String>,
}

// ---------------------------------------------------------------------------
// Nexus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkSource {
    /// Parsed from the Nexus download file name.
    Filename,
    /// From modinfo `homepage=` pointing at nexusmods.com.
    Homepage,
    /// Identified by the archive's MD5 through the Nexus API.
    Md5,
    /// Pasted by the user.
    Manual,
}

/// What links a package to a mod page on Nexus Mods.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusLink {
    pub mod_id: u64,
    /// The exact file on the mod page, when known (MD5 lookup knows it).
    pub file_id: Option<u64>,
    /// Every version of one logical file on a mod page shares a group id
    /// (observed on Nexus's v2 API, 2026: one armour pack's v1…v8 share one
    /// group). An update is a newer file in the SAME group.
    #[serde(default)]
    pub group_id: Option<u64>,
    /// The file's short unique id — the last token of a 2026-style download
    /// name (`… 2026-10-14T14-23Z Rk4vLmN2Q.zip` → `Rk4vLmN2Q`).
    #[serde(default)]
    pub sqid: Option<String>,
    /// The version the local file claims (from the file name or the API).
    pub version: Option<String>,
    /// When the local file was uploaded to Nexus, ms since the epoch, if known.
    pub uploaded_ms: Option<u64>,
    pub source: LinkSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_id_splits_back_into_its_halves() {
        let pkg = PackageId::for_archive("Vernworth Wardrobe Collection.zip", 37_757_015);
        let id = OptionId::new(
            &pkg,
            "Vernworth Wardrobe Collection/Ermine Cape - Variation 2",
        );
        assert_eq!(id.package(), pkg);
        assert_eq!(
            id.root(),
            "Vernworth Wardrobe Collection/Ermine Cape - Variation 2"
        );
        let root_option = OptionId::new(&pkg, "");
        assert_eq!(root_option.root(), "");
    }

    #[test]
    fn archive_ids_ignore_case_and_track_size() {
        assert_eq!(
            PackageId::for_archive("RE Framework.zip", 10),
            PackageId::for_archive("re framework.ZIP", 10)
        );
        assert_ne!(
            PackageId::for_archive("RE Framework.zip", 10),
            PackageId::for_archive("RE Framework.zip", 11)
        );
    }

    #[test]
    fn deploy_target_wire_shape_is_tagged() {
        let t = DeployTarget::Loose {
            game_path: "natives/stm/a.mesh".into(),
        };
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(json["kind"], "loose");
        assert_eq!(json["gamePath"], "natives/stm/a.mesh");
        let p = serde_json::to_value(DeployTarget::Pak {
            file_name: "x.pak".into(),
        })
        .unwrap();
        assert_eq!(p["kind"], "pak");
        assert_eq!(p["fileName"], "x.pak");
    }

    #[test]
    fn kinds_tally_by_target() {
        let payload = vec![
            PayloadFile {
                entry: "a.pak".into(),
                target: DeployTarget::Pak {
                    file_name: "a.pak".into(),
                },
                size: 1,
            },
            PayloadFile {
                entry: "natives/stm/b".into(),
                target: DeployTarget::Loose {
                    game_path: "natives/stm/b".into(),
                },
                size: 1,
            },
        ];
        let kinds = PayloadKinds::tally(&payload);
        assert_eq!((kinds.pak, kinds.loose, kinds.total()), (1, 1, 2));
    }
}
