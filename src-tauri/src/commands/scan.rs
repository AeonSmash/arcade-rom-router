use std::sync::Arc;

use tauri::{AppHandle, State};
use tracing::{error, info};

use crate::db::settings::{self, keys};
use crate::db::{rom_roots, scan_jobs};
use crate::error::{AppError, AppResult};
use crate::model::{JobState, ScanMode};
use crate::scanner::{self, JobControl, ProgressSink, ScanCounters, ScanProgress};
use crate::state::{AppState, TauriProgressSink};

#[tauri::command]
pub async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: Option<ScanMode>,
    rom_root_ids: Option<Vec<i64>>,
) -> AppResult<i64> {
    // Rescan defaults to incremental (SPEC.md section 68).
    let mode = mode.unwrap_or(ScanMode::Quick);

    if state.jobs.active_job_id().is_some() {
        return Err(AppError::user(
            "A scan is already running",
            "Wait for the current scan to finish, or cancel it, before starting another.",
        ));
    }

    let all_roots = rom_roots::list(&state.pool).await?;
    let roots: Vec<_> = all_roots
        .into_iter()
        .filter(|root| root.enabled)
        .filter(|root| {
            rom_root_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(&root.id))
        })
        .collect();

    if roots.is_empty() {
        return Err(AppError::user(
            "No ROM folder to scan",
            "Add at least one enabled ROM folder before scanning.",
        ));
    }

    let worker_count = settings::get_or(
        &state.pool,
        keys::SCAN_WORKER_COUNT,
        scanner::default_worker_count(),
    )
    .await;

    let job_id = scan_jobs::create(&state.pool, mode.job_type()).await?;
    let control = JobControl::new();

    if let Err(error) = state.jobs.begin(job_id, control.clone()) {
        scan_jobs::finish(
            &state.pool,
            job_id,
            JobState::Failed,
            &ScanCounters::default(),
            Some(error.to_string()),
        )
        .await?;
        return Err(error);
    }

    scan_jobs::update_progress(
        &state.pool,
        job_id,
        JobState::Running,
        &ScanCounters::default(),
    )
    .await?;

    let pool = state.pool.clone();
    let jobs = state.jobs.clone();
    let sink: Arc<dyn ProgressSink> = Arc::new(TauriProgressSink::new(app, jobs.clone()));

    tauri::async_runtime::spawn(async move {
        let outcome = scanner::run_scan(
            &pool,
            &roots,
            mode,
            worker_count,
            job_id,
            control,
            sink.clone(),
        )
        .await;

        let (final_state, counters, detail) = match outcome {
            Ok((state, counters)) => (state, counters, None),
            Err(error) => {
                error!(job_id, %error, "scan failed");
                // Tell the interface the scan is over; otherwise it would show a
                // running scan that no longer exists.
                sink.emit(&ScanProgress {
                    job_id,
                    state: JobState::Failed,
                    phase: crate::model::ScanPhase::Finalizing,
                    counters: ScanCounters::default(),
                    current_file: None,
                });
                (
                    JobState::Failed,
                    ScanCounters::default(),
                    Some(error.to_string()),
                )
            }
        };

        if let Err(error) =
            scan_jobs::finish(&pool, job_id, final_state, &counters, detail).await
        {
            error!(job_id, %error, "could not record the final scan state");
        }

        jobs.finish(job_id);
    });

    info!(job_id, ?mode, "scan queued");

    Ok(job_id)
}

fn control_for(state: &AppState, job_id: i64) -> AppResult<JobControl> {
    state.jobs.control(job_id).ok_or_else(|| {
        AppError::user(
            "Scan not running",
            "That scan has already finished or was never started.",
        )
    })
}

#[tauri::command]
pub async fn cancel_scan(state: State<'_, AppState>, job_id: i64) -> AppResult<()> {
    control_for(&state, job_id)?.cancel();
    info!(job_id, "scan cancellation requested");
    Ok(())
}

#[tauri::command]
pub async fn pause_scan(state: State<'_, AppState>, job_id: i64) -> AppResult<()> {
    control_for(&state, job_id)?.pause();
    Ok(())
}

#[tauri::command]
pub async fn resume_scan(state: State<'_, AppState>, job_id: i64) -> AppResult<()> {
    control_for(&state, job_id)?.resume_work();
    Ok(())
}

#[tauri::command]
pub async fn get_scan_status(
    state: State<'_, AppState>,
    job_id: Option<i64>,
) -> AppResult<Option<ScanProgress>> {
    Ok(match job_id {
        Some(id) => state.jobs.latest(id),
        None => state.jobs.latest_active(),
    })
}
