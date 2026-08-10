use tauri::State;

use crate::db::profiles;
use crate::emulator;
use crate::error::AppResult;
use crate::model::{EmulatorProfile, HealthState, RetroArchDiscovery};
use crate::state::AppState;

#[tauri::command]
pub async fn list_emulator_profiles(state: State<'_, AppState>) -> AppResult<Vec<EmulatorProfile>> {
    profiles::list(&state.pool).await
}

#[tauri::command]
pub async fn detect_retroarch(
    state: State<'_, AppState>,
    executable_path: Option<String>,
) -> AppResult<RetroArchDiscovery> {
    let discovery = emulator::discover_retroarch(&state.pool, executable_path).await?;
    let _ = emulator::validate_all_profiles(&state.pool).await?;
    Ok(discovery)
}

#[tauri::command]
pub async fn validate_emulator_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> AppResult<HealthState> {
    emulator::validate_profile(&state.pool, &profile_id).await
}

#[tauri::command]
pub async fn set_emulator_profile_enabled(
    state: State<'_, AppState>,
    profile_id: String,
    enabled: bool,
) -> AppResult<()> {
    profiles::set_enabled(&state.pool, &profile_id, enabled).await
}

#[tauri::command]
pub async fn set_emulator_profile_priority(
    state: State<'_, AppState>,
    profile_id: String,
    priority: i64,
) -> AppResult<()> {
    profiles::set_priority(&state.pool, &profile_id, priority).await
}
