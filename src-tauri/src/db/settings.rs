//! Typed access to the key/value settings table.
//!
//! Values are stored as JSON text so a setting can grow from a scalar into a
//! structure without a migration.

use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppResult;

/// Setting keys used in Phase 0/1. Later phases add matching, emulator, and
/// appearance keys (SPEC.md section 67).
pub mod keys {
    pub const SCAN_WORKER_COUNT: &str = "matching.scanWorkerCount";
    pub const RECURSIVE_SCAN_DEFAULT: &str = "library.recursiveScanDefault";
    pub const ONBOARDING_COMPLETED: &str = "app.onboardingCompleted";
}

pub async fn get_raw(pool: &SqlitePool, key: &str) -> AppResult<Option<String>> {
    let value = sqlx::query_scalar("SELECT value_json FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(value)
}

pub async fn set_raw(pool: &SqlitePool, key: &str, value_json: &str) -> AppResult<()> {
    // Reject anything that is not valid JSON so a malformed write cannot make
    // every later read fail.
    serde_json::from_str::<serde_json::Value>(value_json)?;

    sqlx::query(
        "INSERT INTO settings (key, value_json) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
    )
    .bind(key)
    .bind(value_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get<T: DeserializeOwned>(pool: &SqlitePool, key: &str) -> AppResult<Option<T>> {
    match get_raw(pool, key).await? {
        Some(raw) => Ok(serde_json::from_str(&raw).ok()),
        None => Ok(None),
    }
}

pub async fn get_or<T: DeserializeOwned>(pool: &SqlitePool, key: &str, fallback: T) -> T {
    match get::<T>(pool, key).await {
        Ok(Some(value)) => value,
        _ => fallback,
    }
}

pub async fn set<T: Serialize>(pool: &SqlitePool, key: &str, value: &T) -> AppResult<()> {
    set_raw(pool, key, &serde_json::to_string(value)?).await
}

pub async fn all(pool: &SqlitePool) -> AppResult<serde_json::Map<String, serde_json::Value>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value_json FROM settings")
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(key, raw)| {
            let value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
            (key, value)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn values_round_trip() {
        let pool = connect_in_memory().await.unwrap();
        set(&pool, keys::SCAN_WORKER_COUNT, &4u32).await.unwrap();

        assert_eq!(
            get::<u32>(&pool, keys::SCAN_WORKER_COUNT).await.unwrap(),
            Some(4)
        );
    }

    #[tokio::test]
    async fn writing_the_same_key_twice_updates_it() {
        let pool = connect_in_memory().await.unwrap();
        set(&pool, keys::SCAN_WORKER_COUNT, &4u32).await.unwrap();
        set(&pool, keys::SCAN_WORKER_COUNT, &2u32).await.unwrap();

        assert_eq!(get_or(&pool, keys::SCAN_WORKER_COUNT, 99u32).await, 2);
    }

    #[tokio::test]
    async fn missing_keys_fall_back() {
        let pool = connect_in_memory().await.unwrap();
        assert!(!get_or(&pool, keys::ONBOARDING_COMPLETED, false).await);
    }

    #[tokio::test]
    async fn malformed_json_is_refused() {
        let pool = connect_in_memory().await.unwrap();
        assert!(set_raw(&pool, "bad", "{not json").await.is_err());
        assert!(get_raw(&pool, "bad").await.unwrap().is_none());
    }
}
