//! The Riftstone — the window spine.
//!
//! The Vue window (`src/`) asks; this crate answers by wiring the six crates
//! under `crates/` together: it holds the library, the journal and the Nexus
//! links ([`state`]; no lock is held across a disk, archive or network read),
//! reads the game folder once per change and shares that read ([`fresh`]),
//! remembers its state between launches ([`persist`]), answers every command
//! of `docs/WINDOW-CONTRACT.md` ([`commands`], all off the async runtime) in
//! the shapes of `src/types/rift.ts` ([`dto`]), and tells the window when
//! something changed ([`events`]): a scan, an install, an update check, a
//! folder that changed under the library watcher ([`watch`]).
//!
//! Environment overrides, read once at startup (the WSL bench and the tests):
//! `RIFTSTONE_DATA_HOME` (replaces the app data + cache dirs, and takes the log
//! and the WebView2 profile with it, so a bench run leaves nothing in the real
//! app folders), and through rift-deploy's discovery `RIFTSTONE_GAME_ROOT`,
//! `RIFTSTONE_STEAM_ROOT`, `RIFTSTONE_FLUFFY_ROOT`.

pub mod commands;
pub mod conflicts;
pub mod dto;
pub mod events;
pub mod fresh;
pub mod keys;
pub mod library;
pub mod nexus;
pub mod persist;
pub mod state;
pub mod thumbs;
pub mod watch;

use std::sync::Arc;

use tauri::Manager;

use crate::persist::DataDirs;
use crate::state::{AppState, SpineConfig};

pub fn run() {
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
            // Startup steps 1–3 here (fast); the window shows at once with the
            // cached library, and steps 3–6 run in the background (`wake`).
            let spine = AppState::open(
                SpineConfig::system(dirs),
                Arc::new(events::TauriEvents(app.handle().clone())),
                Box::new(keys::SystemKeyStore),
            );
            app.manage(Arc::clone(&spine));
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

/// Every command of docs/WINDOW-CONTRACT.md under its invoke name. The app
/// and the IPC tests (Tauri's mock runtime) register exactly this list.
pub fn with_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
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
        commands::deploy::adopt_fluffy,
        commands::deploy::set_reframework_switch,
        commands::library::add_root,
        commands::library::remove_root,
        commands::deploy::set_game_root,
        commands::nexus::get_updates,
        commands::nexus::check_updates,
        commands::nexus::identify_packages,
        commands::nexus::link_package,
        commands::nexus::unlink_package,
        commands::nexus::set_nexus_key,
        commands::overview::get_troubled_packages,
        commands::nexus::get_nexus_quota,
        commands::overview::get_settings,
        commands::overview::set_settings,
    ])
}
