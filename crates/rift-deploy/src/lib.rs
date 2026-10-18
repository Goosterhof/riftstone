//! rift-deploy — the Riftstone's hands in the game folder.
//!
//! The most dangerous crate in the workspace: it writes into a player's
//! Dragon's Dogma 2 install. Everything it does is shaped by that.
//!
//! - [`game`] finds the install (Steam's library files, or an env override)
//!   and reads its build id; [`vdf`] parses Valve's KeyValues text.
//! - [`reframework`] reads REFramework's own files — its version, the two
//!   loader switches, the faulty-file list, and from its log which paks it
//!   actually injected at the last launch.
//! - [`journal`] is the write-ahead journal: every install records its intent
//!   before the first byte moves and commits after the last. It reads the
//!   journals older builds wrote, lifted in memory (schema 1 → 2).
//! - [`guard`] is the target guard every write passes through.
//! - [`engine`] plans and performs install, uninstall, pak reorder, the
//!   re-key of moved installs and crash recovery; [`plan`] holds the shapes
//!   it reports.
//! - [`relink`] finds the library option an install whose package left the
//!   mods folder came from, by its files' paths and sizes, and says which
//!   entries of that option's package to read to confirm their bytes (the
//!   fingerprints the journal recorded).
//! - [`state`] reads what is in the game right now; [`preflight`] turns that
//!   into checks for the home frame.
//! - [`launch`] is Play: the route that starts the game, its warnings and its
//!   refusals.
//!
//! ## The invariants (each one is a test)
//!
//! 1. Nothing is written outside `natives/**`, `reframework/**`,
//!    `pak_mods/*.pak` and files directly in the game root, and never into
//!    [`rift_core::dd2::NEVER_WRITE`] — not through a link or junction either,
//!    and never through or over one of the engine's own temporary names.
//! 2. A file that exists and is not this install's own is moved into the
//!    backup store before the new bytes land, and comes back on uninstall.
//! 3. Intent is saved before bytes move; any failure — or a crash, found at
//!    the next launch — rolls the intent back from what is on disk, and the
//!    roll-back deletes only what it can prove the operation wrote.
//! 4. Installs over one path form a stack: uninstalling the top restores
//!    what was below; uninstalling a lower layer hands its backup up. A file
//!    something else replaced breaks the stack: its bytes are never taken
//!    for a layer's.
//! 5. Game paths are case-insensitive, as on NTFS: every ownership, stack and
//!    collision comparison goes through [`rift_core::lower_key`], and every
//!    physical path takes the spelling already on disk.
//! 6. Nothing that is not recorded in the journal is ever deleted, and no
//!    folder the engine did not create is ever removed.
//! 7. A backup that cannot go back without overwriting someone else's change
//!    is kept aside on record ([`ParkedBackup`]) — surfaced in the game
//!    state, restorable, never silently orphaned.
//! 8. Installs the journal took over from files already in the game
//!    ([`Origin::Adopted`]) follow every rule above like the Riftstone's own.

pub mod engine;
pub mod game;
pub mod guard;
pub mod journal;
pub mod launch;
pub mod paks;
pub mod plan;
pub mod preflight;
pub mod reframework;
pub mod relink;
pub mod state;
pub mod vdf;

mod fsops;
mod stack;

pub use engine::{
    discard_parked, install, is_game_running, plan_install, plan_uninstall, recover, rekey,
    rekey_with_files, reorder_paks, restore_parked, uninstall, DeployContext,
};
pub use game::{
    discover_game, discover_game_from, read_build_id, validate_game_root, GameInstall, GameSource,
    DD2_STEAM_APP_ID,
};
pub use journal::{
    journal_path, Below, BelowOwner, DeployKind, DeployedFile, InstalledOption, Intent,
    IntentAction, IntentStep, Journal, Leftover, Origin, ParkedBackup, JOURNAL_SCHEMA,
};
pub use launch::{launch_route, plan_launch, start_exe, LaunchPlan, LaunchRoute, LaunchWarning};
pub use plan::{
    InstallPlan, InstallReport, OwnerKind, ParkedRestoreReport, PlanWarning, PlannedRemoval,
    PlannedWrite, RecoveryReport, RemovalAction, UninstallPlan, UninstallReport, WriteAction,
};
pub use preflight::{preflight, Check, CheckAction, CheckLevel};
pub use reframework::{
    read_pak_load_report, read_status, set_switch, InjectedPak, PakLoadReport, RefStatus, RefSwitch,
};
pub use state::{
    read_game_state, BuildChange, GameState, InstallSummary, MissingFile, PakFile, PakLoadState,
    PakOwner, PakView, ParkedFile, StrayFile,
};
