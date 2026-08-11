use tauri::State;
use tracing::info;

use crate::dat;
use crate::db::dats as repo;
use crate::error::AppResult;
use crate::matcher;
use crate::model::{CategoryStats, DatSource};
use crate::state::AppState;

#[tauri::command]
pub async fn list_dat_sources(state: State<'_, AppState>) -> AppResult<Vec<DatSource>> {
    repo::list(&state.pool).await
}

#[tauri::command]
pub async fn import_dat(
    state: State<'_, AppState>,
    path: String,
    emulator_profile_id: String,
    display_name: Option<String>,
) -> AppResult<DatSource> {
    let source = dat::import_dat(
        &state.pool,
        &path,
        &emulator_profile_id,
        display_name,
    )
    .await?;
    info!(dat_id = source.id, "DAT imported and library rematched");
    Ok(source)
}

#[tauri::command]
pub async fn deactivate_dat(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    repo::deactivate(&state.pool, id).await?;
    matcher::rematch_library(&state.pool).await?;
    Ok(())
}

#[tauri::command]
pub async fn rematch_library(state: State<'_, AppState>) -> AppResult<u64> {
    matcher::rematch_library(&state.pool).await
}

#[tauri::command]
pub async fn import_catver(state: State<'_, AppState>, path: String) -> AppResult<CategoryStats> {
    let stats = dat::import_catver(&state.pool, &path).await?;
    info!(count = stats.count, "CatVer imported");
    Ok(stats)
}

#[tauri::command]
pub async fn get_category_stats(state: State<'_, AppState>) -> AppResult<CategoryStats> {
    dat::category_stats(&state.pool).await
}
