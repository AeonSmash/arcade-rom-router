//! Controller Center (Phase 8): settings and bindings for Router UI navigation.
//!
//! Live detection uses the WebView Gamepad API (SPEC.md §21.3). The backend
//! persists devices and bindings; it does not open HID devices.

use sqlx::SqlitePool;

use crate::db::{self, controllers as controllers_db};
use crate::error::AppResult;
use crate::model::{ControllerBinding, ControllerDevice, ControllerSettings, UiNavAction};

/// Default UI navigation bindings for an Xbox / XInput-style pad (SPEC §21.4).
pub fn xbox_default_bindings() -> Vec<(UiNavAction, i64, &'static str)> {
    vec![
        (UiNavAction::NavigateUp, 12, "D-pad Up"),
        (UiNavAction::NavigateDown, 13, "D-pad Down"),
        (UiNavAction::NavigateLeft, 14, "D-pad Left"),
        (UiNavAction::NavigateRight, 15, "D-pad Right"),
        (UiNavAction::Select, 0, "A"),
        (UiNavAction::Back, 1, "B"),
        (UiNavAction::Favorite, 2, "X"),
        (UiNavAction::Details, 3, "Y"),
        (UiNavAction::PrevFilter, 4, "LB"),
        (UiNavAction::NextFilter, 5, "RB"),
        (UiNavAction::ContextMenu, 9, "Start"),
        (UiNavAction::Search, 8, "Select"),
    ]
}

pub fn classify_preset(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("xbox") || lower.contains("xinput") || lower.contains("045e") {
        "XBOX"
    } else {
        "GENERIC"
    }
}

pub async fn list_devices(pool: &SqlitePool) -> AppResult<Vec<ControllerDevice>> {
    controllers_db::list_devices(pool).await
}

pub async fn upsert_device(
    pool: &SqlitePool,
    device_id: &str,
    display_name: &str,
    vendor_id: Option<i64>,
    product_id: Option<i64>,
) -> AppResult<ControllerDevice> {
    let preset = classify_preset(display_name);
    let device =
        controllers_db::upsert_device(pool, device_id, display_name, vendor_id, product_id, preset)
            .await?;

    // Seed Xbox defaults once for a new device with no bindings.
    let existing = controllers_db::bindings_for(pool, Some(device.id)).await?;
    if existing.is_empty() && preset == "XBOX" {
        for (action, button, label) in xbox_default_bindings() {
            controllers_db::set_binding(
                pool,
                Some(device.id),
                "UI",
                action.as_str(),
                Some(button),
                Some(label),
                None,
                None,
            )
            .await?;
        }
    }
    Ok(device)
}

pub async fn get_settings(pool: &SqlitePool) -> AppResult<ControllerSettings> {
    let devices = controllers_db::list_devices(pool).await?;
    let mut bindings = controllers_db::bindings_for(pool, None).await?;
    // Include per-device bindings too.
    for device in &devices {
        let mut device_bindings = controllers_db::bindings_for(pool, Some(device.id)).await?;
        bindings.append(&mut device_bindings);
    }

    let navigation_enabled: bool =
        db::settings::get_or(pool, "controller.navigationEnabled", true).await;

    Ok(ControllerSettings {
        navigation_enabled,
        devices,
        bindings,
        xbox_defaults: xbox_default_bindings()
            .into_iter()
            .map(|(action, button, label)| ControllerBinding {
                id: 0,
                controller_id: None,
                scope: "UI".into(),
                action: action.as_str().into(),
                button_index: Some(button),
                button_label: Some(label.into()),
                axis_index: None,
                axis_direction: None,
            })
            .collect(),
    })
}

pub async fn set_binding(
    pool: &SqlitePool,
    controller_id: Option<i64>,
    action: &str,
    button_index: Option<i64>,
    button_label: Option<String>,
) -> AppResult<()> {
    controllers_db::set_binding(
        pool,
        controller_id,
        "UI",
        action,
        button_index,
        button_label.as_deref(),
        None,
        None,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xbox_name_gets_xbox_preset() {
        assert_eq!(classify_preset("Xbox Wireless Controller"), "XBOX");
        assert_eq!(classify_preset("XInput Pad"), "XBOX");
        assert_eq!(classify_preset("8BitDo Arcade Stick"), "GENERIC");
    }

    #[test]
    fn xbox_defaults_cover_spec_actions() {
        let actions: Vec<_> = xbox_default_bindings()
            .into_iter()
            .map(|(a, _, _)| a)
            .collect();
        assert!(actions.contains(&UiNavAction::Select));
        assert!(actions.contains(&UiNavAction::Back));
        assert!(actions.contains(&UiNavAction::Search));
    }
}
