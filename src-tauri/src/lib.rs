//! Aeonic Arcadia backend.
//!
//! Phases 0–12: inventory through launch, controllers, hotkeys, media, and
//! save states.

pub mod archive;
pub mod commands;
pub mod controller;
pub mod dat;
pub mod db;
pub mod emulator;
pub mod error;
pub mod launch;
pub mod logging;
pub mod matcher;
pub mod media;
pub mod model;
pub mod routing;
pub mod scanner;
pub mod state;

use std::sync::Arc;

use tauri::Manager;
use tracing::{error, info, warn};

pub use error::{AppError, AppResult};

const DATABASE_FILE: &str = "library.db";
const LOG_DIRECTORY: &str = "logs";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let log_dir = app_data_dir.join(LOG_DIRECTORY);
            let logging = logging::init(&log_dir);

            info!(
                version = env!("CARGO_PKG_VERSION"),
                data = %app_data_dir.display(),
                "Aeonic Arcadia starting"
            );

            let database_path = app_data_dir.join(DATABASE_FILE);
            let pool = tauri::async_runtime::block_on(db::connect(&database_path))
                .map_err(|error| {
                    error!(%error, path = %database_path.display(), "database unavailable");
                    std::io::Error::other(error.to_string())
                })?;

            match tauri::async_runtime::block_on(db::scan_jobs::reconcile_orphans(&pool)) {
                Ok(0) => {}
                Ok(count) => warn!(count, "marked interrupted scan jobs as failed"),
                Err(error) => warn!(%error, "could not reconcile interrupted scan jobs"),
            }

            app.manage(state::AppState {
                pool,
                jobs: Arc::new(state::JobRegistry::new()),
                diagnostics: logging.buffer.clone(),
                app_data_dir,
                log_dir,
            });

            app.manage(logging);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_app_info,
            commands::system::get_diagnostics,
            commands::system::clear_diagnostics,
            commands::rom_roots::list_rom_roots,
            commands::rom_roots::add_rom_root,
            commands::rom_roots::set_rom_root_enabled,
            commands::rom_roots::remove_rom_root,
            commands::scan::start_scan,
            commands::scan::cancel_scan,
            commands::scan::pause_scan,
            commands::scan::resume_scan,
            commands::scan::get_scan_status,
            commands::library::get_archives_page,
            commands::library::get_archive_members,
            commands::library::get_library_summary,
            commands::favorites::toggle_favorite,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::dats::list_dat_sources,
            commands::dats::import_dat,
            commands::dats::deactivate_dat,
            commands::dats::rematch_library,
            commands::dats::import_catver,
            commands::dats::get_category_stats,
            commands::dats::list_genres,
            commands::emulators::list_emulator_profiles,
            commands::emulators::detect_retroarch,
            commands::emulators::validate_emulator_profile,
            commands::emulators::set_emulator_profile_enabled,
            commands::emulators::set_emulator_profile_priority,
            commands::games::get_game_detail,
            commands::games::get_problem_summary,
            commands::games::list_problem_games,
            commands::games::choose_route,
            commands::games::rebuild_library_routes,
            commands::games::set_game_route_override,
            commands::games::set_route_preference_mode,
            commands::games::launch_game,
            commands::controllers::list_controllers,
            commands::controllers::get_controller_settings,
            commands::controllers::report_controller,
            commands::controllers::set_controller_binding,
            commands::controllers::set_controller_navigation_enabled,
            commands::hotkeys::get_hotkey_profile,
            commands::hotkeys::set_hotkey_binding,
            commands::hotkeys::preview_hotkey_fragment,
            commands::hotkeys::apply_hotkey_fragment,
            commands::hotkeys::set_hotkey_profile_enabled,
            commands::hotkeys::mark_hotkey_verified,
            commands::media::get_game_media,
            commands::media::set_media_folder,
            commands::media::get_media_folder,
            commands::media::scan_local_media,
            commands::media::clear_media_cache,
            commands::media::get_emumovies_status,
            commands::media::set_emumovies_enabled,
            commands::media::set_emumovies_credentials,
            commands::media::clear_emumovies_credentials,
            commands::media::fetch_emumovies_media,
            commands::media::sync_emumovies_media,
            commands::save_states::list_save_states,
            commands::save_states::label_save_state,
            commands::save_states::delete_save_state,
            commands::save_states::launch_game_with_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Aeonic Arcadia");
}
