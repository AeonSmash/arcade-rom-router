use tauri::State;

use crate::controller;
use crate::db::settings;
use crate::error::AppResult;
use crate::model::{ControllerDevice, ControllerSettings};
use crate::state::AppState;

#[tauri::command]
pub async fn list_controllers(state: State<'_, AppState>) -> AppResult<Vec<ControllerDevice>> {
    controller::list_devices(&state.pool).await
}

#[tauri::command]
pub async fn get_controller_settings(
    state: State<'_, AppState>,
) -> AppResult<ControllerSettings> {
    controller::get_settings(&state.pool).await
}

#[tauri::command]
pub async fn report_controller(
    state: State<'_, AppState>,
    device_id: String,
    display_name: String,
    vendor_id: Option<i64>,
    product_id: Option<i64>,
) -> AppResult<ControllerDevice> {
    controller::upsert_device(
        &state.pool,
        &device_id,
        &display_name,
        vendor_id,
        product_id,
    )
    .await
}

#[tauri::command]
pub async fn set_controller_binding(
    state: State<'_, AppState>,
    controller_id: Option<i64>,
    action: String,
    button_index: Option<i64>,
    button_label: Option<String>,
) -> AppResult<()> {
    controller::set_binding(
        &state.pool,
        controller_id,
        &action,
        button_index,
        button_label,
    )
    .await
}

#[tauri::command]
pub async fn set_controller_navigation_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    settings::set(&state.pool, "controller.navigationEnabled", &enabled).await
}
