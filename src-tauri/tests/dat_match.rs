//! Phase 2–4: DAT import, matching, and dependency classification.

mod common;

use std::io::Write;
use std::sync::Arc;

use aeonic_arcadia_lib::dat;
use aeonic_arcadia_lib::db::{self, archives, rom_roots};
use aeonic_arcadia_lib::matcher;
use aeonic_arcadia_lib::model::{CompatibilityState, ScanMode};
use aeonic_arcadia_lib::scanner::{self, JobControl, NoopSink};
use aeonic_arcadia_lib::routing;
use sqlx::SqlitePool;
use tempfile::TempDir;

fn write_dat(path: &std::path::Path, body: &str) {
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(body.as_bytes()).unwrap();
}

async fn setup() -> (TempDir, TempDir, SqlitePool, i64) {
    let db_dir = tempfile::tempdir().unwrap();
    let roms = tempfile::tempdir().unwrap();
    let pool = db::connect(&db_dir.path().join("library.db")).await.unwrap();
    let root = rom_roots::insert(&pool, &roms.path().display().to_string(), None, true)
        .await
        .unwrap();
    (db_dir, roms, pool, root.id)
}

/// Plant a fake RetroArch + core so matching can reach VerifiedPlayable.
async fn install_fake_core(pool: &SqlitePool, profile_id: &str, dir: &std::path::Path) {
    let exe = dir.join("retroarch.exe");
    let core = dir.join(format!("{profile_id}_libretro.dll"));
    std::fs::write(&exe, b"MZ").unwrap();
    std::fs::write(&core, b"core").unwrap();
    db::profiles::update_paths(
        pool,
        profile_id,
        Some(&exe.display().to_string()),
        Some(&core.display().to_string()),
        Some("sig"),
    )
    .await
    .unwrap();
    db::profiles::set_health(pool, profile_id, aeonic_arcadia_lib::model::HealthState::Healthy)
        .await
        .unwrap();
}

#[tokio::test]
async fn imports_dat_and_matches_complete_set() {
    let (_db, roms, pool, _root_id) = setup().await;

    // Build a ZIP whose CRC will match the DAT entry.
    let expected = common::write_rom_set(roms.path(), "pacman.zip", 2);
    let r0 = &expected[0];
    let r1 = &expected[1];

    let dat_path = roms.path().join("test.dat");
    write_dat(
        &dat_path,
        &format!(
            r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name><version>1</version></header>
  <game name="pacman">
    <description>Pac-Man</description>
    <rom name="chip00.bin" size="{}" crc="{}"/>
    <rom name="chip01.bin" size="{}" crc="{}"/>
  </game>
</datafile>
"#,
            r0.size, r0.crc32, r1.size, r1.crc32
        ),
    );

    scanner::run_scan(
        &pool,
        &[rom_roots::list(&pool).await.unwrap().into_iter().next().unwrap()],
        ScanMode::Full,
        2,
        1,
        JobControl::new(),
        Arc::new(NoopSink),
    )
    .await
    .unwrap();

    install_fake_core(&pool, "mame2003plus", roms.path()).await;

    let source = dat::import_dat(
        &pool,
        &dat_path.display().to_string(),
        "mame2003plus",
        None,
    )
    .await
    .unwrap();
    assert!(source.active);
    assert_eq!(source.machine_count, 1);

    // import_dat already rematches; count archives with results.
    let matched = matcher::rematch_library(&pool).await.unwrap();
    assert_eq!(matched, 1);

    let page = archives::page(&pool, &archives::ArchiveQuery::default())
        .await
        .unwrap();
    let archive = page.rows.iter().find(|a| a.file_name == "pacman.zip").unwrap();
    let results = db::matches::for_archive(&pool, archive.id).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].state, CompatibilityState::VerifiedPlayable);

    let route = routing::choose_route(&pool, archive.id).await.unwrap().unwrap();
    assert!(route.launchable);
}

#[tokio::test]
async fn incomplete_set_is_classified() {
    let (_db, roms, pool, _) = setup().await;
    let expected = common::write_rom_set(roms.path(), "pacman.zip", 1);
    let r0 = &expected[0];

    let dat_path = roms.path().join("test.dat");
    write_dat(
        &dat_path,
        &format!(
            r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="pacman">
    <rom name="chip00.bin" size="{}" crc="{}"/>
    <rom name="chip01.bin" size="4096" crc="deadbeef"/>
  </game>
</datafile>
"#,
            r0.size, r0.crc32
        ),
    );

    scanner::run_scan(
        &pool,
        &[rom_roots::list(&pool).await.unwrap().remove(0)],
        ScanMode::Full,
        2,
        1,
        JobControl::new(),
        Arc::new(NoopSink),
    )
    .await
    .unwrap();

    install_fake_core(&pool, "mame2010", roms.path()).await;
    dat::import_dat(&pool, &dat_path.display().to_string(), "mame2010", None)
        .await
        .unwrap();

    let archive = archives::page(&pool, &archives::ArchiveQuery::default())
        .await
        .unwrap()
        .rows
        .into_iter()
        .next()
        .unwrap();
    let results = db::matches::for_archive(&pool, archive.id).await.unwrap();
    assert_eq!(results[0].state, CompatibilityState::IncompleteSet);
    assert!(results[0].missing_required >= 1);
}

#[tokio::test]
async fn missing_parent_is_detected() {
    let (_db, roms, pool, _) = setup().await;
    let expected = common::write_rom_set(roms.path(), "pacman.zip", 1);
    let r0 = &expected[0];

    let dat_path = roms.path().join("test.dat");
    write_dat(
        &dat_path,
        &format!(
            r#"<?xml version="1.0"?>
<datafile>
  <header><name>Test</name></header>
  <game name="puckman">
    <description>parent</description>
    <rom name="parent.bin" size="4096" crc="11111111"/>
  </game>
  <game name="pacman" cloneof="puckman" romof="puckman">
    <rom name="chip00.bin" size="{}" crc="{}"/>
  </game>
</datafile>
"#,
            r0.size, r0.crc32
        ),
    );

    scanner::run_scan(
        &pool,
        &[rom_roots::list(&pool).await.unwrap().remove(0)],
        ScanMode::Full,
        2,
        1,
        JobControl::new(),
        Arc::new(NoopSink),
    )
    .await
    .unwrap();

    install_fake_core(&pool, "fbneo", roms.path()).await;
    dat::import_dat(&pool, &dat_path.display().to_string(), "fbneo", None)
        .await
        .unwrap();

    let archive = archives::page(&pool, &archives::ArchiveQuery::default())
        .await
        .unwrap()
        .rows[0]
        .clone();
    let results = db::matches::for_archive(&pool, archive.id).await.unwrap();
    assert_eq!(results[0].state, CompatibilityState::MissingParent);

    let deps = matcher::dependencies_for_archive(&pool, archive.id)
        .await
        .unwrap();
    assert!(deps.iter().any(|d| d.kind == "parent" && !d.present));
}

#[tokio::test]
async fn split_set_verifies_when_parent_and_clone_union_complete() {
    let (_db, roms, pool, _) = setup().await;

    // Parent holds chip A; clone zip holds only the delta chip B.
    let parent_chips = common::write_rom_set(roms.path(), "puckman.zip", 1);
    let clone_chips = common::write_rom_set(roms.path(), "pacman.zip", 1);
    let parent = &parent_chips[0];
    let clone = &clone_chips[0];

    let dat_path = roms.path().join("split.dat");
    write_dat(
        &dat_path,
        &format!(
            r#"<?xml version="1.0"?>
<datafile>
  <header><name>Split</name></header>
  <game name="puckman">
    <description>parent</description>
    <rom name="parent.bin" size="{}" crc="{}"/>
  </game>
  <game name="pacman" cloneof="puckman" romof="puckman">
    <rom name="parent.bin" size="{}" crc="{}" merge="parent.bin"/>
    <rom name="chip00.bin" size="{}" crc="{}"/>
  </game>
</datafile>
"#,
            parent.size, parent.crc32, parent.size, parent.crc32, clone.size, clone.crc32
        ),
    );

    scanner::run_scan(
        &pool,
        &[rom_roots::list(&pool).await.unwrap().remove(0)],
        ScanMode::Full,
        2,
        1,
        JobControl::new(),
        Arc::new(NoopSink),
    )
    .await
    .unwrap();

    install_fake_core(&pool, "fbneo", roms.path()).await;
    dat::import_dat(&pool, &dat_path.display().to_string(), "fbneo", None)
        .await
        .unwrap();

    let page = archives::page(&pool, &archives::ArchiveQuery::default())
        .await
        .unwrap();
    let clone_archive = page
        .rows
        .iter()
        .find(|a| a.file_name == "pacman.zip")
        .unwrap();
    let results = db::matches::for_archive(&pool, clone_archive.id)
        .await
        .unwrap();
    assert_eq!(results[0].state, CompatibilityState::VerifiedPlayable);
    assert_eq!(results[0].missing_required, 0);
    assert!(
        results[0]
            .machine
            .as_ref()
            .is_some_and(|m| m.set_name == "pacman"),
        "must match the filename-anchored machine"
    );
}

#[tokio::test]
async fn wrong_clone_content_never_verified_under_filename() {
    // Regression: sf2ceb.zip previously matched machine sf2ceub (1 local chip)
    // and was reported VERIFIED_PLAYABLE. Filename must win.
    let (_db, roms, pool, _) = setup().await;

    let chips = common::write_rom_set(roms.path(), "sf2ceb.zip", 1);
    let only = &chips[0];

    let dat_path = roms.path().join("fb.dat");
    write_dat(
        &dat_path,
        &format!(
            r#"<?xml version="1.0"?>
<datafile>
  <header><name>FB</name></header>
  <game name="sf2ce">
    <rom name="base.bin" size="4096" crc="aaaaaaaa"/>
    <rom name="extra.bin" size="4096" crc="bbbbbbbb"/>
  </game>
  <game name="sf2ceub" cloneof="sf2ce" romof="sf2ce">
    <rom name="base.bin" size="4096" crc="aaaaaaaa" merge="base.bin"/>
    <rom name="extra.bin" size="4096" crc="bbbbbbbb" merge="extra.bin"/>
    <rom name="delta.bin" size="{}" crc="{}"/>
  </game>
  <game name="sf2ceb" cloneof="sf2ce" romof="sf2ce">
    <rom name="3.ic171" size="524288" crc="a2355d90"/>
    <rom name="5.ic171" size="524288" crc="c6f86e84"/>
    <rom name="delta.bin" size="{}" crc="{}"/>
  </game>
</datafile>
"#,
            only.size, only.crc32, only.size, only.crc32
        ),
    );

    scanner::run_scan(
        &pool,
        &[rom_roots::list(&pool).await.unwrap().remove(0)],
        ScanMode::Full,
        2,
        1,
        JobControl::new(),
        Arc::new(NoopSink),
    )
    .await
    .unwrap();

    install_fake_core(&pool, "fbneo", roms.path()).await;
    dat::import_dat(&pool, &dat_path.display().to_string(), "fbneo", None)
        .await
        .unwrap();

    let archive = archives::page(&pool, &archives::ArchiveQuery::default())
        .await
        .unwrap()
        .rows
        .into_iter()
        .find(|a| a.file_name == "sf2ceb.zip")
        .unwrap();
    let results = db::matches::for_archive(&pool, archive.id).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].machine.as_ref().map(|m| m.set_name.as_str()),
        Some("sf2ceb"),
        "must anchor to filename, not CRC-overlapping clone"
    );
    assert_ne!(results[0].state, CompatibilityState::VerifiedPlayable);
    assert!(
        matches!(
            results[0].state,
            CompatibilityState::IncompleteSet
                | CompatibilityState::KnownSetNameUnverifiedContent
        ),
        "got {:?}",
        results[0].state
    );
    assert!(results[0].missing_required >= 2);

    let route = routing::choose_route(&pool, archive.id).await.unwrap();
    assert!(route.is_some());
    assert!(!route.unwrap().launchable);
}
