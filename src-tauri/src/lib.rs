//! Arcade ROM Router backend.
//!
//! Phase 0/1 scope: repository foundation plus a trustworthy, read-only ROM
//! inventory engine. DAT import, compatibility matching, emulator routing, and
//! launching are later phases and are deliberately absent.

pub mod archive;
pub mod commands;
pub mod db;
pub mod error;
pub mod logging;
pub mod model;
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
                "Arcade ROM Router starting"
            );

            let database_path = app_data_dir.join(DATABASE_FILE);
            let pool = tauri::async_runtime::block_on(db::connect(&database_path))
                .map_err(|error| {
                    error!(%error, path = %database_path.display(), "database unavailable");
                    std::io::Error::other(error.to_string())
                })?;

            // A job left running by a previous crash has no process behind it.
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

            // Held for the process lifetime so buffered log lines reach disk.
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
            commands::settings::get_settings,
            commands::settings::set_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Arcade ROM Router");
}
