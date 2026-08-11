use tauri::State;

use crate::db::archives::{self, ArchiveQuery};
use crate::error::AppResult;
use crate::model::{ArchiveMemberRow, ArchivePage, ArchiveState, LibrarySummary};
use crate::state::AppState;

#[tauri::command]
pub async fn get_archives_page(
    state: State<'_, AppState>,
    rom_root_id: Option<i64>,
    archive_state: Option<ArchiveState>,
    search: Option<String>,
    favorites_only: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<ArchivePage> {
    let query = ArchiveQuery {
        rom_root_id,
        state: archive_state,
        search,
        favorites_only: favorites_only.unwrap_or(false),
        limit: limit.unwrap_or(200),
        offset: offset.unwrap_or(0),
    };

    archives::page(&state.pool, &query).await
}

#[tauri::command]
pub async fn get_archive_members(
    state: State<'_, AppState>,
    archive_id: i64,
) -> AppResult<Vec<ArchiveMemberRow>> {
    archives::members(&state.pool, archive_id).await
}

#[tauri::command]
pub async fn get_library_summary(state: State<'_, AppState>) -> AppResult<LibrarySummary> {
    archives::summary(&state.pool).await
}
