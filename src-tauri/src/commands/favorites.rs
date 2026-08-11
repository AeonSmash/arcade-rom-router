use tauri::State;

use crate::db::favorites;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn toggle_favorite(state: State<'_, AppState>, archive_id: i64) -> AppResult<bool> {
    favorites::toggle(&state.pool, archive_id).await
}
