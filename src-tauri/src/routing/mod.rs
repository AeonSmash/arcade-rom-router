//! Route selection (Phase 6).

use sqlx::SqlitePool;

use crate::db::{matches as matches_db, profiles, routes, settings};
use crate::error::AppResult;
use crate::model::{CompatibilityState, RoutePreferenceMode};

fn is_launchable(state: CompatibilityState) -> bool {
    matches!(
        state,
        CompatibilityState::VerifiedPlayable
            | CompatibilityState::VerifiedPlayableWithDependencies
            | CompatibilityState::PlayableWithAudioSampleWarning
            | CompatibilityState::MultipleValidRoutes
    )
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

/// Rebuilds automatic routes for one archive, preserving user overrides.
pub async fn rebuild_routes_for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<()> {
    let existing = routes::for_archive(pool, archive_id).await?;
    let override_route = existing.into_iter().find(|r| r.user_override);

    routes::clear_for_archive(pool, archive_id).await?;

    let match_rows = matches_db::for_archive(pool, archive_id).await?;
    let mode = preference_mode(pool).await;

    let mut candidates: Vec<_> = match_rows
        .into_iter()
        .filter(|m| is_launchable(m.state))
        .collect();

    // Also keep non-launchable best matches as non-launchable route rows for UI.
    if candidates.is_empty() {
        if let Some(best) = matches_db::for_archive(pool, archive_id).await?.into_iter().next() {
            let profile = profiles::get(pool, &best.emulator_profile_id).await?;
            let reason = format!("Best match: {}", best.state.as_str());
            routes::insert(
                pool,
                &routes::NewRoute {
                    archive_id,
                    machine_id: best.machine_id,
                    emulator_profile_id: best.emulator_profile_id.clone(),
                    match_result_id: best.id,
                    is_selected: override_route.is_none(),
                    selection_reason: Some(reason),
                    user_override: false,
                    launchable: false,
                },
            )
            .await?;
            let _ = profile;
        }
        if let Some(over) = override_route {
            // Re-insert override if it still points at a surviving match.
            if let Some(m) = matches_db::for_archive(pool, archive_id)
                .await?
                .into_iter()
                .find(|m| m.id == over.match_result_id)
            {
                routes::insert(
                    pool,
                    &routes::NewRoute {
                        archive_id,
                        machine_id: m.machine_id,
                        emulator_profile_id: m.emulator_profile_id,
                        match_result_id: m.id,
                        is_selected: true,
                        selection_reason: Some("User override".into()),
                        user_override: true,
                        launchable: is_launchable(m.state),
                    },
                )
                .await?;
            }
        }
        return Ok(());
    }

    candidates.sort_by(|a, b| {
        let wa = preference_weight(mode, &a.emulator_profile_id);
        let wb = preference_weight(mode, &b.emulator_profile_id);
        wb.cmp(&wa)
            .then(
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then_with(|| a.emulator_profile_id.cmp(&b.emulator_profile_id))
    });

    if candidates.len() > 1 {
        // Annotate multi-route situation on the selected match conceptually;
        // each candidate remains its own route.
    }

    let selected_match_id = override_route
        .as_ref()
        .map(|r| r.match_result_id)
        .unwrap_or(candidates[0].id);

    for (index, candidate) in candidates.iter().enumerate() {
        let is_selected = candidate.id == selected_match_id;
        let reason = if override_route
            .as_ref()
            .is_some_and(|r| r.match_result_id == candidate.id)
        {
            "User override".to_string()
        } else if index == 0 {
            format!("Recommended ({})", mode.as_str())
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
                user_override: override_route
                    .as_ref()
                    .is_some_and(|r| r.match_result_id == candidate.id),
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
