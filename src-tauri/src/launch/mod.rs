//! Safe RetroArch launch (Phase 7 + hotkeys/save-state resume).
//!
//! Frontend may only send `{ archiveId, routeId, saveStateId? }`. Paths are
//! resolved here and passed as an argument array — never through a shell
//! (SPEC.md §20 / §43.2).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use sqlx::SqlitePool;
use tracing::info;

use crate::db::{now_iso8601, profiles, routes, save_states as save_states_db};
use crate::emulator::{hotkeys, savestates};
use crate::error::{AppError, AppResult};
use crate::model::LaunchResult;

pub async fn launch_game(
    pool: &SqlitePool,
    log_dir: &Path,
    app_data_dir: &Path,
    archive_id: i64,
    route_id: Option<i64>,
    save_state_id: Option<i64>,
) -> AppResult<LaunchResult> {
    let route = if let Some(id) = route_id {
        routes::get(pool, id)
            .await?
            .filter(|r| r.archive_id == archive_id)
            .ok_or_else(|| {
                AppError::user(
                    "Route not found",
                    "That route does not belong to this archive.",
                )
            })?
    } else {
        routes::selected_for_archive(pool, archive_id)
            .await?
            .ok_or_else(|| {
                AppError::user(
                    "No route selected",
                    "This archive has no selected emulator route yet. Import a DAT, rematch, and configure RetroArch first.",
                )
            })?
    };

    if !route.launchable && !route.user_override {
        return Err(AppError::user(
            "Not ready to launch",
            route
                .selection_reason
                .unwrap_or_else(|| "The selected route is not launchable.".into()),
        ));
    }

    let allow_unverified: bool =
        crate::db::settings::get_or(pool, "routing.allowUnverifiedLaunch", false).await;
    if !route.launchable && route.user_override && !allow_unverified {
        return Err(AppError::user(
            "Unverified launch blocked",
            "This route is a user override that is not verified as complete. Enable “Allow launch even if unverified” in settings to proceed.",
        ));
    }

    use sqlx::Row;
    let row = sqlx::query("SELECT path, file_name FROM archives WHERE id = ?1")
        .bind(archive_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            AppError::user(
                "Archive not found",
                "That archive is no longer in the library.",
            )
        })?;
    let content_path: String = row.get("path");
    let file_name: String = row.get("file_name");
    if !Path::new(&content_path).is_file() {
        return Err(AppError::user(
            "ROM file missing",
            format!("The archive is no longer at:\n{content_path}"),
        ));
    }

    // Cores load by zip filename — refuse routes whose DAT set name disagrees.
    let archive_stem = crate::matcher::normalize_set_name(&file_name);
    if let Some(set_name) = route.machine_set_name.as_deref() {
        if !set_name.eq_ignore_ascii_case(&archive_stem) && !allow_unverified {
            return Err(AppError::user(
                "Wrong set identity",
                format!(
                    "This route matched DAT set “{set_name}”, but the archive is “{archive_stem}.zip”. \
                     Arcade cores load by filename, so that launch would use the wrong (or incomplete) set. \
                     Rematch the library or pick a route whose set name matches the zip."
                ),
            ));
        }
    }

    let profile = profiles::get(pool, &route.emulator_profile_id)
        .await?
        .ok_or_else(|| {
            AppError::config("Profile missing", "The emulator profile no longer exists.")
        })?;

    let exe = profile.executable_path.ok_or_else(|| {
        AppError::user(
            "RetroArch not configured",
            "Set the RetroArch executable in Emulators before launching.",
        )
    })?;
    let core = profile.core_path.ok_or_else(|| {
        AppError::user(
            "Core not installed",
            format!(
                "No core DLL is configured for {}.",
                profile.display_name
            ),
        )
    })?;

    if !Path::new(&exe).is_file() {
        return Err(AppError::user(
            "RetroArch missing",
            format!("The RetroArch executable was not found:\n{exe}"),
        ));
    }
    if !Path::new(&core).is_file() {
        return Err(AppError::user(
            "Core missing",
            format!("The core library was not found:\n{core}"),
        ));
    }

    let mut entry_slot: Option<i64> = None;
    if let Some(state_id) = save_state_id {
        let state = save_states_db::get(pool, state_id)
            .await?
            .filter(|s| s.archive_id == archive_id)
            .ok_or_else(|| {
                AppError::user(
                    "Save state not found",
                    "That save state does not belong to this archive.",
                )
            })?;
        if !Path::new(&state.path).is_file() {
            return Err(AppError::user(
                "Save state missing",
                format!("The save state file is gone:\n{}", state.path),
            ));
        }
        if state.is_entry {
            entry_slot = Some(state.slot);
        } else {
            // Promote to entry state so -e N can load it.
            let _ = savestates::promote_to_entry(Path::new(&state.path), state.slot)?;
            entry_slot = Some(state.slot);
        }
    }

    std::fs::create_dir_all(log_dir).map_err(|source| AppError::Filesystem {
        path: log_dir.display().to_string(),
        source,
    })?;

    let started_at = now_iso8601();
    let log_path = log_dir.join(format!(
        "launch-{}-{}.log",
        archive_id,
        started_at.replace(':', "").replace('.', "")
    ));

    let appendconfig = hotkeys::appendconfig_for_launch(pool, app_data_dir).await?;

    // Argument array only — never shell concatenation.
    let mut command = std::process::Command::new(&exe);
    command.arg("-L").arg(&core).arg(&content_path);
    if let Some(fragment) = appendconfig.as_ref() {
        command.arg("--appendconfig").arg(fragment);
    }
    if let Some(slot) = entry_slot {
        command.arg("-e").arg(slot.to_string());
    }
    command
        .arg("--verbose")
        .arg("--log-file")
        .arg(&log_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = command.spawn().map_err(|source| AppError::Filesystem {
        path: exe.clone(),
        source,
    })?;

    let pid = child.id();

    let history_id: i64 = sqlx::query_scalar(
        "INSERT INTO play_history (archive_id, route_id, started_at, log_path)
         VALUES (?1, ?2, ?3, ?4)
         RETURNING id",
    )
    .bind(archive_id)
    .bind(route.id)
    .bind(&started_at)
    .bind(log_path.display().to_string())
    .fetch_one(pool)
    .await?;

    info!(
        archive_id,
        route_id = route.id,
        profile = %profile.id,
        pid,
        entry_slot,
        appendconfig = ?appendconfig.as_ref().map(PathBuf::as_path),
        "launch started"
    );

    std::mem::drop(child);

    Ok(LaunchResult {
        play_history_id: history_id,
        pid,
        started_at,
        core_path: core,
        content_path,
        log_path: Some(log_path.display().to_string()),
    })
}
