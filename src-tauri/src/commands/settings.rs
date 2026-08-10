use serde_json::Value;
use tauri::State;

use crate::db::settings as repo;
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(Value::Object(repo::all(&state.pool).await?))
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: Value) -> AppResult<()> {
    repo::set(&state.pool, &key, &value).await
}
