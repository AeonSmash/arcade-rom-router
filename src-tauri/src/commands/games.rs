use tauri::State;

use crate::db::{archives, favorites, matches as matches_db, routes};
use crate::error::{AppError, AppResult};
use crate::launch;
use crate::matcher::{self, normalize_set_name};
use crate::model::{
    GameDetail, LaunchResult, ProblemGameRow, ProblemGroup, ProblemSummary, RoutePreferenceMode,
    RouteRow,
};
use crate::routing;
use crate::state::AppState;

fn can_run_from(
    archive_file_name: &str,
    route: Option<&RouteRow>,
    matches: &[crate::model::MatchResultRow],
) -> (String, String) {
    let stem = normalize_set_name(archive_file_name);

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
        // Prefer a fresh multi-DAT explanation over a stale "Best match: …" reason.
        let reason = routing::unplayable_reason(&stem, matches);
        return ("NO".into(), reason);
    }

    if !matches.is_empty() {
        return ("NO".into(), routing::unplayable_reason(&stem, matches));
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
    let (mut can_run, mut can_run_reason) =
        can_run_from(&archive.file_name, selected.as_ref(), &matches);
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

    let is_favorite = favorites::is_favorite(&state.pool, archive_id).await?;

    Ok(GameDetail {
        archive,
        can_run,
        can_run_reason,
        selected_route: selected,
        routes: route_list,
        matches,
        members,
        dependencies,
        is_favorite,
    })
}

#[tauri::command]
pub async fn get_problem_summary(state: State<'_, AppState>) -> AppResult<ProblemSummary> {
    matches_db::problem_summary(&state.pool).await
}

#[tauri::command]
pub async fn list_problem_games(
    state: State<'_, AppState>,
    group: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Vec<ProblemGameRow>> {
    let group = ProblemGroup::parse(&group).ok_or_else(|| {
        AppError::user(
            "Unknown problem group",
            format!("“{group}” is not a recognized Problem Center category."),
        )
    })?;
    matches_db::list_problem_games(
        &state.pool,
        group,
        limit.unwrap_or(200),
        offset.unwrap_or(0),
    )
    .await
}

#[tauri::command]
pub async fn choose_route(state: State<'_, AppState>, archive_id: i64) -> AppResult<Option<RouteRow>> {
    routing::choose_route(&state.pool, archive_id).await
}

#[tauri::command]
pub async fn rebuild_library_routes(state: State<'_, AppState>) -> AppResult<u64> {
    routing::rebuild_library_routes(&state.pool).await
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
    save_state_id: Option<i64>,
) -> AppResult<LaunchResult> {
    let log_dir = state.log_dir.join("launches");
    launch::launch_game(
        &state.pool,
        &log_dir,
        &state.app_data_dir,
        archive_id,
        route_id,
        save_state_id,
    )
    .await
}
