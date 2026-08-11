use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::MediaAsset;

pub async fn for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<Vec<MediaAsset>> {
    let rows = sqlx::query(
        "SELECT * FROM media_assets WHERE archive_id = ?1
         ORDER BY kind,
           CASE source WHEN 'local' THEN 0 WHEN 'emumovies' THEN 1 ELSE 2 END,
           source",
    )
    .bind(archive_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(map_row).collect())
}

pub async fn upsert(
    pool: &SqlitePool,
    archive_id: i64,
    set_name: Option<&str>,
    kind: &str,
    path: &str,
    source: &str,
) -> AppResult<MediaAsset> {
    let now = now_iso8601();
    sqlx::query(
        "INSERT INTO media_assets (archive_id, set_name, kind, path, source, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(archive_id, kind, source) DO UPDATE SET
           set_name = excluded.set_name,
           path = excluded.path,
           fetched_at = excluded.fetched_at",
    )
    .bind(archive_id)
    .bind(set_name)
    .bind(kind)
    .bind(path)
    .bind(source)
    .bind(&now)
    .execute(pool)
    .await?;

    let row = sqlx::query(
        "SELECT * FROM media_assets WHERE archive_id = ?1 AND kind = ?2 AND source = ?3",
    )
    .bind(archive_id)
    .bind(kind)
    .bind(source)
    .fetch_one(pool)
    .await?;
    Ok(map_row(&row))
}

pub async fn clear_all(pool: &SqlitePool) -> AppResult<u64> {
    let result = sqlx::query("DELETE FROM media_assets").execute(pool).await?;
    Ok(result.rows_affected())
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> MediaAsset {
    MediaAsset {
        id: row.get("id"),
        archive_id: row.get("archive_id"),
        set_name: row.get("set_name"),
        kind: row.get("kind"),
        path: row.get("path"),
        source: row.get("source"),
        width: row.get("width"),
        height: row.get("height"),
        sha256: row.get("sha256"),
        fetched_at: row.get("fetched_at"),
        asset_url: None,
    }
}
