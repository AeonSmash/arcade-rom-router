use tauri::State;

use crate::db::{archives, matches as matches_db, routes};
use crate::error::{AppError, AppResult};
use crate::launch;
use crate::matcher;
use crate::model::{
    CompatibilityState, GameDetail, LaunchResult, ProblemSummary, RoutePreferenceMode, RouteRow,
};
use crate::routing;
use crate::state::AppState;

fn can_run_from(route: Option<&RouteRow>, matches: &[crate::model::MatchResultRow]) -> (String, String) {
    if let Some(route) = route {
        if route.launchable {
            return (
                "YES".into(),
                route
                    .selection_reason
                    .clone()
                    .unwrap_or_else(|| "A verified route is ready.".into()),
            );
        }
        return (
            "NO".into(),
            route
                .selection_reason
                .clone()
                .unwrap_or_else(|| "The selected route is not launchable.".into()),
        );
    }

    if let Some(best) = matches.first() {
        let reason = match best.state {
            CompatibilityState::MissingParent => "Matched, but a parent set is missing.",
            CompatibilityState::MissingBios => "Matched, but a required BIOS set is missing.",
            CompatibilityState::MissingChd => "Matched, but a required CHD is missing.",
            CompatibilityState::IncompleteSet => "ROM contents are incomplete for the matched set.",
            CompatibilityState::CoreNotInstalled => {
                "Matched to a definition, but the emulator core is not installed."
            }
            CompatibilityState::DatNotInstalled => "No DAT is active for a matching profile.",
            CompatibilityState::Unidentified => "No matching machine definition was found.",
            other => other.as_str(),
        };
        let can = if matches!(
            best.state,
            CompatibilityState::IncompleteSet
                | CompatibilityState::MissingParent
                | CompatibilityState::MissingBios
                | CompatibilityState::MissingChd
                | CompatibilityState::CoreNotInstalled
        ) {
            "MAYBE"
        } else {
            "NO"
        };
        return (can.into(), reason.into());
    }

    (
        "NO".into(),
        "No match against active DATs. Import a DAT that matches this set's generation, or rematch after adding one.".into(),
    )
}

#[tauri::command]
pub async fn get_game_detail(state: State<'_, AppState>, archive_id: i64) -> AppResult<GameDetail> {
    let archive = archives::get(&state.pool, archive_id)
        .await?
        .ok_or_else(|| {
            AppError::user(
                "Archive not found",
                "That archive is no longer in the library.",
            )
        })?;

    let members = archives::members(&state.pool, archive_id).await?;
    let matches = matches_db::for_archive(&state.pool, archive_id).await?;
    let route_list = routes::for_archive(&state.pool, archive_id).await?;
    let selected = route_list.iter().find(|r| r.is_selected).cloned();
    let dependencies = matcher::dependencies_for_archive(&state.pool, archive_id).await?;
    let (mut can_run, mut can_run_reason) = can_run_from(selected.as_ref(), &matches);
    if matches.is_empty() && selected.is_none() {
        let active_dats: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM dat_sources WHERE active = 1")
                .fetch_one(&state.pool)
                .await
                .unwrap_or(0);
        if active_dats == 0 {
            can_run = "NO".into();
            can_run_reason = "No DAT imported yet. Open DATs, import a definition for your ROM set generation, then rematch.".into();
        }
    }

    Ok(GameDetail {
        archive,
        can_run,
        can_run_reason,
        selected_route: selected,
        routes: route_list,
        matches,
        members,
        dependencies,
    })
}

#[tauri::command]
pub async fn get_problem_summary(state: State<'_, AppState>) -> AppResult<ProblemSummary> {
    matches_db::problem_summary(&state.pool).await
}

#[tauri::command]
pub async fn choose_route(state: State<'_, AppState>, archive_id: i64) -> AppResult<Option<RouteRow>> {
    routing::choose_route(&state.pool, archive_id).await
}

#[tauri::command]
pub async fn set_game_route_override(
    state: State<'_, AppState>,
    archive_id: i64,
    route_id: Option<i64>,
) -> AppResult<()> {
    routes::set_override(&state.pool, archive_id, route_id).await
}

#[tauri::command]
pub async fn set_route_preference_mode(
    state: State<'_, AppState>,
    mode: RoutePreferenceMode,
) -> AppResult<()> {
    crate::db::settings::set(&state.pool, "routing.preferenceMode", &mode.as_str()).await
}

#[tauri::command]
pub async fn launch_game(
    state: State<'_, AppState>,
    archive_id: i64,
    route_id: Option<i64>,
) -> AppResult<LaunchResult> {
    let log_dir = state.log_dir.join("launches");
    launch::launch_game(&state.pool, &log_dir, archive_id, route_id).await
}
