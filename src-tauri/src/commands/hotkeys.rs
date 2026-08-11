use tauri::State;

use crate::emulator::hotkeys;
use crate::error::AppResult;
use crate::model::{HotkeyFragmentPreview, HotkeyProfile};
use crate::state::AppState;

#[tauri::command]
pub async fn get_hotkey_profile(state: State<'_, AppState>) -> AppResult<HotkeyProfile> {
    hotkeys::get_profile(&state.pool).await
}

#[tauri::command]
pub async fn set_hotkey_binding(
    state: State<'_, AppState>,
    exit_btn: Option<i64>,
    exit_btn_label: Option<String>,
    enable_btn: Option<i64>,
    enable_btn_label: Option<String>,
) -> AppResult<HotkeyProfile> {
    hotkeys::set_binding(
        &state.pool,
        exit_btn,
        exit_btn_label,
        enable_btn,
        enable_btn_label,
    )
    .await
}

#[tauri::command]
pub async fn preview_hotkey_fragment(
    state: State<'_, AppState>,
) -> AppResult<HotkeyFragmentPreview> {
    hotkeys::preview(&state.pool, &state.app_data_dir).await
}

#[tauri::command]
pub async fn apply_hotkey_fragment(state: State<'_, AppState>) -> AppResult<HotkeyProfile> {
    hotkeys::apply(&state.pool, &state.app_data_dir).await
}

#[tauri::command]
pub async fn set_hotkey_profile_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<HotkeyProfile> {
    hotkeys::set_enabled(&state.pool, enabled).await
}

#[tauri::command]
pub async fn mark_hotkey_verified(
    state: State<'_, AppState>,
    verified: bool,
) -> AppResult<HotkeyProfile> {
    hotkeys::mark_verified(&state.pool, verified).await
}
