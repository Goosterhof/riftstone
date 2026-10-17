//! The window contract, Rust side: every shape that crosses the Tauri bridge.
//!
//! A field-for-field mirror of `src/types/rift.ts` (authoritative). Every
//! struct serializes camelCase; every TS union is an internally tagged enum
//! (`kind`, or `state` for [`ScanState`]) with camelCase variant fields, so the
//! JSON matches the TS EXACTLY. `T | null` is an `Option` that serializes as
//! `null` — never skipped. Change one side, change both, in the same commit:
//! the tests at the bottom pin every key the window reads.
//!
//! The crates under `crates/` serialize their own types too, but the wire is
//! owned here: a field a crate adds must not leak into the window unasked.
//! The `From` conversions below are the only place crate types become wire
//! types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shared atoms (mirrors of rift-core)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageFormat {
    Zip,
    SevenZip,
    Rar,
    Folder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeployKind {
    Pak,
    Loose,
    Reframework,
    Root,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadKinds {
    pub pak: u32,
    pub loose: u32,
    pub reframework: u32,
    pub root: u32,
}

/// v0.3: the one library, the mods folder beside the installed
/// `riftstone.exe` (or `RIFTSTONE_MODS_DIR` on a bench).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModsFolderView {
    pub path: String,
    /// False only when the folder is missing and could not be made.
    pub exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProblemKind {
    Unreadable,
    NothingToInstall,
    FilesSkipped,
    MissingParent,
    MissingRequirement,
    ModinfoGarbled,
    OutdatedFormats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub kind: ProblemKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub screenshot: Option<String>,
    pub homepage: Option<String>,
    pub addon_for: Option<String>,
    pub name_as_bundle: Option<String>,
    pub requires: Vec<String>,
    pub dummy: bool,
    pub menu_priority: i32,
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DeployTarget {
    #[serde(rename_all = "camelCase")]
    Pak { file_name: String },
    #[serde(rename_all = "camelCase")]
    Loose { game_path: String },
    #[serde(rename_all = "camelCase")]
    Reframework { game_path: String },
    #[serde(rename_all = "camelCase")]
    Root { game_path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadFile {
    pub entry: String,
    pub target: DeployTarget,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    Modinfo,
    Picture,
    Document,
    UnknownPlace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub entry: String,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModOption {
    pub id: String,
    pub package_id: String,
    pub root: String,
    pub name: String,
    pub info: Option<ModInfo>,
    pub screenshot: Option<String>,
    pub payload: Vec<PayloadFile>,
    pub skipped: Vec<SkippedFile>,
    pub kinds: PayloadKinds,
    pub parent: Option<String>,
    pub addon_for: Option<String>,
    pub bundle: Option<String>,
    pub menu_only: bool,
    pub trail: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkSource {
    Filename,
    Homepage,
    Md5,
    Manual,
    /// v0.3: the stone fetched this exact file from Nexus itself.
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusLink {
    pub mod_id: u64,
    pub file_id: Option<u64>,
    pub group_id: Option<u64>,
    pub sqid: Option<String>,
    pub version: Option<String>,
    pub uploaded_ms: Option<u64>,
    pub source: LinkSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
}

// ---------------------------------------------------------------------------
// get_overview — the home frame
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameSource {
    Steam,
    Env,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildChanged {
    pub recorded: String,
    pub now: String,
}

/// v0.3: how Play starts the game: through Steam (the game was found
/// through Steam), or `DD2.exe` in the game folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchRoute {
    Steam,
    Exe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub root: String,
    pub build_id: Option<String>,
    pub source: GameSource,
    /// DD2.exe runs, as the game watcher last saw it.
    pub running: bool,
    /// v0.3: when the stone first saw this run of the game (ms); `None`
    /// while the game is not running.
    pub started_ms: Option<u64>,
    /// v0.3: the route Play takes.
    pub launch: LaunchRoute,
    pub build_changed: Option<BuildChanged>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReframeworkSummary {
    pub installed: bool,
    pub version: Option<String>,
    pub build_date: Option<String>,
    pub loose_file_loader: Option<bool>,
    pub pak_directory: Option<bool>,
    pub faulty_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStats {
    pub packages: usize,
    pub options: usize,
    pub entries: usize,
    pub installed_options: usize,
    pub updates_available: usize,
    pub conflicts: usize,
    pub last_scan_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ScanState {
    Idle,
    #[serde(rename_all = "camelCase")]
    Scanning {
        done: usize,
        total: usize,
        current: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckLevel {
    Ok,
    Info,
    Warn,
    Error,
}

/// The two REFramework switches the window may flip (`set_reframework_switch`
/// and the `enableSwitch` check action).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SwitchKey {
    LooseFileLoader,
    PakDirectory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CheckAction {
    EnableSwitch {
        switch: SwitchKey,
    },
    PickGameFolder,
    #[serde(rename_all = "camelCase")]
    OpenUrl {
        url: String,
    },
    Rescan,
    /// v0.2: open the parked-backup review (`InstalledView.parked`).
    ReviewParked,
    /// v0.3: open the mods folder in Explorer (`open_mods_folder`).
    OpenModsFolder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub level: CheckLevel,
    pub title: String,
    pub detail: String,
    pub action: Option<CheckAction>,
}

/// v0.3: who the stored key belongs to (`/v1/users/validate.json`). Never
/// the key, never the e-mail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusAccountView {
    pub name: String,
    pub premium: bool,
    pub supporter: bool,
    /// When the key was last validated, ms since the epoch.
    pub checked_ms: u64,
}

/// v0.3: the stored key's standing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NexusKeyStanding {
    /// No key; everything anonymous still works.
    #[default]
    None,
    /// Validated this session; `account` is set.
    Connected,
    /// A key is stored but not validated this session yet.
    Unchecked,
    /// Nexus refused the stored key at its last validation.
    Rejected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusSummary {
    pub last_check_ms: Option<u64>,
    pub checking: bool,
    pub linked: usize,
    pub unlinked: usize,
    pub updates: usize,
    pub has_api_key: bool,
    /// v0.3
    pub key: NexusKeyStanding,
    /// v0.3: set exactly when `key` is `connected`.
    pub account: Option<NexusAccountView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryAction {
    Install,
    Uninstall,
    Reorder,
    /// v0.2: putting a parked file back (`restore_parked`).
    RestoreParked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryNote {
    pub option_name: String,
    pub action: RecoveryAction,
    pub files_rolled_back: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub game: Option<GameSummary>,
    pub reframework: Option<ReframeworkSummary>,
    /// v0.3: the one library folder.
    pub mods: ModsFolderView,
    pub library: LibraryStats,
    pub scan: ScanState,
    pub checks: Vec<Check>,
    pub nexus: NexusSummary,
    pub recovered: Option<RecoveryNote>,
}

// ---------------------------------------------------------------------------
// search — the headline ask
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchedField {
    Name,
    Trail,
    Bundle,
    Category,
    Author,
    Description,
    PackageName,
    Version,
    File,
    /// v0.2: the package's Nexus category or one of its tags.
    NexusLabel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    pub installed: Option<bool>,
    pub include_menu_only: bool,
    /// v0.2: Nexus categories, ANY-of (a mod has one category). Empty = no
    /// filter. Read as empty when a query leaves it out (a v0.1 query).
    #[serde(default)]
    pub categories: Vec<String>,
    /// v0.2: Nexus tags, ALL-of (each tag narrows). Empty = no filter.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub filters: SearchFilters,
    /// 0 = no cap: every match (v0.3; v0.2 capped 0 at 200). A v0.2 query's
    /// `rootIds` is read and ignored.
    pub limit: usize,
}

/// Who put an install in the journal: the Riftstone itself, or the journal
/// took it over from files already in the game (`adopted`, v0.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallOrigin {
    Riftstone,
    Adopted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallBadge {
    pub origin: InstallOrigin,
    pub installed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateBadge {
    Available,
    Current,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub option_id: String,
    pub package_id: String,
    pub score: f32,
    pub name: String,
    pub trail: Vec<String>,
    pub bundle: Option<String>,
    pub package_name: String,
    pub package_format: PackageFormat,
    pub author: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub description_snippet: Option<String>,
    pub has_screenshot: bool,
    pub kinds: PayloadKinds,
    pub menu_only: bool,
    pub matched: Vec<MatchedField>,
    pub file_matches: Vec<String>,
    pub installed: Option<InstallBadge>,
    pub update: Option<UpdateBadge>,
    pub conflicted: bool,
    /// v0.2: the linked mod's Nexus category (`None`: unlinked or never checked).
    pub nexus_category: Option<String>,
    /// v0.2: the linked mod's Nexus tags, Nexus's own spelling.
    pub nexus_tags: Vec<String>,
    /// v0.2: Nexus flags the linked mod as adult content.
    pub adult: bool,
}

// ---------------------------------------------------------------------------
// v0.2 — Nexus labels as filters (get_labels)
// ---------------------------------------------------------------------------

/// What Nexus says about a linked mod, from the latest update check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusLabels {
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub adult: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelCount {
    /// Nexus's spelling. Filters compare case-insensitively.
    pub name: String,
    /// Installable options carrying it, so a chip predicts its results.
    pub options: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelFacets {
    /// Count descending, then name.
    pub categories: Vec<LabelCount>,
    pub tags: Vec<LabelCount>,
    /// Packages a label filter hides: at least one installable option and
    /// no category or tag (unlinked, or linked but never checked).
    pub unlabeled_packages: usize,
}

// ---------------------------------------------------------------------------
// get_option / get_package
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub id: String,
    pub path: String,
    pub file_name: String,
    pub display_name: String,
    pub format: PackageFormat,
    pub size: u64,
    pub modified_ms: u64,
    pub entry_count: usize,
    pub option_count: usize,
    pub nexus: Option<NexusLink>,
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionRef {
    pub option_id: String,
    pub name: String,
    pub menu_only: bool,
    pub has_screenshot: bool,
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFile {
    pub game_path: String,
    pub kind: DeployKind,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledInfo {
    pub seq: u64,
    pub installed_at: String,
    pub version: Option<String>,
    pub game_build: Option<String>,
    pub origin: InstallOrigin,
    pub files: Vec<InstalledFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerRef {
    pub option_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathHashView {
    pub lower: u32,
    pub upper: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConflictKey {
    #[serde(rename_all = "camelCase")]
    Path { path: String },
    #[serde(rename_all = "camelCase")]
    Hash {
        hash: PathHashView,
        path: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictView {
    pub key: ConflictKey,
    /// Loser first, winner last.
    pub layers: Vec<LayerRef>,
    pub winner: LayerRef,
    /// v0.2: a pak in this conflict carries table flags rift-pak does not
    /// know, so the stone cannot be sure the hash is what the engine reads.
    pub unconfirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFileView {
    pub file_id: u64,
    pub name: String,
    pub version: Option<String>,
    pub uploaded_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateStateView {
    Current,
    UpdateAvailable { newer: NexusFileView },
    LocalArchived { newer: Option<NexusFileView> },
    Unpinned,
    ModUnavailable { status: String },
    Unlinked,
    NotChecked,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRow {
    pub package_id: String,
    pub package_name: String,
    pub link: Option<NexusLink>,
    pub mod_name: Option<String>,
    pub local_version: Option<String>,
    pub local_uploaded_ms: Option<u64>,
    pub state: UpdateStateView,
    pub page_url: Option<String>,
    pub checked_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionDetail {
    pub option: ModOption,
    pub package: PackageSummary,
    pub parent_name: Option<String>,
    pub children: Vec<OptionRef>,
    pub installed: Option<InstalledInfo>,
    pub conflicts: Vec<ConflictView>,
    pub update: Option<UpdateRow>,
    /// v0.2: `None` when the package is unlinked, never checked, or its
    /// answer carried no labels (the index drops a blank entry).
    pub labels: Option<NexusLabels>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TreeNodeKind {
    Option,
    Bundle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    /// `None` for a bundle node.
    pub option_id: Option<String>,
    pub label: String,
    pub kind: TreeNodeKind,
    pub menu_only: bool,
    pub has_screenshot: bool,
    /// A bundle: any child is installed.
    pub installed: bool,
    /// v0.3: the option takes part in a conflict (a bundle: any child does).
    pub conflicted: bool,
    pub kinds: PayloadKinds,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDetail {
    pub package: PackageSummary,
    pub tree: Vec<TreeNode>,
    pub entries: Vec<ArchiveEntry>,
}

// ---------------------------------------------------------------------------
// v0.3 — the library tree (get_library): the Rift's idle view
// ---------------------------------------------------------------------------

/// One package of the mods folder with its options nested the way their
/// menus nest them (rift-catalog `tree::nest`), without the entry list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPackage {
    pub package: PackageSummary,
    /// Options that install something (not menu-only).
    pub installable: usize,
    /// Options of this package in your world now (journal installs).
    pub installed: usize,
    /// `None` when the package is not linked to Nexus.
    pub update: Option<UpdateBadge>,
    /// `None` when Nexus has not labelled the package.
    pub labels: Option<NexusLabels>,
    /// Nexus flags the linked mod adult.
    pub adult: bool,
    /// At least one option of it takes part in a conflict.
    pub conflicted: bool,
    /// Empty for a package that yielded no options (its problems say why).
    pub tree: Vec<TreeNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub mods: ModsFolderView,
    /// Every package in the mods folder, troubled ones too, by display name
    /// (case-blind), then file name.
    pub packages: Vec<LibraryPackage>,
}

// ---------------------------------------------------------------------------
// install / uninstall
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WriteAction {
    Create,
    /// Over a file an install in the journal owns (adopted installs too).
    #[serde(rename_all = "camelCase")]
    OverwriteRiftstone {
        option_name: String,
    },
    OverwriteUnowned,
    Reinstall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedWrite {
    pub entry: String,
    pub game_path: String,
    pub kind: DeployKind,
    pub size: u64,
    pub action: WriteAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlanWarning {
    PakFolderOff,
    LooseLoaderOff,
    ReframeworkMissing,
    GameRunning,
    #[serde(rename_all = "camelCase")]
    RequiresMissing {
        name: String,
    },
    #[serde(rename_all = "camelCase")]
    Conflicts {
        option_names: Vec<String>,
        paths: usize,
    },
    #[serde(rename_all = "camelCase")]
    OutdatedFormats {
        count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanView {
    pub option_id: String,
    pub name: String,
    pub writes: Vec<PlannedWrite>,
    pub total_bytes: u64,
    pub warnings: Vec<PlanWarning>,
    /// A full sentence when the install cannot run.
    pub refused: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResultView {
    pub option_id: String,
    pub files_written: usize,
    pub bytes_written: u64,
    pub backups: usize,
    pub duration_ms: u64,
    pub restart_needed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemovalAction {
    Delete,
    /// v0.2: the file is already gone from the game folder.
    AlreadyGone,
    #[serde(rename_all = "camelCase")]
    RestoreBackup {
        owner: String,
    },
    KeepChanged,
    KeepVanilla,
    #[serde(rename_all = "camelCase")]
    HandDown {
        to_option_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedRemoval {
    pub game_path: String,
    pub kind: DeployKind,
    pub action: RemovalAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPlanView {
    pub option_id: String,
    pub name: String,
    pub removals: Vec<PlannedRemoval>,
    pub refused: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallResultView {
    pub option_id: String,
    pub files_removed: usize,
    pub files_restored: usize,
    /// Left in place: changed by something else, or possibly vanilla.
    pub files_kept: usize,
    /// v0.2: handed down to the install below (it now owns them).
    pub files_handed_down: usize,
    /// v0.2: files kept aside in the backup store (`InstalledView.parked`).
    pub parked: usize,
    pub restart_needed: bool,
}

// ---------------------------------------------------------------------------
// get_installed — what is in the game now
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRow {
    /// Opaque: a library option's id, or, for an adopted install that matched
    /// no library option, a stand-in (`adopted#<label>` since v0.3).
    pub option_id: String,
    pub name: String,
    pub package_name: String,
    /// The mods folder holds no package for this install: its package left
    /// the folder, or (a stand-in) it never had one. Uninstall still works.
    pub package_missing: bool,
    pub version: Option<String>,
    pub installed_at: String,
    pub game_build: Option<String>,
    pub origin: InstallOrigin,
    pub kinds: PayloadKinds,
    pub file_count: usize,
    pub paks: Vec<PakView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PakLoadState {
    #[serde(rename_all = "camelCase")]
    Loaded {
        slot: u32,
        native_name: String,
    },
    Pending,
    NotLoaded,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PakView {
    pub file_name: String,
    pub size: u64,
    pub load: PakLoadState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StrayFile {
    pub game_path: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingFile {
    pub option_id: String,
    pub option_name: String,
    pub game_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectedPak {
    pub file_name: String,
    pub native_name: String,
    pub slot: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PakLoadReport {
    pub log_started: Option<String>,
    pub injected: Vec<InjectedPak>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledView {
    pub installs: Vec<InstalledRow>,
    /// Option ids of pak-bearing installs, lowest priority first (last wins).
    pub pak_order: Vec<String>,
    pub conflicts: Vec<ConflictView>,
    pub strays: Vec<StrayFile>,
    pub missing: Vec<MissingFile>,
    pub pak_load: Option<PakLoadReport>,
    pub pak_count_warning: usize,
    /// v0.2: files kept aside in the backup store, each with put back / discard.
    pub parked: Vec<ParkedFileView>,
}

/// Whose bytes a parked file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OwnerKind {
    Unowned,
    /// v0.3: the owner is an adopted install.
    Adopted,
    Riftstone,
}

/// v0.2: a file the stone kept aside rather than delete or force back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParkedFileView {
    /// Its handle in the backup store: what `restore_parked` / `discard_parked` take.
    pub store: String,
    /// Where it belongs in the game folder.
    pub game_path: String,
    /// Whose bytes it holds: "the file that was there before", or an
    /// install's name.
    pub owner: String,
    pub owner_kind: OwnerKind,
    /// Its size in the store; `None` when the kept file is gone from it.
    pub size: Option<u64>,
    /// RFC 3339, UTC.
    pub parked_at: String,
    /// Why it was kept aside, one sentence.
    pub reason: String,
    /// Something sits at `game_path` now: putting it back swaps that out.
    pub occupied: bool,
}

/// v0.2: what `restore_parked` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParkedRestoreReport {
    pub game_path: String,
    pub restored: bool,
    /// The store handle of the file that sat at `game_path`, now kept aside
    /// in its turn.
    pub swapped_out: Option<String>,
    pub restart_needed: bool,
}

// ---------------------------------------------------------------------------
// v0.3 — Play (plan_launch, launch_game)
// ---------------------------------------------------------------------------

/// Reasons a launch may not go as the player expects; never blockers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LaunchWarning {
    /// Pak installs exist but REFramework's dinput8.dll is missing.
    #[serde(rename_all = "camelCase")]
    ReframeworkMissing { paks: usize },
    /// REFramework's pak folder switch is off.
    #[serde(rename_all = "camelCase")]
    PakFolderOff { paks: usize },
    /// The loose file loader is off while loose files are installed.
    #[serde(rename_all = "camelCase")]
    LooseLoaderOff { files: usize },
    /// Steam updated the game since the newest install.
    #[serde(rename_all = "camelCase")]
    BuildChanged { recorded: String, now: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlanView {
    pub launch: LaunchRoute,
    pub warnings: Vec<LaunchWarning>,
    /// A full sentence when Play cannot start now.
    pub refused: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReport {
    pub launch: LaunchRoute,
    /// When the stone asked Steam or Windows to start the game (ms).
    pub launched_ms: u64,
}

// ---------------------------------------------------------------------------
// Nexus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusQuota {
    pub hourly_remaining: u64,
    pub hourly_limit: u64,
    pub daily_remaining: u64,
    pub daily_limit: u64,
    pub hourly_reset_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesView {
    pub last_check_ms: Option<u64>,
    pub rows: Vec<UpdateRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5Result {
    pub package_id: String,
    pub matched: bool,
    pub link: Option<NexusLink>,
}

// ---------------------------------------------------------------------------
// v0.2 — Find on Nexus by name (find_on_nexus)
// ---------------------------------------------------------------------------

/// A Nexus mod that may be where a package came from. A find never links:
/// the player picks one, and the window links it with `link_package`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusCandidate {
    pub mod_id: u64,
    pub name: String,
    /// The author Nexus shows, else the uploader's name.
    pub author: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub adult: bool,
    pub downloads: Option<u64>,
    pub endorsements: Option<u64>,
    /// Nexus's summary, whitespace-collapsed, at most 200 characters.
    pub summary: Option<String>,
    /// The mod's last update on Nexus, ms since the epoch.
    pub updated_ms: Option<i64>,
    /// `https://www.nexusmods.com/dragonsdogma2/mods/<modId>`
    pub page_url: String,
    /// Packages in the library already linked to this mod.
    pub linked_packages: usize,
}

impl NexusCandidate {
    /// rift-nexus's candidate as the window shows it, with how many of the
    /// library's packages are linked to that mod right now.
    pub fn of(candidate: &rift_nexus::ModCandidate, linked_packages: usize) -> Self {
        NexusCandidate {
            mod_id: candidate.mod_id,
            name: candidate.name.clone(),
            author: candidate.author.clone(),
            version: candidate.version.clone(),
            category: candidate.category.clone(),
            adult: candidate.adult,
            downloads: candidate.downloads,
            endorsements: candidate.endorsements,
            summary: candidate.summary.clone(),
            updated_ms: candidate.updated_ms,
            page_url: rift_nexus::mod_page_url(candidate.mod_id),
            linked_packages,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFindings {
    pub package_id: String,
    /// The words sent to Nexus (the set answered last): from the package's
    /// display name, or, when it holds nothing searchable, from its first
    /// option's bundle or name. `""` when none of those held anything to
    /// search for: then nothing was sent and `candidates` is empty.
    pub terms: String,
    /// Nexus's relevance order, at most 5.
    pub candidates: Vec<NexusCandidate>,
}

// ---------------------------------------------------------------------------
// v0.3 — a mod's files on Nexus (get_mod_files), browsing Nexus (browse_nexus)
// ---------------------------------------------------------------------------

/// Nexus's file categories. Retired: `oldVersion`, `removed`, `archived`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NexusFileCategory {
    Main,
    Update,
    Optional,
    OldVersion,
    Miscellaneous,
    Removed,
    Archived,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFileEntry {
    pub file_id: u64,
    /// The file's name on its Nexus page (not the archive's file name).
    pub name: String,
    pub version: Option<String>,
    pub category: NexusFileCategory,
    /// Bytes; `None` when Nexus does not say.
    pub size: Option<u64>,
    pub uploaded_ms: u64,
    /// Whitespace-collapsed, at most 400 characters.
    pub description: Option<String>,
    /// The file's own patch notes, one line each.
    pub changelog: Vec<String>,
    /// A package in the mods folder is pinned to this exact file.
    pub in_library: bool,
    /// The newest file of its group that is not retired: what an update fetches.
    pub newest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusModFiles {
    pub mod_id: u64,
    /// The mod's name on Nexus; `None` when Nexus did not say.
    pub mod_name: Option<String>,
    /// `https://www.nexusmods.com/dragonsdogma2/mods/<modId>`
    pub page_url: String,
    pub adult: bool,
    /// Main, update, optional, miscellaneous, then the retired ones; newest
    /// upload first within a category.
    pub files: Vec<NexusFileEntry>,
}

/// How `browse_nexus` orders Dragon's Dogma 2 mods. `relevance` needs text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NexusBrowseSort {
    #[default]
    Endorsements,
    Downloads,
    Updated,
    Relevance,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusBrowseQuery {
    /// Words the player typed; `""` browses the whole game by `sort`.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sort: NexusBrowseSort,
    /// Paging: 0, 20, 40 …
    #[serde(default)]
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusBrowsePage {
    /// Every match Nexus counts, across pages.
    pub total: u64,
    pub offset: u64,
    /// The same card shape as Find on Nexus.
    pub mods: Vec<NexusCandidate>,
}

// ---------------------------------------------------------------------------
// v0.3 — the key's extras: tracked mods and endorsements (get_nexus_standings)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndorseStatus {
    Endorsed,
    Abstained,
}

/// One decided endorsement: `{ modId, status }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndorsementView {
    pub mod_id: u64,
    pub status: EndorseStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusStandings {
    /// Dragon's Dogma 2 mods this account tracks on Nexus, as cards.
    pub tracked: Vec<NexusCandidate>,
    /// This account's decided endorsements; a mod absent here is undecided.
    pub endorsements: Vec<EndorsementView>,
    /// When the spine last asked Nexus (ms).
    pub checked_ms: u64,
}

// ---------------------------------------------------------------------------
// v0.3 — downloads into the mods folder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadPhase {
    /// A free account's download, parked until the site's Mod Manager
    /// Download link (nxm://) for the same mod and file arrives.
    Waiting,
    Queued,
    /// Asking Nexus for a download link.
    Linking,
    Downloading,
    /// The size, then the MD5 round trip.
    Verifying,
    /// In the mods folder.
    Landed,
    Failed,
    Cancelled,
}

impl DownloadPhase {
    /// Landed, failed or cancelled: the row is over (`clear_downloads`
    /// forgets these).
    pub fn is_over(self) -> bool {
        matches!(
            self,
            DownloadPhase::Landed | DownloadPhase::Failed | DownloadPhase::Cancelled
        )
    }
}

/// One download of this session. Serialize-only: `failure` is rift-core's
/// `RiftError`, which crosses the bridge exactly as a rejected command's
/// error does (`{ kind, detail }`) and is never read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadView {
    pub id: String,
    pub mod_id: u64,
    pub file_id: u64,
    /// The mod's name on Nexus, when known.
    pub mod_name: Option<String>,
    /// The file's name on its Nexus page, when known.
    pub file_name: Option<String>,
    pub version: Option<String>,
    /// The package this download updates, when it started from an update row.
    pub for_package_id: Option<String>,
    pub phase: DownloadPhase,
    pub bytes_done: u64,
    /// `None` until Nexus or the server states it.
    pub bytes_total: Option<u64>,
    /// The archive's file name in the mods folder once landed.
    pub landed_as: Option<String>,
    /// The new package's id once landed.
    pub package_id: Option<String>,
    /// The same file was in the mods folder already: nothing new was written.
    pub already_had: bool,
    /// For phase `failed`: the failure the window voices.
    pub failure: Option<rift_core::RiftError>,
    /// For phase `waiting`: the file's page on Nexus, where Mod Manager
    /// Download sends the link back to the stone.
    pub nexus_page_url: Option<String>,
    pub started_ms: u64,
    pub finished_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// v0.3 — the nxm:// handler (Nexus's Mod Manager Download button)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NxmHandlerState {
    /// Windows sends nxm:// links to this riftstone.exe.
    Ours,
    /// No program answers them.
    None,
    /// Another program answers them (`other` names its exe).
    Other,
    /// Not on Windows (the bench).
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NxmHandlerView {
    pub state: NxmHandlerState,
    /// The other program's file name only (`OtherManager.exe`); never a path.
    pub other: Option<String>,
    /// Turning the stone off gives the link back to this program's file
    /// name (the handler the stone replaced), or to nobody.
    pub gives_back_to: Option<String>,
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

/// v0.2: which screenshots are veiled (blurred until hovered).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VeilMode {
    #[default]
    Off,
    /// Options of mods Nexus flags as adult, and of packages Nexus has not
    /// labelled yet (no category, no tags, not adult): a veil protects, so
    /// the unknown is veiled. The window decides it (`ledger.ts adultnessOf`).
    Adult,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// v0.2: replaces v0.1's `veilThumbnails: bool` (`settings.json` keeps
    /// reading the old key; see `persist::StoredSettings`). Default `off`.
    pub veil: VeilMode,
    /// Check Nexus on launch when the last check is older than 12 hours.
    pub auto_check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            veil: VeilMode::Off,
            auto_check_updates: true,
        }
    }
}

// ---------------------------------------------------------------------------
// events (payloads)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressEvent {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgressEvent {
    pub option_id: String,
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdatesPhase {
    Linking,
    Hashing,
    Checking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatesProgressEvent {
    pub phase: UpdatesPhase,
    pub done: usize,
    pub total: usize,
}

/// v0.3: `rift://download-progress`, at most one per 200 ms per download,
/// plus the last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub id: String,
    pub phase: DownloadPhase,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
}

/// v0.3: `rift://nxm-received`. `accepted`: a download was started or a
/// waiting one resumed (`download_id`); otherwise `refusal` says why. The
/// ids are `None` when the link could not be read. Serialize-only, like
/// [`DownloadView`]: `refusal` is rift-core's `RiftError`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NxmReceivedEvent {
    pub mod_id: Option<u64>,
    pub file_id: Option<u64>,
    pub accepted: bool,
    pub download_id: Option<String>,
    pub refusal: Option<rift_core::RiftError>,
}

/// The `{}` payload of `rift://library-changed`, `rift://game-changed`,
/// `rift://updates-changed` and `rift://downloads-changed`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Nothing {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThumbnailSize {
    Card,
    Full,
}

// ---------------------------------------------------------------------------
// Conversions: crate types → wire types
// ---------------------------------------------------------------------------

/// A path as the window shows it (the OS spelling, lossy only for bytes that
/// are not Unicode — a Windows path never has any).
pub fn path_text(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

impl From<rift_core::PackageFormat> for PackageFormat {
    fn from(format: rift_core::PackageFormat) -> Self {
        match format {
            rift_core::PackageFormat::Zip => PackageFormat::Zip,
            rift_core::PackageFormat::SevenZip => PackageFormat::SevenZip,
            rift_core::PackageFormat::Rar => PackageFormat::Rar,
            rift_core::PackageFormat::Folder => PackageFormat::Folder,
        }
    }
}

impl From<rift_deploy::DeployKind> for DeployKind {
    fn from(kind: rift_deploy::DeployKind) -> Self {
        match kind {
            rift_deploy::DeployKind::Pak => DeployKind::Pak,
            rift_deploy::DeployKind::Loose => DeployKind::Loose,
            rift_deploy::DeployKind::Reframework => DeployKind::Reframework,
            rift_deploy::DeployKind::Root => DeployKind::Root,
        }
    }
}

impl From<rift_core::PayloadKinds> for PayloadKinds {
    fn from(kinds: rift_core::PayloadKinds) -> Self {
        PayloadKinds {
            pak: kinds.pak,
            loose: kinds.loose,
            reframework: kinds.reframework,
            root: kinds.root,
        }
    }
}

impl From<rift_core::ProblemKind> for ProblemKind {
    fn from(kind: rift_core::ProblemKind) -> Self {
        match kind {
            rift_core::ProblemKind::Unreadable => ProblemKind::Unreadable,
            rift_core::ProblemKind::NothingToInstall => ProblemKind::NothingToInstall,
            rift_core::ProblemKind::FilesSkipped => ProblemKind::FilesSkipped,
            rift_core::ProblemKind::MissingParent => ProblemKind::MissingParent,
            rift_core::ProblemKind::MissingRequirement => ProblemKind::MissingRequirement,
            rift_core::ProblemKind::ModinfoGarbled => ProblemKind::ModinfoGarbled,
            rift_core::ProblemKind::OutdatedFormats => ProblemKind::OutdatedFormats,
        }
    }
}

impl From<&rift_core::Problem> for Problem {
    fn from(problem: &rift_core::Problem) -> Self {
        Problem {
            kind: problem.kind.into(),
            message: problem.message.clone(),
        }
    }
}

impl From<&rift_core::ModInfo> for ModInfo {
    fn from(info: &rift_core::ModInfo) -> Self {
        ModInfo {
            name: info.name.clone(),
            description: info.description.clone(),
            author: info.author.clone(),
            version: info.version.clone(),
            category: info.category.clone(),
            screenshot: info.screenshot.clone(),
            homepage: info.homepage.clone(),
            addon_for: info.addon_for.clone(),
            name_as_bundle: info.name_as_bundle.clone(),
            requires: info.requires.clone(),
            dummy: info.dummy,
            menu_priority: info.menu_priority,
            extra: info.extra.clone(),
        }
    }
}

impl From<&rift_core::DeployTarget> for DeployTarget {
    fn from(target: &rift_core::DeployTarget) -> Self {
        match target {
            rift_core::DeployTarget::Pak { file_name } => DeployTarget::Pak {
                file_name: file_name.clone(),
            },
            rift_core::DeployTarget::Loose { game_path } => DeployTarget::Loose {
                game_path: game_path.clone(),
            },
            rift_core::DeployTarget::Reframework { game_path } => DeployTarget::Reframework {
                game_path: game_path.clone(),
            },
            rift_core::DeployTarget::Root { game_path } => DeployTarget::Root {
                game_path: game_path.clone(),
            },
        }
    }
}

impl From<rift_core::SkipReason> for SkipReason {
    fn from(reason: rift_core::SkipReason) -> Self {
        match reason {
            rift_core::SkipReason::Modinfo => SkipReason::Modinfo,
            rift_core::SkipReason::Picture => SkipReason::Picture,
            rift_core::SkipReason::Document => SkipReason::Document,
            rift_core::SkipReason::UnknownPlace => SkipReason::UnknownPlace,
        }
    }
}

impl From<&rift_core::ModOption> for ModOption {
    fn from(option: &rift_core::ModOption) -> Self {
        ModOption {
            id: option.id.0.clone(),
            package_id: option.package_id.0.clone(),
            root: option.root.clone(),
            name: option.name.clone(),
            info: option.info.as_ref().map(ModInfo::from),
            screenshot: option.screenshot.clone(),
            payload: option
                .payload
                .iter()
                .map(|file| PayloadFile {
                    entry: file.entry.clone(),
                    target: DeployTarget::from(&file.target),
                    size: file.size,
                })
                .collect(),
            skipped: option
                .skipped
                .iter()
                .map(|file| SkippedFile {
                    entry: file.entry.clone(),
                    reason: file.reason.into(),
                })
                .collect(),
            kinds: option.kinds.into(),
            parent: option.parent.as_ref().map(|id| id.0.clone()),
            addon_for: option.addon_for.clone(),
            bundle: option.bundle.clone(),
            menu_only: option.menu_only,
            trail: option.trail.clone(),
        }
    }
}

impl From<rift_core::LinkSource> for LinkSource {
    fn from(source: rift_core::LinkSource) -> Self {
        match source {
            rift_core::LinkSource::Filename => LinkSource::Filename,
            rift_core::LinkSource::Homepage => LinkSource::Homepage,
            rift_core::LinkSource::Md5 => LinkSource::Md5,
            rift_core::LinkSource::Manual => LinkSource::Manual,
            rift_core::LinkSource::Download => LinkSource::Download,
        }
    }
}

impl From<&rift_core::NexusLink> for NexusLink {
    fn from(link: &rift_core::NexusLink) -> Self {
        NexusLink {
            mod_id: link.mod_id,
            file_id: link.file_id,
            group_id: link.group_id,
            sqid: link.sqid.clone(),
            version: link.version.clone(),
            uploaded_ms: link.uploaded_ms,
            source: link.source.into(),
        }
    }
}

impl From<&rift_core::ArchiveEntry> for ArchiveEntry {
    fn from(entry: &rift_core::ArchiveEntry) -> Self {
        ArchiveEntry {
            path: entry.path.clone(),
            size: entry.size,
        }
    }
}

impl From<rift_deploy::GameSource> for GameSource {
    fn from(source: rift_deploy::GameSource) -> Self {
        match source {
            rift_deploy::GameSource::Steam => GameSource::Steam,
            rift_deploy::GameSource::Env => GameSource::Env,
            rift_deploy::GameSource::Manual => GameSource::Manual,
        }
    }
}

impl From<&rift_deploy::RefStatus> for ReframeworkSummary {
    fn from(status: &rift_deploy::RefStatus) -> Self {
        ReframeworkSummary {
            installed: status.installed,
            version: status.version.clone(),
            build_date: status.build_date.clone(),
            loose_file_loader: status.loose_file_loader,
            pak_directory: status.pak_directory,
            faulty_files: status.faulty_files.clone(),
        }
    }
}

impl From<rift_deploy::RefSwitch> for SwitchKey {
    fn from(switch: rift_deploy::RefSwitch) -> Self {
        match switch {
            rift_deploy::RefSwitch::LooseFileLoader => SwitchKey::LooseFileLoader,
            rift_deploy::RefSwitch::PakDirectory => SwitchKey::PakDirectory,
        }
    }
}

impl From<SwitchKey> for rift_deploy::RefSwitch {
    fn from(switch: SwitchKey) -> Self {
        match switch {
            SwitchKey::LooseFileLoader => rift_deploy::RefSwitch::LooseFileLoader,
            SwitchKey::PakDirectory => rift_deploy::RefSwitch::PakDirectory,
        }
    }
}

impl From<rift_deploy::CheckLevel> for CheckLevel {
    fn from(level: rift_deploy::CheckLevel) -> Self {
        match level {
            rift_deploy::CheckLevel::Ok => CheckLevel::Ok,
            rift_deploy::CheckLevel::Info => CheckLevel::Info,
            rift_deploy::CheckLevel::Warn => CheckLevel::Warn,
            rift_deploy::CheckLevel::Error => CheckLevel::Error,
        }
    }
}

impl From<&rift_deploy::CheckAction> for CheckAction {
    /// The window's button for a preflight check. `reviewParked` opens the
    /// parked-backup review, whose rows carry `restore_parked` and
    /// `discard_parked`.
    fn from(action: &rift_deploy::CheckAction) -> Self {
        match action {
            rift_deploy::CheckAction::EnableSwitch { switch_key } => CheckAction::EnableSwitch {
                switch: (*switch_key).into(),
            },
            rift_deploy::CheckAction::PickGameFolder => CheckAction::PickGameFolder,
            rift_deploy::CheckAction::OpenUrl { url } => CheckAction::OpenUrl { url: url.clone() },
            rift_deploy::CheckAction::Rescan => CheckAction::Rescan,
            rift_deploy::CheckAction::ReviewParked => CheckAction::ReviewParked,
        }
    }
}

/// The checks whose facts the window draws as rows of its own: the Preflight
/// page lists the game, its build, REFramework, both switches and the mods
/// folder itself, and skips the checks under its `OWN_CHECKS` ids
/// (`src/pages/PreflightPage.vue`). Besides `game` and `reframework`, whose
/// ids both sides share, each row is here as the id its check is made under
/// → the window's: rift-deploy names three of them differently, and `mods`
/// is the spine's own mods-folder check (v0.3).
pub const WINDOW_CHECK_IDS: &[(&str, &str)] = &[
    ("buildChanged", "build"),
    ("pakDirectory", "pakdir"),
    ("looseFileLoader", "loose"),
    ("mods", "mods"),
];

/// rift-deploy's checks the window shows from other overview fields and never
/// as a check: REFramework's faulty files are a row of the Preflight page's
/// own, drawn from `reframework.faultyFiles`.
pub const SHOWN_ELSEWHERE: &[&str] = &["faultyFiles"];

/// The id a preflight check crosses the bridge under.
pub fn window_check_id(deploy_id: &str) -> &str {
    WINDOW_CHECK_IDS
        .iter()
        .find(|(deploy, _)| *deploy == deploy_id)
        .map_or(deploy_id, |(_, window)| window)
}

impl From<&rift_deploy::Check> for Check {
    fn from(check: &rift_deploy::Check) -> Self {
        Check {
            id: window_check_id(&check.id).to_string(),
            level: check.level.into(),
            title: check.title.clone(),
            detail: check.detail.clone(),
            action: check.action.as_ref().map(CheckAction::from),
        }
    }
}

impl From<&rift_deploy::Origin> for InstallOrigin {
    fn from(origin: &rift_deploy::Origin) -> Self {
        match origin {
            rift_deploy::Origin::Riftstone => InstallOrigin::Riftstone,
            rift_deploy::Origin::Adopted { .. } => InstallOrigin::Adopted,
        }
    }
}

impl From<&rift_deploy::InstalledOption> for InstalledInfo {
    fn from(install: &rift_deploy::InstalledOption) -> Self {
        InstalledInfo {
            seq: install.seq,
            installed_at: install.installed_at.clone(),
            version: install.version.clone(),
            game_build: install.game_build.clone(),
            origin: (&install.origin).into(),
            files: install
                .files
                .iter()
                .map(|file| InstalledFile {
                    game_path: file.game_path.clone(),
                    kind: file.kind.into(),
                    size: file.size,
                })
                .collect(),
        }
    }
}

impl From<&rift_deploy::WriteAction> for WriteAction {
    fn from(action: &rift_deploy::WriteAction) -> Self {
        match action {
            rift_deploy::WriteAction::Create => WriteAction::Create,
            rift_deploy::WriteAction::OverwriteRiftstone { option_name, .. } => {
                WriteAction::OverwriteRiftstone {
                    option_name: option_name.clone(),
                }
            }
            rift_deploy::WriteAction::OverwriteUnowned => WriteAction::OverwriteUnowned,
            rift_deploy::WriteAction::Reinstall => WriteAction::Reinstall,
        }
    }
}

impl From<&rift_deploy::PlanWarning> for PlanWarning {
    fn from(warning: &rift_deploy::PlanWarning) -> Self {
        match warning {
            rift_deploy::PlanWarning::PakFolderOff => PlanWarning::PakFolderOff,
            rift_deploy::PlanWarning::LooseLoaderOff => PlanWarning::LooseLoaderOff,
            rift_deploy::PlanWarning::ReframeworkMissing => PlanWarning::ReframeworkMissing,
            rift_deploy::PlanWarning::GameRunning => PlanWarning::GameRunning,
            rift_deploy::PlanWarning::RequiresMissing { name } => {
                PlanWarning::RequiresMissing { name: name.clone() }
            }
            rift_deploy::PlanWarning::Conflicts {
                option_names,
                paths,
            } => PlanWarning::Conflicts {
                option_names: option_names.clone(),
                paths: *paths,
            },
            rift_deploy::PlanWarning::OutdatedFormats { count } => {
                PlanWarning::OutdatedFormats { count: *count }
            }
        }
    }
}

impl From<&rift_deploy::InstallPlan> for InstallPlanView {
    fn from(plan: &rift_deploy::InstallPlan) -> Self {
        InstallPlanView {
            option_id: plan.option_id.0.clone(),
            name: plan.name.clone(),
            writes: plan
                .writes
                .iter()
                .map(|write| PlannedWrite {
                    entry: write.entry.clone(),
                    game_path: write.game_path.clone(),
                    kind: write.kind.into(),
                    size: write.size,
                    action: (&write.action).into(),
                })
                .collect(),
            total_bytes: plan.total_bytes,
            warnings: plan.warnings.iter().map(PlanWarning::from).collect(),
            refused: plan.refused.clone(),
        }
    }
}

impl From<&rift_deploy::InstallReport> for InstallResultView {
    fn from(report: &rift_deploy::InstallReport) -> Self {
        InstallResultView {
            option_id: report.option_id.0.clone(),
            files_written: report.files_written,
            bytes_written: report.bytes_written,
            backups: report.backups,
            duration_ms: report.duration_ms,
            restart_needed: report.restart_needed,
        }
    }
}

impl From<&rift_deploy::RemovalAction> for RemovalAction {
    fn from(action: &rift_deploy::RemovalAction) -> Self {
        match action {
            rift_deploy::RemovalAction::Delete => RemovalAction::Delete,
            rift_deploy::RemovalAction::RestoreBackup { owner, .. } => {
                RemovalAction::RestoreBackup {
                    owner: owner.clone(),
                }
            }
            rift_deploy::RemovalAction::KeepChanged => RemovalAction::KeepChanged,
            rift_deploy::RemovalAction::KeepVanilla => RemovalAction::KeepVanilla,
            rift_deploy::RemovalAction::HandDown { to_option_name, .. } => {
                RemovalAction::HandDown {
                    to_option_name: to_option_name.clone(),
                }
            }
            rift_deploy::RemovalAction::AlreadyGone => RemovalAction::AlreadyGone,
        }
    }
}

impl From<&rift_deploy::UninstallPlan> for UninstallPlanView {
    fn from(plan: &rift_deploy::UninstallPlan) -> Self {
        UninstallPlanView {
            option_id: plan.option_id.0.clone(),
            name: plan.name.clone(),
            removals: plan
                .removals
                .iter()
                .map(|removal| PlannedRemoval {
                    game_path: removal.game_path.clone(),
                    kind: removal.kind.into(),
                    action: (&removal.action).into(),
                })
                .collect(),
            refused: plan.refused.clone(),
        }
    }
}

impl From<&rift_deploy::UninstallReport> for UninstallResultView {
    fn from(report: &rift_deploy::UninstallReport) -> Self {
        UninstallResultView {
            option_id: report.option_id.0.clone(),
            files_removed: report.files_removed,
            files_restored: report.files_restored,
            files_kept: report.files_kept,
            files_handed_down: report.files_handed_down,
            parked: report.parked.len(),
            restart_needed: report.restart_needed,
        }
    }
}

impl From<&rift_deploy::LaunchRoute> for LaunchRoute {
    fn from(route: &rift_deploy::LaunchRoute) -> Self {
        match route {
            rift_deploy::LaunchRoute::Steam { .. } => LaunchRoute::Steam,
            rift_deploy::LaunchRoute::Exe { .. } => LaunchRoute::Exe,
        }
    }
}

impl From<&rift_deploy::LaunchWarning> for LaunchWarning {
    fn from(warning: &rift_deploy::LaunchWarning) -> Self {
        match warning {
            rift_deploy::LaunchWarning::ReframeworkMissing { paks } => {
                LaunchWarning::ReframeworkMissing { paks: *paks }
            }
            rift_deploy::LaunchWarning::PakFolderOff { paks } => {
                LaunchWarning::PakFolderOff { paks: *paks }
            }
            rift_deploy::LaunchWarning::LooseLoaderOff { files } => {
                LaunchWarning::LooseLoaderOff { files: *files }
            }
            rift_deploy::LaunchWarning::BuildChanged { recorded, now } => {
                LaunchWarning::BuildChanged {
                    recorded: recorded.clone(),
                    now: now.clone(),
                }
            }
        }
    }
}

impl From<&rift_deploy::LaunchPlan> for LaunchPlanView {
    fn from(plan: &rift_deploy::LaunchPlan) -> Self {
        LaunchPlanView {
            launch: (&plan.route).into(),
            warnings: plan.warnings.iter().map(LaunchWarning::from).collect(),
            refused: plan.refused.clone(),
        }
    }
}

impl From<rift_deploy::OwnerKind> for OwnerKind {
    fn from(kind: rift_deploy::OwnerKind) -> Self {
        match kind {
            rift_deploy::OwnerKind::Unowned => OwnerKind::Unowned,
            rift_deploy::OwnerKind::Adopted => OwnerKind::Adopted,
            rift_deploy::OwnerKind::Riftstone => OwnerKind::Riftstone,
        }
    }
}

impl From<&rift_deploy::ParkedFile> for ParkedFileView {
    fn from(file: &rift_deploy::ParkedFile) -> Self {
        ParkedFileView {
            store: file.store.clone(),
            game_path: file.game_path.clone(),
            owner: file.owner.clone(),
            owner_kind: file.owner_kind.into(),
            size: file.size,
            parked_at: file.parked_at.clone(),
            reason: file.reason.clone(),
            occupied: file.occupied,
        }
    }
}

impl From<&rift_deploy::ParkedRestoreReport> for ParkedRestoreReport {
    /// The engine answers only once the kept file is back in place, so a
    /// report always says `restored`; a file that could not go back is an
    /// error (refused, or rolled back), never a report.
    fn from(report: &rift_deploy::ParkedRestoreReport) -> Self {
        ParkedRestoreReport {
            game_path: report.game_path.clone(),
            restored: true,
            swapped_out: report.swapped_out.as_ref().map(|kept| kept.store.clone()),
            restart_needed: report.restart_needed,
        }
    }
}

impl From<&rift_deploy::RecoveryReport> for RecoveryNote {
    fn from(report: &rift_deploy::RecoveryReport) -> Self {
        RecoveryNote {
            option_name: report.option_name.clone(),
            action: match report.action {
                rift_deploy::IntentAction::Install => RecoveryAction::Install,
                rift_deploy::IntentAction::Uninstall => RecoveryAction::Uninstall,
                rift_deploy::IntentAction::Reorder => RecoveryAction::Reorder,
                rift_deploy::IntentAction::RestoreParked => RecoveryAction::RestoreParked,
            },
            files_rolled_back: report.files_rolled_back,
        }
    }
}

impl From<&rift_deploy::PakLoadState> for PakLoadState {
    fn from(state: &rift_deploy::PakLoadState) -> Self {
        match state {
            rift_deploy::PakLoadState::Loaded { slot, native_name } => PakLoadState::Loaded {
                slot: *slot,
                native_name: native_name.clone(),
            },
            rift_deploy::PakLoadState::Pending => PakLoadState::Pending,
            rift_deploy::PakLoadState::NotLoaded => PakLoadState::NotLoaded,
            rift_deploy::PakLoadState::Unknown => PakLoadState::Unknown,
        }
    }
}

impl From<&rift_deploy::PakView> for PakView {
    fn from(view: &rift_deploy::PakView) -> Self {
        PakView {
            file_name: view.file_name.clone(),
            size: view.size,
            load: (&view.load).into(),
        }
    }
}

impl From<&rift_deploy::StrayFile> for StrayFile {
    fn from(stray: &rift_deploy::StrayFile) -> Self {
        StrayFile {
            game_path: stray.game_path.clone(),
            size: stray.size,
        }
    }
}

impl From<&rift_deploy::MissingFile> for MissingFile {
    fn from(missing: &rift_deploy::MissingFile) -> Self {
        MissingFile {
            option_id: missing.option_id.0.clone(),
            option_name: missing.option_name.clone(),
            game_path: missing.game_path.clone(),
        }
    }
}

impl From<&rift_deploy::PakLoadReport> for PakLoadReport {
    fn from(report: &rift_deploy::PakLoadReport) -> Self {
        PakLoadReport {
            log_started: report.log_started.clone(),
            injected: report
                .injected
                .iter()
                .map(|pak| InjectedPak {
                    file_name: pak.file_name.clone(),
                    native_name: pak.native_name.clone(),
                    slot: pak.slot,
                })
                .collect(),
            error: report.error.clone(),
        }
    }
}

impl From<&rift_catalog::MatchedField> for MatchedField {
    fn from(field: &rift_catalog::MatchedField) -> Self {
        match field {
            rift_catalog::MatchedField::Name => MatchedField::Name,
            rift_catalog::MatchedField::Trail => MatchedField::Trail,
            rift_catalog::MatchedField::Bundle => MatchedField::Bundle,
            rift_catalog::MatchedField::Category => MatchedField::Category,
            rift_catalog::MatchedField::Author => MatchedField::Author,
            rift_catalog::MatchedField::Description => MatchedField::Description,
            rift_catalog::MatchedField::PackageName => MatchedField::PackageName,
            rift_catalog::MatchedField::Version => MatchedField::Version,
            rift_catalog::MatchedField::File => MatchedField::File,
            rift_catalog::MatchedField::NexusLabel => MatchedField::NexusLabel,
        }
    }
}

impl From<&SearchQuery> for rift_catalog::SearchQuery {
    fn from(query: &SearchQuery) -> Self {
        rift_catalog::SearchQuery {
            text: query.text.clone(),
            filters: rift_catalog::SearchFilters {
                installed: query.filters.installed,
                include_menu_only: query.filters.include_menu_only,
                categories: query.filters.categories.clone(),
                tags: query.filters.tags.clone(),
            },
            limit: query.limit,
        }
    }
}

impl From<&rift_catalog::PackageLabels> for NexusLabels {
    fn from(labels: &rift_catalog::PackageLabels) -> Self {
        NexusLabels {
            category: labels.category.clone(),
            tags: labels.tags.clone(),
            adult: labels.adult,
        }
    }
}

impl From<&rift_catalog::LabelCount> for LabelCount {
    fn from(count: &rift_catalog::LabelCount) -> Self {
        LabelCount {
            name: count.name.clone(),
            options: count.options,
        }
    }
}

impl From<&rift_catalog::LabelFacets> for LabelFacets {
    fn from(facets: &rift_catalog::LabelFacets) -> Self {
        LabelFacets {
            categories: facets.categories.iter().map(LabelCount::from).collect(),
            tags: facets.tags.iter().map(LabelCount::from).collect(),
            unlabeled_packages: facets.unlabeled_packages,
        }
    }
}

impl From<&rift_nexus::NexusFile> for NexusFileView {
    fn from(file: &rift_nexus::NexusFile) -> Self {
        NexusFileView {
            file_id: file.file_id,
            name: file.name.clone(),
            version: file.version.clone(),
            uploaded_ms: file.uploaded_ms,
        }
    }
}

impl From<&rift_nexus::UpdateState> for UpdateStateView {
    fn from(state: &rift_nexus::UpdateState) -> Self {
        match state {
            rift_nexus::UpdateState::Current => UpdateStateView::Current,
            rift_nexus::UpdateState::UpdateAvailable { newer } => {
                UpdateStateView::UpdateAvailable {
                    newer: newer.into(),
                }
            }
            rift_nexus::UpdateState::LocalArchived { newer } => UpdateStateView::LocalArchived {
                newer: newer.as_ref().map(NexusFileView::from),
            },
            rift_nexus::UpdateState::Unpinned => UpdateStateView::Unpinned,
            rift_nexus::UpdateState::ModUnavailable { status } => UpdateStateView::ModUnavailable {
                status: status.clone(),
            },
        }
    }
}

impl From<rift_nexus::FileCategory> for NexusFileCategory {
    fn from(category: rift_nexus::FileCategory) -> Self {
        match category {
            rift_nexus::FileCategory::Main => NexusFileCategory::Main,
            rift_nexus::FileCategory::Update => NexusFileCategory::Update,
            rift_nexus::FileCategory::Optional => NexusFileCategory::Optional,
            rift_nexus::FileCategory::OldVersion => NexusFileCategory::OldVersion,
            rift_nexus::FileCategory::Miscellaneous => NexusFileCategory::Miscellaneous,
            rift_nexus::FileCategory::Removed => NexusFileCategory::Removed,
            rift_nexus::FileCategory::Archived => NexusFileCategory::Archived,
            rift_nexus::FileCategory::Unknown => NexusFileCategory::Unknown,
        }
    }
}

impl From<NexusBrowseSort> for rift_nexus::BrowseSort {
    fn from(sort: NexusBrowseSort) -> Self {
        match sort {
            NexusBrowseSort::Endorsements => rift_nexus::BrowseSort::Endorsements,
            NexusBrowseSort::Downloads => rift_nexus::BrowseSort::Downloads,
            NexusBrowseSort::Updated => rift_nexus::BrowseSort::Updated,
            NexusBrowseSort::Relevance => rift_nexus::BrowseSort::Relevance,
        }
    }
}

impl From<rift_nexus::EndorseStatus> for EndorseStatus {
    fn from(status: rift_nexus::EndorseStatus) -> Self {
        match status {
            rift_nexus::EndorseStatus::Endorsed => EndorseStatus::Endorsed,
            rift_nexus::EndorseStatus::Abstained => EndorseStatus::Abstained,
        }
    }
}

impl From<EndorseStatus> for rift_nexus::EndorseStatus {
    fn from(status: EndorseStatus) -> Self {
        match status {
            EndorseStatus::Endorsed => rift_nexus::EndorseStatus::Endorsed,
            EndorseStatus::Abstained => rift_nexus::EndorseStatus::Abstained,
        }
    }
}

impl NexusCandidate {
    /// A mod the account tracks, as a card: named through the batched v2 mod
    /// lookup, which knows the name, the mod-level version, the category,
    /// the adult flag and the last update, and leaves the rest unknown.
    /// `None` for `known` is a mod that lookup did not return (gone from
    /// Nexus): it still gets a card, so it can be untracked.
    pub fn of_tracked(
        mod_id: u64,
        known: Option<&rift_nexus::NexusMod>,
        linked_packages: usize,
    ) -> Self {
        NexusCandidate {
            mod_id,
            name: known
                .map(|known| known.name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("Nexus mod {mod_id}")),
            author: None,
            version: known.and_then(|known| known.version.clone()),
            category: known.and_then(|known| known.category.clone()),
            adult: known.is_some_and(|known| known.adult),
            downloads: None,
            endorsements: None,
            summary: None,
            updated_ms: known
                .and_then(|known| known.updated_ms)
                .and_then(|ms| i64::try_from(ms).ok()),
            page_url: rift_nexus::mod_page_url(mod_id),
            linked_packages,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — every key the TS reads, every union tag, every string literal.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::{NexusReason, RiftError};
    use serde::de::DeserializeOwned;
    use serde_json::{json, Value};

    /// The object keys of `value`, sorted.
    fn keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .unwrap_or_else(|| panic!("expected an object, got {value}"))
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    fn sorted(expected: &[&str]) -> Vec<String> {
        let mut keys: Vec<String> = expected.iter().map(|k| (*k).to_string()).collect();
        keys.sort();
        keys
    }

    /// Serialize, assert the exact key set, deserialize back, compare.
    fn round_trip<T>(sample: &T, expected: &[&str]) -> Value
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_value(sample).unwrap();
        assert_eq!(keys(&json), sorted(expected), "keys of {json}");
        let back: T = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(&back, sample, "round trip of {json}");
        json
    }

    /// A string-literal union: each variant serializes to exactly its TS text.
    fn literals<T>(cases: &[(T, &str)])
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        for (value, text) in cases {
            assert_eq!(serde_json::to_value(value).unwrap(), json!(text));
            let back: T = serde_json::from_value(json!(text)).unwrap();
            assert_eq!(&back, value);
        }
    }

    fn kinds() -> PayloadKinds {
        PayloadKinds {
            pak: 1,
            loose: 2,
            reframework: 0,
            root: 0,
        }
    }

    fn link() -> NexusLink {
        NexusLink {
            mod_id: 2207,
            file_id: Some(2391),
            group_id: Some(31),
            sqid: Some("Rk4vLmN2Q".into()),
            version: Some("8".into()),
            uploaded_ms: Some(1_791_987_780_000),
            source: LinkSource::Filename,
        }
    }

    fn layer(name: &str) -> LayerRef {
        LayerRef {
            option_id: format!("00000000000000aa#{name}"),
            name: name.into(),
        }
    }

    fn file_view() -> NexusFileView {
        NexusFileView {
            file_id: 9700,
            name: "Vernworth Cloaks".into(),
            version: Some("9".into()),
            uploaded_ms: 1_760_500_000_000,
        }
    }

    fn package_summary() -> PackageSummary {
        PackageSummary {
            id: "00000000000000aa".into(),
            path: "D:\\Vernworth\\The Riftstone\\mods\\Vernworth Cloaks.zip".into(),
            file_name: "Vernworth Cloaks.zip".into(),
            display_name: "Vernworth Cloaks".into(),
            format: PackageFormat::Zip,
            size: 1024,
            modified_ms: 1_760_000_000_000,
            entry_count: 3,
            option_count: 1,
            nexus: Some(link()),
            problems: vec![Problem {
                kind: ProblemKind::FilesSkipped,
                message: "1 file sits in a folder the game never reads.".into(),
            }],
        }
    }

    fn sample_option() -> ModOption {
        ModOption {
            id: "00000000000000aa#Glow".into(),
            package_id: "00000000000000aa".into(),
            root: "Glow".into(),
            name: "Lantern Glow Recolor".into(),
            info: Some(ModInfo {
                name: Some("Lantern Glow Recolor".into()),
                description: None,
                author: Some("Vern".into()),
                version: Some("2.1".into()),
                category: None,
                screenshot: Some("screenshot.png".into()),
                homepage: None,
                addon_for: None,
                name_as_bundle: None,
                requires: vec!["REFramework".into()],
                dummy: false,
                menu_priority: 0,
                extra: BTreeMap::from([("installtype".into(), "normal".into())]),
            }),
            screenshot: Some("Glow/screenshot.png".into()),
            payload: vec![PayloadFile {
                entry: "Glow/LanternGlow.pak".into(),
                target: DeployTarget::Pak {
                    file_name: "LanternGlow.pak".into(),
                },
                size: 4096,
            }],
            skipped: vec![SkippedFile {
                entry: "Glow/modinfo.ini".into(),
                reason: SkipReason::Modinfo,
            }],
            kinds: kinds(),
            parent: None,
            addon_for: None,
            bundle: None,
            menu_only: false,
            trail: vec!["Lantern Glow Recolor".into()],
        }
    }

    fn update_row() -> UpdateRow {
        UpdateRow {
            package_id: "00000000000000aa".into(),
            package_name: "Vernworth Cloaks".into(),
            link: Some(link()),
            mod_name: Some("Vernworth Cloaks".into()),
            local_version: Some("8".into()),
            local_uploaded_ms: Some(1_791_987_780_000),
            state: UpdateStateView::UpdateAvailable { newer: file_view() },
            page_url: Some("https://www.nexusmods.com/dragonsdogma2/mods/2207".into()),
            checked_ms: Some(1_760_600_000_000),
        }
    }

    // -- string-literal unions -------------------------------------------

    #[test]
    fn string_unions_speak_the_ts_literals() {
        literals(&[
            (PackageFormat::Zip, "zip"),
            (PackageFormat::SevenZip, "sevenZip"),
            (PackageFormat::Rar, "rar"),
            (PackageFormat::Folder, "folder"),
        ]);
        literals(&[
            (DeployKind::Pak, "pak"),
            (DeployKind::Loose, "loose"),
            (DeployKind::Reframework, "reframework"),
            (DeployKind::Root, "root"),
        ]);
        literals(&[
            (ProblemKind::Unreadable, "unreadable"),
            (ProblemKind::NothingToInstall, "nothingToInstall"),
            (ProblemKind::FilesSkipped, "filesSkipped"),
            (ProblemKind::MissingParent, "missingParent"),
            (ProblemKind::MissingRequirement, "missingRequirement"),
            (ProblemKind::ModinfoGarbled, "modinfoGarbled"),
            (ProblemKind::OutdatedFormats, "outdatedFormats"),
        ]);
        literals(&[
            (SkipReason::Modinfo, "modinfo"),
            (SkipReason::Picture, "picture"),
            (SkipReason::Document, "document"),
            (SkipReason::UnknownPlace, "unknownPlace"),
        ]);
        literals(&[
            (LinkSource::Filename, "filename"),
            (LinkSource::Homepage, "homepage"),
            (LinkSource::Md5, "md5"),
            (LinkSource::Manual, "manual"),
            (LinkSource::Download, "download"),
        ]);
        literals(&[
            (GameSource::Steam, "steam"),
            (GameSource::Env, "env"),
            (GameSource::Manual, "manual"),
        ]);
        literals(&[
            (CheckLevel::Ok, "ok"),
            (CheckLevel::Info, "info"),
            (CheckLevel::Warn, "warn"),
            (CheckLevel::Error, "error"),
        ]);
        literals(&[
            (SwitchKey::LooseFileLoader, "looseFileLoader"),
            (SwitchKey::PakDirectory, "pakDirectory"),
        ]);
        literals(&[
            (RecoveryAction::Install, "install"),
            (RecoveryAction::Uninstall, "uninstall"),
            (RecoveryAction::Reorder, "reorder"),
            (RecoveryAction::RestoreParked, "restoreParked"),
        ]);
        literals(&[
            (MatchedField::Name, "name"),
            (MatchedField::Trail, "trail"),
            (MatchedField::Bundle, "bundle"),
            (MatchedField::Category, "category"),
            (MatchedField::Author, "author"),
            (MatchedField::Description, "description"),
            (MatchedField::PackageName, "packageName"),
            (MatchedField::Version, "version"),
            (MatchedField::File, "file"),
            (MatchedField::NexusLabel, "nexusLabel"),
        ]);
        literals(&[
            (VeilMode::Off, "off"),
            (VeilMode::Adult, "adult"),
            (VeilMode::All, "all"),
        ]);
        literals(&[
            (OwnerKind::Unowned, "unowned"),
            (OwnerKind::Adopted, "adopted"),
            (OwnerKind::Riftstone, "riftstone"),
        ]);
        literals(&[
            (InstallOrigin::Riftstone, "riftstone"),
            (InstallOrigin::Adopted, "adopted"),
        ]);
        literals(&[(LaunchRoute::Steam, "steam"), (LaunchRoute::Exe, "exe")]);
        literals(&[
            (NexusKeyStanding::None, "none"),
            (NexusKeyStanding::Connected, "connected"),
            (NexusKeyStanding::Unchecked, "unchecked"),
            (NexusKeyStanding::Rejected, "rejected"),
        ]);
        literals(&[
            (UpdateBadge::Available, "available"),
            (UpdateBadge::Current, "current"),
            (UpdateBadge::Unknown, "unknown"),
        ]);
        literals(&[
            (TreeNodeKind::Option, "option"),
            (TreeNodeKind::Bundle, "bundle"),
        ]);
        literals(&[
            (UpdatesPhase::Linking, "linking"),
            (UpdatesPhase::Hashing, "hashing"),
            (UpdatesPhase::Checking, "checking"),
        ]);
        literals(&[(ThumbnailSize::Card, "card"), (ThumbnailSize::Full, "full")]);
    }

    // -- tagged unions: one round trip per variant ------------------------

    #[test]
    fn deploy_target_union() {
        let pak = round_trip(
            &DeployTarget::Pak {
                file_name: "LanternGlow.pak".into(),
            },
            &["kind", "fileName"],
        );
        assert_eq!(pak["kind"], "pak");
        for (target, kind) in [
            (
                DeployTarget::Loose {
                    game_path: "natives/stm/a.mesh.260421070".into(),
                },
                "loose",
            ),
            (
                DeployTarget::Reframework {
                    game_path: "reframework/autorun/glow.lua".into(),
                },
                "reframework",
            ),
            (
                DeployTarget::Root {
                    game_path: "dinput8.dll".into(),
                },
                "root",
            ),
        ] {
            let json = round_trip(&target, &["kind", "gamePath"]);
            assert_eq!(json["kind"], kind);
        }
    }

    #[test]
    fn scan_state_union_is_tagged_by_state() {
        let idle = round_trip(&ScanState::Idle, &["state"]);
        assert_eq!(idle, json!({ "state": "idle" }));
        let scanning = round_trip(
            &ScanState::Scanning {
                done: 3,
                total: 11,
                current: "Vernworth Cloaks.zip".into(),
            },
            &["state", "done", "total", "current"],
        );
        assert_eq!(scanning["state"], "scanning");
    }

    #[test]
    fn check_action_union() {
        for (action, kind) in [
            (CheckAction::PickGameFolder, "pickGameFolder"),
            (CheckAction::Rescan, "rescan"),
            (CheckAction::ReviewParked, "reviewParked"),
            (CheckAction::OpenModsFolder, "openModsFolder"),
        ] {
            assert_eq!(round_trip(&action, &["kind"])["kind"], kind);
        }
        let switch = round_trip(
            &CheckAction::EnableSwitch {
                switch: SwitchKey::PakDirectory,
            },
            &["kind", "switch"],
        );
        assert_eq!(
            switch,
            json!({ "kind": "enableSwitch", "switch": "pakDirectory" })
        );
        let url = round_trip(
            &CheckAction::OpenUrl {
                url: "https://github.com/praydog/REFramework-nightly/releases".into(),
            },
            &["kind", "url"],
        );
        assert_eq!(url["kind"], "openUrl");
    }

    #[test]
    fn conflict_key_union() {
        let path = round_trip(
            &ConflictKey::Path {
                path: "natives/stm/a.mesh".into(),
            },
            &["kind", "path"],
        );
        assert_eq!(path["kind"], "path");
        let hash = round_trip(
            &ConflictKey::Hash {
                hash: PathHashView {
                    lower: 0x65B4_86A1,
                    upper: 0x958E_DD0C,
                },
                path: None,
            },
            &["kind", "hash", "path"],
        );
        assert_eq!(hash["kind"], "hash");
        assert_eq!(keys(&hash["hash"]), sorted(&["lower", "upper"]));
        assert!(
            hash["path"].is_null(),
            "a hash with no loose name is null, not absent"
        );
    }

    #[test]
    fn update_state_union() {
        for (state, kind) in [
            (UpdateStateView::Current, "current"),
            (UpdateStateView::Unpinned, "unpinned"),
            (UpdateStateView::Unlinked, "unlinked"),
            (UpdateStateView::NotChecked, "notChecked"),
        ] {
            assert_eq!(round_trip(&state, &["kind"])["kind"], kind);
        }
        let available = round_trip(
            &UpdateStateView::UpdateAvailable { newer: file_view() },
            &["kind", "newer"],
        );
        assert_eq!(available["kind"], "updateAvailable");
        assert_eq!(
            keys(&available["newer"]),
            sorted(&["fileId", "name", "version", "uploadedMs"])
        );
        let archived = round_trip(
            &UpdateStateView::LocalArchived { newer: None },
            &["kind", "newer"],
        );
        assert_eq!(archived["kind"], "localArchived");
        assert!(archived["newer"].is_null());
        round_trip(
            &UpdateStateView::LocalArchived {
                newer: Some(file_view()),
            },
            &["kind", "newer"],
        );
        let gone = round_trip(
            &UpdateStateView::ModUnavailable {
                status: "hidden".into(),
            },
            &["kind", "status"],
        );
        assert_eq!(gone["kind"], "modUnavailable");
        let error = round_trip(
            &UpdateStateView::Error {
                message: "Nexus could not list this mod's files.".into(),
            },
            &["kind", "message"],
        );
        assert_eq!(error["kind"], "error");
    }

    #[test]
    fn write_action_union() {
        for (action, kind) in [
            (WriteAction::Create, "create"),
            (WriteAction::OverwriteUnowned, "overwriteUnowned"),
            (WriteAction::Reinstall, "reinstall"),
        ] {
            assert_eq!(round_trip(&action, &["kind"])["kind"], kind);
        }
        let riftstone = round_trip(
            &WriteAction::OverwriteRiftstone {
                option_name: "Lantern Glow Recolor".into(),
            },
            &["kind", "optionName"],
        );
        assert_eq!(riftstone["kind"], "overwriteRiftstone");
    }

    #[test]
    fn plan_warning_union() {
        for (warning, kind) in [
            (PlanWarning::PakFolderOff, "pakFolderOff"),
            (PlanWarning::LooseLoaderOff, "looseLoaderOff"),
            (PlanWarning::ReframeworkMissing, "reframeworkMissing"),
            (PlanWarning::GameRunning, "gameRunning"),
        ] {
            assert_eq!(round_trip(&warning, &["kind"])["kind"], kind);
        }
        assert_eq!(
            round_trip(
                &PlanWarning::RequiresMissing {
                    name: "REFramework".into()
                },
                &["kind", "name"]
            )["kind"],
            "requiresMissing"
        );
        assert_eq!(
            round_trip(
                &PlanWarning::Conflicts {
                    option_names: vec!["Lantern Glow Recolor".into()],
                    paths: 3
                },
                &["kind", "optionNames", "paths"]
            )["kind"],
            "conflicts"
        );
        assert_eq!(
            round_trip(
                &PlanWarning::OutdatedFormats { count: 2 },
                &["kind", "count"]
            )["kind"],
            "outdatedFormats"
        );
    }

    #[test]
    fn removal_action_union() {
        for (action, kind) in [
            (RemovalAction::Delete, "delete"),
            (RemovalAction::AlreadyGone, "alreadyGone"),
            (RemovalAction::KeepChanged, "keepChanged"),
            (RemovalAction::KeepVanilla, "keepVanilla"),
        ] {
            assert_eq!(round_trip(&action, &["kind"])["kind"], kind);
        }
        assert_eq!(
            round_trip(
                &RemovalAction::RestoreBackup {
                    owner: "Lantern Glow Recolor".into()
                },
                &["kind", "owner"]
            )["kind"],
            "restoreBackup"
        );
        assert_eq!(
            round_trip(
                &RemovalAction::HandDown {
                    to_option_name: "Vernworth Cloaks".into()
                },
                &["kind", "toOptionName"]
            )["kind"],
            "handDown"
        );
    }

    #[test]
    fn pak_load_state_union() {
        for (state, kind) in [
            (PakLoadState::Pending, "pending"),
            (PakLoadState::NotLoaded, "notLoaded"),
            (PakLoadState::Unknown, "unknown"),
        ] {
            assert_eq!(round_trip(&state, &["kind"])["kind"], kind);
        }
        let loaded = round_trip(
            &PakLoadState::Loaded {
                slot: 103,
                native_name: "re_chunk_000.pak.sub_000.pak.patch_001.pak".into(),
            },
            &["kind", "slot", "nativeName"],
        );
        assert_eq!(loaded["kind"], "loaded");
    }

    // -- structs: the exact key sets the TS interfaces declare --------------

    #[test]
    fn library_atoms_carry_exactly_the_ts_keys() {
        round_trip(
            &ModsFolderView {
                path: "D:\\Vernworth\\The Riftstone\\mods".into(),
                exists: true,
            },
            &["path", "exists"],
        );
        round_trip(&kinds(), &["pak", "loose", "reframework", "root"]);
        round_trip(
            &link(),
            &[
                "modId",
                "fileId",
                "groupId",
                "sqid",
                "version",
                "uploadedMs",
                "source",
            ],
        );
        let option = sample_option();
        let json = round_trip(
            &option,
            &[
                "id",
                "packageId",
                "root",
                "name",
                "info",
                "screenshot",
                "payload",
                "skipped",
                "kinds",
                "parent",
                "addonFor",
                "bundle",
                "menuOnly",
                "trail",
            ],
        );
        assert_eq!(
            keys(&json["info"]),
            sorted(&[
                "name",
                "description",
                "author",
                "version",
                "category",
                "screenshot",
                "homepage",
                "addonFor",
                "nameAsBundle",
                "requires",
                "dummy",
                "menuPriority",
                "extra",
            ])
        );
        assert_eq!(
            keys(&json["payload"][0]),
            sorted(&["entry", "target", "size"])
        );
        assert_eq!(keys(&json["skipped"][0]), sorted(&["entry", "reason"]));
        round_trip(
            &ArchiveEntry {
                path: "Glow/LanternGlow.pak".into(),
                size: 4096,
            },
            &["path", "size"],
        );
    }

    #[test]
    fn overview_carries_exactly_the_ts_keys() {
        let overview = Overview {
            game: Some(GameSummary {
                root: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Dragons Dogma 2".into(),
                build_id: Some("25290080".into()),
                source: GameSource::Steam,
                running: true,
                started_ms: Some(1_792_000_000_000),
                launch: LaunchRoute::Steam,
                build_changed: Some(BuildChanged {
                    recorded: "25290080".into(),
                    now: "25391144".into(),
                }),
            }),
            reframework: Some(ReframeworkSummary {
                installed: true,
                version: Some("v1.5.9.1+514".into()),
                build_date: Some("11.10.2026".into()),
                loose_file_loader: Some(true),
                pak_directory: None,
                faulty_files: vec![],
            }),
            mods: ModsFolderView {
                path: "D:\\Vernworth\\The Riftstone\\mods".into(),
                exists: true,
            },
            library: LibraryStats {
                packages: 11,
                options: 300,
                entries: 3000,
                installed_options: 5,
                updates_available: 1,
                conflicts: 3,
                last_scan_ms: None,
            },
            scan: ScanState::Idle,
            checks: vec![Check {
                id: "game".into(),
                level: CheckLevel::Error,
                title: "Dragon's Dogma 2 was not found".into(),
                detail: "Point the Riftstone at the game folder.".into(),
                action: Some(CheckAction::PickGameFolder),
            }],
            nexus: NexusSummary {
                last_check_ms: None,
                checking: false,
                linked: 4,
                unlinked: 7,
                updates: 1,
                has_api_key: true,
                key: NexusKeyStanding::Connected,
                account: Some(NexusAccountView {
                    name: "Arisen of Vernworth".into(),
                    premium: true,
                    supporter: false,
                    checked_ms: 1_792_000_000_000,
                }),
            },
            recovered: Some(RecoveryNote {
                option_name: "Lantern Glow Recolor".into(),
                action: RecoveryAction::Install,
                files_rolled_back: 2,
            }),
        };
        let json = round_trip(
            &overview,
            &[
                "game",
                "reframework",
                "mods",
                "library",
                "scan",
                "checks",
                "nexus",
                "recovered",
            ],
        );
        assert_eq!(
            keys(&json["game"]),
            sorted(&[
                "root",
                "buildId",
                "source",
                "running",
                "startedMs",
                "launch",
                "buildChanged"
            ])
        );
        assert_eq!(json["game"]["launch"], "steam");
        assert_eq!(keys(&json["mods"]), sorted(&["path", "exists"]));
        assert_eq!(
            keys(&json["game"]["buildChanged"]),
            sorted(&["recorded", "now"])
        );
        assert_eq!(
            keys(&json["reframework"]),
            sorted(&[
                "installed",
                "version",
                "buildDate",
                "looseFileLoader",
                "pakDirectory",
                "faultyFiles",
            ])
        );
        assert!(json["reframework"]["pakDirectory"].is_null());
        assert_eq!(
            keys(&json["library"]),
            sorted(&[
                "packages",
                "options",
                "entries",
                "installedOptions",
                "updatesAvailable",
                "conflicts",
                "lastScanMs",
            ])
        );
        assert_eq!(
            keys(&json["checks"][0]),
            sorted(&["id", "level", "title", "detail", "action"])
        );
        assert_eq!(
            keys(&json["nexus"]),
            sorted(&[
                "lastCheckMs",
                "checking",
                "linked",
                "unlinked",
                "updates",
                "hasApiKey",
                "key",
                "account",
            ])
        );
        assert_eq!(
            keys(&json["nexus"]["account"]),
            sorted(&["name", "premium", "supporter", "checkedMs"])
        );
        // No key: "none", and the account is null, never absent.
        let bare = serde_json::to_value(NexusSummary::default()).unwrap();
        assert_eq!(bare["key"], "none");
        assert!(bare["account"].is_null());
        // Not running: startedMs is null, never absent.
        let stopped = serde_json::to_value(GameSummary {
            running: false,
            started_ms: None,
            launch: LaunchRoute::Exe,
            ..overview.game.clone().unwrap()
        })
        .unwrap();
        assert!(stopped["startedMs"].is_null());
        assert_eq!(stopped["launch"], "exe");
        assert_eq!(
            keys(&json["recovered"]),
            sorted(&["optionName", "action", "filesRolledBack"])
        );
    }

    #[test]
    fn search_shapes_carry_exactly_the_ts_keys() {
        let query: SearchQuery = serde_json::from_value(json!({
            "text": "\"ermine cape\" ermine_cape",
            "filters": {
                "installed": null, "includeMenuOnly": false,
                "categories": ["Armour", "Visuals"], "tags": ["Utilities for Players"]
            },
            "limit": 0
        }))
        .unwrap();
        assert_eq!(query.filters.installed, None);
        assert_eq!(query.filters.categories, vec!["Armour", "Visuals"]);
        assert_eq!(query.filters.tags, vec!["Utilities for Players"]);
        let json = round_trip(&query, &["text", "filters", "limit"]);
        assert_eq!(
            keys(&json["filters"]),
            sorted(&["installed", "includeMenuOnly", "categories", "tags"])
        );
        // The crate gets both label filters as the window sent them, and the
        // window's 0 as the catalog's "no cap".
        let crate_query = rift_catalog::SearchQuery::from(&query);
        assert_eq!(crate_query.filters.categories, query.filters.categories);
        assert_eq!(crate_query.filters.tags, query.filters.tags);
        assert_eq!(crate_query.limit, 0);
        // A v0.1 query (no label filters) still reads: no label filter; a
        // v0.2 query's root filter is read and ignored.
        let v01: SearchQuery = serde_json::from_value(json!({
            "text": "",
            "filters": { "installed": null, "rootIds": ["custom-1"], "includeMenuOnly": false },
            "limit": 0
        }))
        .unwrap();
        assert!(v01.filters.categories.is_empty() && v01.filters.tags.is_empty());
        assert_eq!(
            keys(&serde_json::to_value(&v01).unwrap()["filters"]),
            sorted(&["installed", "includeMenuOnly", "categories", "tags"])
        );

        let result = SearchResult {
            option_id: "00000000000000aa#Glow".into(),
            package_id: "00000000000000aa".into(),
            score: 20.0,
            name: "Lantern Glow Recolor".into(),
            trail: vec![],
            bundle: None,
            package_name: "Lantern Glow Recolor".into(),
            package_format: PackageFormat::Zip,
            author: None,
            version: None,
            category: None,
            description_snippet: None,
            has_screenshot: true,
            kinds: kinds(),
            menu_only: false,
            matched: vec![MatchedField::Name, MatchedField::File],
            file_matches: vec!["Glow/LanternGlow.pak".into()],
            installed: Some(InstallBadge {
                origin: InstallOrigin::Adopted,
                installed_at: Some("2026-10-16T10:00:00Z".into()),
            }),
            update: Some(UpdateBadge::Current),
            conflicted: false,
            nexus_category: Some("Visuals".into()),
            nexus_tags: vec!["Utilities for Players".into()],
            adult: false,
        };
        let json = round_trip(
            &result,
            &[
                "optionId",
                "packageId",
                "score",
                "name",
                "trail",
                "bundle",
                "packageName",
                "packageFormat",
                "author",
                "version",
                "category",
                "descriptionSnippet",
                "hasScreenshot",
                "kinds",
                "menuOnly",
                "matched",
                "fileMatches",
                "installed",
                "update",
                "conflicted",
                "nexusCategory",
                "nexusTags",
                "adult",
            ],
        );
        assert_eq!(keys(&json["installed"]), sorted(&["origin", "installedAt"]));
        // An unlabeled package: null category, no tags — present, never absent.
        let bare = serde_json::to_value(SearchResult {
            nexus_category: None,
            nexus_tags: vec![],
            ..result
        })
        .unwrap();
        assert!(bare["nexusCategory"].is_null());
        assert_eq!(bare["nexusTags"], json!([]));
    }

    #[test]
    fn label_shapes_carry_exactly_the_ts_keys() {
        round_trip(
            &NexusLabels {
                category: Some("Armour".into()),
                tags: vec!["Cloaks".into()],
                adult: true,
            },
            &["category", "tags", "adult"],
        );
        let facets = round_trip(
            &LabelFacets {
                categories: vec![LabelCount {
                    name: "Armour".into(),
                    options: 3,
                }],
                tags: vec![LabelCount {
                    name: "Utilities for Players".into(),
                    options: 1,
                }],
                unlabeled_packages: 2,
            },
            &["categories", "tags", "unlabeledPackages"],
        );
        assert_eq!(keys(&facets["categories"][0]), sorted(&["name", "options"]));
        assert_eq!(keys(&facets["tags"][0]), sorted(&["name", "options"]));

        // From the catalog, field for field.
        let from_catalog = LabelFacets::from(&rift_catalog::LabelFacets {
            categories: vec![rift_catalog::LabelCount {
                name: "Visuals".into(),
                options: 4,
            }],
            tags: vec![],
            unlabeled_packages: 7,
        });
        assert_eq!(
            serde_json::to_value(from_catalog).unwrap(),
            json!({
                "categories": [{ "name": "Visuals", "options": 4 }],
                "tags": [],
                "unlabeledPackages": 7
            })
        );
        let labels = NexusLabels::from(&rift_catalog::PackageLabels {
            category: None,
            tags: vec!["Cloaks".into()],
            adult: true,
        });
        assert_eq!(
            serde_json::to_value(labels).unwrap(),
            json!({ "category": null, "tags": ["Cloaks"], "adult": true })
        );
    }

    #[test]
    fn option_and_package_details_carry_exactly_the_ts_keys() {
        let package = round_trip(
            &package_summary(),
            &[
                "id",
                "path",
                "fileName",
                "displayName",
                "format",
                "size",
                "modifiedMs",
                "entryCount",
                "optionCount",
                "nexus",
                "problems",
            ],
        );
        assert_eq!(keys(&package["problems"][0]), sorted(&["kind", "message"]));

        let conflict = ConflictView {
            key: ConflictKey::Path {
                path: "natives/stm/a.mesh".into(),
            },
            layers: vec![layer("Old"), layer("New")],
            winner: layer("New"),
            unconfirmed: false,
        };
        let json = round_trip(&conflict, &["key", "layers", "winner", "unconfirmed"]);
        assert_eq!(keys(&json["winner"]), sorted(&["optionId", "name"]));
        // v0.2: the unconfirmed flag crosses the bridge.
        let flagged = serde_json::to_value(ConflictView {
            unconfirmed: true,
            ..conflict.clone()
        })
        .unwrap();
        assert_eq!(flagged["unconfirmed"], true);

        round_trip(
            &update_row(),
            &[
                "packageId",
                "packageName",
                "link",
                "modName",
                "localVersion",
                "localUploadedMs",
                "state",
                "pageUrl",
                "checkedMs",
            ],
        );
        round_trip(
            &OptionRef {
                option_id: "00000000000000aa#Glow".into(),
                name: "Lantern Glow Recolor".into(),
                menu_only: false,
                has_screenshot: true,
                installed: false,
            },
            &["optionId", "name", "menuOnly", "hasScreenshot", "installed"],
        );
        let info = round_trip(
            &InstalledInfo {
                seq: 4,
                installed_at: "2026-10-16T10:00:00Z".into(),
                version: Some("2.1".into()),
                game_build: Some("25290080".into()),
                origin: InstallOrigin::Riftstone,
                files: vec![InstalledFile {
                    game_path: "pak_mods/zrift-0000-LanternGlow.pak".into(),
                    kind: DeployKind::Pak,
                    size: 4096,
                }],
            },
            &[
                "seq",
                "installedAt",
                "version",
                "gameBuild",
                "origin",
                "files",
            ],
        );
        assert_eq!(
            keys(&info["files"][0]),
            sorted(&["gamePath", "kind", "size"])
        );

        let detail = OptionDetail {
            option: sample_option(),
            package: package_summary(),
            parent_name: Some("Cloaks".into()),
            children: vec![],
            installed: None,
            conflicts: vec![conflict.clone()],
            update: Some(update_row()),
            labels: Some(NexusLabels {
                category: Some("Armour".into()),
                tags: vec!["Cloaks".into(), "Lore-Friendly".into()],
                adult: false,
            }),
        };
        let json = round_trip(
            &detail,
            &[
                "option",
                "package",
                "parentName",
                "children",
                "installed",
                "conflicts",
                "update",
                "labels",
            ],
        );
        assert_eq!(
            keys(&json["labels"]),
            sorted(&["category", "tags", "adult"])
        );
        let unlabeled = serde_json::to_value(OptionDetail {
            labels: None,
            ..detail
        })
        .unwrap();
        assert!(
            unlabeled["labels"].is_null(),
            "unlinked or never checked: null, not absent"
        );

        let tree = round_trip(&tree_node(), &TREE_NODE_KEYS);
        assert!(tree["optionId"].is_null());
        assert_eq!(tree["conflicted"], true);
    }

    const TREE_NODE_KEYS: [&str; 9] = [
        "optionId",
        "label",
        "kind",
        "menuOnly",
        "hasScreenshot",
        "installed",
        "conflicted",
        "kinds",
        "children",
    ];

    fn tree_node() -> TreeNode {
        TreeNode {
            option_id: None,
            label: "Cloaks".into(),
            kind: TreeNodeKind::Bundle,
            menu_only: false,
            has_screenshot: false,
            installed: false,
            conflicted: true,
            kinds: kinds(),
            children: vec![TreeNode {
                option_id: Some("00000000000000aa#Cloaks/Ermine Cape".into()),
                label: "Ermine Cape".into(),
                kind: TreeNodeKind::Option,
                menu_only: false,
                has_screenshot: true,
                installed: false,
                conflicted: true,
                kinds: kinds(),
                children: vec![],
            }],
        }
    }

    #[test]
    fn the_library_view_carries_exactly_the_ts_keys() {
        let view = LibraryView {
            mods: ModsFolderView {
                path: "D:\\Vernworth\\The Riftstone\\mods".into(),
                exists: true,
            },
            packages: vec![LibraryPackage {
                package: package_summary(),
                installable: 2,
                installed: 1,
                update: Some(UpdateBadge::Available),
                labels: Some(NexusLabels {
                    category: Some("Armour".into()),
                    tags: vec!["Cloaks".into()],
                    adult: false,
                }),
                adult: false,
                conflicted: true,
                tree: vec![tree_node()],
            }],
        };
        let json = round_trip(&view, &["mods", "packages"]);
        assert_eq!(
            keys(&json["packages"][0]),
            sorted(&[
                "package",
                "installable",
                "installed",
                "update",
                "labels",
                "adult",
                "conflicted",
                "tree",
            ])
        );
        assert_eq!(
            keys(&json["packages"][0]["tree"][0]),
            sorted(&TREE_NODE_KEYS)
        );
        assert_eq!(
            keys(&json["packages"][0]["tree"][0]["children"][0]),
            sorted(&TREE_NODE_KEYS)
        );
        // Unlinked and unlabelled: null, never absent.
        let bare = serde_json::to_value(LibraryPackage {
            update: None,
            labels: None,
            tree: vec![],
            ..view.packages[0].clone()
        })
        .unwrap();
        assert!(bare["update"].is_null() && bare["labels"].is_null());
        assert_eq!(bare["tree"], json!([]));
    }

    #[test]
    fn play_shapes_carry_exactly_the_ts_keys() {
        let plan = round_trip(
            &LaunchPlanView {
                launch: LaunchRoute::Exe,
                warnings: vec![
                    LaunchWarning::ReframeworkMissing { paks: 2 },
                    LaunchWarning::PakFolderOff { paks: 2 },
                    LaunchWarning::LooseLoaderOff { files: 3 },
                    LaunchWarning::BuildChanged {
                        recorded: "25290080".into(),
                        now: "25391144".into(),
                    },
                ],
                refused: None,
            },
            &["launch", "warnings", "refused"],
        );
        assert_eq!(plan["launch"], "exe");
        assert!(plan["refused"].is_null(), "null, never absent");
        assert_eq!(
            plan["warnings"],
            json!([
                { "kind": "reframeworkMissing", "paks": 2 },
                { "kind": "pakFolderOff", "paks": 2 },
                { "kind": "looseLoaderOff", "files": 3 },
                { "kind": "buildChanged", "recorded": "25290080", "now": "25391144" }
            ])
        );
        round_trip(
            &LaunchReport {
                launch: LaunchRoute::Steam,
                launched_ms: 1_792_000_000_000,
            },
            &["launch", "launchedMs"],
        );

        // From rift-deploy's plan, field for field: the route crosses as its name.
        let from_crate = LaunchPlanView::from(&rift_deploy::LaunchPlan {
            route: rift_deploy::LaunchRoute::Exe {
                exe: std::path::PathBuf::from("/game/DD2.exe"),
                cwd: std::path::PathBuf::from("/game"),
            },
            warnings: vec![rift_deploy::LaunchWarning::LooseLoaderOff { files: 1 }],
            refused: Some("Dragon's Dogma 2 is already running.".into()),
        });
        assert_eq!(
            serde_json::to_value(from_crate).unwrap(),
            json!({
                "launch": "exe",
                "warnings": [{ "kind": "looseLoaderOff", "files": 1 }],
                "refused": "Dragon's Dogma 2 is already running."
            }),
            "the exe's path never crosses the bridge"
        );
        // rift-deploy serializes its warnings the same way.
        for warning in [
            rift_deploy::LaunchWarning::ReframeworkMissing { paks: 1 },
            rift_deploy::LaunchWarning::PakFolderOff { paks: 1 },
            rift_deploy::LaunchWarning::LooseLoaderOff { files: 1 },
            rift_deploy::LaunchWarning::BuildChanged {
                recorded: "1".into(),
                now: "2".into(),
            },
        ] {
            assert_eq!(
                serde_json::to_value(LaunchWarning::from(&warning)).unwrap(),
                serde_json::to_value(&warning).unwrap()
            );
        }
    }

    #[test]
    fn install_shapes_carry_exactly_the_ts_keys() {
        let plan = round_trip(
            &InstallPlanView {
                option_id: "00000000000000aa#Glow".into(),
                name: "Lantern Glow Recolor".into(),
                writes: vec![PlannedWrite {
                    entry: "Glow/LanternGlow.pak".into(),
                    game_path: "pak_mods/zrift-0000-LanternGlow.pak".into(),
                    kind: DeployKind::Pak,
                    size: 4096,
                    action: WriteAction::Create,
                }],
                total_bytes: 4096,
                warnings: vec![PlanWarning::GameRunning],
                refused: None,
            },
            &[
                "optionId",
                "name",
                "writes",
                "totalBytes",
                "warnings",
                "refused",
            ],
        );
        assert_eq!(
            keys(&plan["writes"][0]),
            sorted(&["entry", "gamePath", "kind", "size", "action"])
        );
        round_trip(
            &InstallResultView {
                option_id: "00000000000000aa#Glow".into(),
                files_written: 1,
                bytes_written: 4096,
                backups: 0,
                duration_ms: 12,
                restart_needed: false,
            },
            &[
                "optionId",
                "filesWritten",
                "bytesWritten",
                "backups",
                "durationMs",
                "restartNeeded",
            ],
        );
        let plan = round_trip(
            &UninstallPlanView {
                option_id: "00000000000000aa#Glow".into(),
                name: "Lantern Glow Recolor".into(),
                removals: vec![PlannedRemoval {
                    game_path: "pak_mods/zrift-0000-LanternGlow.pak".into(),
                    kind: DeployKind::Pak,
                    action: RemovalAction::Delete,
                }],
                refused: None,
            },
            &["optionId", "name", "removals", "refused"],
        );
        assert_eq!(
            keys(&plan["removals"][0]),
            sorted(&["gamePath", "kind", "action"])
        );
        round_trip(
            &UninstallResultView {
                option_id: "00000000000000aa#Glow".into(),
                files_removed: 1,
                files_restored: 0,
                files_kept: 1,
                files_handed_down: 2,
                parked: 1,
                restart_needed: false,
            },
            &[
                "optionId",
                "filesRemoved",
                "filesRestored",
                "filesKept",
                "filesHandedDown",
                "parked",
                "restartNeeded",
            ],
        );
    }

    #[test]
    fn installed_view_carries_exactly_the_ts_keys() {
        let view = InstalledView {
            installs: vec![InstalledRow {
                option_id: "00000000000000aa#Glow".into(),
                name: "Lantern Glow Recolor".into(),
                package_name: "Lantern Glow Recolor".into(),
                package_missing: false,
                version: None,
                installed_at: "2026-10-16T10:00:00Z".into(),
                game_build: None,
                origin: InstallOrigin::Adopted,
                kinds: kinds(),
                file_count: 1,
                paks: vec![PakView {
                    file_name: "zrift-0000-LanternGlow.pak".into(),
                    size: 4096,
                    load: PakLoadState::Pending,
                }],
            }],
            pak_order: vec!["00000000000000aa#Glow".into()],
            conflicts: vec![],
            strays: vec![StrayFile {
                game_path: "pak_mods/old_test_mod.pak".into(),
                size: 1,
            }],
            missing: vec![MissingFile {
                option_id: "00000000000000aa#Glow".into(),
                option_name: "Lantern Glow Recolor".into(),
                game_path: "natives/stm/a.mesh".into(),
            }],
            pak_load: Some(PakLoadReport {
                log_started: Some("2026-10-14 21:34:14.970".into()),
                injected: vec![InjectedPak {
                    file_name: "zrift-0000-LanternGlow.pak".into(),
                    native_name: "re_chunk_000.pak.sub_000.pak.patch_001.pak".into(),
                    slot: 103,
                }],
                error: None,
            }),
            pak_count_warning: 100,
            parked: vec![ParkedFileView {
                store: "backups/3/natives/stm/lantern/glow.tex.251211553".into(),
                game_path: "natives/stm/lantern/glow.tex.251211553".into(),
                owner: "the file that was there before".into(),
                owner_kind: OwnerKind::Unowned,
                size: None,
                parked_at: "2026-10-16T10:00:00Z".into(),
                reason: "Something else had changed the file at its path.".into(),
                occupied: true,
            }],
        };
        let json = round_trip(
            &view,
            &[
                "installs",
                "pakOrder",
                "conflicts",
                "strays",
                "missing",
                "pakLoad",
                "pakCountWarning",
                "parked",
            ],
        );
        assert_eq!(
            keys(&json["parked"][0]),
            sorted(&[
                "store",
                "gamePath",
                "owner",
                "ownerKind",
                "size",
                "parkedAt",
                "reason",
                "occupied",
            ])
        );
        assert!(
            json["parked"][0]["size"].is_null(),
            "a kept file gone from the store has a null size"
        );
        assert_eq!(
            keys(&json["installs"][0]),
            sorted(&[
                "optionId",
                "name",
                "packageName",
                "packageMissing",
                "version",
                "installedAt",
                "gameBuild",
                "origin",
                "kinds",
                "fileCount",
                "paks",
            ])
        );
        assert_eq!(
            keys(&json["installs"][0]["paks"][0]),
            sorted(&["fileName", "size", "load"])
        );
        assert_eq!(keys(&json["strays"][0]), sorted(&["gamePath", "size"]));
        assert_eq!(
            keys(&json["missing"][0]),
            sorted(&["optionId", "optionName", "gamePath"])
        );
        assert_eq!(
            keys(&json["pakLoad"]),
            sorted(&["logStarted", "injected", "error"])
        );
        assert_eq!(
            keys(&json["pakLoad"]["injected"][0]),
            sorted(&["fileName", "nativeName", "slot"])
        );
    }

    #[test]
    fn nexus_settings_and_event_shapes_carry_exactly_the_ts_keys() {
        round_trip(
            &NexusQuota {
                hourly_remaining: 486,
                hourly_limit: 500,
                daily_remaining: 19_840,
                daily_limit: 20_000,
                hourly_reset_ms: None,
            },
            &[
                "hourlyRemaining",
                "hourlyLimit",
                "dailyRemaining",
                "dailyLimit",
                "hourlyResetMs",
            ],
        );
        round_trip(
            &UpdatesView {
                last_check_ms: None,
                rows: vec![update_row()],
            },
            &["lastCheckMs", "rows"],
        );
        round_trip(
            &Md5Result {
                package_id: "00000000000000aa".into(),
                matched: true,
                link: Some(link()),
            },
            &["packageId", "matched", "link"],
        );
        let settings: Settings =
            serde_json::from_value(json!({ "veil": "adult", "autoCheckUpdates": false })).unwrap();
        assert_eq!(settings.veil, VeilMode::Adult);
        round_trip(&settings, &["veil", "autoCheckUpdates"]);
        assert_eq!(
            Settings::default(),
            Settings {
                veil: VeilMode::Off,
                auto_check_updates: true
            },
            "the TS documents these defaults"
        );
        let report = round_trip(
            &ParkedRestoreReport {
                game_path: "natives/stm/lantern/glow.tex.251211553".into(),
                restored: true,
                swapped_out: None,
                restart_needed: false,
            },
            &["gamePath", "restored", "swappedOut", "restartNeeded"],
        );
        assert!(report["swappedOut"].is_null());
        round_trip(
            &ScanProgressEvent {
                done: 1,
                total: 2,
                current: "Vernworth Cloaks.zip".into(),
            },
            &["done", "total", "current"],
        );
        round_trip(
            &InstallProgressEvent {
                option_id: "00000000000000aa#Glow".into(),
                files_done: 1,
                files_total: 2,
                bytes_done: 10,
                bytes_total: 20,
            },
            &[
                "optionId",
                "filesDone",
                "filesTotal",
                "bytesDone",
                "bytesTotal",
            ],
        );
        round_trip(
            &UpdatesProgressEvent {
                phase: UpdatesPhase::Hashing,
                done: 1,
                total: 3,
            },
            &["phase", "done", "total"],
        );
        assert_eq!(serde_json::to_value(Nothing {}).unwrap(), json!({}));
    }

    const CANDIDATE_KEYS: [&str; 12] = [
        "modId",
        "name",
        "author",
        "version",
        "category",
        "adult",
        "downloads",
        "endorsements",
        "summary",
        "updatedMs",
        "pageUrl",
        "linkedPackages",
    ];

    fn candidate() -> NexusCandidate {
        NexusCandidate {
            mod_id: 2207,
            name: "Lantern Glow Recolor".into(),
            author: Some("Vern".into()),
            version: Some("2.1".into()),
            category: Some("Visuals".into()),
            adult: false,
            downloads: Some(15_230),
            endorsements: Some(412),
            summary: Some("Warmer lanterns for Vernworth's streets at night.".into()),
            updated_ms: Some(1_714_786_245_000),
            page_url: "https://www.nexusmods.com/dragonsdogma2/mods/2207".into(),
            linked_packages: 1,
        }
    }

    #[test]
    fn find_shapes_carry_exactly_the_ts_keys() {
        let findings = round_trip(
            &NexusFindings {
                package_id: "00000000000000aa".into(),
                terms: "Lantern Glow Recolor".into(),
                candidates: vec![candidate()],
            },
            &["packageId", "terms", "candidates"],
        );
        assert_eq!(keys(&findings["candidates"][0]), sorted(&CANDIDATE_KEYS));
        round_trip(&candidate(), &CANDIDATE_KEYS);

        // `T | null` is null, never absent.
        let bare = serde_json::to_value(NexusCandidate {
            author: None,
            version: None,
            category: None,
            downloads: None,
            endorsements: None,
            summary: None,
            updated_ms: None,
            ..candidate()
        })
        .unwrap();
        assert_eq!(keys(&bare), sorted(&CANDIDATE_KEYS));
        for key in [
            "author",
            "version",
            "category",
            "downloads",
            "endorsements",
            "summary",
            "updatedMs",
        ] {
            assert!(bare[key].is_null(), "{key} in {bare}");
        }

        // Nothing searchable: no words, no candidates.
        let nothing = round_trip(
            &NexusFindings {
                package_id: "00000000000000aa".into(),
                terms: String::new(),
                candidates: vec![],
            },
            &["packageId", "terms", "candidates"],
        );
        assert_eq!(nothing["terms"], "");
        assert_eq!(nothing["candidates"], json!([]));

        // From rift-nexus, field for field; the page URL is built from the id.
        let from_crate = NexusCandidate::of(
            &rift_nexus::ModCandidate {
                mod_id: 5150,
                name: "Vernworth Cloaks Collection".into(),
                author: None,
                version: Some("2".into()),
                category: Some("Armour".into()),
                adult: true,
                downloads: Some(9),
                endorsements: None,
                summary: Some("Cloaks for the cold roads.".into()),
                updated_ms: Some(1_791_987_830_000),
            },
            3,
        );
        assert_eq!(
            serde_json::to_value(from_crate).unwrap(),
            json!({
                "modId": 5150,
                "name": "Vernworth Cloaks Collection",
                "author": null,
                "version": "2",
                "category": "Armour",
                "adult": true,
                "downloads": 9,
                "endorsements": null,
                "summary": "Cloaks for the cold roads.",
                "updatedMs": 1_791_987_830_000_i64,
                "pageUrl": "https://www.nexusmods.com/dragonsdogma2/mods/5150",
                "linkedPackages": 3
            })
        );
    }

    fn file_entry() -> NexusFileEntry {
        NexusFileEntry {
            file_id: 9001,
            name: "Lantern Glow Recolor".into(),
            version: Some("2.2".into()),
            category: NexusFileCategory::Main,
            size: Some(12_845_056),
            uploaded_ms: 1_791_987_780_000,
            description: Some("Warmer amber for every hand lantern.".into()),
            changelog: vec!["Amber for every lantern.".into()],
            in_library: false,
            newest: true,
        }
    }

    fn download_view() -> DownloadView {
        DownloadView {
            id: "18f3a2b4c5d-1".into(),
            mod_id: 2207,
            file_id: 9001,
            mod_name: Some("Lantern Glow Recolor".into()),
            file_name: Some("Lantern Glow Recolor".into()),
            version: Some("2.2".into()),
            for_package_id: None,
            phase: DownloadPhase::Waiting,
            bytes_done: 0,
            bytes_total: Some(12_845_056),
            landed_as: None,
            package_id: None,
            already_had: false,
            failure: None,
            nexus_page_url: Some(
                "https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=9001&nmm=1"
                    .into(),
            ),
            started_ms: 1_791_987_780_000,
            finished_ms: None,
        }
    }

    const DOWNLOAD_KEYS: [&str; 17] = [
        "id",
        "modId",
        "fileId",
        "modName",
        "fileName",
        "version",
        "forPackageId",
        "phase",
        "bytesDone",
        "bytesTotal",
        "landedAs",
        "packageId",
        "alreadyHad",
        "failure",
        "nexusPageUrl",
        "startedMs",
        "finishedMs",
    ];

    #[test]
    fn v03_nexus_shapes_carry_exactly_the_ts_keys() {
        literals(&[
            (NexusFileCategory::Main, "main"),
            (NexusFileCategory::Update, "update"),
            (NexusFileCategory::Optional, "optional"),
            (NexusFileCategory::OldVersion, "oldVersion"),
            (NexusFileCategory::Miscellaneous, "miscellaneous"),
            (NexusFileCategory::Removed, "removed"),
            (NexusFileCategory::Archived, "archived"),
            (NexusFileCategory::Unknown, "unknown"),
        ]);
        literals(&[
            (NexusBrowseSort::Endorsements, "endorsements"),
            (NexusBrowseSort::Downloads, "downloads"),
            (NexusBrowseSort::Updated, "updated"),
            (NexusBrowseSort::Relevance, "relevance"),
        ]);
        literals(&[
            (EndorseStatus::Endorsed, "endorsed"),
            (EndorseStatus::Abstained, "abstained"),
        ]);
        literals(&[
            (DownloadPhase::Waiting, "waiting"),
            (DownloadPhase::Queued, "queued"),
            (DownloadPhase::Linking, "linking"),
            (DownloadPhase::Downloading, "downloading"),
            (DownloadPhase::Verifying, "verifying"),
            (DownloadPhase::Landed, "landed"),
            (DownloadPhase::Failed, "failed"),
            (DownloadPhase::Cancelled, "cancelled"),
        ]);
        literals(&[
            (NxmHandlerState::Ours, "ours"),
            (NxmHandlerState::None, "none"),
            (NxmHandlerState::Other, "other"),
            (NxmHandlerState::Unsupported, "unsupported"),
        ]);

        let entry = round_trip(
            &file_entry(),
            &[
                "fileId",
                "name",
                "version",
                "category",
                "size",
                "uploadedMs",
                "description",
                "changelog",
                "inLibrary",
                "newest",
            ],
        );
        assert_eq!(entry["category"], "main");
        let files = round_trip(
            &NexusModFiles {
                mod_id: 2207,
                mod_name: None,
                page_url: "https://www.nexusmods.com/dragonsdogma2/mods/2207".into(),
                adult: false,
                files: vec![file_entry()],
            },
            &["modId", "modName", "pageUrl", "adult", "files"],
        );
        assert!(files["modName"].is_null(), "null, never absent");

        let query = round_trip(
            &NexusBrowseQuery {
                text: "lantern".into(),
                sort: NexusBrowseSort::Updated,
                offset: 20,
            },
            &["text", "sort", "offset"],
        );
        assert_eq!(query["sort"], "updated");
        round_trip(
            &NexusBrowsePage {
                total: 41,
                offset: 20,
                mods: vec![candidate()],
            },
            &["total", "offset", "mods"],
        );

        let standings = round_trip(
            &NexusStandings {
                tracked: vec![candidate()],
                endorsements: vec![EndorsementView {
                    mod_id: 2207,
                    status: EndorseStatus::Endorsed,
                }],
                checked_ms: 1_791_987_780_000,
            },
            &["tracked", "endorsements", "checkedMs"],
        );
        assert_eq!(
            standings["endorsements"][0],
            json!({ "modId": 2207, "status": "endorsed" })
        );
        assert_eq!(keys(&standings["tracked"][0]), sorted(&CANDIDATE_KEYS));

        // Downloads and their events serialize only (`failure`/`refusal` are
        // rift-core's RiftError): the keys, and the error as the window reads it.
        let waiting = serde_json::to_value(download_view()).unwrap();
        assert_eq!(keys(&waiting), sorted(&DOWNLOAD_KEYS));
        for key in [
            "forPackageId",
            "landedAs",
            "packageId",
            "failure",
            "finishedMs",
        ] {
            assert!(waiting[key].is_null(), "{key} in {waiting}");
        }
        let failed = serde_json::to_value(DownloadView {
            phase: DownloadPhase::Failed,
            failure: Some(rift_core::RiftError::nexus_because(
                rift_core::NexusReason::LinkExpired,
                "The download link from Nexus Mods has expired.",
            )),
            nexus_page_url: None,
            finished_ms: Some(1_791_987_790_000),
            ..download_view()
        })
        .unwrap();
        assert_eq!(keys(&failed), sorted(&DOWNLOAD_KEYS));
        assert_eq!(
            failed["failure"],
            json!({ "kind": "nexus", "detail": {
                "detail": "The download link from Nexus Mods has expired.",
                "reason": "linkExpired"
            } })
        );
        assert_eq!(failed["phase"], "failed");

        round_trip(
            &DownloadProgressEvent {
                id: "18f3a2b4c5d-1".into(),
                phase: DownloadPhase::Downloading,
                bytes_done: 1_048_576,
                bytes_total: None,
            },
            &["id", "phase", "bytesDone", "bytesTotal"],
        );
        let accepted = serde_json::to_value(NxmReceivedEvent {
            mod_id: Some(2207),
            file_id: Some(9001),
            accepted: true,
            download_id: Some("18f3a2b4c5d-1".into()),
            refusal: None,
        })
        .unwrap();
        assert_eq!(
            keys(&accepted),
            sorted(&["modId", "fileId", "accepted", "downloadId", "refusal"])
        );
        assert!(accepted["refusal"].is_null());
        let refused = serde_json::to_value(NxmReceivedEvent {
            mod_id: None,
            file_id: None,
            accepted: false,
            download_id: None,
            refusal: Some(rift_core::RiftError::refused(
                "That Nexus Mods link opens a collection.",
            )),
        })
        .unwrap();
        assert!(refused["modId"].is_null() && refused["downloadId"].is_null());
        assert_eq!(refused["refusal"]["kind"], "refused");

        let handler = round_trip(
            &NxmHandlerView {
                state: NxmHandlerState::Ours,
                other: None,
                gives_back_to: Some("OtherManager.exe".into()),
            },
            &["state", "other", "givesBackTo"],
        );
        assert_eq!(handler["state"], "ours");
    }

    #[test]
    fn v03_crate_shapes_convert_for_the_window() {
        assert_eq!(
            NexusFileCategory::from(rift_nexus::FileCategory::OldVersion),
            NexusFileCategory::OldVersion
        );
        assert_eq!(
            rift_nexus::BrowseSort::from(NexusBrowseSort::Updated),
            rift_nexus::BrowseSort::Updated
        );
        assert_eq!(
            EndorseStatus::from(rift_nexus::EndorseStatus::Abstained),
            EndorseStatus::Abstained
        );
        assert_eq!(
            rift_nexus::EndorseStatus::from(EndorseStatus::Endorsed),
            rift_nexus::EndorseStatus::Endorsed
        );
        assert!(DownloadPhase::Landed.is_over() && DownloadPhase::Cancelled.is_over());
        assert!(!DownloadPhase::Verifying.is_over() && !DownloadPhase::Waiting.is_over());

        // A tracked mod as a card: named by the v2 lookup, the rest unknown.
        let known = rift_nexus::NexusMod {
            mod_id: 2207,
            name: "Lantern Glow Recolor".into(),
            version: Some("2.2".into()),
            updated_ms: Some(1_791_987_780_000),
            status: "published".into(),
            category: Some("Visuals".into()),
            tags: vec![],
            adult: true,
        };
        let card = serde_json::to_value(NexusCandidate::of_tracked(2207, Some(&known), 2)).unwrap();
        assert_eq!(keys(&card), sorted(&CANDIDATE_KEYS));
        assert_eq!(card["name"], "Lantern Glow Recolor");
        assert_eq!(card["adult"], true);
        assert_eq!(card["linkedPackages"], 2);
        assert_eq!(card["updatedMs"], 1_791_987_780_000_i64);
        assert!(card["author"].is_null() && card["downloads"].is_null());
        let gone = NexusCandidate::of_tracked(31337, None, 0);
        assert_eq!(
            gone.name, "Nexus mod 31337",
            "a card even when Nexus forgot the mod"
        );
        assert_eq!(
            gone.page_url,
            "https://www.nexusmods.com/dragonsdogma2/mods/31337"
        );
    }

    fn parked_backup(store: &str) -> rift_deploy::ParkedBackup {
        rift_deploy::ParkedBackup {
            store: store.into(),
            game_path: "natives/stm/lantern/glow.tex.251211553".into(),
            owner: rift_deploy::BelowOwner::Unowned,
            parked_at: "2026-10-16T10:00:00Z".into(),
            reason: "Something else had changed the file at its path.".into(),
        }
    }

    #[test]
    fn v02_crate_shapes_reach_the_window_unfolded() {
        // A handed-down file is no longer counted as kept; parked files are counted.
        let uninstall = UninstallResultView::from(&rift_deploy::UninstallReport {
            option_id: rift_core::OptionId("00000000000000aa#Glow".into()),
            files_removed: 1,
            files_restored: 2,
            files_kept: 3,
            files_handed_down: 4,
            restart_needed: true,
            parked: vec![parked_backup("backups/5/a"), parked_backup("backups/5/b")],
        });
        assert_eq!(
            (
                uninstall.files_removed,
                uninstall.files_restored,
                uninstall.files_kept,
                uninstall.files_handed_down,
                uninstall.parked
            ),
            (1, 2, 3, 4, 2)
        );
        assert!(uninstall.restart_needed);
        assert_eq!(
            MatchedField::from(&rift_catalog::MatchedField::NexusLabel),
            MatchedField::NexusLabel
        );

        let check = Check::from(&rift_deploy::Check {
            id: "parkedBackups".into(),
            level: rift_deploy::CheckLevel::Warn,
            title: "1 file(s) kept aside in the backup store".into(),
            detail: "…".into(),
            action: Some(rift_deploy::CheckAction::ReviewParked),
        });
        assert_eq!(check.id, "parkedBackups");
        assert_eq!(
            serde_json::to_value(&check).unwrap()["action"],
            json!({ "kind": "reviewParked" })
        );

        let recovered = RecoveryNote::from(&rift_deploy::RecoveryReport {
            action: rift_deploy::IntentAction::RestoreParked,
            option_id: rift_core::OptionId(String::new()),
            option_name: "the file kept aside for natives/stm/lantern/glow.tex.251211553".into(),
            files_rolled_back: 1,
            files_kept: 0,
            parked: vec![],
        });
        assert_eq!(recovered.action, RecoveryAction::RestoreParked);

        let kept = ParkedFileView::from(&rift_deploy::ParkedFile {
            store: "backups/5/natives/stm/lantern/glow.tex.251211553".into(),
            game_path: "natives/stm/lantern/glow.tex.251211553".into(),
            owner: "Lantern Glow Recolor".into(),
            owner_kind: rift_deploy::OwnerKind::Riftstone,
            size: Some(4096),
            parked_at: "2026-10-16T10:00:00Z".into(),
            reason: "Something else had changed the file at its path.".into(),
            occupied: false,
        });
        assert_eq!(kept.owner_kind, OwnerKind::Riftstone);
        assert_eq!(kept.size, Some(4096));
        assert_eq!(
            kept.store,
            "backups/5/natives/stm/lantern/glow.tex.251211553"
        );

        let swap = ParkedRestoreReport::from(&rift_deploy::ParkedRestoreReport {
            game_path: "natives/stm/lantern/glow.tex.251211553".into(),
            restored: parked_backup("backups/5/natives/stm/lantern/glow.tex.251211553"),
            swapped_out: Some(parked_backup(
                "backups/9/natives/stm/lantern/glow.tex.251211553",
            )),
            restart_needed: false,
        });
        assert_eq!(
            serde_json::to_value(&swap).unwrap(),
            json!({
                "gamePath": "natives/stm/lantern/glow.tex.251211553",
                "restored": true,
                "swappedOut": "backups/9/natives/stm/lantern/glow.tex.251211553",
                "restartNeeded": false
            }),
            "the swapped-out file crosses as its store handle"
        );
    }

    #[test]
    fn crate_shapes_convert_without_losing_the_window_fields() {
        let removal: RemovalAction = (&rift_deploy::RemovalAction::AlreadyGone).into();
        assert_eq!(removal, RemovalAction::AlreadyGone);
        let write: WriteAction = (&rift_deploy::WriteAction::OverwriteRiftstone {
            option_name: "Lantern Glow Recolor".into(),
            option_id: rift_core::OptionId("00000000000000aa#Glow".into()),
        })
            .into();
        assert_eq!(
            serde_json::to_value(write).unwrap(),
            json!({ "kind": "overwriteRiftstone", "optionName": "Lantern Glow Recolor" })
        );
        let check: Check = (&rift_deploy::Check {
            id: "pakDirectory".into(),
            level: rift_deploy::CheckLevel::Warn,
            title: "REFramework's pak folder is switched off".into(),
            detail: "…".into(),
            action: Some(rift_deploy::CheckAction::EnableSwitch {
                switch_key: rift_deploy::RefSwitch::PakDirectory,
            }),
        })
            .into();
        assert_eq!(
            serde_json::to_value(&check).unwrap()["action"],
            json!({ "kind": "enableSwitch", "switch": "pakDirectory" })
        );
        assert_eq!(
            check.id, "pakdir",
            "the id the window draws its own row for"
        );
        let state: UpdateStateView = (&rift_nexus::UpdateState::ModUnavailable {
            status: "hidden".into(),
        })
            .into();
        assert_eq!(
            serde_json::to_value(state).unwrap(),
            json!({ "kind": "modUnavailable", "status": "hidden" })
        );
    }

    /// The Preflight page, as the window builds it.
    const PREFLIGHT_PAGE: &str = include_str!("../../src/pages/PreflightPage.vue");

    /// The ids in the page's `OWN_CHECKS` set: the checks it skips because it
    /// draws their rows itself.
    fn own_checks(page: &str) -> Vec<String> {
        let start = page
            .find("OWN_CHECKS = new Set([")
            .expect("PreflightPage.vue still declares OWN_CHECKS")
            + "OWN_CHECKS = new Set([".len();
        let end = start + page[start..].find("])").expect("OWN_CHECKS closes");
        page[start..end]
            .split(',')
            .map(|id| id.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|id| !id.is_empty())
            .collect()
    }

    #[test]
    fn checks_the_window_draws_itself_cross_under_its_own_row_ids() {
        let own = own_checks(PREFLIGHT_PAGE);
        // rift-deploy's renamed checks land on the page's own rows…
        for (deploy, window) in WINDOW_CHECK_IDS {
            assert!(
                own.iter().any(|id| id == window),
                "{deploy} crosses as {window}, which PreflightPage.vue's OWN_CHECKS {own:?} does not skip — its row would show twice"
            );
            assert_eq!(window_check_id(deploy), *window);
        }
        // …so do the ids both sides already share (the spine's game check,
        // rift-deploy's REFramework check); the spine's mods-folder check is
        // in the table under its own id…
        for shared in ["game", "reframework"] {
            assert!(own.iter().any(|id| id == shared), "{shared} in {own:?}");
            assert_eq!(window_check_id(shared), shared);
        }
        // …and every own row is accounted for.
        for id in &own {
            assert!(
                ["game", "reframework"].contains(&id.as_str())
                    || WINDOW_CHECK_IDS.iter().any(|(_, window)| window == id),
                "PreflightPage.vue draws {id} itself; the spine must say which check that is"
            );
        }
        // The faulty-file row is drawn from the overview's REFramework summary.
        for shown in SHOWN_ELSEWHERE {
            assert_eq!(*shown, "faultyFiles");
            assert!(
                PREFLIGHT_PAGE.contains("ref_?.faultyFiles.length"),
                "the page no longer draws faulty files itself: stop leaving the {shown} check out"
            );
        }
        // Every other check passes through as rift-deploy names it.
        assert_eq!(window_check_id("paksLoaded"), "paksLoaded");
        assert_eq!(window_check_id("parkedBackups"), "parkedBackups");
    }

    // -- the error literals: rift-core's own types, read off the wire ------

    /// The window's types, as it is built.
    const WIRE: &str = include_str!("../../src/types/rift.ts");

    /// `text` without its comments — block comments (the TSDoc between a
    /// union's members) and lines that are only a `//` comment — so a quoted
    /// word in a comment is never read as a literal. (A `//` after code is
    /// left alone: `"rift://…"` is a literal.)
    fn without_comments(text: &str) -> String {
        let mut blocks_out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find("/*") {
            blocks_out.push_str(&rest[..start]);
            let comment = &rest[start..];
            rest = comment.find("*/").map_or("", |end| &comment[end + 2..]);
        }
        blocks_out.push_str(rest);
        blocks_out
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The string literals between `head` and the first `;` after it, in
    /// `wire`, sorted: the members of a union of literals.
    fn union_after(wire: &str, head: &str) -> Vec<String> {
        let start = wire
            .find(head)
            .unwrap_or_else(|| panic!("rift.ts declares {head}"))
            + head.len();
        let body = &wire[start..];
        let body = &body[..body
            .find(';')
            .unwrap_or_else(|| panic!("{head} closes with a ;"))];
        let mut literals: Vec<String> = body
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect();
        literals.sort();
        literals
    }

    /// Every `RiftError` kind, each once. The match has no wildcard arm: a
    /// kind added to rift-core does not build here until it is listed.
    fn every_rift_error() -> Vec<RiftError> {
        let text = String::new;
        let all = vec![
            RiftError::Unreadable {
                path: text(),
                detail: text(),
            },
            RiftError::UnsupportedFormat { path: text() },
            RiftError::Encrypted { path: text() },
            RiftError::CorruptArchive {
                path: text(),
                detail: text(),
            },
            RiftError::UnsafeEntryPath { entry: text() },
            RiftError::EntryNotFound { entry: text() },
            RiftError::GameNotFound,
            RiftError::Refused { reason: text() },
            RiftError::Journal { detail: text() },
            RiftError::Nexus {
                detail: text(),
                reason: None,
            },
            RiftError::Io { detail: text() },
        ];
        for err in &all {
            match err {
                RiftError::Unreadable { .. }
                | RiftError::UnsupportedFormat { .. }
                | RiftError::Encrypted { .. }
                | RiftError::CorruptArchive { .. }
                | RiftError::UnsafeEntryPath { .. }
                | RiftError::EntryNotFound { .. }
                | RiftError::GameNotFound
                | RiftError::Refused { .. }
                | RiftError::Journal { .. }
                | RiftError::Nexus { .. }
                | RiftError::Io { .. } => {}
            }
        }
        all
    }

    /// Every `NexusReason`, each once. As above, a reason added to
    /// rift-core does not build here until it is listed.
    fn every_nexus_reason() -> Vec<NexusReason> {
        use NexusReason as R;
        let all = vec![
            R::Offline,
            R::RateLimited,
            R::NoKey,
            R::KeyRejected,
            R::PremiumRequired,
            R::LinkExpired,
            R::LinkNotYours,
            R::LinkRefused,
            R::FileGone,
            R::SizeMismatch,
            R::NotTheFile,
            R::EndorseNotDownloaded,
            R::EndorseTooSoon,
            R::EndorseOwnMod,
        ];
        for reason in &all {
            match reason {
                R::Offline
                | R::RateLimited
                | R::NoKey
                | R::KeyRejected
                | R::PremiumRequired
                | R::LinkExpired
                | R::LinkNotYours
                | R::LinkRefused
                | R::FileGone
                | R::SizeMismatch
                | R::NotTheFile
                | R::EndorseNotDownloaded
                | R::EndorseTooSoon
                | R::EndorseOwnMod => {}
            }
        }
        all
    }

    #[test]
    fn error_kinds_and_nexus_reasons_are_the_literals_the_window_reads() {
        let wire = without_comments(WIRE);
        let interface = &wire[wire
            .find("export interface RiftError {")
            .expect("rift.ts declares RiftError")..];
        let mut kinds: Vec<String> = every_rift_error()
            .iter()
            .map(|err| {
                serde_json::to_value(err).unwrap()["kind"]
                    .as_str()
                    .expect("every RiftError crosses with a kind")
                    .to_string()
            })
            .collect();
        kinds.sort();
        assert_eq!(
            kinds,
            union_after(interface, "kind:"),
            "rift-core's RiftError kinds and rift.ts's RiftError.kind are one list"
        );

        let mut reasons: Vec<String> = every_nexus_reason()
            .iter()
            .map(|reason| {
                serde_json::to_value(reason)
                    .unwrap()
                    .as_str()
                    .expect("a reason crosses as a string")
                    .to_string()
            })
            .collect();
        reasons.sort();
        assert_eq!(
            reasons,
            union_after(&wire, "export type NexusReason ="),
            "rift-core's NexusReason and rift.ts's NexusReason are one list"
        );
    }

    #[test]
    fn the_literal_reader_skips_comments_and_stops_at_the_union_s_end() {
        let wire = without_comments(
            "export type Mood =\n  | \"calm\"\n  /** not \"stormy\" */\n  // nor \"wild\"\n  | \"grim\";\nconst x = \"after\";",
        );
        assert_eq!(union_after(&wire, "export type Mood ="), ["calm", "grim"]);
        assert_eq!(
            without_comments("const e = \"rift://scan-progress\";"),
            "const e = \"rift://scan-progress\";"
        );
    }
}
