//! SQLite access layer.
//!
//! Migrations are embedded at compile time, so a release build carries its own
//! schema and needs no `DATABASE_URL`. All statements are prepared and
//! parameterized (SPEC.md section 43.3).

pub mod archives;
pub mod categories;
pub mod controllers;
pub mod dats;
pub mod favorites;
pub mod hotkeys;
pub mod machines;
pub mod matches;
pub mod media;
pub mod profiles;
pub mod rom_roots;
pub mod routes;
pub mod save_states;
pub mod scan_jobs;
pub mod settings;

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

use crate::error::AppResult;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Opens (creating if needed) the library database and applies all migrations.
pub async fn connect(path: &Path) -> AppResult<SqlitePool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| crate::error::AppError::Filesystem {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .unwrap_or_else(|_| SqliteConnectOptions::new().filename(path))
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(15));

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(30))
        .connect_with(options)
        .await?;

    MIGRATOR.run(&pool).await?;

    Ok(pool)
}

/// An in-memory database with the full schema applied, for tests.
#[cfg(test)]
pub async fn connect_in_memory() -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true)
        .shared_cache(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(options)
        .await?;

    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

pub fn now_iso8601() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_create_the_phase_one_schema() {
        let pool = connect_in_memory().await.unwrap();

        let tables: Vec<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .unwrap();

        for expected in [
            "archive_members",
            "archives",
            "rom_roots",
            "scan_jobs",
            "settings",
        ] {
            assert!(tables.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[tokio::test]
    async fn crc_indexes_exist_for_the_future_matching_engine() {
        let pool = connect_in_memory().await.unwrap();

        let indexes: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'archive_members'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert!(indexes.contains(&"idx_members_crc32".to_string()));
        assert!(indexes.contains(&"idx_members_crc32_size".to_string()));
    }

    #[tokio::test]
    async fn deleting_a_root_cascades_to_its_archives() {
        let pool = connect_in_memory().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();

        let root = rom_roots::insert(&pool, "D:\\Arcade", None, true)
            .await
            .unwrap();
        let archive_id = archives::upsert(
            &pool,
            &archives::ArchiveUpsert {
                rom_root_id: root.id,
                path: "D:\\Arcade\\a.zip".into(),
                file_name: "a.zip".into(),
                extension: "zip".into(),
                size_bytes: 10,
                modified_at: None,
                quick_signature: "sig".into(),
                sha256: None,
                state: crate::model::ArchiveState::Indexed,
                member_count: 0,
                unsafe_member_count: 0,
                error_detail: None,
            },
        )
        .await
        .unwrap();
        assert!(archive_id > 0);

        rom_roots::delete(&pool, root.id).await.unwrap();

        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM archives")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
