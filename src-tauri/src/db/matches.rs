use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::AppResult;
use crate::model::{CompatibilityState, MatchConfidence, MatchResultRow, ProblemSummary};

pub struct NewMatchResult {
    pub archive_id: i64,
    pub machine_id: i64,
    pub emulator_profile_id: String,
    pub dat_source_id: i64,
    pub state: CompatibilityState,
    pub confidence: MatchConfidence,
    pub matched_required: i64,
    pub missing_required: i64,
    pub wrong_required: i64,
    pub score: f64,
    pub evidence_json: String,
}

pub async fn clear_for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM routes WHERE archive_id = ?1",
    )
    .bind(archive_id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM match_results WHERE archive_id = ?1")
        .bind(archive_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn clear_all(pool: &SqlitePool) -> AppResult<()> {
    sqlx::query("DELETE FROM routes").execute(pool).await?;
    sqlx::query("DELETE FROM match_results").execute(pool).await?;
    Ok(())
}

pub async fn insert(pool: &SqlitePool, row: &NewMatchResult) -> AppResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO match_results (
             archive_id, machine_id, emulator_profile_id, dat_source_id,
             state, confidence, matched_required, missing_required, wrong_required,
             score, evidence_json, created_at
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         RETURNING id",
    )
    .bind(row.archive_id)
    .bind(row.machine_id)
    .bind(&row.emulator_profile_id)
    .bind(row.dat_source_id)
    .bind(row.state.as_str())
    .bind(row.confidence.as_str())
    .bind(row.matched_required)
    .bind(row.missing_required)
    .bind(row.wrong_required)
    .bind(row.score)
    .bind(&row.evidence_json)
    .bind(now_iso8601())
    .fetch_one(pool)
    .await?;
    Ok(id)
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> MatchResultRow {
    let state = CompatibilityState::parse(&row.get::<String, _>("state"))
        .unwrap_or(CompatibilityState::Unidentified);
    let confidence = MatchConfidence::parse(&row.get::<String, _>("confidence"))
        .unwrap_or(MatchConfidence::Unknown);

    MatchResultRow {
        id: row.get("id"),
        archive_id: row.get("archive_id"),
        machine_id: row.get("machine_id"),
        emulator_profile_id: row.get("emulator_profile_id"),
        dat_source_id: row.get("dat_source_id"),
        state,
        confidence,
        matched_required: row.get("matched_required"),
        missing_required: row.get("missing_required"),
        wrong_required: row.get("wrong_required"),
        score: row.get("score"),
        evidence_json: row.get("evidence_json"),
        created_at: row.get("created_at"),
        machine: None,
        profile_display_name: row.try_get("profile_display_name").ok(),
    }
}

pub async fn for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<Vec<MatchResultRow>> {
    let rows = sqlx::query(
        "SELECT mr.*, p.display_name AS profile_display_name
         FROM match_results mr
         LEFT JOIN emulator_profiles p ON p.id = mr.emulator_profile_id
         WHERE mr.archive_id = ?1
         ORDER BY mr.score DESC, mr.id",
    )
    .bind(archive_id)
    .fetch_all(pool)
    .await?;

    let mut results: Vec<_> = rows.iter().map(map_row).collect();
    for result in &mut results {
        result.machine = crate::db::machines::get_summary(pool, result.machine_id).await?;
    }
    Ok(results)
}

pub async fn best_state_for_archive(
    pool: &SqlitePool,
    archive_id: i64,
) -> AppResult<Option<CompatibilityState>> {
    let state: Option<String> = sqlx::query_scalar(
        "SELECT state FROM match_results WHERE archive_id = ?1 ORDER BY score DESC LIMIT 1",
    )
    .bind(archive_id)
    .fetch_optional(pool)
    .await?;
    Ok(state.and_then(|s| CompatibilityState::parse(&s)))
}

pub async fn problem_summary(pool: &SqlitePool) -> AppResult<ProblemSummary> {
    // Count distinct archives by their best match state's problem class.
    let rows = sqlx::query(
        "WITH best AS (
             SELECT archive_id, state,
                    ROW_NUMBER() OVER (PARTITION BY archive_id ORDER BY score DESC, id) AS rn
             FROM match_results
         )
         SELECT state, COUNT(*) AS n FROM best WHERE rn = 1 GROUP BY state",
    )
    .fetch_all(pool)
    .await?;

    let mut summary = ProblemSummary {
        missing_parent: 0,
        missing_bios: 0,
        missing_device: 0,
        missing_chd: 0,
        incomplete_set: 0,
        unidentified: 0,
        unreadable: 0,
        core_not_installed: 0,
        dat_not_installed: 0,
    };

    for row in rows {
        let state = row.get::<String, _>("state");
        let n: i64 = row.get("n");
        match CompatibilityState::parse(&state) {
            Some(CompatibilityState::MissingParent) => summary.missing_parent = n,
            Some(CompatibilityState::MissingBios) => summary.missing_bios = n,
            Some(CompatibilityState::MissingDevice) => summary.missing_device = n,
            Some(CompatibilityState::MissingChd) => summary.missing_chd = n,
            Some(CompatibilityState::IncompleteSet) | Some(CompatibilityState::WrongRomRevision) => {
                summary.incomplete_set += n;
            }
            Some(CompatibilityState::Unidentified)
            | Some(CompatibilityState::KnownSetNameUnverifiedContent)
            | Some(CompatibilityState::RecognizedRomContentAmbiguousSet) => {
                summary.unidentified += n;
            }
            Some(CompatibilityState::ArchiveUnreadable) => summary.unreadable = n,
            Some(CompatibilityState::CoreNotInstalled)
            | Some(CompatibilityState::EmulatorNotInstalled) => {
                summary.core_not_installed += n;
            }
            Some(CompatibilityState::DatNotInstalled) => summary.dat_not_installed = n,
            _ => {}
        }
    }

    // Archives with no match at all and unreadable inventory states.
    let unmatched: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM archives a
         WHERE NOT EXISTS (SELECT 1 FROM match_results m WHERE m.archive_id = a.id)
           AND a.archive_state = 'INDEXED'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    summary.unidentified += unmatched;

    let unreadable_inv: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM archives WHERE archive_state = 'ARCHIVE_UNREADABLE'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    summary.unreadable = summary.unreadable.max(unreadable_inv);

    // Profiles that are enabled but have no active DAT cannot produce routes.
    // Count these even when no match_results rows exist yet (common first-run case).
    let profiles_missing_dat: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM emulator_profiles p
         WHERE p.enabled = 1
           AND NOT EXISTS (
               SELECT 1 FROM dat_sources d
               WHERE d.emulator_profile_id = p.id AND d.active = 1
           )",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    summary.dat_not_installed = summary.dat_not_installed.max(profiles_missing_dat);

    Ok(summary)
}
