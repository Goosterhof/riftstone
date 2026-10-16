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
pub enum RootKind {
    Fluffy,
    Downloads,
    Custom,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRoot {
    pub id: String,
    pub path: String,
    pub label: String,
    pub kind: RootKind,
    pub dd2_shaped_only: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub root: String,
    pub build_id: Option<String>,
    pub source: GameSource,
    pub running: bool,
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
pub struct FluffySummary {
    pub root: String,
    pub mods_dir: String,
    pub installed_count: usize,
    pub unadopted_count: usize,
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
    AdoptFluffy,
    EnableSwitch {
        switch: SwitchKey,
    },
    PickGameFolder,
    #[serde(rename_all = "camelCase")]
    OpenUrl {
        url: String,
    },
    Rescan,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusSummary {
    pub last_check_ms: Option<u64>,
    pub checking: bool,
    pub linked: usize,
    pub unlinked: usize,
    pub updates: usize,
    pub has_api_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryAction {
    Install,
    Uninstall,
    Reorder,
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
    pub fluffy: Option<FluffySummary>,
    pub roots: Vec<LibraryRoot>,
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    pub installed: Option<bool>,
    pub root_ids: Vec<String>,
    pub include_menu_only: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub filters: SearchFilters,
    /// 0 = backend default (200).
    pub limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallOrigin {
    Riftstone,
    Fluffy,
    FluffyUnadopted,
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
    pub root_id: String,
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
}

// ---------------------------------------------------------------------------
// get_option / get_package
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub id: String,
    pub root_id: String,
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

/// Who put an install in the journal: the Riftstone itself, or Fluffy (adopted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JournalOrigin {
    Riftstone,
    Fluffy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledInfo {
    pub seq: u64,
    pub installed_at: String,
    pub version: Option<String>,
    pub game_build: Option<String>,
    pub origin: JournalOrigin,
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
    /// A pak in this conflict carries feature flags no reference reader
    /// describes, so its hashes are unconfirmed. Kept off the wire until
    /// `src/types/rift.ts` grows the field (see the spine's contract request).
    #[serde(skip)]
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
    /// Installed by Fluffy and not adopted yet (matched by name).
    pub fluffy_installed: bool,
    pub conflicts: Vec<ConflictView>,
    pub update: Option<UpdateRow>,
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
    pub installed: bool,
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
// install / uninstall
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WriteAction {
    Create,
    #[serde(rename_all = "camelCase")]
    OverwriteRiftstone {
        option_name: String,
    },
    #[serde(rename_all = "camelCase")]
    OverwriteFluffy {
        section: String,
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
    pub files_kept: usize,
    pub restart_needed: bool,
}

// ---------------------------------------------------------------------------
// get_installed — what is in the game now
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRow {
    /// A library option's id — or, for an adopted Fluffy install that matched
    /// no option, the stand-in `fluffy#<section>` (rift-deploy's adoption
    /// scheme; conflict layers use it too for un-adopted Fluffy installs).
    pub option_id: String,
    pub name: String,
    pub package_name: String,
    /// The library holds no package for this install: its package left the
    /// library, or (a `fluffy#` stand-in) it never had one there.
    pub package_missing: bool,
    pub version: Option<String>,
    pub installed_at: String,
    pub game_build: Option<String>,
    pub origin: JournalOrigin,
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
pub struct FluffyModView {
    pub section: String,
    pub name: String,
    pub file_count: usize,
    pub matched_option_id: Option<String>,
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
    pub fluffy_unadopted: Vec<FluffyModView>,
    pub pak_load: Option<PakLoadReport>,
    pub pak_count_warning: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptResultView {
    pub adopted: usize,
    pub already_adopted: usize,
    pub missing_files: usize,
    pub unmatched: Vec<String>,
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
// settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub veil_thumbnails: bool,
    pub auto_check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            veil_thumbnails: false,
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

/// The `{}` payload of `rift://library-changed`, `rift://game-changed` and
/// `rift://updates-changed`.
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

impl From<rift_core::RootKind> for RootKind {
    fn from(kind: rift_core::RootKind) -> Self {
        match kind {
            rift_core::RootKind::Fluffy => RootKind::Fluffy,
            rift_core::RootKind::Downloads => RootKind::Downloads,
            rift_core::RootKind::Custom => RootKind::Custom,
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

impl From<&rift_core::LibraryRoot> for LibraryRoot {
    fn from(root: &rift_core::LibraryRoot) -> Self {
        LibraryRoot {
            id: root.id.clone(),
            path: path_text(&root.path),
            label: root.label.clone(),
            kind: root.kind.into(),
            dd2_shaped_only: root.dd2_shaped_only,
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

impl CheckAction {
    /// The window's action for a preflight check, when it has one. Reviewing
    /// the files kept aside has no window action yet (contract request: a
    /// `reviewParked` kind and the commands behind it), so that check comes
    /// without a button.
    pub fn from_deploy(action: &rift_deploy::CheckAction) -> Option<CheckAction> {
        Some(match action {
            rift_deploy::CheckAction::AdoptFluffy => CheckAction::AdoptFluffy,
            rift_deploy::CheckAction::EnableSwitch { switch_key } => CheckAction::EnableSwitch {
                switch: (*switch_key).into(),
            },
            rift_deploy::CheckAction::PickGameFolder => CheckAction::PickGameFolder,
            rift_deploy::CheckAction::OpenUrl { url } => CheckAction::OpenUrl { url: url.clone() },
            rift_deploy::CheckAction::Rescan => CheckAction::Rescan,
            rift_deploy::CheckAction::ReviewParked => return None,
        })
    }
}

/// rift-deploy's preflight checks whose facts the window draws as rows of
/// its own: the Preflight page lists the game, its build, REFramework, both
/// switches and Fluffy itself, and skips the checks under its `OWN_CHECKS`
/// ids (`src/pages/PreflightPage.vue`). rift-deploy names four of them
/// differently: its id → the window's.
pub const WINDOW_CHECK_IDS: &[(&str, &str)] = &[
    ("buildChanged", "build"),
    ("pakDirectory", "pakdir"),
    ("looseFileLoader", "loose"),
    ("fluffyUnadopted", "fluffy"),
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
            action: check.action.as_ref().and_then(CheckAction::from_deploy),
        }
    }
}

impl From<&rift_deploy::Origin> for JournalOrigin {
    fn from(origin: &rift_deploy::Origin) -> Self {
        match origin {
            rift_deploy::Origin::Riftstone => JournalOrigin::Riftstone,
            rift_deploy::Origin::AdoptedFromFluffy { .. } => JournalOrigin::Fluffy,
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
            rift_deploy::WriteAction::OverwriteFluffy { section } => WriteAction::OverwriteFluffy {
                section: section.clone(),
            },
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
            // The file is gone already: the end state is the one a delete
            // leaves. (Contract request: an `alreadyGone` kind in the TS.)
            rift_deploy::RemovalAction::AlreadyGone => RemovalAction::Delete,
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
            // A handed-down file stays in the game too: the install above
            // owns it now.
            files_kept: report.files_kept + report.files_handed_down,
            restart_needed: report.restart_needed,
        }
    }
}

impl From<&rift_deploy::AdoptReport> for AdoptResultView {
    fn from(report: &rift_deploy::AdoptReport) -> Self {
        AdoptResultView {
            adopted: report.adopted,
            already_adopted: report.already_adopted,
            missing_files: report.missing_files,
            unmatched: report.unmatched.clone(),
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
                // Putting a kept file back is a placement, like an install.
                // (Contract request: a `restoreParked` action in the TS.)
                rift_deploy::IntentAction::RestoreParked => RecoveryAction::Install,
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
        }
    }
}

impl From<&SearchQuery> for rift_catalog::SearchQuery {
    fn from(query: &SearchQuery) -> Self {
        rift_catalog::SearchQuery {
            text: query.text.clone(),
            filters: rift_catalog::SearchFilters {
                installed: query.filters.installed,
                root_ids: query.filters.root_ids.clone(),
                include_menu_only: query.filters.include_menu_only,
            },
            limit: query.limit,
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

// ---------------------------------------------------------------------------
// Tests — every key the TS reads, every union tag, every string literal.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
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
            root_id: "fluffy".into(),
            path: "C:\\Modding\\Fluffy\\Games\\DragonsDogma2\\Mods\\Vernworth Cloaks.zip".into(),
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
            (RootKind::Fluffy, "fluffy"),
            (RootKind::Downloads, "downloads"),
            (RootKind::Custom, "custom"),
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
        ]);
        literals(&[
            (InstallOrigin::Riftstone, "riftstone"),
            (InstallOrigin::Fluffy, "fluffy"),
            (InstallOrigin::FluffyUnadopted, "fluffyUnadopted"),
        ]);
        literals(&[
            (UpdateBadge::Available, "available"),
            (UpdateBadge::Current, "current"),
            (UpdateBadge::Unknown, "unknown"),
        ]);
        literals(&[
            (JournalOrigin::Riftstone, "riftstone"),
            (JournalOrigin::Fluffy, "fluffy"),
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
            (CheckAction::AdoptFluffy, "adoptFluffy"),
            (CheckAction::PickGameFolder, "pickGameFolder"),
            (CheckAction::Rescan, "rescan"),
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
        let fluffy = round_trip(
            &WriteAction::OverwriteFluffy {
                section: "LanternGlow-v2.1".into(),
            },
            &["kind", "section"],
        );
        assert_eq!(fluffy["kind"], "overwriteFluffy");
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
            &LibraryRoot {
                id: "downloads".into(),
                path: "C:\\Users\\arisen\\Downloads".into(),
                label: "Downloads".into(),
                kind: RootKind::Downloads,
                dd2_shaped_only: true,
            },
            &["id", "path", "label", "kind", "dd2ShapedOnly"],
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
        let option = ModOption {
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
        };
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
                running: false,
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
            fluffy: Some(FluffySummary {
                root: "C:\\Modding\\Fluffy".into(),
                mods_dir: "C:\\Modding\\Fluffy\\Games\\DragonsDogma2\\Mods".into(),
                installed_count: 6,
                unadopted_count: 1,
            }),
            roots: vec![],
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
                has_api_key: false,
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
                "fluffy",
                "roots",
                "library",
                "scan",
                "checks",
                "nexus",
                "recovered",
            ],
        );
        assert_eq!(
            keys(&json["game"]),
            sorted(&["root", "buildId", "source", "running", "buildChanged"])
        );
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
            keys(&json["fluffy"]),
            sorted(&["root", "modsDir", "installedCount", "unadoptedCount"])
        );
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
            ])
        );
        assert_eq!(
            keys(&json["recovered"]),
            sorted(&["optionName", "action", "filesRolledBack"])
        );
    }

    #[test]
    fn search_shapes_carry_exactly_the_ts_keys() {
        let query: SearchQuery = serde_json::from_value(json!({
            "text": "\"ermine cape\" top_314",
            "filters": { "installed": null, "rootIds": ["fluffy"], "includeMenuOnly": false },
            "limit": 0
        }))
        .unwrap();
        assert_eq!(query.filters.root_ids, vec!["fluffy".to_string()]);
        assert_eq!(query.filters.installed, None);
        round_trip(&query, &["text", "filters", "limit"]);
        let result = SearchResult {
            option_id: "00000000000000aa#Glow".into(),
            package_id: "00000000000000aa".into(),
            score: 20.0,
            name: "Lantern Glow Recolor".into(),
            trail: vec![],
            bundle: None,
            package_name: "Lantern Glow Recolor".into(),
            package_format: PackageFormat::Zip,
            root_id: "fluffy".into(),
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
                origin: InstallOrigin::FluffyUnadopted,
                installed_at: None,
            }),
            update: Some(UpdateBadge::Current),
            conflicted: false,
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
                "rootId",
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
            ],
        );
        assert_eq!(keys(&json["installed"]), sorted(&["origin", "installedAt"]));
    }

    #[test]
    fn option_and_package_details_carry_exactly_the_ts_keys() {
        let package = round_trip(
            &package_summary(),
            &[
                "id",
                "rootId",
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
        let json = round_trip(&conflict, &["key", "layers", "winner"]);
        assert_eq!(keys(&json["winner"]), sorted(&["optionId", "name"]));
        // The unconfirmed flag stays off the wire until the TS grows it.
        let flagged = serde_json::to_value(ConflictView {
            unconfirmed: true,
            ..conflict.clone()
        })
        .unwrap();
        assert!(flagged.get("unconfirmed").is_none());

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
                origin: JournalOrigin::Riftstone,
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

        let tree = round_trip(
            &TreeNode {
                option_id: None,
                label: "Cloaks".into(),
                kind: TreeNodeKind::Bundle,
                menu_only: false,
                has_screenshot: false,
                installed: false,
                kinds: kinds(),
                children: vec![],
            },
            &[
                "optionId",
                "label",
                "kind",
                "menuOnly",
                "hasScreenshot",
                "installed",
                "kinds",
                "children",
            ],
        );
        assert!(tree["optionId"].is_null());
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
                files_kept: 0,
                restart_needed: false,
            },
            &[
                "optionId",
                "filesRemoved",
                "filesRestored",
                "filesKept",
                "restartNeeded",
            ],
        );
        round_trip(
            &AdoptResultView {
                adopted: 1,
                already_adopted: 5,
                missing_files: 0,
                unmatched: vec!["Ox Cart Express".into()],
            },
            &["adopted", "alreadyAdopted", "missingFiles", "unmatched"],
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
                origin: JournalOrigin::Fluffy,
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
            fluffy_unadopted: vec![FluffyModView {
                section: "OxCart".into(),
                name: "Ox Cart Express".into(),
                file_count: 1,
                matched_option_id: None,
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
        };
        let json = round_trip(
            &view,
            &[
                "installs",
                "pakOrder",
                "conflicts",
                "strays",
                "missing",
                "fluffyUnadopted",
                "pakLoad",
                "pakCountWarning",
            ],
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
            keys(&json["fluffyUnadopted"][0]),
            sorted(&["section", "name", "fileCount", "matchedOptionId",])
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
            serde_json::from_value(json!({ "veilThumbnails": true, "autoCheckUpdates": false }))
                .unwrap();
        round_trip(&settings, &["veilThumbnails", "autoCheckUpdates"]);
        assert_eq!(
            Settings::default(),
            Settings {
                veil_thumbnails: false,
                auto_check_updates: true
            },
            "the TS documents these defaults"
        );
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

    #[test]
    fn crate_shapes_convert_without_losing_the_window_fields() {
        let removal: RemovalAction = (&rift_deploy::RemovalAction::AlreadyGone).into();
        assert_eq!(removal, RemovalAction::Delete);
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
        // rift-deploy's REFramework check)…
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
}
