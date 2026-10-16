//! Preflight: the game state turned into a short list of checks for the
//! home frame — what is fine, what needs a look, and the one action that
//! fixes each.

use serde::Serialize;

use crate::reframework::RefSwitch;
use crate::state::{GameState, PakLoadState, PakOwner};

/// Where REFramework's nightly builds (the ones current DD2 needs) live.
pub const REFRAMEWORK_URL: &str = "https://github.com/praydog/REFramework-nightly/releases";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// Stable id (`reframework`, `pakDirectory`, `paksLoaded`, …).
    pub id: String,
    pub level: CheckLevel,
    pub title: String,
    pub detail: String,
    pub action: Option<CheckAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckLevel {
    Ok,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CheckAction {
    AdoptFluffy,
    EnableSwitch {
        #[serde(rename = "switch")]
        switch_key: RefSwitch,
    },
    PickGameFolder,
    #[serde(rename_all = "camelCase")]
    OpenUrl {
        url: String,
    },
    Rescan,
    /// Show the files kept aside ([`GameState::parked`]), each with "put back"
    /// ([`crate::restore_parked`]) and "discard" ([`crate::discard_parked`]).
    ReviewParked,
}

/// Every check that applies to `state`, most severe first.
pub fn preflight(state: &GameState) -> Vec<Check> {
    let mut checks = Vec::new();
    let mut add = |id: &str,
                   level: CheckLevel,
                   title: String,
                   detail: String,
                   action: Option<CheckAction>| {
        checks.push(Check {
            id: id.to_string(),
            level,
            title,
            detail,
            action,
        });
    };
    let reframework = &state.reframework;
    let managed_paks = state
        .paks
        .iter()
        .filter(|p| matches!(p.owner, PakOwner::Riftstone { .. }))
        .count();
    let any_paks = !state.paks.is_empty();
    let installed_loose = state.installs.iter().any(|i| i.kinds.loose > 0)
        || state
            .strays
            .iter()
            .any(|s| s.game_path.to_lowercase().starts_with("natives/"));
    let needs_reframework = any_paks
        || installed_loose
        || state
            .installs
            .iter()
            .any(|i| i.kinds.reframework > 0 || i.kinds.pak > 0);

    if state.interrupted {
        add(
            "interrupted",
            CheckLevel::Error,
            "An install or uninstall was interrupted".into(),
            "The journal still holds an unfinished change to the game folder. Restart the Riftstone; it puts the folder back the way it was at launch.".into(),
            None,
        );
    }

    if reframework.installed {
        add(
            "reframework",
            CheckLevel::Ok,
            match &reframework.version {
                Some(version) => format!("REFramework {version} is installed"),
                None => "REFramework is installed".into(),
            },
            match &reframework.build_date {
                Some(date) => format!("Built {date}. It loads pak_mods/ and loose natives/ files."),
                None => "It loads pak_mods/ and loose natives/ files.".into(),
            },
            None,
        );
    } else {
        add(
            "reframework",
            if needs_reframework { CheckLevel::Error } else { CheckLevel::Warn },
            "REFramework is not installed".into(),
            "Pak mods, loose files and scripts load through REFramework (dinput8.dll in the game folder). Install a current nightly build before playing with mods.".into(),
            Some(CheckAction::OpenUrl {
                url: REFRAMEWORK_URL.into(),
            }),
        );
    }

    if any_paks && reframework.pak_directory == Some(false) {
        add(
            "pakDirectory",
            CheckLevel::Warn,
            "REFramework's pak folder is switched off".into(),
            format!(
                "{} pak(s) sit in pak_mods/, but IntegrityCheckBypass_LoadPakDirectory is false in re2_fw_config.txt, so none of them load.",
                state.paks.len()
            ),
            Some(CheckAction::EnableSwitch {
                switch_key: RefSwitch::PakDirectory,
            }),
        );
    }

    if installed_loose && reframework.loose_file_loader != Some(true) {
        add(
            "looseFileLoader",
            CheckLevel::Warn,
            "REFramework's loose file loader is off".into(),
            "Loose natives/ files are installed, but LooseFileLoader_Enabled is not true in re2_fw_config.txt, so the game ignores them.".into(),
            Some(CheckAction::EnableSwitch {
                switch_key: RefSwitch::LooseFileLoader,
            }),
        );
    }

    if managed_paks > 0 || any_paks {
        match &state.pak_load {
            None => add(
                "paksLoaded",
                CheckLevel::Info,
                "No REFramework log yet".into(),
                "REFramework writes re2_framework_log.txt when the game starts. Launch Dragon's Dogma 2 once and the Riftstone can tell which paks loaded.".into(),
                None,
            ),
            Some(report) if report.error.is_some() => add(
                "paksLoaded",
                CheckLevel::Error,
                "REFramework loaded none of the paks".into(),
                format!(
                    "Its log says: {}. Check that REFramework is current for this game version.",
                    report.error.clone().unwrap_or_default()
                ),
                Some(CheckAction::OpenUrl {
                    url: REFRAMEWORK_URL.into(),
                }),
            ),
            Some(_) => {
                let count = |want: fn(&PakLoadState) -> bool| {
                    state
                        .paks
                        .iter()
                        .filter(|p| p.loads && matches!(p.owner, PakOwner::Riftstone { .. }) && want(&p.load))
                        .count()
                };
                let not_loaded = count(|l| matches!(l, PakLoadState::NotLoaded));
                let pending = count(|l| matches!(l, PakLoadState::Pending));
                let loaded = count(|l| matches!(l, PakLoadState::Loaded { .. }));
                if not_loaded > 0 {
                    add(
                        "paksLoaded",
                        CheckLevel::Warn,
                        format!("{not_loaded} pak(s) did not load at the last launch"),
                        "They were in pak_mods/ when the game started, yet REFramework's log never injected them. Check the pak folder switch and REFramework's version.".into(),
                        None,
                    );
                } else if pending > 0 {
                    add(
                        "paksLoaded",
                        CheckLevel::Info,
                        format!("{pending} pak(s) load at the next launch"),
                        "They were installed or reordered after the game last started.".into(),
                        None,
                    );
                } else if loaded > 0 {
                    add(
                        "paksLoaded",
                        CheckLevel::Ok,
                        format!("All {loaded} managed pak(s) loaded at the last launch"),
                        "REFramework's log injected every pak the Riftstone manages.".into(),
                        None,
                    );
                }
            }
        }
    }

    if state.pak_count > state.pak_count_warning {
        add(
            "pakCount",
            CheckLevel::Warn,
            format!("{} paks in pak_mods/", state.pak_count),
            format!(
                "REFramework hands out pak slots without a bounds check; above {} the game risks crashing at launch. Merge or uninstall a few.",
                state.pak_count_warning
            ),
            None,
        );
    }

    let nested = state.paks.iter().filter(|p| p.nested).count();
    if nested > 0 {
        add(
            "pakNested",
            CheckLevel::Warn,
            format!("{nested} pak(s) sit in subfolders of pak_mods/"),
            "REFramework walks into subfolders, so these load in an order nobody chose. Keep pak_mods/ flat.".into(),
            None,
        );
    }

    let wrong_case = state.paks.iter().filter(|p| !p.loads).count();
    if wrong_case > 0 {
        add(
            "pakExtension",
            CheckLevel::Warn,
            format!("{wrong_case} pak(s) never load"),
            "Their extension is not a lowercase .pak, and REFramework's check is case-sensitive. Rename them to end in .pak.".into(),
            None,
        );
    }

    // A pak nobody manages loading after the Riftstone's own overrides them.
    let last_managed = state
        .paks
        .iter()
        .filter(|p| matches!(p.owner, PakOwner::Riftstone { .. }))
        .filter_map(|p| p.load_index)
        .max();
    if let Some(last) = last_managed {
        let overriding = state
            .paks
            .iter()
            .filter(|p| !matches!(p.owner, PakOwner::Riftstone { .. }))
            .filter(|p| p.load_index.is_some_and(|i| i > last))
            .count();
        if overriding > 0 {
            add(
                "strayPakOrder",
                CheckLevel::Warn,
                format!("{overriding} pak(s) the Riftstone does not manage load last"),
                "Their names sort after the Riftstone's own paks, so they override them. Adopt them from Fluffy or move them out of pak_mods/.".into(),
                None,
            );
        }
    }

    if !state.fluffy_unadopted.is_empty() {
        add(
            "fluffyUnadopted",
            CheckLevel::Info,
            format!("{} mod(s) installed by Fluffy", state.fluffy_unadopted.len()),
            "Adopt them to uninstall them and order their paks from the Riftstone. Fluffy's own files stay untouched.".into(),
            Some(CheckAction::AdoptFluffy),
        );
    }

    if !state.missing.is_empty() {
        add(
            "missingFiles",
            CheckLevel::Warn,
            format!("{} installed file(s) are gone", state.missing.len()),
            "Something deleted files the Riftstone installed. Reinstall the affected mods to put them back.".into(),
            None,
        );
    }

    if !state.parked.is_empty() {
        add(
            "parkedBackups",
            CheckLevel::Warn,
            format!(
                "{} file(s) kept aside in the backup store",
                state.parked.len()
            ),
            "Each one should have gone back into the game folder, but something else had changed the file at its path, so the Riftstone kept it instead of overwriting that change. Put each one back, or discard it if you no longer need it.".into(),
            Some(CheckAction::ReviewParked),
        );
    }

    if !state.strays.is_empty() {
        add(
            "strays",
            CheckLevel::Info,
            format!("{} file(s) in mod folders belong to no one", state.strays.len()),
            "Neither the Riftstone nor Fluffy installed them. They are listed so nothing hides; the Riftstone never removes them.".into(),
            None,
        );
    }

    if let Some(change) = &state.build_changed {
        add(
            "buildChanged",
            CheckLevel::Warn,
            "Dragon's Dogma 2 updated since your last install".into(),
            format!(
                "Steam build {} → {}. Mods built for the old build may break; check their pages for updates.",
                change.recorded, change.now
            ),
            None,
        );
    }

    if state.outdated_files > 0 {
        add(
            "outdatedFormats",
            CheckLevel::Warn,
            format!("{} installed file(s) use pre-TU 3.2 formats", state.outdated_files),
            "Their asset suffixes come from before the September 2026 engine update, so the game will not load them. The mods need updates.".into(),
            None,
        );
    }

    if !reframework.faulty_files.is_empty() {
        add(
            "faultyFiles",
            CheckLevel::Warn,
            format!("REFramework flagged {} faulty file(s)", reframework.faulty_files.len()),
            "Its FaultyFileDetector caught files the game could not read. They are listed in reframework_faulty_files.txt in the game folder.".into(),
            None,
        );
    }

    if state.crash_reporter_renamed {
        add(
            "crashReporter",
            CheckLevel::Info,
            "Fluffy renamed the game's crash reporter".into(),
            "CrashReport.exe is now CrashReport_FMMBACKUP.exe. Steam's \"Verify integrity of game files\" restores it; the Riftstone leaves it alone.".into(),
            None,
        );
    }

    if state.running {
        add(
            "gameRunning",
            CheckLevel::Info,
            "Dragon's Dogma 2 is running".into(),
            "Installs land now and load at the next launch. Uninstalling paks and changing their order waits until the game closes.".into(),
            None,
        );
    }

    checks.sort_by_key(|check| std::cmp::Reverse(check.level));
    checks
}
