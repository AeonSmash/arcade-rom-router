use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::logging::DiagnosticEntry;
use crate::scanner;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub phase: String,
    pub app_data_dir: String,
    pub log_dir: String,
    pub default_worker_count: usize,
}

/// Round-trips basic runtime facts; also the Phase 0 smoke test that the
/// frontend and backend are talking to each other.
#[tauri::command]
pub async fn get_app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        name: "Arcade ROM Router".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        phase: "Phase 7 — Emulator Routing".into(),
        app_data_dir: state.app_data_dir.display().to_string(),
        log_dir: state.log_dir.display().to_string(),
        default_worker_count: scanner::default_worker_count(),
    })
}

#[tauri::command]
pub async fn get_diagnostics(state: State<'_, AppState>) -> AppResult<Vec<DiagnosticEntry>> {
    Ok(state.diagnostics.snapshot())
}

#[tauri::command]
pub async fn clear_diagnostics(state: State<'_, AppState>) -> AppResult<()> {
    state.diagnostics.clear();
    Ok(())
}
