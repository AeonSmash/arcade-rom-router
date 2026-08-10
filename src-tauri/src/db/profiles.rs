use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::{EmulatorProfile, HealthState};

fn map_row(row: &sqlx::sqlite::SqliteRow) -> EmulatorProfile {
    let health: String = row.get("health_state");
    EmulatorProfile {
        id: row.get("id"),
        display_name: row.get("display_name"),
        runner_type: row.get("runner_type"),
        executable_path: row.get("executable_path"),
        core_path: row.get("core_path"),
        core_signature: row.get("core_signature"),
        enabled: row.get::<i64, _>("enabled") != 0,
        priority: row.get("priority"),
        settings_json: row.get("settings_json"),
        last_health_check: row.get("last_health_check"),
        health_state: HealthState::parse(&health).unwrap_or(HealthState::Unknown),
        games_matched: 0,
        has_active_dat: false,
    }
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<EmulatorProfile>> {
    let rows = sqlx::query("SELECT * FROM emulator_profiles ORDER BY priority DESC, id")
        .fetch_all(pool)
        .await?;

    let mut profiles: Vec<_> = rows.iter().map(map_row).collect();
    for profile in &mut profiles {
        let has_dat: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM dat_sources WHERE emulator_profile_id = ?1 AND active = 1",
        )
        .bind(&profile.id)
        .fetch_one(pool)
        .await?;
        profile.has_active_dat = has_dat > 0;

        let matched: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT archive_id) FROM match_results WHERE emulator_profile_id = ?1",
        )
        .bind(&profile.id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        profile.games_matched = matched;
    }

    Ok(profiles)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Option<EmulatorProfile>> {
    let row = sqlx::query("SELECT * FROM emulator_profiles WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn update_paths(
    pool: &SqlitePool,
    id: &str,
    executable_path: Option<&str>,
    core_path: Option<&str>,
    core_signature: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE emulator_profiles SET
             executable_path = COALESCE(?2, executable_path),
             core_path = COALESCE(?3, core_path),
             core_signature = COALESCE(?4, core_signature)
         WHERE id = ?1",
    )
    .bind(id)
    .bind(executable_path)
    .bind(core_path)
    .bind(core_signature)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<()> {
    sqlx::query("UPDATE emulator_profiles SET enabled = ?2 WHERE id = ?1")
        .bind(id)
        .bind(i64::from(enabled))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_health(pool: &SqlitePool, id: &str, state: HealthState) -> AppResult<()> {
    sqlx::query(
        "UPDATE emulator_profiles SET health_state = ?2, last_health_check = ?3 WHERE id = ?1",
    )
    .bind(id)
    .bind(state.as_str())
    .bind(now_iso8601())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_priority(pool: &SqlitePool, id: &str, priority: i64) -> AppResult<()> {
    sqlx::query("UPDATE emulator_profiles SET priority = ?2 WHERE id = ?1")
        .bind(id)
        .bind(priority)
        .execute(pool)
        .await?;
    Ok(())
}
