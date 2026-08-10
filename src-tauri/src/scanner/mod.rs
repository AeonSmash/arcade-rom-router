//! The incremental, cancellable scan engine (SPEC.md section 12).
//!
//! A scan enumerates candidate files, skips anything whose quick signature is
//! unchanged, inspects the remainder through a bounded worker pool, and commits
//! results in batched transactions. Cancelling stops new work but keeps every
//! batch already committed, so partial results are always consistent.

pub mod enumerate;
pub mod signature;

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::archive::{self, fs_readonly};
use crate::db::archives::{self, ArchiveUpsert, ArchiveWithMembers};
use crate::db::rom_roots;
use crate::error::AppResult;
use crate::model::{ArchiveState, JobState, RomRoot, ScanMode, ScanPhase};
use enumerate::Candidate;
use signature::quick_signature;

/// Archives committed per transaction. Large enough to amortize the commit,
/// small enough that a cancelled scan loses little work.
pub const COMMIT_BATCH_SIZE: usize = 100;

/// Minimum spacing between progress events, so a fast scan of thousands of
/// files cannot flood the webview.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// SPEC.md section 30.3: leave a core for the rest of the system and never use
/// more than eight, so a scan does not make the machine unusable.
pub fn default_worker_count() -> usize {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    logical.saturating_sub(1).clamp(1, 8)
}

pub fn clamp_worker_count(requested: usize) -> usize {
    requested.clamp(1, 64)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCounters {
    pub total_candidates: u64,
    pub processed: u64,
    pub inspected: u64,
    pub reused_from_cache: u64,
    pub unreadable: u64,
    pub removed: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub job_id: i64,
    pub state: JobState,
    pub phase: ScanPhase,
    pub counters: ScanCounters,
    pub current_file: Option<String>,
}

/// Where progress events go. The application sends them to the webview; tests
/// collect them in memory.
pub trait ProgressSink: Send + Sync + 'static {
    fn emit(&self, progress: &ScanProgress);
}

pub struct NoopSink;

impl ProgressSink for NoopSink {
    fn emit(&self, _progress: &ScanProgress) {}
}

/// Cancellation and pause signalling for one running scan.
#[derive(Clone)]
pub struct JobControl {
    cancel: CancellationToken,
    paused: Arc<AtomicBool>,
    resume: Arc<Notify>,
}

impl Default for JobControl {
    fn default() -> Self {
        Self::new()
    }
}

impl JobControl {
    pub fn new() -> Self {
        Self {
            cancel: CancellationToken::new(),
            paused: Arc::new(AtomicBool::new(false)),
            resume: Arc::new(Notify::new()),
        }
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
        // A paused job must still be able to observe the cancellation.
        self.resume_work();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume_work(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.resume.notify_waiters();
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    async fn wait_while_paused(&self) {
        while self.is_paused() && !self.is_cancelled() {
            tokio::select! {
                _ = self.resume.notified() => {}
                _ = self.cancel.cancelled() => break,
            }
        }
    }
}

struct ProgressReporter {
    job_id: i64,
    sink: Arc<dyn ProgressSink>,
    counters: ScanCounters,
    phase: ScanPhase,
    current_file: Option<String>,
    last_emit: Instant,
}

impl ProgressReporter {
    fn new(job_id: i64, sink: Arc<dyn ProgressSink>) -> Self {
        Self {
            job_id,
            sink,
            counters: ScanCounters::default(),
            phase: ScanPhase::EnumeratingFiles,
            current_file: None,
            last_emit: Instant::now() - PROGRESS_INTERVAL,
        }
    }

    fn snapshot(&self, state: JobState) -> ScanProgress {
        ScanProgress {
            job_id: self.job_id,
            state,
            phase: self.phase,
            counters: self.counters,
            current_file: self.current_file.clone(),
        }
    }

    fn emit(&mut self, state: JobState) {
        self.last_emit = Instant::now();
        self.sink.emit(&self.snapshot(state));
    }

    fn emit_throttled(&mut self, state: JobState) {
        if self.last_emit.elapsed() >= PROGRESS_INTERVAL {
            self.emit(state);
        }
    }
}

struct InspectTask {
    candidate: Candidate,
    quick_signature: String,
    rom_root_id: i64,
    hash_whole_file: bool,
}

fn system_time_to_iso(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Blocking inspection of one file, run on the blocking thread pool.
fn inspect_blocking(task: InspectTask) -> ArchiveWithMembers {
    let InspectTask {
        candidate,
        quick_signature,
        rom_root_id,
        hash_whole_file,
    } = task;

    let inspected = archive::inspect(&candidate.path, &candidate.extension);

    let sha256 = if hash_whole_file {
        match fs_readonly::sha256_file(&candidate.path) {
            Ok(digest) => Some(digest),
            Err(error) => {
                warn!(path = %candidate.path.display(), %error, "deep verify hash failed");
                None
            }
        }
    } else {
        None
    };

    ArchiveWithMembers {
        archive: ArchiveUpsert {
            rom_root_id,
            path: candidate.path.display().to_string(),
            file_name: candidate.file_name,
            extension: candidate.extension,
            size_bytes: candidate.size_bytes as i64,
            modified_at: candidate.modified.map(system_time_to_iso),
            quick_signature,
            sha256,
            state: inspected.state,
            member_count: inspected.member_count(),
            unsafe_member_count: inspected.unsafe_member_count(),
            error_detail: inspected.error_detail,
        },
        members: inspected.members,
    }
}

/// Runs one scan to completion, cancellation, or failure.
pub async fn run_scan(
    pool: &SqlitePool,
    roots: &[RomRoot],
    mode: ScanMode,
    worker_count: usize,
    job_id: i64,
    control: JobControl,
    sink: Arc<dyn ProgressSink>,
) -> AppResult<(JobState, ScanCounters)> {
    let worker_count = clamp_worker_count(worker_count);
    let mut reporter = ProgressReporter::new(job_id, sink);

    info!(job_id, ?mode, worker_count, roots = roots.len(), "scan started");

    // Phase 1: enumerate every root before inspecting anything, so the UI can
    // show a real total rather than a moving target.
    reporter.phase = ScanPhase::EnumeratingFiles;
    reporter.emit(JobState::Running);

    let mut work: Vec<(RomRoot, Vec<Candidate>)> = Vec::with_capacity(roots.len());
    for root in roots {
        if control.is_cancelled() {
            break;
        }

        let found = enumerate::enumerate(Path::new(&root.path), root.recursive);
        for warning in &found.warnings {
            warn!(root = %root.path, warning, "enumeration warning");
        }

        reporter.counters.total_candidates += found.candidates.len() as u64;
        work.push((root.clone(), found.candidates));
        reporter.emit(JobState::Running);
    }

    // Phase 2: inspect.
    reporter.phase = ScanPhase::InspectingArchives;
    reporter.emit(JobState::Running);

    for (root, candidates) in work {
        if control.is_cancelled() {
            break;
        }

        let cache = if mode.uses_cache() {
            archives::signature_cache(pool, root.id).await?
        } else {
            Default::default()
        };

        let mut seen: HashSet<String> = HashSet::with_capacity(candidates.len());
        let mut cache_hits: Vec<String> = Vec::new();
        let mut queue: Vec<(Candidate, String)> = Vec::new();

        for candidate in candidates {
            let path = candidate.path.display().to_string();
            let signature =
                quick_signature(&path, candidate.size_bytes, candidate.modified);
            seen.insert(path.clone());

            if cache.get(&path).is_some_and(|cached| *cached == signature) {
                cache_hits.push(path);
            } else {
                queue.push((candidate, signature));
            }
        }

        debug!(
            root = %root.path,
            cached = cache_hits.len(),
            to_inspect = queue.len(),
            "cache comparison complete"
        );

        reporter.counters.reused_from_cache += cache_hits.len() as u64;
        reporter.counters.processed += cache_hits.len() as u64;
        archives::touch_scanned(pool, &cache_hits).await?;
        reporter.emit(JobState::Running);

        let mut pending = queue.into_iter();
        let mut inflight: JoinSet<ArchiveWithMembers> = JoinSet::new();
        let mut batch: Vec<ArchiveWithMembers> = Vec::with_capacity(COMMIT_BATCH_SIZE);

        for _ in 0..worker_count {
            let Some((candidate, quick_signature)) = pending.next() else {
                break;
            };
            inflight.spawn_blocking(move || {
                inspect_blocking(InspectTask {
                    candidate,
                    quick_signature,
                    rom_root_id: root.id,
                    hash_whole_file: mode.hashes_whole_file(),
                })
            });
        }

        while let Some(joined) = inflight.join_next().await {
            control.wait_while_paused().await;

            match joined {
                Ok(record) => {
                    if record.archive.state == ArchiveState::ArchiveUnreadable {
                        reporter.counters.unreadable += 1;
                        warn!(
                            path = %record.archive.path,
                            detail = record.archive.error_detail.as_deref().unwrap_or(""),
                            "archive could not be read"
                        );
                    }
                    reporter.counters.inspected += 1;
                    reporter.counters.processed += 1;
                    reporter.current_file = Some(record.archive.file_name.clone());
                    batch.push(record);
                }
                Err(error) => {
                    // A worker panicked. Count the file as processed so the
                    // scan can still finish rather than hanging on a total that
                    // is never reached.
                    error!(%error, "archive inspection task failed");
                    reporter.counters.processed += 1;
                }
            }

            if batch.len() >= COMMIT_BATCH_SIZE {
                archives::commit_batch(pool, &batch).await?;
                batch.clear();
                // New rows are now durable and queryable, so report immediately
                // rather than waiting for the throttle window.
                reporter.emit(JobState::Running);
            } else {
                reporter.emit_throttled(JobState::Running);
            }

            // Checked after reporting, so the last state the interface sees
            // reflects everything that was committed.
            if control.is_cancelled() {
                inflight.abort_all();
                while inflight.join_next().await.is_some() {}
                break;
            }

            if let Some((candidate, quick_signature)) = pending.next() {
                inflight.spawn_blocking(move || {
                    inspect_blocking(InspectTask {
                        candidate,
                        quick_signature,
                        rom_root_id: root.id,
                        hash_whole_file: mode.hashes_whole_file(),
                    })
                });
            }
        }

        // Whatever survived the loop is committed, including on cancellation.
        archives::commit_batch(pool, &batch).await?;

        if control.is_cancelled() {
            break;
        }

        // Pruning is only safe once a root has been walked in full; doing it
        // after a cancelled pass would delete records for files that simply
        // were not reached.
        reporter.counters.removed += archives::remove_absent(pool, root.id, &seen).await?;
        rom_roots::mark_scanned(pool, root.id).await?;
    }

    reporter.phase = ScanPhase::Finalizing;
    reporter.current_file = None;

    let final_state = if control.is_cancelled() {
        JobState::Cancelled
    } else {
        JobState::Completed
    };
    reporter.emit(final_state);

    info!(job_id, ?final_state, counters = ?reporter.counters, "scan finished");

    Ok((final_state, reporter.counters))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_count_leaves_headroom_and_is_capped() {
        let workers = default_worker_count();
        assert!((1..=8).contains(&workers));
    }

    #[test]
    fn requested_worker_counts_are_clamped_to_a_sane_range() {
        assert_eq!(clamp_worker_count(0), 1);
        assert_eq!(clamp_worker_count(4), 4);
        assert_eq!(clamp_worker_count(100_000), 64);
    }

    #[tokio::test]
    async fn pausing_blocks_until_resumed() {
        let control = JobControl::new();
        control.pause();
        assert!(control.is_paused());

        let waiter = {
            let control = control.clone();
            tokio::spawn(async move {
                control.wait_while_paused().await;
            })
        };

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());

        control.resume_work();
        tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("resume should release the waiter")
            .unwrap();
    }

    #[tokio::test]
    async fn cancelling_releases_a_paused_job() {
        let control = JobControl::new();
        control.pause();

        let waiter = {
            let control = control.clone();
            tokio::spawn(async move {
                control.wait_while_paused().await;
            })
        };

        control.cancel();
        tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("cancel should release the waiter")
            .unwrap();

        assert!(control.is_cancelled());
    }
}
