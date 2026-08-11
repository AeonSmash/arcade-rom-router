use sqlx::{Row, SqlitePool};

use crate::error::AppResult;
use crate::model::SaveStateRow;

pub async fn for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<Vec<SaveStateRow>> {
    let rows = sqlx::query(
        "SELECT * FROM save_states WHERE archive_id = ?1 ORDER BY is_entry, slot",
    )
    .bind(archive_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(map_row).collect())
}

pub async fn replace_for_archive(
    pool: &SqlitePool,
    archive_id: i64,
    rows: &[SaveStateRow],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM save_states WHERE archive_id = ?1")
        .bind(archive_id)
        .execute(&mut *tx)
        .await?;
    for row in rows {
        sqlx::query(
            "INSERT INTO save_states
               (archive_id, slot, path, size_bytes, modified_at, label, thumbnail_path, is_entry)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(archive_id)
        .bind(row.slot)
        .bind(&row.path)
        .bind(row.size_bytes)
        .bind(&row.modified_at)
        .bind(&row.label)
        .bind(&row.thumbnail_path)
        .bind(row.is_entry)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn set_label(pool: &SqlitePool, id: i64, label: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE save_states SET label = ?1 WHERE id = ?2")
        .bind(label)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Option<SaveStateRow>> {
    let row = sqlx::query("SELECT * FROM save_states WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<Option<SaveStateRow>> {
    let existing = get(pool, id).await?;
    if existing.is_some() {
        sqlx::query("DELETE FROM save_states WHERE id = ?1")
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(existing)
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> SaveStateRow {
    SaveStateRow {
        id: row.get("id"),
        archive_id: row.get("archive_id"),
        slot: row.get("slot"),
        path: row.get("path"),
        size_bytes: row.get("size_bytes"),
        modified_at: row.get("modified_at"),
        label: row.get("label"),
        thumbnail_path: row.get("thumbnail_path"),
        is_entry: row.get::<i64, _>("is_entry") != 0,
        thumbnail_url: None,
    }
}
