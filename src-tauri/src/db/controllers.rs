use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::ControllerDevice;
use crate::model::ControllerBinding;

pub async fn list_devices(pool: &SqlitePool) -> AppResult<Vec<ControllerDevice>> {
    let rows = sqlx::query("SELECT * FROM controllers ORDER BY port, display_name")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(map_device).collect())
}

pub async fn upsert_device(
    pool: &SqlitePool,
    device_id: &str,
    display_name: &str,
    vendor_id: Option<i64>,
    product_id: Option<i64>,
    preset: &str,
) -> AppResult<ControllerDevice> {
    let now = now_iso8601();
    sqlx::query(
        "INSERT INTO controllers (device_id, display_name, vendor_id, product_id, preset, port, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)
         ON CONFLICT(device_id) DO UPDATE SET
           display_name = excluded.display_name,
           vendor_id = excluded.vendor_id,
           product_id = excluded.product_id,
           preset = excluded.preset,
           last_seen_at = excluded.last_seen_at",
    )
    .bind(device_id)
    .bind(display_name)
    .bind(vendor_id)
    .bind(product_id)
    .bind(preset)
    .bind(&now)
    .execute(pool)
    .await?;

    let row = sqlx::query("SELECT * FROM controllers WHERE device_id = ?1")
        .bind(device_id)
        .fetch_one(pool)
        .await?;
    Ok(map_device(&row))
}

pub async fn bindings_for(
    pool: &SqlitePool,
    controller_id: Option<i64>,
) -> AppResult<Vec<ControllerBinding>> {
    let rows = if let Some(id) = controller_id {
        sqlx::query("SELECT * FROM controller_bindings WHERE controller_id = ?1 ORDER BY action")
            .bind(id)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query(
            "SELECT * FROM controller_bindings WHERE controller_id IS NULL ORDER BY action",
        )
        .fetch_all(pool)
        .await?
    };
    Ok(rows.iter().map(map_binding).collect())
}

pub async fn set_binding(
    pool: &SqlitePool,
    controller_id: Option<i64>,
    scope: &str,
    action: &str,
    button_index: Option<i64>,
    button_label: Option<&str>,
    axis_index: Option<i64>,
    axis_direction: Option<&str>,
) -> AppResult<()> {
    // SQLite UNIQUE with NULLs: delete then insert for NULL controller_id.
    if let Some(id) = controller_id {
        sqlx::query(
            "INSERT INTO controller_bindings
               (controller_id, scope, action, button_index, button_label, axis_index, axis_direction)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(controller_id, scope, action) DO UPDATE SET
               button_index = excluded.button_index,
               button_label = excluded.button_label,
               axis_index = excluded.axis_index,
               axis_direction = excluded.axis_direction",
        )
        .bind(id)
        .bind(scope)
        .bind(action)
        .bind(button_index)
        .bind(button_label)
        .bind(axis_index)
        .bind(axis_direction)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "DELETE FROM controller_bindings
             WHERE controller_id IS NULL AND scope = ?1 AND action = ?2",
        )
        .bind(scope)
        .bind(action)
        .execute(pool)
        .await?;
        sqlx::query(
            "INSERT INTO controller_bindings
               (controller_id, scope, action, button_index, button_label, axis_index, axis_direction)
             VALUES (NULL, ?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(scope)
        .bind(action)
        .bind(button_index)
        .bind(button_label)
        .bind(axis_index)
        .bind(axis_direction)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn map_device(row: &sqlx::sqlite::SqliteRow) -> ControllerDevice {
    ControllerDevice {
        id: row.get("id"),
        device_id: row.get("device_id"),
        display_name: row.get("display_name"),
        vendor_id: row.get("vendor_id"),
        product_id: row.get("product_id"),
        preset: row.get("preset"),
        port: row.get("port"),
        last_seen_at: row.get("last_seen_at"),
        notes: row.get("notes"),
    }
}

fn map_binding(row: &sqlx::sqlite::SqliteRow) -> ControllerBinding {
    ControllerBinding {
        id: row.get("id"),
        controller_id: row.get("controller_id"),
        scope: row.get("scope"),
        action: row.get("action"),
        button_index: row.get("button_index"),
        button_label: row.get("button_label"),
        axis_index: row.get("axis_index"),
        axis_direction: row.get("axis_direction"),
    }
}
