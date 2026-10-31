//! The Riftstone — the window spine.
//!
//! The Vue window (`src/`) asks; this crate answers by wiring the six crates
//! under `crates/` together: it holds the library (the one mods folder), the
//! journal and the Nexus links ([`state`]; no lock is held across a disk,
//! archive or network read), gives v0.2's ids their v0.3 spelling after each
//! scan ([`migrate`]), reads the game folder once per change and shares that
//! read ([`fresh`]), remembers its state between launches ([`persist`]),
//! answers every command of `docs/WINDOW-CONTRACT.md` ([`commands`], all off
//! the async runtime) in the shapes of `src/types/rift.ts` ([`dto`]), starts
//! the game and watches it run ([`play`]), and tells the window when
//! something changed ([`events`]): a scan, an install, an update check, the
//! mods folder changing under its watcher ([`watch`]), the game starting or
//! stopping. v0.3's key extras: the account ([`account`]), downloads into the
//! mods folder ([`downloads`]), and the nxm handler ([`nxm`], through the
//! registry seam [`protocol`]) that takes the site's Mod Manager Download
//! links — `tauri-plugin-single-instance` is the first plugin, so a second
//! launch hands its link to the running stone.
//! v0.4: the Muster ([`muster`]) reads the save slots, read-only, and names their parties
//! from the game's own text.
//!
//! Environment overrides, read once at startup (the WSL bench and the tests):
//! `RIFTSTONE_DATA_HOME` (replaces the app data + cache dirs, and takes the log
//! and the WebView2 profile with it, so a bench run leaves nothing in the real
//! app folders), `RIFTSTONE_MODS_DIR` (the mods folder, else `mods` beside
//! `riftstone.exe`), and through rift-deploy's discovery
//! `RIFTSTONE_GAME_ROOT`, `RIFTSTONE_STEAM_ROOT`.

pub mod account;
pub mod commands;
pub mod conflicts;
pub mod downloads;
pub mod dto;
pub mod events;
pub mod family;
pub mod fray;
pub mod fresh;
pub mod keys;
pub mod landing;
pub mod library;
pub mod migrate;
pub mod muster;
pub mod nexus;
pub mod nxm;
#[cfg(feature = "pawn")]
pub mod pawn;
pub mod persist;
pub mod play;
pub mod protocol;
pub mod save_backup;
pub mod state;
pub mod thumbs;
pub mod watch;

use std::sync::Arc;

use tauri::Manager;

use crate::persist::DataDirs;
use crate::state::{mods_dir_from, AppState, SpineConfig, MODS_DIR_ENV};

/// A second launch (`riftstone.exe "nxm://…"`, Windows answering the site's
/// Mod Manager Download button) handed its arguments over: each nxm link
/// goes to the spine (held at its door while the launch's key validation or
/// the window's first frame is still to come), and the window comes
/// forward. This runs on the window's own thread, so nothing here waits on
/// the network.
fn second_launch<R: tauri::Runtime>(app: &tauri::AppHandle<R>, args: Vec<String>, _cwd: String) {
    match app.try_state::<Arc<AppState>>() {
        Some(spine) => nxm::second_launch(spine.inner(), &args),
        None if !nxm::nxm_arguments(&args).is_empty() => log::warn!(
            "a Mod Manager Download link arrived before the Riftstone was awake; press the button again"
        ),
        None => {}
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn run() {
    // The uninstaller's last word (`riftstone.exe --give-back-nxm`, from
    // `windows/hooks.nsh` before a file is removed): give the nxm:// links
    // back and exit. Before any plugin, so a running stone is not handed the
    // argument and no window opens. (`args_os`: an argument that is not
    // Unicode must never stop the stone from starting.)
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg == nxm::GIVE_BACK_ARG)
    {
        std::process::exit(nxm::give_back_on_uninstall());
    }
    let data_home = DataDirs::from_env();
    let webview_dir = data_home.as_ref().map(|dirs| dirs.data.join("webview"));
    let log_file = match &data_home {
        Some(dirs) => tauri_plugin_log::TargetKind::Folder {
            path: dirs.data.join("logs"),
            file_name: None,
        },
        None => tauri_plugin_log::TargetKind::LogDir { file_name: None },
    };
    let builder = tauri::Builder::default()
        // First, before anything else sets up: a second launch hands its
        // arguments to the running stone and exits.
        .plugin(tauri_plugin_single_instance::init(second_launch))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(log_file))
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(move |app| {
            let dirs = match data_home {
                Some(dirs) => dirs,
                None => DataDirs {
                    data: app.path().app_data_dir()?,
                    cache: app.path().app_cache_dir()?,
                },
            };
            log::info!("the Riftstone keeps its state in {}", dirs.data.display());
            // The one library: `RIFTSTONE_MODS_DIR`, else `mods` beside the
            // installed riftstone.exe (resolved once, never canonicalized).
            let exe = std::env::current_exe().unwrap_or_else(|err| {
                log::warn!("could not find where riftstone.exe runs from ({err}); the mods folder is read beside the working folder");
                std::path::PathBuf::from("riftstone.exe")
            });
            let library_dir = mods_dir_from(&exe, std::env::var_os(MODS_DIR_ENV));
            log::info!("the mods folder is {}", library_dir.display());
            // Startup steps 1–3 here (fast); the window shows at once with the
            // cached library, and steps 4–8 run in the background (`wake`).
            let spine = AppState::open(
                SpineConfig::system(dirs, library_dir),
                Arc::new(events::TauriEvents(app.handle().clone())),
                Box::new(keys::SystemKeyStore),
            );
            app.manage(Arc::clone(&spine));
            app.manage(Arc::new(fray::FrayDesk::default()));
            // THE WORKSHOP's desk (v0.7): only a build with the pawn (RS-10).
            #[cfg(feature = "pawn")]
            app.manage(pawn::desk::Desk::new(&spine.config.dirs.data));
            // A cold start for a Mod Manager Download link: held at the door
            // until the key is validated and the window listens (startup
            // step 9).
            let args: Vec<String> = std::env::args().collect();
            spine.queue_launch_links(nxm::nxm_arguments(&args));
            spine.wake();
            // The window of tauri.conf.json (`create: false`), built here so a
            // bench data home takes the WebView2 profile along: left to itself,
            // Tauri makes its own folder under the real app-local dir first.
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == "main")
                .cloned()
                .ok_or("tauri.conf.json has no main window")?;
            let mut window = tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?;
            if let Some(dir) = webview_dir {
                window = window.data_directory(dir);
            }
            window.build()?;
            Ok(())
        });
    with_commands(builder)
        .run(tauri::generate_context!())
        .expect("the Riftstone failed to open its window");
}

/// Every command of docs/WINDOW-CONTRACT.md under its invoke name, and any
/// given after them. One list for every build: the pawn's build adds THE
/// WORKSHOP's (RS-10).
macro_rules! riftstone_handler {
    ($($extra:tt)*) => {
        tauri::generate_handler![
        commands::overview::get_overview,
        commands::overview::rescan,
        commands::library::search,
        commands::library::get_option,
        commands::library::get_package,
        commands::library::get_thumbnail,
        commands::deploy::plan_install,
        commands::deploy::install,
        commands::deploy::plan_uninstall,
        commands::deploy::uninstall,
        commands::deploy::get_installed,
        commands::deploy::reorder_paks,
        commands::deploy::set_reframework_switch,
        commands::deploy::set_game_root,
        commands::library::get_library,
        commands::library::open_mods_folder,
        commands::play::plan_launch,
        commands::play::launch_game,
        commands::nexus::get_updates,
        commands::nexus::check_updates,
        commands::nexus::identify_packages,
        commands::nexus::find_on_nexus,
        commands::nexus::link_package,
        commands::nexus::unlink_package,
        commands::nexus::set_nexus_key,
        commands::nexus::refresh_nexus_account,
        commands::browse::get_mod_files,
        commands::browse::browse_nexus,
        commands::standings::get_nexus_standings,
        commands::standings::set_tracked,
        commands::standings::set_endorsement,
        commands::downloads::get_downloads,
        commands::downloads::start_download,
        commands::downloads::cancel_download,
        commands::downloads::clear_downloads,
        commands::nxm::get_nxm_handler,
        commands::nxm::set_nxm_handler,
        commands::overview::get_troubled_packages,
        commands::nexus::get_nexus_quota,
        commands::nexus::get_labels,
        commands::deploy::restore_parked,
        commands::deploy::discard_parked,
        commands::overview::get_settings,
        commands::overview::set_settings,
        commands::muster::get_muster_slots,
        commands::muster::get_muster,
        commands::muster::open_saves_folder,
        commands::save_backup::backup_saves,
        commands::versions::plan_keep_newest,
        commands::versions::keep_newest,
        commands::versions::put_away,
        commands::versions::retry_put_away,
        commands::versions::get_older_versions,
        commands::versions::bring_back,
        commands::versions::open_older_versions_folder,
        commands::versions::join_versions,
        commands::versions::part_versions,
        commands::versions::decline_version_question,
        commands::fray::fray_frame,
        commands::fray::open_fray,
            $($extra)*
        ]
    };
}

/// Every command of docs/WINDOW-CONTRACT.md under its invoke name. The app
/// and the IPC tests (Tauri's mock runtime) register exactly this list; a
/// build with the `pawn` feature also registers § v0.7's (RS-10).
pub fn with_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    #[cfg(not(feature = "pawn"))]
    return builder.invoke_handler(riftstone_handler!());
    #[cfg(feature = "pawn")]
    builder.invoke_handler(riftstone_handler!(
        pawn::commands::get_workshop,
        pawn::commands::summon_pawn,
        pawn::commands::pawn_write,
        pawn::commands::pawn_resize,
        pawn::commands::depart_pawn,
        pawn::commands::answer_pawn_play,
        pawn::commands::get_fence,
        pawn::commands::run_fence_probe,
        pawn::commands::plan_bring_through,
        pawn::commands::bring_through,
        pawn::commands::plan_put_back,
        pawn::commands::put_back,
        pawn::commands::open_workshop_folder,
        pawn::commands::set_pawn_session,
        pawn::commands::set_pawn_inputs,
    ))
}
