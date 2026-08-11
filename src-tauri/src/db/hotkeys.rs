use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::{AppError, AppResult};
use crate::model::HotkeyProfile;

pub async fn get_default(pool: &SqlitePool) -> AppResult<HotkeyProfile> {
    let row = sqlx::query("SELECT * FROM hotkey_profiles ORDER BY id LIMIT 1")
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            AppError::config(
                "Hotkey profile missing",
                "The default hotkey profile was not seeded.",
            )
        })?;
    Ok(map_row(&row))
}

pub async fn update_binding(
    pool: &SqlitePool,
    exit_btn: Option<i64>,
    exit_btn_label: Option<&str>,
    enable_btn: Option<i64>,
    enable_btn_label: Option<&str>,
) -> AppResult<HotkeyProfile> {
    let now = now_iso8601();
    sqlx::query(
        "UPDATE hotkey_profiles SET
           exit_btn = ?1,
           exit_btn_label = ?2,
           enable_btn = ?3,
           enable_btn_label = ?4,
           verified = 0,
           updated_at = ?5
         WHERE id = (SELECT id FROM hotkey_profiles ORDER BY id LIMIT 1)",
    )
    .bind(exit_btn)
    .bind(exit_btn_label)
    .bind(enable_btn)
    .bind(enable_btn_label)
    .bind(&now)
    .execute(pool)
    .await?;
    get_default(pool).await
}

pub async fn set_enabled(pool: &SqlitePool, enabled: bool) -> AppResult<HotkeyProfile> {
    let now = now_iso8601();
    sqlx::query(
        "UPDATE hotkey_profiles SET enabled = ?1, updated_at = ?2
         WHERE id = (SELECT id FROM hotkey_profiles ORDER BY id LIMIT 1)",
    )
    .bind(enabled)
    .bind(&now)
    .execute(pool)
    .await?;
    get_default(pool).await
}

pub async fn set_fragment_path(
    pool: &SqlitePool,
    path: &str,
    verified: bool,
) -> AppResult<HotkeyProfile> {
    let now = now_iso8601();
    sqlx::query(
        "UPDATE hotkey_profiles SET fragment_path = ?1, verified = ?2, updated_at = ?3
         WHERE id = (SELECT id FROM hotkey_profiles ORDER BY id LIMIT 1)",
    )
    .bind(path)
    .bind(verified)
    .bind(&now)
    .execute(pool)
    .await?;
    get_default(pool).await
}

pub async fn set_verified(pool: &SqlitePool, verified: bool) -> AppResult<HotkeyProfile> {
    let now = now_iso8601();
    sqlx::query(
        "UPDATE hotkey_profiles SET verified = ?1, updated_at = ?2
         WHERE id = (SELECT id FROM hotkey_profiles ORDER BY id LIMIT 1)",
    )
    .bind(verified)
    .bind(&now)
    .execute(pool)
    .await?;
    get_default(pool).await
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> HotkeyProfile {
    HotkeyProfile {
        id: row.get("id"),
        name: row.get("name"),
        enabled: row.get::<i64, _>("enabled") != 0,
        exit_btn: row.get("exit_btn"),
        exit_btn_label: row.get("exit_btn_label"),
        enable_btn: row.get("enable_btn"),
        enable_btn_label: row.get("enable_btn_label"),
        fragment_path: row.get("fragment_path"),
        verified: row.get::<i64, _>("verified") != 0,
        updated_at: row.get("updated_at"),
    }
}
