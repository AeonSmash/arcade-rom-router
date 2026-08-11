use sqlx::{Row, SqlitePool};

use crate::dat::catver::CategoryEntry;
use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::CategoryStats;

/// Replace all category rows with a fresh CatVer import.
pub async fn replace_all(
    pool: &SqlitePool,
    entries: &[CategoryEntry],
    source_path: &str,
) -> AppResult<CategoryStats> {
    let imported_at = now_iso8601();
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM set_categories")
        .execute(&mut *tx)
        .await?;

    for entry in entries {
        sqlx::query(
            "INSERT INTO set_categories (set_name, category, source_path, imported_at)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&entry.set_name)
        .bind(&entry.category)
        .bind(source_path)
        .bind(&imported_at)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(CategoryStats {
        count: entries.len() as i64,
        source_path: Some(source_path.to_string()),
        imported_at: Some(imported_at),
    })
}

pub async fn stats(pool: &SqlitePool) -> AppResult<CategoryStats> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM set_categories")
        .fetch_one(pool)
        .await?;

    if count == 0 {
        return Ok(CategoryStats {
            count: 0,
            source_path: None,
            imported_at: None,
        });
    }

    let row = sqlx::query(
        "SELECT source_path, imported_at FROM set_categories
         ORDER BY imported_at DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await?;

    Ok(CategoryStats {
        count,
        source_path: row.get("source_path"),
        imported_at: row.get("imported_at"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn replace_all_is_idempotent_refresh() {
        let pool = connect_in_memory().await.unwrap();
        let first = replace_all(
            &pool,
            &[CategoryEntry {
                set_name: "1942".into(),
                category: "Shooter".into(),
            }],
            "a.ini",
        )
        .await
        .unwrap();
        assert_eq!(first.count, 1);

        let second = replace_all(
            &pool,
            &[
                CategoryEntry {
                    set_name: "1942".into(),
                    category: "Shooter / Flying Vertical".into(),
                },
                CategoryEntry {
                    set_name: "mspacman".into(),
                    category: "Maze / Collect".into(),
                },
            ],
            "b.ini",
        )
        .await
        .unwrap();
        assert_eq!(second.count, 2);
        assert_eq!(stats(&pool).await.unwrap().count, 2);

        let cat: String = sqlx::query_scalar(
            "SELECT category FROM set_categories WHERE set_name = '1942' COLLATE NOCASE",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(cat, "Shooter / Flying Vertical");
    }
}
