//! Favorites table access (SPEC §53).

use sqlx::SqlitePool;

use crate::db;
use crate::error::AppResult;

pub async fn is_favorite(pool: &SqlitePool, archive_id: i64) -> AppResult<bool> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM favorites WHERE archive_id = ?1 LIMIT 1")
            .bind(archive_id)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

/// Inserts or removes a favorite. Returns `true` when the archive is favorited after the call.
pub async fn toggle(pool: &SqlitePool, archive_id: i64) -> AppResult<bool> {
    if is_favorite(pool, archive_id).await? {
        sqlx::query("DELETE FROM favorites WHERE archive_id = ?1")
            .bind(archive_id)
            .execute(pool)
            .await?;
        Ok(false)
    } else {
        sqlx::query("INSERT INTO favorites (archive_id, created_at) VALUES (?1, ?2)")
            .bind(archive_id)
            .bind(db::now_iso8601())
            .execute(pool)
            .await?;
        Ok(true)
    }
}

pub async fn list_archive_ids(pool: &SqlitePool) -> AppResult<Vec<i64>> {
    let ids = sqlx::query_scalar("SELECT archive_id FROM favorites ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(ids)
}

pub async fn count(pool: &SqlitePool) -> AppResult<i64> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM favorites")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{archives, connect_in_memory, rom_roots};
    use crate::model::ArchiveState;

    async fn seed_archive(pool: &SqlitePool) -> i64 {
        let root = rom_roots::insert(pool, "D:\\Arcade", None, true)
            .await
            .unwrap();
        archives::upsert(
            pool,
            &archives::ArchiveUpsert {
                rom_root_id: root.id,
                path: "D:\\Arcade\\galaga.zip".into(),
                file_name: "galaga.zip".into(),
                extension: "zip".into(),
                size_bytes: 100,
                modified_at: None,
                quick_signature: "sig".into(),
                sha256: None,
                state: ArchiveState::Indexed,
                member_count: 1,
                unsafe_member_count: 0,
                error_detail: None,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn toggle_adds_and_removes() {
        let pool = connect_in_memory().await.unwrap();
        let id = seed_archive(&pool).await;

        assert!(!is_favorite(&pool, id).await.unwrap());
        assert!(toggle(&pool, id).await.unwrap());
        assert!(is_favorite(&pool, id).await.unwrap());
        assert_eq!(count(&pool).await.unwrap(), 1);
        assert!(!toggle(&pool, id).await.unwrap());
        assert!(!is_favorite(&pool, id).await.unwrap());
        assert_eq!(count(&pool).await.unwrap(), 0);
    }
}
