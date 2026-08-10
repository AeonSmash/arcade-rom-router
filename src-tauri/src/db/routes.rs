use sqlx::{Row, SqlitePool};

use crate::error::AppResult;
use crate::model::{CompatibilityState, RouteRow};

pub struct NewRoute {
    pub archive_id: i64,
    pub machine_id: i64,
    pub emulator_profile_id: String,
    pub match_result_id: i64,
    pub is_selected: bool,
    pub selection_reason: Option<String>,
    pub user_override: bool,
    pub launchable: bool,
}

pub async fn clear_for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM routes WHERE archive_id = ?1 AND user_override = 0")
        .bind(archive_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert(pool: &SqlitePool, route: &NewRoute) -> AppResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO routes (
             archive_id, machine_id, emulator_profile_id, match_result_id,
             is_selected, selection_reason, user_override, launchable
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         RETURNING id",
    )
    .bind(route.archive_id)
    .bind(route.machine_id)
    .bind(&route.emulator_profile_id)
    .bind(route.match_result_id)
    .bind(i64::from(route.is_selected))
    .bind(&route.selection_reason)
    .bind(i64::from(route.user_override))
    .bind(i64::from(route.launchable))
    .fetch_one(pool)
    .await?;
    Ok(id)
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> RouteRow {
    let state = row
        .try_get::<String, _>("state")
        .ok()
        .and_then(|s| CompatibilityState::parse(&s));

    RouteRow {
        id: row.get("id"),
        archive_id: row.get("archive_id"),
        machine_id: row.get("machine_id"),
        emulator_profile_id: row.get("emulator_profile_id"),
        match_result_id: row.get("match_result_id"),
        is_selected: row.get::<i64, _>("is_selected") != 0,
        selection_reason: row.get("selection_reason"),
        user_override: row.get::<i64, _>("user_override") != 0,
        launchable: row.get::<i64, _>("launchable") != 0,
        profile_display_name: row.try_get("profile_display_name").ok(),
        machine_set_name: row.try_get("machine_set_name").ok(),
        state,
    }
}

pub async fn for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<Vec<RouteRow>> {
    let rows = sqlx::query(
        "SELECT r.*, p.display_name AS profile_display_name, m.set_name AS machine_set_name,
                mr.state AS state
         FROM routes r
         LEFT JOIN emulator_profiles p ON p.id = r.emulator_profile_id
         LEFT JOIN machines m ON m.id = r.machine_id
         LEFT JOIN match_results mr ON mr.id = r.match_result_id
         WHERE r.archive_id = ?1
         ORDER BY r.is_selected DESC, r.launchable DESC, r.id",
    )
    .bind(archive_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(map_row).collect())
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Option<RouteRow>> {
    let row = sqlx::query(
        "SELECT r.*, p.display_name AS profile_display_name, m.set_name AS machine_set_name,
                mr.state AS state
         FROM routes r
         LEFT JOIN emulator_profiles p ON p.id = r.emulator_profile_id
         LEFT JOIN machines m ON m.id = r.machine_id
         LEFT JOIN match_results mr ON mr.id = r.match_result_id
         WHERE r.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn selected_for_archive(
    pool: &SqlitePool,
    archive_id: i64,
) -> AppResult<Option<RouteRow>> {
    let row = sqlx::query(
        "SELECT r.*, p.display_name AS profile_display_name, m.set_name AS machine_set_name,
                mr.state AS state
         FROM routes r
         LEFT JOIN emulator_profiles p ON p.id = r.emulator_profile_id
         LEFT JOIN machines m ON m.id = r.machine_id
         LEFT JOIN match_results mr ON mr.id = r.match_result_id
         WHERE r.archive_id = ?1 AND r.is_selected = 1
         LIMIT 1",
    )
    .bind(archive_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn set_override(
    pool: &SqlitePool,
    archive_id: i64,
    route_id: Option<i64>,
) -> AppResult<()> {
    sqlx::query("UPDATE routes SET is_selected = 0, user_override = 0 WHERE archive_id = ?1")
        .bind(archive_id)
        .execute(pool)
        .await?;

    if let Some(id) = route_id {
        sqlx::query(
            "UPDATE routes SET is_selected = 1, user_override = 1,
                 selection_reason = 'User override'
             WHERE id = ?1 AND archive_id = ?2",
        )
        .bind(id)
        .bind(archive_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}
