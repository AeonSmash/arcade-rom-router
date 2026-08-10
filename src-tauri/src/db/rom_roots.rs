use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::RomRoot;

fn map_row(row: &sqlx::sqlite::SqliteRow) -> RomRoot {
    RomRoot {
        id: row.get("id"),
        path: row.get("path"),
        label: row.get("label"),
        recursive: row.get::<i64, _>("recursive") != 0,
        enabled: row.get::<i64, _>("enabled") != 0,
        read_only: row.get::<i64, _>("read_only") != 0,
        watch_changes: row.get::<i64, _>("watch_changes") != 0,
        created_at: row.get("created_at"),
        last_scan_at: row.get("last_scan_at"),
    }
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<RomRoot>> {
    let rows = sqlx::query("SELECT * FROM rom_roots ORDER BY id")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(map_row).collect())
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Option<RomRoot>> {
    let row = sqlx::query("SELECT * FROM rom_roots WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn find_by_path(pool: &SqlitePool, path: &str) -> AppResult<Option<RomRoot>> {
    let row = sqlx::query("SELECT * FROM rom_roots WHERE path = ?1")
        .bind(path)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(map_row))
}

pub async fn insert(
    pool: &SqlitePool,
    path: &str,
    label: Option<&str>,
    recursive: bool,
) -> AppResult<RomRoot> {
    // `read_only` defaults to 1 in the schema and is never lowered by this
    // code path; SPEC.md section 10 requires read-only to be the default.
    let row = sqlx::query(
        "INSERT INTO rom_roots (path, label, recursive, enabled, read_only, watch_changes, created_at)
         VALUES (?1, ?2, ?3, 1, 1, 0, ?4)
         RETURNING *",
    )
    .bind(path)
    .bind(label)
    .bind(i64::from(recursive))
    .bind(now_iso8601())
    .fetch_one(pool)
    .await?;

    Ok(map_row(&row))
}

pub async fn set_enabled(pool: &SqlitePool, id: i64, enabled: bool) -> AppResult<()> {
    sqlx::query("UPDATE rom_roots SET enabled = ?2 WHERE id = ?1")
        .bind(id)
        .bind(i64::from(enabled))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_scanned(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("UPDATE rom_roots SET last_scan_at = ?2 WHERE id = ?1")
        .bind(id)
        .bind(now_iso8601())
        .execute(pool)
        .await?;
    Ok(())
}

/// Removes a root and, by cascade, its cached inventory.
///
/// This only affects the application's own database. Nothing inside the ROM
/// folder itself is touched.
pub async fn delete(pool: &SqlitePool, id: i64) -> AppResult<()> {
    sqlx::query("DELETE FROM rom_roots WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn new_roots_are_read_only_and_enabled() {
        let pool = connect_in_memory().await.unwrap();
        let root = insert(&pool, "D:\\Arcade", Some("Main"), true).await.unwrap();

        assert!(root.read_only);
        assert!(root.enabled);
        assert!(root.recursive);
        assert!(!root.watch_changes);
        assert_eq!(root.label.as_deref(), Some("Main"));
    }

    #[tokio::test]
    async fn paths_are_unique() {
        let pool = connect_in_memory().await.unwrap();
        insert(&pool, "D:\\Arcade", None, true).await.unwrap();

        assert!(insert(&pool, "D:\\Arcade", None, true).await.is_err());
    }

    #[tokio::test]
    async fn roots_can_be_listed_and_disabled() {
        let pool = connect_in_memory().await.unwrap();
        let root = insert(&pool, "D:\\Arcade", None, true).await.unwrap();
        insert(&pool, "E:\\More", None, false).await.unwrap();

        assert_eq!(list(&pool).await.unwrap().len(), 2);

        set_enabled(&pool, root.id, false).await.unwrap();
        assert!(!get(&pool, root.id).await.unwrap().unwrap().enabled);
    }
}
