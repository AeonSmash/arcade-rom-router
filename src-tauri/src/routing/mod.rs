//! Route selection (Phase 6).
//!
//! Libretro arcade cores identify the romset by archive filename. A match
//! against a differently-named DAT machine is never a valid launch route,
//! even if its CRC overlap looks “complete”.

use sqlx::SqlitePool;

use crate::db::{archives, matches as matches_db, profiles, routes, settings};
use crate::error::AppResult;
use crate::matcher::normalize_set_name;
use crate::model::{CompatibilityState, MatchResultRow, RoutePreferenceMode};

fn is_launchable_state(state: CompatibilityState) -> bool {
    matches!(
        state,
        CompatibilityState::VerifiedPlayable
            | CompatibilityState::VerifiedPlayableWithDependencies
            | CompatibilityState::PlayableWithAudioSampleWarning
            | CompatibilityState::MultipleValidRoutes
    )
}

/// Launch requires a verified state AND the DAT set name to equal the zip stem.
fn is_launchable_route(m: &MatchResultRow, archive_stem: &str) -> bool {
    if !is_launchable_state(m.state) {
        return false;
    }
    m.machine
        .as_ref()
        .is_some_and(|machine| machine.set_name.eq_ignore_ascii_case(archive_stem))
}

fn preference_weight(mode: RoutePreferenceMode, profile_id: &str) -> i64 {
    match mode {
        RoutePreferenceMode::Balanced => match profile_id {
            "fbneo" => 100,
            "mame_current" => 80,
            "mame2003plus" => 70,
            "mame2010" => 60,
            _ => 40,
        },
        RoutePreferenceMode::MaximumLegacy => match profile_id {
            "mame2003plus" | "mame2003" => 100,
            "mame2010" => 80,
            "fbneo" => 70,
            _ => 40,
        },
        RoutePreferenceMode::Preservation => match profile_id {
            "mame_current" => 100,
            "mame2016" | "mame2015" => 80,
            _ => 40,
        },
        RoutePreferenceMode::Performance => match profile_id {
            "fbneo" => 100,
            "mame2003plus" => 90,
            "mame2010" => 70,
            _ => 40,
        },
    }
}

async fn preference_mode(pool: &SqlitePool) -> RoutePreferenceMode {
    let raw: String = settings::get_or(
        pool,
        "routing.preferenceMode",
        "BALANCED".to_string(),
    )
    .await;
    RoutePreferenceMode::parse(&raw).unwrap_or(RoutePreferenceMode::Balanced)
}

fn skipped_preference_note(
    mode: RoutePreferenceMode,
    all_matches: &[MatchResultRow],
    selected_profile: &str,
    archive_stem: &str,
) -> Option<String> {
    let mut best: Option<(&MatchResultRow, i64)> = None;
    for m in all_matches {
        if is_launchable_route(m, archive_stem) {
            continue;
        }
        if m.emulator_profile_id == selected_profile {
            continue;
        }
        let w = preference_weight(mode, &m.emulator_profile_id);
        let selected_w = preference_weight(mode, selected_profile);
        if w <= selected_w {
            continue;
        }
        let replace = match best {
            None => true,
            Some((_, prev_w)) => w > prev_w,
        };
        if replace {
            best = Some((m, w));
        }
    }
    best.map(|(m, _)| {
        let name_mismatch = m
            .machine
            .as_ref()
            .is_some_and(|machine| !machine.set_name.eq_ignore_ascii_case(archive_stem));
        let detail = if name_mismatch {
            format!(
                " (matched {} ≠ {})",
                m.machine.as_ref().map(|x| x.set_name.as_str()).unwrap_or("?"),
                archive_stem
            )
        } else if m.missing_required > 0 {
            format!(", {} chips missing", m.missing_required)
        } else {
            format!(" ({})", m.state.as_str())
        };
        format!("{} skipped{detail}", m.emulator_profile_id)
    })
}

/// Pick the best non-launchable row to surface in the UI when nothing can run.
fn best_blocked_match<'a>(
    match_rows: &'a [MatchResultRow],
    archive_stem: &str,
) -> Option<&'a MatchResultRow> {
    // Prefer an exact-name match (even if incomplete) over a wrong-name CRC hit.
    // Among exact names, fewest missing chips wins — score alone ties all incompletes.
    let exact: Vec<_> = match_rows
        .iter()
        .filter(|m| {
            m.machine
                .as_ref()
                .is_some_and(|machine| machine.set_name.eq_ignore_ascii_case(archive_stem))
        })
        .collect();
    if let Some(best) = exact.into_iter().max_by(|a, b| {
        b.missing_required
            .cmp(&a.missing_required)
            .then_with(|| a.matched_required.cmp(&b.matched_required))
            .then_with(|| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }) {
        return Some(best);
    }
    match_rows.first()
}

fn format_chip_list(chips: &[String], limit: usize) -> String {
    if chips.is_empty() {
        return String::new();
    }
    let shown: Vec<&str> = chips.iter().take(limit).map(|s| s.as_str()).collect();
    let mut text = shown.join(", ");
    if chips.len() > limit {
        text.push_str(&format!(", +{} more", chips.len() - limit));
    }
    text
}

/// Human-readable reason when no filename-correct verified route exists.
pub fn unplayable_reason(archive_stem: &str, match_rows: &[MatchResultRow]) -> String {
    let Some(best) = best_blocked_match(match_rows, archive_stem) else {
        return "Unplayable — no match against active DATs.".into();
    };

    let name_mismatch = best
        .machine
        .as_ref()
        .is_some_and(|m| !m.set_name.eq_ignore_ascii_case(archive_stem));
    if name_mismatch {
        return format!(
            "Unplayable: best CRC hit is “{}”, but the zip is “{}” (cores load by filename)",
            best.machine
                .as_ref()
                .map(|m| m.set_name.as_str())
                .unwrap_or("?"),
            archive_stem
        );
    }

    let profile = best
        .profile_display_name
        .as_deref()
        .unwrap_or(best.emulator_profile_id.as_str());
    let required = best.matched_required + best.missing_required + best.wrong_required;
    let chips = format_chip_list(&best.missing_chips, 6);

    match best.state {
        CompatibilityState::IncompleteSet if best.missing_required > 0 => {
            if chips.is_empty() {
                format!(
                    "Unplayable — no installed DAT has a complete set for “{archive_stem}”. Closest: {profile} {}/{required} required ({} missing).",
                    best.matched_required, best.missing_required
                )
            } else {
                format!(
                    "Unplayable — no installed DAT has a complete set for “{archive_stem}”. Closest: {profile} {}/{required} (missing: {chips}).",
                    best.matched_required
                )
            }
        }
        CompatibilityState::WrongRomRevision => {
            if chips.is_empty() {
                format!(
                    "Unplayable — wrong ROM revision for “{archive_stem}” on every installed DAT. Closest: {profile}."
                )
            } else {
                format!(
                    "Unplayable — wrong ROM revision for “{archive_stem}”. Closest: {profile} (bad/missing: {chips})."
                )
            }
        }
        CompatibilityState::MissingParent => {
            format!("Unplayable — matched “{archive_stem}” but a parent set is missing from the library.")
        }
        CompatibilityState::MissingBios => {
            format!("Unplayable — matched “{archive_stem}” but a required BIOS set is missing.")
        }
        CompatibilityState::MissingChd | CompatibilityState::MissingDevice => {
            format!(
                "Unplayable — matched “{archive_stem}” on {profile} but a required dependency is missing ({})",
                best.state.as_str()
            )
        }
        other => format!(
            "Unplayable — closest installed DAT is {profile} ({})",
            other.as_str()
        ),
    }
}

/// Rebuilds automatic routes for one archive, preserving user overrides only
/// when they still point at a filename-correct launchable match.
pub async fn rebuild_routes_for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<()> {
    let archive = archives::get(pool, archive_id).await?;
    let archive_stem = archive
        .as_ref()
        .map(|a| normalize_set_name(&a.file_name))
        .unwrap_or_default();

    let existing = routes::for_archive(pool, archive_id).await?;
    let override_route = existing.into_iter().find(|r| r.user_override);

    // Drop every prior route (including overrides); a still-valid override is
    // reinserted below from the captured match_result_id.
    routes::clear_for_archive(pool, archive_id).await?;

    let match_rows = matches_db::for_archive(pool, archive_id).await?;
    let mode = preference_mode(pool).await;

    let mut candidates: Vec<_> = match_rows
        .iter()
        .filter(|m| is_launchable_route(m, &archive_stem))
        .cloned()
        .collect();

    // Stale override that no longer names the zip correctly cannot stay launchable.
    let override_still_valid = override_route.as_ref().and_then(|over| {
        match_rows.iter().find(|m| m.id == over.match_result_id).and_then(|m| {
            if is_launchable_route(m, &archive_stem) {
                Some(m.clone())
            } else {
                None
            }
        })
    });

    if candidates.is_empty() {
        if let Some(best) = best_blocked_match(&match_rows, &archive_stem) {
            let reason = unplayable_reason(&archive_stem, &match_rows);
            routes::insert(
                pool,
                &routes::NewRoute {
                    archive_id,
                    machine_id: best.machine_id,
                    emulator_profile_id: best.emulator_profile_id.clone(),
                    match_result_id: best.id,
                    is_selected: true,
                    selection_reason: Some(reason),
                    user_override: false,
                    launchable: false,
                },
            )
            .await?;
            let _ = profiles::get(pool, &best.emulator_profile_id).await?;
        }
        return Ok(());
    }

    candidates.sort_by(|a, b| {
        let wa = preference_weight(mode, &a.emulator_profile_id);
        let wb = preference_weight(mode, &b.emulator_profile_id);
        wb.cmp(&wa)
            .then(b.matched_required.cmp(&a.matched_required))
            .then(
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then_with(|| a.emulator_profile_id.cmp(&b.emulator_profile_id))
    });

    let selected_match_id = override_still_valid
        .as_ref()
        .map(|m| m.id)
        .unwrap_or(candidates[0].id);

    for (index, candidate) in candidates.iter().enumerate() {
        let is_selected = candidate.id == selected_match_id;
        let is_override = override_still_valid
            .as_ref()
            .is_some_and(|m| m.id == candidate.id);
        let reason = if is_override {
            "User override".to_string()
        } else if index == 0 || candidate.id == selected_match_id {
            let mut base = format!("Recommended ({})", mode.as_str());
            if let Some(note) =
                skipped_preference_note(mode, &match_rows, &candidate.emulator_profile_id, &archive_stem)
            {
                base = format!("{base} — {note}");
            }
            base
        } else {
            "Alternate compatible route".into()
        };

        routes::insert(
            pool,
            &routes::NewRoute {
                archive_id,
                machine_id: candidate.machine_id,
                emulator_profile_id: candidate.emulator_profile_id.clone(),
                match_result_id: candidate.id,
                is_selected,
                selection_reason: Some(reason),
                user_override: is_override,
                launchable: true,
            },
        )
        .await?;
    }

    Ok(())
}

pub async fn choose_route(pool: &SqlitePool, archive_id: i64) -> AppResult<Option<crate::model::RouteRow>> {
    rebuild_routes_for_archive(pool, archive_id).await?;
    routes::selected_for_archive(pool, archive_id).await
}

/// Rebuild routes for every archive from existing match_results (no rematch).
/// Use after upgrading routing rules so wrong-emulator Play buttons clear
/// without re-scanning DAT content.
pub async fn rebuild_library_routes(pool: &SqlitePool) -> AppResult<u64> {
    let page = archives::page(
        pool,
        &archives::ArchiveQuery {
            limit: 100_000,
            favorites_only: false,
            ..Default::default()
        },
    )
    .await?;
    let mut n = 0u64;
    for archive in &page.rows {
        rebuild_routes_for_archive(pool, archive.id).await?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MatchConfidence, MachineSummary};

    fn row(
        profile: &str,
        state: CompatibilityState,
        set_name: &str,
        matched: i64,
        score: f64,
    ) -> MatchResultRow {
        MatchResultRow {
            id: matched,
            archive_id: 1,
            machine_id: matched,
            emulator_profile_id: profile.into(),
            dat_source_id: 1,
            state,
            confidence: MatchConfidence::Verified,
            matched_required: matched,
            missing_required: 0,
            wrong_required: 0,
            score,
            evidence_json: "[]".into(),
            created_at: String::new(),
            machine: Some(MachineSummary {
                id: matched,
                dat_source_id: 1,
                set_name: set_name.into(),
                description: None,
                year: None,
                manufacturer: None,
                clone_of: None,
                rom_of: None,
                is_bios: false,
                runnable: Some(true),
            }),
            profile_display_name: None,
            missing_chips: Vec::new(),
        }
    }

    #[test]
    fn closest_incomplete_prefers_fewest_missing() {
        let mut a = row(
            "fbneo",
            CompatibilityState::IncompleteSet,
            "1942",
            18,
            30.0,
        );
        a.missing_required = 9;
        a.missing_chips = vec!["a.bin".into()];
        let mut b = row(
            "mame2003plus",
            CompatibilityState::IncompleteSet,
            "1942",
            23,
            30.0,
        );
        b.missing_required = 4;
        b.profile_display_name = Some("MAME 2003-Plus".into());
        b.missing_chips = vec![
            "01d_sb-2.bin".into(),
            "01m_sb-9.bin".into(),
            "02d_sb-3.bin".into(),
            "k06_sb-1.bin".into(),
        ];
        let rows = vec![a, b];
        let best = best_blocked_match(&rows, "1942").unwrap();
        assert_eq!(best.emulator_profile_id, "mame2003plus");
        let reason = unplayable_reason("1942", &rows);
        assert!(reason.contains("Unplayable"));
        assert!(reason.contains("MAME 2003-Plus"));
        assert!(reason.contains("01d_sb-2.bin"));
    }

    #[test]
    fn wrong_set_name_is_never_launchable() {
        let m = row(
            "fbneo",
            CompatibilityState::VerifiedPlayable,
            "1943u",
            3,
            85.0,
        );
        assert!(!is_launchable_route(&m, "1943"));
        assert!(is_launchable_route(&m, "1943u"));
    }

    #[test]
    fn exact_name_verified_is_launchable() {
        let m = row(
            "mame2003plus",
            CompatibilityState::VerifiedPlayable,
            "1943",
            37,
            100.0,
        );
        assert!(is_launchable_route(&m, "1943"));
    }
}
