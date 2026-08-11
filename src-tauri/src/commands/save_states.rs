use tauri::State;

use crate::emulator::savestates;
use crate::error::AppResult;
use crate::launch;
use crate::model::{LaunchResult, SaveStateRow};
use crate::state::AppState;

#[tauri::command]
pub async fn list_save_states(
    state: State<'_, AppState>,
    archive_id: i64,
) -> AppResult<Vec<SaveStateRow>> {
    savestates::list_for_archive(&state.pool, archive_id).await
}

#[tauri::command]
pub async fn label_save_state(
    state: State<'_, AppState>,
    id: i64,
    label: Option<String>,
) -> AppResult<()> {
    savestates::label(&state.pool, id, label).await
}

#[tauri::command]
pub async fn delete_save_state(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    savestates::delete_state(&state.pool, id).await
}

#[tauri::command]
pub async fn launch_game_with_state(
    state: State<'_, AppState>,
    archive_id: i64,
    save_state_id: i64,
    route_id: Option<i64>,
) -> AppResult<LaunchResult> {
    let log_dir = state.log_dir.join("launches");
    launch::launch_game(
        &state.pool,
        &log_dir,
        &state.app_data_dir,
        archive_id,
        route_id,
        Some(save_state_id),
    )
    .await
}
