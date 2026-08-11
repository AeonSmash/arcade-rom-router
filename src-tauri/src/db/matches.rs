use sqlx::{Row, SqlitePool};

use crate::db::now_iso8601;
use crate::error::{AppError, AppResult};
use crate::model::{
    CompatibilityState, MatchConfidence, MatchResultRow, ProblemGameRow, ProblemGroup,
    ProblemSummary,
};

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

    let evidence_json: String = row.get("evidence_json");
    let missing_chips = parse_missing_chips(&evidence_json);
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
        evidence_json,
        created_at: row.get("created_at"),
        machine: None,
        profile_display_name: row.try_get("profile_display_name").ok(),
        missing_chips,
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

fn is_launchable_state(state: &str) -> bool {
    matches!(
        state,
        "VERIFIED_PLAYABLE"
            | "VERIFIED_PLAYABLE_WITH_DEPENDENCIES"
            | "PLAYABLE_WITH_AUDIO_SAMPLE_WARNING"
            | "MULTIPLE_VALID_ROUTES"
    )
}

fn parse_missing_chips(evidence_json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(evidence_json) else {
        return Vec::new();
    };
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|e| {
            let ty = e.get("type")?.as_str()?;
            if matches!(ty, "rom-missing" | "rom-missing-crc" | "rom-wrong-revision") {
                e.get("name")?.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect()
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
        playable_on_other_emulator: 0,
        no_working_emulator: 0,
        wrong_rom_revision: 0,
    };

    for row in rows {
        let state = row.get::<String, _>("state");
        let n: i64 = row.get("n");
        match CompatibilityState::parse(&state) {
            Some(CompatibilityState::MissingParent) => summary.missing_parent = n,
            Some(CompatibilityState::MissingBios) => summary.missing_bios = n,
            Some(CompatibilityState::MissingDevice) => summary.missing_device = n,
            Some(CompatibilityState::MissingChd) => summary.missing_chd = n,
            Some(CompatibilityState::IncompleteSet) => summary.incomplete_set += n,
            Some(CompatibilityState::WrongRomRevision) => summary.wrong_rom_revision += n,
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

    // Preferred profile (by global preference weights) cannot launch, but another can.
    // Preference order mirrors routing BALANCED defaults: fbneo > mame_current >
    // mame2003plus > mame2010 > everything else.
    let playable_elsewhere: i64 = sqlx::query_scalar(
        "WITH ranked AS (
             SELECT archive_id, emulator_profile_id, state,
                    CASE emulator_profile_id
                        WHEN 'fbneo' THEN 100
                        WHEN 'mame_current' THEN 80
                        WHEN 'mame2003plus' THEN 70
                        WHEN 'mame2003' THEN 65
                        WHEN 'mame2010' THEN 60
                        WHEN 'mame2015' THEN 50
                        WHEN 'mame2016' THEN 45
                        ELSE 40
                    END AS pref
             FROM match_results
         ),
         preferred AS (
             SELECT archive_id, state, pref,
                    ROW_NUMBER() OVER (PARTITION BY archive_id ORDER BY pref DESC) AS rn
             FROM ranked
         )
         SELECT COUNT(*) FROM preferred p
         WHERE p.rn = 1
           AND p.state NOT IN (
               'VERIFIED_PLAYABLE',
               'VERIFIED_PLAYABLE_WITH_DEPENDENCIES',
               'PLAYABLE_WITH_AUDIO_SAMPLE_WARNING',
               'MULTIPLE_VALID_ROUTES'
           )
           AND EXISTS (
               SELECT 1 FROM match_results m
               WHERE m.archive_id = p.archive_id
                 AND m.state IN (
                     'VERIFIED_PLAYABLE',
                     'VERIFIED_PLAYABLE_WITH_DEPENDENCIES',
                     'PLAYABLE_WITH_AUDIO_SAMPLE_WARNING',
                     'MULTIPLE_VALID_ROUTES'
                 )
           )",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    summary.playable_on_other_emulator = playable_elsewhere;

    // Archives with at least one match, none of which are launchable.
    let no_working: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT m.archive_id) FROM match_results m
         WHERE NOT EXISTS (
             SELECT 1 FROM match_results m2
             WHERE m2.archive_id = m.archive_id
               AND m2.state IN (
                   'VERIFIED_PLAYABLE',
                   'VERIFIED_PLAYABLE_WITH_DEPENDENCIES',
                   'PLAYABLE_WITH_AUDIO_SAMPLE_WARNING',
                   'MULTIPLE_VALID_ROUTES'
               )
         )",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    summary.no_working_emulator = no_working;

    Ok(summary)
}

fn group_states(group: ProblemGroup) -> &'static [&'static str] {
    match group {
        ProblemGroup::MissingParent => &["MISSING_PARENT"],
        ProblemGroup::MissingBios => &["MISSING_BIOS"],
        ProblemGroup::MissingDevice => &["MISSING_DEVICE"],
        ProblemGroup::MissingChd => &["MISSING_CHD"],
        ProblemGroup::IncompleteSet => &["INCOMPLETE_SET"],
        ProblemGroup::WrongRomRevision => &["WRONG_ROM_REVISION"],
        ProblemGroup::Unidentified => &[
            "UNIDENTIFIED",
            "KNOWN_SET_NAME_UNVERIFIED_CONTENT",
            "RECOGNIZED_ROM_CONTENT_AMBIGUOUS_SET",
        ],
        ProblemGroup::Unreadable => &["ARCHIVE_UNREADABLE"],
        ProblemGroup::CoreNotInstalled => &["CORE_NOT_INSTALLED", "EMULATOR_NOT_INSTALLED"],
        ProblemGroup::DatNotInstalled => &["DAT_NOT_INSTALLED"],
        ProblemGroup::PlayableOnOtherEmulator | ProblemGroup::NoWorkingEmulator => &[],
    }
}

pub async fn list_problem_games(
    pool: &SqlitePool,
    group: ProblemGroup,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<ProblemGameRow>> {
    let limit = limit.clamp(1, 500);
    let offset = offset.max(0);

    let rows = match group {
        ProblemGroup::PlayableOnOtherEmulator => {
            sqlx::query(
                "WITH ranked AS (
                     SELECT mr.*,
                            CASE mr.emulator_profile_id
                                WHEN 'fbneo' THEN 100
                                WHEN 'mame_current' THEN 80
                                WHEN 'mame2003plus' THEN 70
                                WHEN 'mame2003' THEN 65
                                WHEN 'mame2010' THEN 60
                                WHEN 'mame2015' THEN 50
                                WHEN 'mame2016' THEN 45
                                ELSE 40
                            END AS pref
                     FROM match_results mr
                 ),
                 preferred AS (
                     SELECT *,
                            ROW_NUMBER() OVER (
                                PARTITION BY archive_id ORDER BY pref DESC, id
                            ) AS rn
                     FROM ranked
                 )
                 SELECT b.id AS match_result_id, b.archive_id, a.file_name,
                        b.state, b.emulator_profile_id, b.missing_required,
                        b.matched_required, b.wrong_required, b.evidence_json,
                        p.display_name AS profile_display_name,
                        m.set_name
                 FROM preferred b
                 JOIN archives a ON a.id = b.archive_id
                 LEFT JOIN emulator_profiles p ON p.id = b.emulator_profile_id
                 LEFT JOIN machines m ON m.id = b.machine_id
                 WHERE b.rn = 1
                   AND b.state NOT IN (
                       'VERIFIED_PLAYABLE',
                       'VERIFIED_PLAYABLE_WITH_DEPENDENCIES',
                       'PLAYABLE_WITH_AUDIO_SAMPLE_WARNING',
                       'MULTIPLE_VALID_ROUTES'
                   )
                   AND EXISTS (
                       SELECT 1 FROM match_results m2
                       WHERE m2.archive_id = b.archive_id
                         AND m2.state IN (
                             'VERIFIED_PLAYABLE',
                             'VERIFIED_PLAYABLE_WITH_DEPENDENCIES',
                             'PLAYABLE_WITH_AUDIO_SAMPLE_WARNING',
                             'MULTIPLE_VALID_ROUTES'
                         )
                   )
                 ORDER BY a.file_name
                 LIMIT ?1 OFFSET ?2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        ProblemGroup::NoWorkingEmulator => {
            // Closest exact-name DAT (fewest missing chips), not highest score.
            sqlx::query(
                "WITH ranked AS (
                     SELECT mr.*,
                            a.file_name AS archive_file_name,
                            m.set_name AS machine_set_name,
                            ROW_NUMBER() OVER (
                                PARTITION BY mr.archive_id
                                ORDER BY
                                    CASE
                                        WHEN lower(m.set_name) = lower(
                                            replace(replace(a.file_name, '.zip', ''), '.7z', '')
                                        ) THEN 0
                                        ELSE 1
                                    END,
                                    mr.missing_required ASC,
                                    mr.matched_required DESC,
                                    mr.score DESC,
                                    mr.id
                            ) AS rn
                     FROM match_results mr
                     JOIN archives a ON a.id = mr.archive_id
                     LEFT JOIN machines m ON m.id = mr.machine_id
                 )
                 SELECT b.id AS match_result_id, b.archive_id, b.archive_file_name AS file_name,
                        b.state, b.emulator_profile_id, b.missing_required,
                        b.matched_required, b.wrong_required, b.evidence_json,
                        p.display_name AS profile_display_name,
                        b.machine_set_name AS set_name
                 FROM ranked b
                 LEFT JOIN emulator_profiles p ON p.id = b.emulator_profile_id
                 WHERE b.rn = 1
                   AND NOT EXISTS (
                       SELECT 1 FROM match_results m2
                       WHERE m2.archive_id = b.archive_id
                         AND m2.state IN (
                             'VERIFIED_PLAYABLE',
                             'VERIFIED_PLAYABLE_WITH_DEPENDENCIES',
                             'PLAYABLE_WITH_AUDIO_SAMPLE_WARNING',
                             'MULTIPLE_VALID_ROUTES'
                         )
                   )
                 ORDER BY b.archive_file_name
                 LIMIT ?1 OFFSET ?2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        ProblemGroup::Unreadable => {
            sqlx::query(
                "SELECT 0 AS match_result_id, a.id AS archive_id, a.file_name,
                        'ARCHIVE_UNREADABLE' AS state, '' AS emulator_profile_id,
                        0 AS missing_required, 0 AS matched_required, 0 AS wrong_required,
                        '[]' AS evidence_json, NULL AS profile_display_name,
                        NULL AS set_name
                 FROM archives a
                 WHERE a.archive_state = 'ARCHIVE_UNREADABLE'
                 ORDER BY a.file_name
                 LIMIT ?1 OFFSET ?2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?
        }
        other => {
            let states = group_states(other);
            if states.is_empty() {
                return Err(AppError::internal("Unknown problem group"));
            }
            let placeholders = states
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", i + 3))
                .collect::<Vec<_>>()
                .join(",");
            // Audited: placeholders are fixed integers from our state list.
            let sql = format!(
                "WITH best AS (
                     SELECT mr.*,
                            ROW_NUMBER() OVER (
                                PARTITION BY mr.archive_id ORDER BY mr.score DESC, mr.id
                            ) AS rn
                     FROM match_results mr
                 )
                 SELECT b.id AS match_result_id, b.archive_id, a.file_name,
                        b.state, b.emulator_profile_id, b.missing_required,
                        b.matched_required, b.wrong_required, b.evidence_json,
                        p.display_name AS profile_display_name,
                        m.set_name
                 FROM best b
                 JOIN archives a ON a.id = b.archive_id
                 LEFT JOIN emulator_profiles p ON p.id = b.emulator_profile_id
                 LEFT JOIN machines m ON m.id = b.machine_id
                 WHERE b.rn = 1 AND b.state IN ({placeholders})
                 ORDER BY a.file_name
                 LIMIT ?1 OFFSET ?2"
            );
            let mut query = sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(limit)
                .bind(offset);
            for state in states {
                query = query.bind(*state);
            }
            query.fetch_all(pool).await?
        }
    };

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let archive_id: i64 = row.get("archive_id");
        let evidence: String = row.get("evidence_json");
        let missing_chips = parse_missing_chips(&evidence);
        let missing_required: i64 = row.get("missing_required");
        let matched_required: i64 = row.get("matched_required");
        let wrong_required: i64 = row.get("wrong_required");
        let required_count = matched_required + missing_required + wrong_required;
        let state_str: String = row.get("state");
        let state =
            CompatibilityState::parse(&state_str).unwrap_or(CompatibilityState::Unidentified);

        let sibling_rows = sqlx::query(
            "SELECT emulator_profile_id, state FROM match_results WHERE archive_id = ?1",
        )
        .bind(archive_id)
        .fetch_all(pool)
        .await?;

        let mut works_on_profiles = Vec::new();
        for sib in &sibling_rows {
            let st: String = sib.get("state");
            if is_launchable_state(&st) {
                works_on_profiles.push(sib.get::<String, _>("emulator_profile_id"));
            }
        }
        works_on_profiles.sort();
        works_on_profiles.dedup();

        let suggestion = if let Some(p) = works_on_profiles.first() {
            Some(format!("Use {p}"))
        } else if missing_required > 0 {
            let profile = row
                .try_get::<Option<String>, _>("profile_display_name")
                .ok()
                .flatten()
                .unwrap_or_else(|| row.get::<String, _>("emulator_profile_id"));
            Some(format!(
                "Closest DAT: {profile} {matched_required}/{required_count} — set is incomplete on every installed emulator"
            ))
        } else {
            Some("No installed DAT can run this archive".into())
        };

        out.push(ProblemGameRow {
            archive_id,
            file_name: row.get("file_name"),
            set_name: row.try_get("set_name").ok(),
            state,
            emulator_profile_id: row.get("emulator_profile_id"),
            profile_display_name: row.try_get("profile_display_name").ok(),
            missing_count: missing_required,
            required_count,
            missing_chips,
            works_on_profiles,
            suggestion,
            match_result_id: row.get("match_result_id"),
        });
    }

    Ok(out)
}
