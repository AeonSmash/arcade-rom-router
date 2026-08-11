use std::collections::{HashMap, HashSet};

use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use crate::archive::zip_reader::ZipMember;
use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::{ArchiveMemberRow, ArchivePage, ArchiveRow, ArchiveState, LibrarySummary};

/// Everything the scanner learned about one file, ready to be persisted.
#[derive(Debug, Clone)]
pub struct ArchiveUpsert {
    pub rom_root_id: i64,
    pub path: String,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: Option<String>,
    pub quick_signature: String,
    pub sha256: Option<String>,
    pub state: ArchiveState,
    pub member_count: i64,
    pub unsafe_member_count: i64,
    pub error_detail: Option<String>,
}

/// One archive plus its members, written together so an archive row and its
/// member rows can never disagree.
#[derive(Debug, Clone)]
pub struct ArchiveWithMembers {
    pub archive: ArchiveUpsert,
    pub members: Vec<ZipMember>,
}

const MEMBER_COLUMNS: usize = 9;
const MEMBER_CHUNK_ROWS: usize = 64;

async fn upsert_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &ArchiveUpsert,
) -> AppResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO archives (
             rom_root_id, path, file_name, extension, size_bytes, modified_at,
             quick_signature, sha256, archive_state, member_count,
             unsafe_member_count, error_detail, last_scanned_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(path) DO UPDATE SET
             rom_root_id         = excluded.rom_root_id,
             file_name           = excluded.file_name,
             extension           = excluded.extension,
             size_bytes          = excluded.size_bytes,
             modified_at         = excluded.modified_at,
             quick_signature     = excluded.quick_signature,
             sha256              = COALESCE(excluded.sha256, archives.sha256),
             archive_state       = excluded.archive_state,
             member_count        = excluded.member_count,
             unsafe_member_count = excluded.unsafe_member_count,
             error_detail        = excluded.error_detail,
             last_scanned_at     = excluded.last_scanned_at
         RETURNING id",
    )
    .bind(record.rom_root_id)
    .bind(&record.path)
    .bind(&record.file_name)
    .bind(&record.extension)
    .bind(record.size_bytes)
    .bind(&record.modified_at)
    .bind(&record.quick_signature)
    .bind(&record.sha256)
    .bind(record.state.as_str())
    .bind(record.member_count)
    .bind(record.unsafe_member_count)
    .bind(&record.error_detail)
    .bind(now_iso8601())
    .fetch_one(&mut **tx)
    .await?;

    Ok(id)
}

async fn write_members(
    tx: &mut Transaction<'_, Sqlite>,
    archive_id: i64,
    members: &[ZipMember],
) -> AppResult<()> {
    sqlx::query("DELETE FROM archive_members WHERE archive_id = ?1")
        .bind(archive_id)
        .execute(&mut **tx)
        .await?;

    for chunk in members.chunks(MEMBER_CHUNK_ROWS) {
        let placeholders = (0..chunk.len())
            .map(|_| format!("({})", ["?"; MEMBER_COLUMNS].join(",")))
            .collect::<Vec<_>>()
            .join(",");

        let sql = format!(
            "INSERT INTO archive_members (
                 archive_id, member_name, size_bytes, compressed_size_bytes,
                 crc32, sha1, compression_method, is_directory, name_is_safe
             ) VALUES {placeholders}"
        );

        // Audited: the only dynamic part is the placeholder count, derived from
        // `chunk.len()`. Every value is bound.
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for member in chunk {
            query = query
                .bind(archive_id)
                .bind(&member.name)
                .bind(member.uncompressed_size as i64)
                .bind(member.compressed_size as i64)
                .bind(&member.crc32)
                .bind(Option::<String>::None)
                .bind(&member.compression_method)
                .bind(i64::from(member.is_directory))
                .bind(i64::from(member.name_is_safe));
        }
        query.execute(&mut **tx).await?;
    }

    Ok(())
}

/// Commits a batch of inspection results in a single transaction.
///
/// Batching keeps a cancelled scan's partial results consistent: whatever was
/// committed is complete and correct, and the remainder is simply absent.
pub async fn commit_batch(pool: &SqlitePool, batch: &[ArchiveWithMembers]) -> AppResult<()> {
    if batch.is_empty() {
        return Ok(());
    }

    let mut tx = pool.begin().await?;
    for item in batch {
        let archive_id = upsert_in_tx(&mut tx, &item.archive).await?;
        write_members(&mut tx, archive_id, &item.members).await?;
    }
    tx.commit().await?;

    Ok(())
}

/// Convenience wrapper used by tests and single-file operations.
pub async fn upsert(pool: &SqlitePool, record: &ArchiveUpsert) -> AppResult<i64> {
    let mut tx = pool.begin().await?;
    let id = upsert_in_tx(&mut tx, record).await?;
    tx.commit().await?;
    Ok(id)
}

/// Path to quick-signature map, used to decide which files need re-inspection.
pub async fn signature_cache(
    pool: &SqlitePool,
    rom_root_id: i64,
) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query("SELECT path, quick_signature FROM archives WHERE rom_root_id = ?1")
        .bind(rom_root_id)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.get::<String, _>("path"), row.get::<String, _>("quick_signature")))
        .collect())
}

/// Marks cached archives as seen by this scan without rewriting their members.
pub async fn touch_scanned(pool: &SqlitePool, paths: &[String]) -> AppResult<()> {
    if paths.is_empty() {
        return Ok(());
    }

    let timestamp = now_iso8601();
    let mut tx = pool.begin().await?;
    for chunk in paths.chunks(256) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "UPDATE archives SET last_scanned_at = ? WHERE path IN ({placeholders})"
        );
        // Audited: placeholder count only; paths are bound.
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(&timestamp);
        for path in chunk {
            query = query.bind(path);
        }
        query.execute(&mut *tx).await?;
    }
    tx.commit().await?;

    Ok(())
}

/// Drops inventory rows for files that no longer exist in the root.
///
/// Only the application's cached record is removed; nothing on disk is deleted.
pub async fn remove_absent(
    pool: &SqlitePool,
    rom_root_id: i64,
    seen: &HashSet<String>,
) -> AppResult<u64> {
    let known: Vec<String> =
        sqlx::query_scalar("SELECT path FROM archives WHERE rom_root_id = ?1")
            .bind(rom_root_id)
            .fetch_all(pool)
            .await?;

    let absent: Vec<String> = known.into_iter().filter(|p| !seen.contains(p)).collect();
    if absent.is_empty() {
        return Ok(0);
    }

    let mut removed = 0u64;
    let mut tx = pool.begin().await?;
    for chunk in absent.chunks(256) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!("DELETE FROM archives WHERE path IN ({placeholders})");
        // Audited: placeholder count only; paths are bound.
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for path in chunk {
            query = query.bind(path);
        }
        removed += query.execute(&mut *tx).await?.rows_affected();
    }
    tx.commit().await?;

    Ok(removed)
}

#[derive(Debug, Clone)]
pub struct ArchiveQuery {
    pub rom_root_id: Option<i64>,
    pub state: Option<ArchiveState>,
    pub search: Option<String>,
    pub favorites_only: bool,
    pub limit: i64,
    pub offset: i64,
}

impl Default for ArchiveQuery {
    fn default() -> Self {
        Self {
            rom_root_id: None,
            state: None,
            search: None,
            favorites_only: false,
            limit: 200,
            offset: 0,
        }
    }
}

fn map_archive_row(row: &sqlx::sqlite::SqliteRow) -> ArchiveRow {
    let stored_state: String = row.get("archive_state");
    let is_favorite = row
        .try_get::<i64, _>("is_favorite")
        .map(|v| v != 0)
        .unwrap_or(false);
    ArchiveRow {
        id: row.get("id"),
        rom_root_id: row.get("rom_root_id"),
        path: row.get("path"),
        file_name: row.get("file_name"),
        extension: row.get("extension"),
        size_bytes: row.get("size_bytes"),
        modified_at: row.get("modified_at"),
        sha256: row.get("sha256"),
        // A row written by a newer schema than this build understands is shown
        // as unreadable rather than silently mislabelled as indexed.
        archive_state: ArchiveState::parse(&stored_state)
            .unwrap_or(ArchiveState::ArchiveUnreadable),
        member_count: row.get("member_count"),
        unsafe_member_count: row.get("unsafe_member_count"),
        error_detail: row.get("error_detail"),
        last_scanned_at: row.get("last_scanned_at"),
        is_favorite,
    }
}

type SqliteQuery<'q> = sqlx::query::Query<'q, Sqlite, sqlx::sqlite::SqliteArguments>;

/// Binds the filter values in the same order the conditions were appended.
///
/// Values are always bound, never interpolated; only the shape of the WHERE
/// clause is built as text.
fn bind_filter_values<'q>(
    mut q: SqliteQuery<'q>,
    query: &ArchiveQuery,
    search_term: Option<String>,
) -> SqliteQuery<'q> {
    if let Some(id) = query.rom_root_id {
        q = q.bind(id);
    }
    if let Some(state) = query.state {
        q = q.bind(state.as_str());
    }
    if let Some(term) = search_term {
        q = q.bind(term);
    }
    q
}

pub async fn page(pool: &SqlitePool, query: &ArchiveQuery) -> AppResult<ArchivePage> {
    let mut conditions: Vec<&str> = Vec::new();
    if query.rom_root_id.is_some() {
        conditions.push("a.rom_root_id = ?");
    }
    if query.state.is_some() {
        conditions.push("a.archive_state = ?");
    }
    if query.favorites_only {
        conditions.push("f.archive_id IS NOT NULL");
    }
    let search_term = query
        .search
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    if search_term.is_some() {
        conditions.push("a.file_name LIKE ? ESCAPE '\\'");
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let bind_filters = |q| bind_filter_values(q, query, search_term.clone());

    // Audited: `where_clause` is assembled from the fixed fragments above and
    // contains no caller-supplied text.
    let count_sql = format!(
        "SELECT COUNT(*) FROM archives a
         LEFT JOIN favorites f ON f.archive_id = a.id
         {where_clause}"
    );
    let total_matching: i64 = bind_filters(sqlx::query(sqlx::AssertSqlSafe(count_sql)))
        .fetch_one(pool)
        .await?
        .get(0);

    let rows_sql = format!(
        "SELECT a.*, CASE WHEN f.archive_id IS NULL THEN 0 ELSE 1 END AS is_favorite
         FROM archives a
         LEFT JOIN favorites f ON f.archive_id = a.id
         {where_clause}
         ORDER BY a.file_name COLLATE NOCASE, a.id
         LIMIT ? OFFSET ?"
    );
    let rows = bind_filters(sqlx::query(sqlx::AssertSqlSafe(rows_sql)))
        .bind(query.limit.clamp(1, 5_000))
        .bind(query.offset.max(0))
        .fetch_all(pool)
        .await?;

    Ok(ArchivePage {
        rows: rows.iter().map(map_archive_row).collect(),
        total_matching,
        summary: summary(pool).await?,
    })
}

pub async fn summary(pool: &SqlitePool) -> AppResult<LibrarySummary> {
    let row = sqlx::query(
        "SELECT
             COUNT(*) AS total,
             COALESCE(SUM(archive_state = 'INDEXED'), 0) AS indexed,
             COALESCE(SUM(archive_state = 'DISK_IMAGE_INDEXED'), 0) AS disk_images,
             COALESCE(SUM(archive_state = 'ARCHIVE_UNREADABLE'), 0) AS unreadable
         FROM archives",
    )
    .fetch_one(pool)
    .await?;

    let favorites: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM favorites")
        .fetch_one(pool)
        .await?;

    Ok(LibrarySummary {
        total: row.get("total"),
        indexed: row.get("indexed"),
        disk_images: row.get("disk_images"),
        unreadable: row.get("unreadable"),
        favorites,
    })
}

pub async fn get(pool: &SqlitePool, id: i64) -> AppResult<Option<ArchiveRow>> {
    let row = sqlx::query(
        "SELECT a.*, CASE WHEN f.archive_id IS NULL THEN 0 ELSE 1 END AS is_favorite
         FROM archives a
         LEFT JOIN favorites f ON f.archive_id = a.id
         WHERE a.id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(map_archive_row))
}

pub async fn members(pool: &SqlitePool, archive_id: i64) -> AppResult<Vec<ArchiveMemberRow>> {
    let rows = sqlx::query(
        "SELECT * FROM archive_members WHERE archive_id = ?1 ORDER BY member_name COLLATE NOCASE",
    )
    .bind(archive_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ArchiveMemberRow {
            member_name: row.get("member_name"),
            size_bytes: row.get("size_bytes"),
            compressed_size_bytes: row.get("compressed_size_bytes"),
            crc32: row.get("crc32"),
            sha1: row.get("sha1"),
            compression_method: row.get("compression_method"),
            is_directory: row.get::<i64, _>("is_directory") != 0,
            name_is_safe: row.get::<i64, _>("name_is_safe") != 0,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connect_in_memory, rom_roots};

    fn sample(root_id: i64, name: &str, state: ArchiveState, members: usize) -> ArchiveWithMembers {
        ArchiveWithMembers {
            archive: ArchiveUpsert {
                rom_root_id: root_id,
                path: format!("D:\\Arcade\\{name}"),
                file_name: name.to_string(),
                extension: "zip".into(),
                size_bytes: 1024,
                modified_at: Some("2026-08-09T00:00:00.000Z".into()),
                quick_signature: format!("sig-{name}"),
                sha256: None,
                state,
                member_count: members as i64,
                unsafe_member_count: 0,
                error_detail: None,
            },
            members: (0..members)
                .map(|i| ZipMember {
                    name: format!("chip{i}.bin"),
                    uncompressed_size: 4096,
                    compressed_size: 2048,
                    crc32: format!("{:08x}", 0x1000_0000u32 + i as u32),
                    compression_method: "Deflated".into(),
                    is_directory: false,
                    name_is_safe: true,
                })
                .collect(),
        }
    }

    async fn setup() -> (SqlitePool, i64) {
        let pool = connect_in_memory().await.unwrap();
        let root = rom_roots::insert(&pool, "D:\\Arcade", None, true)
            .await
            .unwrap();
        (pool, root.id)
    }

    #[tokio::test]
    async fn batch_writes_archives_and_members() {
        let (pool, root_id) = setup().await;

        commit_batch(
            &pool,
            &[
                sample(root_id, "1942.zip", ArchiveState::Indexed, 14),
                sample(root_id, "sf2.zip", ArchiveState::Indexed, 21),
            ],
        )
        .await
        .unwrap();

        let page = page(&pool, &ArchiveQuery::default()).await.unwrap();
        assert_eq!(page.total_matching, 2);
        assert_eq!(page.summary.indexed, 2);

        let sf2 = page.rows.iter().find(|r| r.file_name == "sf2.zip").unwrap();
        assert_eq!(sf2.member_count, 21);
        assert_eq!(members(&pool, sf2.id).await.unwrap().len(), 21);
    }

    #[tokio::test]
    async fn rescanning_replaces_members_instead_of_duplicating_them() {
        let (pool, root_id) = setup().await;

        commit_batch(&pool, &[sample(root_id, "1942.zip", ArchiveState::Indexed, 14)])
            .await
            .unwrap();
        commit_batch(&pool, &[sample(root_id, "1942.zip", ArchiveState::Indexed, 3)])
            .await
            .unwrap();

        let page = page(&pool, &ArchiveQuery::default()).await.unwrap();
        assert_eq!(page.total_matching, 1);
        assert_eq!(members(&pool, page.rows[0].id).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn summary_counts_each_state_separately() {
        let (pool, root_id) = setup().await;

        commit_batch(
            &pool,
            &[
                sample(root_id, "ok.zip", ArchiveState::Indexed, 2),
                sample(root_id, "broken.zip", ArchiveState::ArchiveUnreadable, 0),
                sample(root_id, "disk.chd", ArchiveState::DiskImageIndexed, 0),
            ],
        )
        .await
        .unwrap();

        let summary = summary(&pool).await.unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.indexed, 1);
        assert_eq!(summary.unreadable, 1);
        assert_eq!(summary.disk_images, 1);
    }

    #[tokio::test]
    async fn queries_filter_by_state_and_search_text() {
        let (pool, root_id) = setup().await;

        commit_batch(
            &pool,
            &[
                sample(root_id, "1942.zip", ArchiveState::Indexed, 1),
                sample(root_id, "sf2.zip", ArchiveState::Indexed, 1),
                sample(root_id, "broken.zip", ArchiveState::ArchiveUnreadable, 0),
            ],
        )
        .await
        .unwrap();

        let unreadable = page(
            &pool,
            &ArchiveQuery {
                state: Some(ArchiveState::ArchiveUnreadable),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(unreadable.total_matching, 1);
        assert_eq!(unreadable.rows[0].file_name, "broken.zip");

        let searched = page(
            &pool,
            &ArchiveQuery {
                search: Some("sf".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(searched.total_matching, 1);
    }

    #[tokio::test]
    async fn favorites_only_filters_and_flags_rows() {
        use crate::db::favorites;

        let (pool, root_id) = setup().await;
        commit_batch(
            &pool,
            &[
                sample(root_id, "1942.zip", ArchiveState::Indexed, 1),
                sample(root_id, "sf2.zip", ArchiveState::Indexed, 1),
            ],
        )
        .await
        .unwrap();

        let all = page(&pool, &ArchiveQuery::default()).await.unwrap();
        assert_eq!(all.total_matching, 2);
        assert!(!all.rows[0].is_favorite);

        let fav_id = all.rows.iter().find(|r| r.file_name == "sf2.zip").unwrap().id;
        assert!(favorites::toggle(&pool, fav_id).await.unwrap());

        let favs = page(
            &pool,
            &ArchiveQuery {
                favorites_only: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(favs.total_matching, 1);
        assert_eq!(favs.rows[0].file_name, "sf2.zip");
        assert!(favs.rows[0].is_favorite);
        assert_eq!(favs.summary.favorites, 1);
    }

    #[tokio::test]
    async fn search_wildcards_are_treated_as_literal_text() {
        let (pool, root_id) = setup().await;

        commit_batch(
            &pool,
            &[
                sample(root_id, "1942.zip", ArchiveState::Indexed, 1),
                sample(root_id, "sf2.zip", ArchiveState::Indexed, 1),
            ],
        )
        .await
        .unwrap();

        let searched = page(
            &pool,
            &ArchiveQuery {
                search: Some("%".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(searched.total_matching, 0);
    }

    #[tokio::test]
    async fn absent_files_are_dropped_from_the_cache() {
        let (pool, root_id) = setup().await;

        commit_batch(
            &pool,
            &[
                sample(root_id, "kept.zip", ArchiveState::Indexed, 1),
                sample(root_id, "gone.zip", ArchiveState::Indexed, 1),
            ],
        )
        .await
        .unwrap();

        let seen: HashSet<String> = ["D:\\Arcade\\kept.zip".to_string()].into_iter().collect();
        assert_eq!(remove_absent(&pool, root_id, &seen).await.unwrap(), 1);

        let page = page(&pool, &ArchiveQuery::default()).await.unwrap();
        assert_eq!(page.total_matching, 1);
        assert_eq!(page.rows[0].file_name, "kept.zip");
    }

    #[tokio::test]
    async fn signature_cache_reports_stored_signatures() {
        let (pool, root_id) = setup().await;
        commit_batch(&pool, &[sample(root_id, "1942.zip", ArchiveState::Indexed, 1)])
            .await
            .unwrap();

        let cache = signature_cache(&pool, root_id).await.unwrap();
        assert_eq!(
            cache.get("D:\\Arcade\\1942.zip").map(String::as_str),
            Some("sig-1942.zip")
        );
    }
}
