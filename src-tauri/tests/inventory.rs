//! End-to-end tests for the Phase 1 inventory engine.
//!
//! These drive the real scan engine against synthetic ROM folders and assert on
//! what lands in the database, rather than on internal calls.

mod common;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use arcade_rom_router_lib::db::archives::{self, ArchiveQuery};
use arcade_rom_router_lib::db::{self, rom_roots};
use arcade_rom_router_lib::model::{ArchiveRow, ArchiveState, JobState, RomRoot, ScanMode};
use arcade_rom_router_lib::scanner::{
    self, JobControl, NoopSink, ProgressSink, ScanCounters, ScanProgress, COMMIT_BATCH_SIZE,
};
use sqlx::SqlitePool;
use tempfile::TempDir;

/// A temporary application database plus a temporary ROM folder.
struct Harness {
    _db_dir: TempDir,
    roms: TempDir,
    pool: SqlitePool,
    root: RomRoot,
}

impl Harness {
    async fn new() -> Self {
        let db_dir = tempfile::tempdir().unwrap();
        let roms = tempfile::tempdir().unwrap();

        let pool = db::connect(&db_dir.path().join("library.db")).await.unwrap();
        let root = rom_roots::insert(
            &pool,
            &roms.path().display().to_string(),
            Some("Test root"),
            true,
        )
        .await
        .unwrap();

        Self {
            _db_dir: db_dir,
            roms,
            pool,
            root,
        }
    }

    fn rom_dir(&self) -> &Path {
        self.roms.path()
    }

    async fn scan(&self, mode: ScanMode) -> (JobState, ScanCounters) {
        self.scan_with(mode, JobControl::new(), Arc::new(NoopSink))
            .await
    }

    async fn scan_with(
        &self,
        mode: ScanMode,
        control: JobControl,
        sink: Arc<dyn ProgressSink>,
    ) -> (JobState, ScanCounters) {
        scanner::run_scan(
            &self.pool,
            std::slice::from_ref(&self.root),
            mode,
            4,
            1,
            control,
            sink,
        )
        .await
        .unwrap()
    }

    async fn rows(&self) -> Vec<ArchiveRow> {
        archives::page(
            &self.pool,
            &ArchiveQuery {
                limit: 5_000,
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .rows
    }

    async fn row(&self, file_name: &str) -> ArchiveRow {
        self.rows()
            .await
            .into_iter()
            .find(|row| row.file_name == file_name)
            .unwrap_or_else(|| panic!("{file_name} is missing from the inventory"))
    }
}

#[tokio::test]
async fn a_scan_records_every_member_and_its_central_directory_crc() {
    let harness = Harness::new().await;
    let expected = common::write_rom_set(harness.rom_dir(), "1942.zip", 14);
    common::write_rom_set(harness.rom_dir(), "sf2.zip", 21);

    let (state, counters) = harness.scan(ScanMode::Full).await;

    assert_eq!(state, JobState::Completed);
    assert_eq!(counters.total_candidates, 2);
    assert_eq!(counters.inspected, 2);
    assert_eq!(counters.unreadable, 0);

    let row = harness.row("1942.zip").await;
    assert_eq!(row.archive_state, ArchiveState::Indexed);
    assert_eq!(row.member_count, 14);

    let stored = archives::members(&harness.pool, row.id).await.unwrap();
    let by_name: HashMap<_, _> = stored
        .iter()
        .map(|member| (member.member_name.as_str(), member))
        .collect();

    for member in &expected {
        let actual = by_name
            .get(member.name.as_str())
            .unwrap_or_else(|| panic!("member {} was not recorded", member.name));

        assert_eq!(actual.crc32.as_deref(), Some(member.crc32.as_str()));
        assert_eq!(actual.size_bytes, Some(member.size as i64));
        assert!(actual.name_is_safe);
    }
}

#[tokio::test]
async fn a_normal_scan_does_not_hash_whole_files() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "1942.zip", 3);

    harness.scan(ScanMode::Full).await;

    assert_eq!(harness.row("1942.zip").await.sha256, None);
}

#[tokio::test]
async fn deep_verify_records_a_hash_of_each_file() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "1942.zip", 3);

    harness.scan(ScanMode::DeepVerify).await;

    let sha256 = harness.row("1942.zip").await.sha256.unwrap();
    assert_eq!(sha256.len(), 64);
}

#[tokio::test]
async fn a_damaged_archive_is_reported_without_aborting_the_scan() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "good-a.zip", 4);
    common::write_damaged_zip(&harness.rom_dir().join("broken.zip"));
    common::write_truncated_zip(&harness.rom_dir().join("truncated.zip"));
    common::write_rom_set(harness.rom_dir(), "good-b.zip", 6);

    let (state, counters) = harness.scan(ScanMode::Full).await;

    assert_eq!(state, JobState::Completed);
    assert_eq!(counters.total_candidates, 4);
    assert_eq!(counters.unreadable, 2);

    for name in ["good-a.zip", "good-b.zip"] {
        assert_eq!(harness.row(name).await.archive_state, ArchiveState::Indexed);
    }

    let broken = harness.row("broken.zip").await;
    assert_eq!(broken.archive_state, ArchiveState::ArchiveUnreadable);
    assert_eq!(broken.member_count, 0);
    assert!(
        broken.error_detail.is_some(),
        "the parse error must be kept for diagnostics"
    );
}

#[tokio::test]
async fn member_names_that_escape_the_archive_are_recorded_and_flagged() {
    let harness = Harness::new().await;
    common::write_zip_with_traversal_names(&harness.rom_dir().join("hostile.zip"));

    harness.scan(ScanMode::Full).await;

    let row = harness.row("hostile.zip").await;
    assert_eq!(row.archive_state, ArchiveState::Indexed);
    assert_eq!(row.member_count, 3);
    assert_eq!(row.unsafe_member_count, 2);

    let members = archives::members(&harness.pool, row.id).await.unwrap();
    let unsafe_names: Vec<&str> = members
        .iter()
        .filter(|member| !member.name_is_safe)
        .map(|member| member.member_name.as_str())
        .collect();

    assert_eq!(unsafe_names.len(), 2);
    assert!(unsafe_names.iter().all(|name| name.contains("..")));
}

#[tokio::test]
async fn disk_images_are_indexed_without_being_hashed() {
    let harness = Harness::new().await;
    common::write_chd(&harness.rom_dir().join("area51").join("area51.chd"), 8192);

    let (_, counters) = harness.scan(ScanMode::Full).await;
    assert_eq!(counters.total_candidates, 1);

    let row = harness.row("area51.chd").await;
    assert_eq!(row.archive_state, ArchiveState::DiskImageIndexed);
    assert_eq!(row.member_count, 0);
    assert_eq!(row.sha256, None);
    assert_eq!(row.size_bytes, 8192);
}

#[tokio::test]
async fn non_rom_files_are_never_inventoried() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "1942.zip", 2);
    std::fs::write(harness.rom_dir().join("readme.txt"), b"notes").unwrap();
    std::fs::write(harness.rom_dir().join("setup.exe"), b"MZ").unwrap();
    std::fs::write(harness.rom_dir().join("run.bat"), b"@echo off").unwrap();

    let (_, counters) = harness.scan(ScanMode::Full).await;

    assert_eq!(counters.total_candidates, 1);
    assert_eq!(harness.rows().await.len(), 1);
}

#[tokio::test]
async fn an_incremental_rescan_reuses_unchanged_archives() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "1942.zip", 5);
    common::write_rom_set(harness.rom_dir(), "sf2.zip", 5);

    let (_, first) = harness.scan(ScanMode::Full).await;
    assert_eq!(first.inspected, 2);
    assert_eq!(first.reused_from_cache, 0);

    let (_, second) = harness.scan(ScanMode::Quick).await;
    assert_eq!(second.inspected, 0);
    assert_eq!(second.reused_from_cache, 2);
    assert_eq!(second.processed, 2);
}

#[tokio::test]
async fn a_changed_archive_is_reinspected_on_the_next_incremental_scan() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "1942.zip", 5);
    common::write_rom_set(harness.rom_dir(), "sf2.zip", 5);
    harness.scan(ScanMode::Full).await;

    // Rewrite one archive with a different shape so both its size and its
    // modification time change.
    common::write_rom_set(harness.rom_dir(), "sf2.zip", 12);

    let (_, counters) = harness.scan(ScanMode::Quick).await;

    assert_eq!(counters.inspected, 1);
    assert_eq!(counters.reused_from_cache, 1);
    assert_eq!(harness.row("sf2.zip").await.member_count, 12);
    assert_eq!(harness.row("1942.zip").await.member_count, 5);
}

#[tokio::test]
async fn a_full_rescan_ignores_the_cache() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "1942.zip", 5);
    harness.scan(ScanMode::Full).await;

    let (_, counters) = harness.scan(ScanMode::Full).await;

    assert_eq!(counters.inspected, 1);
    assert_eq!(counters.reused_from_cache, 0);
}

#[tokio::test]
async fn a_deleted_file_is_dropped_from_the_inventory() {
    let harness = Harness::new().await;
    common::write_rom_set(harness.rom_dir(), "keep.zip", 3);
    common::write_rom_set(harness.rom_dir(), "remove.zip", 3);
    harness.scan(ScanMode::Full).await;
    assert_eq!(harness.rows().await.len(), 2);

    std::fs::remove_file(harness.rom_dir().join("remove.zip")).unwrap();

    let (_, counters) = harness.scan(ScanMode::Quick).await;

    assert_eq!(counters.removed, 1);
    let remaining = harness.rows().await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].file_name, "keep.zip");
}

/// Cancels the scan as soon as the first batch has been committed.
struct CancelAfterFirstBatch {
    control: JobControl,
    seen: Mutex<Vec<ScanProgress>>,
}

impl ProgressSink for CancelAfterFirstBatch {
    fn emit(&self, progress: &ScanProgress) {
        self.seen.lock().unwrap().push(progress.clone());

        if progress.counters.inspected as usize >= COMMIT_BATCH_SIZE {
            self.control.cancel();
        }
    }
}

#[tokio::test]
async fn cancelling_keeps_every_batch_that_was_already_committed() {
    let harness = Harness::new().await;

    let total = COMMIT_BATCH_SIZE * 2 + 50;
    for index in 0..total {
        common::write_rom_set(harness.rom_dir(), &format!("game{index:04}.zip"), 3);
    }

    let control = JobControl::new();
    let sink = Arc::new(CancelAfterFirstBatch {
        control: control.clone(),
        seen: Mutex::new(Vec::new()),
    });

    let (state, counters) = harness
        .scan_with(ScanMode::Full, control, sink.clone())
        .await;

    assert_eq!(state, JobState::Cancelled);
    assert!(
        counters.processed < total as u64,
        "cancelling must stop work early, but {} of {total} were processed",
        counters.processed
    );

    // The first committed batch survives, and nothing beyond it was written.
    let rows = harness.rows().await;
    assert_eq!(rows.len(), COMMIT_BATCH_SIZE);

    // Every surviving row is complete: an archive is never persisted without
    // the members that were read from it.
    for row in &rows {
        let members = archives::members(&harness.pool, row.id).await.unwrap();
        assert_eq!(members.len() as i64, row.member_count);
        assert_eq!(row.member_count, 3);
    }

    let progress = sink.seen.lock().unwrap();
    assert_eq!(progress.last().unwrap().state, JobState::Cancelled);
}

#[tokio::test]
async fn a_cancelled_scan_does_not_prune_files_it_never_reached() {
    let harness = Harness::new().await;

    let total = COMMIT_BATCH_SIZE * 2;
    for index in 0..total {
        common::write_rom_set(harness.rom_dir(), &format!("game{index:04}.zip"), 2);
    }
    harness.scan(ScanMode::Full).await;
    assert_eq!(harness.rows().await.len(), total);

    let control = JobControl::new();
    let sink = Arc::new(CancelAfterFirstBatch {
        control: control.clone(),
        seen: Mutex::new(Vec::new()),
    });
    harness.scan_with(ScanMode::Full, control, sink).await;

    // Pruning only runs after a root has been walked in full, so the records
    // for archives the cancelled pass never reached are still there.
    assert_eq!(harness.rows().await.len(), total);
}

#[tokio::test]
async fn resuming_after_cancellation_completes_the_inventory() {
    let harness = Harness::new().await;

    let total = COMMIT_BATCH_SIZE + 20;
    for index in 0..total {
        common::write_rom_set(harness.rom_dir(), &format!("game{index:04}.zip"), 2);
    }

    let control = JobControl::new();
    let sink = Arc::new(CancelAfterFirstBatch {
        control: control.clone(),
        seen: Mutex::new(Vec::new()),
    });
    harness.scan_with(ScanMode::Full, control, sink).await;
    assert_eq!(harness.rows().await.len(), COMMIT_BATCH_SIZE);

    let (state, counters) = harness.scan(ScanMode::Quick).await;

    assert_eq!(state, JobState::Completed);
    assert_eq!(counters.reused_from_cache, COMMIT_BATCH_SIZE as u64);
    assert_eq!(counters.inspected, 20);
    assert_eq!(harness.rows().await.len(), total);
}

/// SPEC.md Scenario I, and the project's first non-negotiable principle: the
/// user's collection is evidence, and scanning must never alter it.
#[tokio::test]
async fn scanning_never_modifies_a_single_source_file() {
    let harness = Harness::new().await;

    common::write_rom_set(harness.rom_dir(), "1942.zip", 14);
    common::write_rom_set(harness.rom_dir(), "sf2.zip", 21);
    common::write_rom_set(&harness.rom_dir().join("nested"), "pacman.zip", 8);
    common::write_zip_with_traversal_names(&harness.rom_dir().join("hostile.zip"));
    common::write_damaged_zip(&harness.rom_dir().join("broken.zip"));
    common::write_truncated_zip(&harness.rom_dir().join("truncated.zip"));
    common::write_chd(&harness.rom_dir().join("area51.chd"), 16384);
    std::fs::write(harness.rom_dir().join("readme.txt"), b"do not touch").unwrap();

    let before = common::hash_tree(harness.rom_dir());
    assert!(before.len() >= 8);

    // Every mode, including the one that opens and reads each file in full.
    harness.scan(ScanMode::Full).await;
    harness.scan(ScanMode::Quick).await;
    harness.scan(ScanMode::DeepVerify).await;

    let after = common::hash_tree(harness.rom_dir());

    assert_eq!(
        before.len(),
        after.len(),
        "the scan added or removed files in the ROM folder"
    );

    for (path, digest) in &before {
        match after.get(path) {
            Some(current) => assert_eq!(
                current,
                digest,
                "contents of {} changed during scanning",
                path.display()
            ),
            None => panic!("{} was removed during scanning", path.display()),
        }
    }
}
