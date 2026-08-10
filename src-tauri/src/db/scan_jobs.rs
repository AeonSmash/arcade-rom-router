use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::{JobState, JobType};
use crate::scanner::ScanCounters;

pub async fn create(pool: &SqlitePool, job_type: JobType) -> AppResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO scan_jobs (job_type, state, started_at) VALUES (?1, ?2, ?3) RETURNING id",
    )
    .bind(job_type.as_str())
    .bind(JobState::Queued.as_str())
    .bind(now_iso8601())
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn update_progress(
    pool: &SqlitePool,
    id: i64,
    state: JobState,
    counters: &ScanCounters,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE scan_jobs SET
             state = ?2,
             total_candidates = ?3,
             processed = ?4,
             inspected = ?5,
             reused_from_cache = ?6,
             unreadable = ?7,
             removed = ?8
         WHERE id = ?1",
    )
    .bind(id)
    .bind(state.as_str())
    .bind(counters.total_candidates as i64)
    .bind(counters.processed as i64)
    .bind(counters.inspected as i64)
    .bind(counters.reused_from_cache as i64)
    .bind(counters.unreadable as i64)
    .bind(counters.removed as i64)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn finish(
    pool: &SqlitePool,
    id: i64,
    state: JobState,
    counters: &ScanCounters,
    error_detail: Option<String>,
) -> AppResult<()> {
    update_progress(pool, id, state, counters).await?;

    sqlx::query("UPDATE scan_jobs SET ended_at = ?2, error_detail = ?3 WHERE id = ?1")
        .bind(id)
        .bind(now_iso8601())
        .bind(error_detail)
        .execute(pool)
        .await?;

    Ok(())
}

/// Marks jobs left `RUNNING` by a crash or forced exit as failed.
///
/// Without this a stale row would make the UI show a scan that no longer has a
/// process behind it.
pub async fn reconcile_orphans(pool: &SqlitePool) -> AppResult<u64> {
    let affected = sqlx::query(
        "UPDATE scan_jobs
         SET state = ?1, ended_at = ?2, error_detail = 'Interrupted by application shutdown'
         WHERE state IN ('QUEUED', 'RUNNING', 'PAUSED', 'CANCELLING')",
    )
    .bind(JobState::Failed.as_str())
    .bind(now_iso8601())
    .execute(pool)
    .await?
    .rows_affected();

    Ok(affected)
}

pub async fn state_of(pool: &SqlitePool, id: i64) -> AppResult<Option<JobState>> {
    let row = sqlx::query("SELECT state FROM scan_jobs WHERE id = ?1")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(row.and_then(|row| JobState::parse(row.get::<String, _>("state").as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connect_in_memory;

    #[tokio::test]
    async fn jobs_start_queued_and_can_be_finished() {
        let pool = connect_in_memory().await.unwrap();
        let id = create(&pool, JobType::FullScan).await.unwrap();

        assert_eq!(state_of(&pool, id).await.unwrap(), Some(JobState::Queued));

        let counters = ScanCounters {
            total_candidates: 10,
            processed: 10,
            inspected: 8,
            reused_from_cache: 2,
            unreadable: 1,
            removed: 0,
        };
        finish(&pool, id, JobState::Completed, &counters, None)
            .await
            .unwrap();

        assert_eq!(state_of(&pool, id).await.unwrap(), Some(JobState::Completed));

        let processed: i64 = sqlx::query_scalar("SELECT processed FROM scan_jobs WHERE id = ?1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(processed, 10);
    }

    #[tokio::test]
    async fn interrupted_jobs_are_reconciled_on_startup() {
        let pool = connect_in_memory().await.unwrap();
        let id = create(&pool, JobType::IncrementalScan).await.unwrap();
        update_progress(&pool, id, JobState::Running, &ScanCounters::default())
            .await
            .unwrap();

        assert_eq!(reconcile_orphans(&pool).await.unwrap(), 1);
        assert_eq!(state_of(&pool, id).await.unwrap(), Some(JobState::Failed));
    }

    #[tokio::test]
    async fn completed_jobs_are_left_alone_by_reconciliation() {
        let pool = connect_in_memory().await.unwrap();
        let id = create(&pool, JobType::FullScan).await.unwrap();
        finish(&pool, id, JobState::Completed, &ScanCounters::default(), None)
            .await
            .unwrap();

        assert_eq!(reconcile_orphans(&pool).await.unwrap(), 0);
        assert_eq!(state_of(&pool, id).await.unwrap(), Some(JobState::Completed));
    }
}
