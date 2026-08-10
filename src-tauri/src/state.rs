//! Shared application state and the registry of running scan jobs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

use crate::error::{AppError, AppResult};
use crate::logging::DiagnosticBuffer;
use crate::scanner::{JobControl, ProgressSink, ScanProgress};

/// Event name the frontend subscribes to for scan progress.
pub const SCAN_PROGRESS_EVENT: &str = "scan://progress";

#[derive(Default)]
struct RegistryInner {
    active: Option<i64>,
    controls: HashMap<i64, JobControl>,
    latest: HashMap<i64, ScanProgress>,
}

/// Tracks the running scan and the last progress snapshot for each job.
///
/// One scan at a time: concurrent scans of overlapping roots would race on the
/// same archive rows for no user benefit.
#[derive(Default)]
pub struct JobRegistry {
    inner: Mutex<RegistryInner>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin(&self, job_id: i64, control: JobControl) -> AppResult<()> {
        let mut inner = self.lock();

        if let Some(active) = inner.active {
            return Err(AppError::user(
                "A scan is already running",
                format!("Scan #{active} must finish or be cancelled before another can start."),
            ));
        }

        inner.active = Some(job_id);
        inner.controls.insert(job_id, control);
        Ok(())
    }

    pub fn finish(&self, job_id: i64) {
        let mut inner = self.lock();
        inner.controls.remove(&job_id);
        if inner.active == Some(job_id) {
            inner.active = None;
        }
    }

    pub fn record(&self, progress: ScanProgress) {
        let mut inner = self.lock();
        inner.latest.insert(progress.job_id, progress);

        // Keep the map from growing without bound across many scans.
        if inner.latest.len() > 32 {
            let active = inner.active;
            let mut ids: Vec<i64> = inner.latest.keys().copied().collect();
            ids.sort_unstable();
            for id in ids.into_iter().take(16) {
                if Some(id) != active {
                    inner.latest.remove(&id);
                }
            }
        }
    }

    pub fn control(&self, job_id: i64) -> Option<JobControl> {
        self.lock().controls.get(&job_id).cloned()
    }

    pub fn active_job_id(&self) -> Option<i64> {
        self.lock().active
    }

    pub fn latest(&self, job_id: i64) -> Option<ScanProgress> {
        self.lock().latest.get(&job_id).cloned()
    }

    pub fn latest_active(&self) -> Option<ScanProgress> {
        let inner = self.lock();
        inner
            .active
            .and_then(|id| inner.latest.get(&id))
            .cloned()
    }

    /// Recovers rather than propagates a poisoned lock: a panic in an unrelated
    /// job must not make scan control permanently unusable.
    fn lock(&self) -> std::sync::MutexGuard<'_, RegistryInner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct AppState {
    pub pool: SqlitePool,
    pub jobs: Arc<JobRegistry>,
    pub diagnostics: DiagnosticBuffer,
    pub app_data_dir: PathBuf,
    pub log_dir: PathBuf,
}

/// Forwards progress to the webview and keeps the registry's snapshot current.
pub struct TauriProgressSink {
    app: AppHandle,
    jobs: Arc<JobRegistry>,
}

impl TauriProgressSink {
    pub fn new(app: AppHandle, jobs: Arc<JobRegistry>) -> Self {
        Self { app, jobs }
    }
}

impl ProgressSink for TauriProgressSink {
    fn emit(&self, progress: &ScanProgress) {
        self.jobs.record(progress.clone());
        if let Err(error) = self.app.emit(SCAN_PROGRESS_EVENT, progress) {
            tracing::warn!(%error, "could not deliver scan progress to the interface");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{JobState, ScanPhase};
    use crate::scanner::ScanCounters;

    fn progress(job_id: i64) -> ScanProgress {
        ScanProgress {
            job_id,
            state: JobState::Running,
            phase: ScanPhase::InspectingArchives,
            counters: ScanCounters::default(),
            current_file: None,
        }
    }

    #[test]
    fn only_one_scan_may_run_at_a_time() {
        let registry = JobRegistry::new();
        registry.begin(1, JobControl::new()).unwrap();

        assert!(registry.begin(2, JobControl::new()).is_err());

        registry.finish(1);
        assert!(registry.begin(2, JobControl::new()).is_ok());
    }

    #[test]
    fn control_handles_are_available_while_a_job_runs() {
        let registry = JobRegistry::new();
        let control = JobControl::new();
        registry.begin(7, control).unwrap();

        registry.control(7).unwrap().cancel();
        assert!(registry.control(7).unwrap().is_cancelled());

        registry.finish(7);
        assert!(registry.control(7).is_none());
        assert_eq!(registry.active_job_id(), None);
    }

    #[test]
    fn the_active_job_snapshot_is_retained() {
        let registry = JobRegistry::new();
        registry.begin(3, JobControl::new()).unwrap();
        registry.record(progress(3));

        assert_eq!(registry.latest_active().unwrap().job_id, 3);

        for id in 100..200 {
            registry.record(progress(id));
        }
        assert!(registry.latest_active().is_some(), "active job must survive pruning");
    }
}
