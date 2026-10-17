//! What the engine says before and after it acts. Every shape serializes
//! camelCase with a `kind` tag on its enums, the way `src/types/rift.ts`
//! spells them (`InstallPlanView`, `UninstallPlanView`, …), so the window
//! crate can pass most of them straight through.

use rift_core::OptionId;
use serde::Serialize;

use crate::journal::{DeployKind, IntentAction, ParkedBackup};

/// What installing one option would do. Nothing is written to make it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub option_id: OptionId,
    pub name: String,
    pub writes: Vec<PlannedWrite>,
    pub total_bytes: u64,
    pub warnings: Vec<PlanWarning>,
    /// A full sentence when the install cannot run; `writes` is then empty.
    pub refused: Option<String>,
}

/// One file the install would put in the game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedWrite {
    /// The package entry it comes from.
    pub entry: String,
    /// Where it lands, relative to the game root, in the spelling on disk.
    pub game_path: String,
    pub kind: DeployKind,
    pub size: u64,
    pub action: WriteAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WriteAction {
    /// Nothing is there yet.
    Create,
    /// A file an install in the journal owns (an adopted one too) is there:
    /// it is backed up and comes back when this one is uninstalled.
    #[serde(rename_all = "camelCase")]
    OverwriteRiftstone {
        option_name: String,
        option_id: OptionId,
    },
    /// A file no install claims is there: backed up, and it comes back when
    /// this one is uninstalled.
    OverwriteUnowned,
    /// The option is already installed; this file is its own.
    Reinstall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlanWarning {
    /// Paks are in the payload but REFramework's pak folder loading is off.
    PakFolderOff,
    /// Loose files are in the payload but REFramework's loose file loader is off.
    LooseLoaderOff,
    /// The payload needs REFramework and it is not installed.
    ReframeworkMissing,
    /// Dragon's Dogma 2 is running: the change loads at the next launch.
    GameRunning,
    /// A `requires=` name that is not installed.
    #[serde(rename_all = "camelCase")]
    RequiresMissing { name: String },
    /// Installs in the journal (adopted ones too) this install overwrites.
    #[serde(rename_all = "camelCase")]
    Conflicts {
        option_names: Vec<String>,
        paths: usize,
    },
    /// Loose files carrying asset suffixes from before the TU 3.2 engine update.
    #[serde(rename_all = "camelCase")]
    OutdatedFormats { count: usize },
}

/// What an install did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub option_id: OptionId,
    pub files_written: usize,
    pub bytes_written: u64,
    /// Files moved into the backup store first.
    pub backups: usize,
    pub duration_ms: u64,
    /// The game is running: paks and new loose files load at the next launch.
    pub restart_needed: bool,
}

/// What uninstalling one option would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPlan {
    pub option_id: OptionId,
    pub name: String,
    pub removals: Vec<PlannedRemoval>,
    pub refused: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedRemoval {
    pub game_path: String,
    pub kind: DeployKind,
    pub action: RemovalAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RemovalAction {
    /// The file is this install's and nothing was below it: deleted.
    Delete,
    /// The file that was there before comes back from the backup store.
    #[serde(rename_all = "camelCase")]
    RestoreBackup {
        owner: String,
        owner_kind: OwnerKind,
    },
    /// Something else changed the file since: it is left alone.
    KeepChanged,
    /// What was below may be a game file: the file is left alone.
    KeepVanilla,
    /// A later install sits on top of this one: it inherits this one's
    /// backup; the game file does not change.
    #[serde(rename_all = "camelCase")]
    HandDown {
        to_option_name: String,
        to_option_id: OptionId,
    },
    /// The file is already gone.
    AlreadyGone,
}

/// Whose bytes a backup holds, for the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OwnerKind {
    /// No install: "the file that was there before".
    Unowned,
    /// An adopted install ([`crate::Origin::Adopted`]).
    Adopted,
    /// One of the Riftstone's own installs.
    Riftstone,
}

/// What an uninstall did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallReport {
    pub option_id: OptionId,
    pub files_removed: usize,
    pub files_restored: usize,
    /// Left in place: changed by something else, or possibly vanilla.
    pub files_kept: usize,
    pub files_handed_down: usize,
    pub restart_needed: bool,
    /// Files that could not go back because something else had changed the
    /// file at their path: kept aside in the backup store, on record
    /// ([`crate::Journal::parked`]), never deleted.
    pub parked: Vec<ParkedBackup>,
}

/// What recovery rolled back at launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReport {
    pub action: IntentAction,
    pub option_id: OptionId,
    pub option_name: String,
    pub files_rolled_back: usize,
    /// Files left alone because something else changed them meanwhile.
    pub files_kept: usize,
    /// Backups that could not go back because something else now sits at
    /// their path: kept aside in the backup store, on record.
    pub parked: Vec<ParkedBackup>,
}

/// What putting a kept file back did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParkedRestoreReport {
    pub game_path: String,
    /// The kept file, back in the game folder now.
    pub restored: ParkedBackup,
    /// The file that sat at the path, kept aside in its turn (put it back the
    /// same way to undo the swap).
    pub swapped_out: Option<ParkedBackup>,
    pub restart_needed: bool,
}
