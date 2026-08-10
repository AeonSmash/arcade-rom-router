use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::DatSource;

pub struct NewDatSource {
    pub emulator_profile_id: String,
    pub display_name: String,
    pub source_type: String,
    pub version: Option<String>,
    pub path: String,
    pub sha256: String,
    pub machine_count: i64,
    pub rom_entry_count: i64,
    pub disk_entry_count: i64,
    pub parser_version: i64,
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> DatSource {
    DatSource {
        id: row.get("id"),
        emulator_profile_id: row.get("emulator_profile_id"),
        display_name: row.get("display_name"),
        source_type: row.get("source_type"),
        version: row.get("version"),
        path: row.get("path"),
        sha256: row.get("sha256"),
        machine_count: row.get("machine_count"),
        rom_entry_count: row.get("rom_entry_count"),
        disk_entry_count: row.get("disk_entry_count"),
        imported_at: row.get("imported_at"),
        active: row.get::<i64, _>("active") != 0,
        parser_version: row.get("parser_version"),
    }
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<DatSource>> {
    let rows = sqlx::query("SELECT * FROM dat_sources ORDER BY imported_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(map_row).collect())
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Option<DatSource>> {
    let row = sqlx::query("SELECT * FROM dat_sources WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn find_by_sha256(
    pool: &SqlitePool,
    profile_id: &str,
    sha256: &str,
) -> AppResult<Option<DatSource>> {
    let row = sqlx::query(
        "SELECT * FROM dat_sources WHERE emulator_profile_id = ?1 AND sha256 = ?2",
    )
    .bind(profile_id)
    .bind(sha256)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn active_for_profile(
    pool: &SqlitePool,
    profile_id: &str,
) -> AppResult<Option<DatSource>> {
    let row = sqlx::query(
        "SELECT * FROM dat_sources WHERE emulator_profile_id = ?1 AND active = 1 LIMIT 1",
    )
    .bind(profile_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn list_active(pool: &SqlitePool) -> AppResult<Vec<DatSource>> {
    let rows = sqlx::query("SELECT * FROM dat_sources WHERE active = 1")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(map_row).collect())
}

pub async fn active_for_profile_tx(
    tx: &mut Transaction<'_, Sqlite>,
    profile_id: &str,
) -> AppResult<Option<DatSource>> {
    let row = sqlx::query(
        "SELECT * FROM dat_sources WHERE emulator_profile_id = ?1 AND active = 1 LIMIT 1",
    )
    .bind(profile_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn deactivate_tx(tx: &mut Transaction<'_, Sqlite>, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE dat_sources SET active = 0 WHERE id = ?1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn deactivate(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    deactivate_tx(&mut tx, id).await?;
    clear_results_for_dat_tx(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn clear_results_for_dat_tx(
    tx: &mut Transaction<'_, Sqlite>,
    dat_id: i64,
) -> AppResult<()> {
    sqlx::query("DELETE FROM routes WHERE match_result_id IN (SELECT id FROM match_results WHERE dat_source_id = ?1)")
        .bind(dat_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM match_results WHERE dat_source_id = ?1")
        .bind(dat_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    new: &NewDatSource,
) -> AppResult<DatSource> {
    let row = sqlx::query(
        "INSERT INTO dat_sources (
             emulator_profile_id, display_name, source_type, version, path, sha256,
             machine_count, rom_entry_count, disk_entry_count, imported_at, active, parser_version
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11)
         RETURNING *",
    )
    .bind(&new.emulator_profile_id)
    .bind(&new.display_name)
    .bind(&new.source_type)
    .bind(&new.version)
    .bind(&new.path)
    .bind(&new.sha256)
    .bind(new.machine_count)
    .bind(new.rom_entry_count)
    .bind(new.disk_entry_count)
    .bind(now_iso8601())
    .bind(new.parser_version)
    .fetch_one(&mut **tx)
    .await?;

    Ok(map_row(&row))
}
