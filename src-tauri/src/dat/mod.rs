//! DAT definition import (Phase 2).

pub mod catver;
pub mod parser;

use sqlx::SqlitePool;
use tracing::info;

use crate::db::{self, categories, dats, machines as machines_db};
use crate::error::{AppError, AppResult};
use crate::matcher;
use crate::model::{CategoryStats, DatSource};

/// Import a CatVer.ini `[Category]` map used for the library Genre column.
pub async fn import_catver(pool: &SqlitePool, path: &str) -> AppResult<CategoryStats> {
    let file_path = std::path::Path::new(path);
    if !file_path.is_file() {
        return Err(AppError::user(
            "CatVer file not found",
            format!("Nothing exists at this location:\n{path}"),
        ));
    }
    let entries = catver::parse_file(file_path)?;
    let stats = categories::replace_all(pool, &entries, path).await?;
    info!(count = stats.count, path, "CatVer categories imported");
    Ok(stats)
}

pub async fn category_stats(pool: &SqlitePool) -> AppResult<CategoryStats> {
    categories::stats(pool).await
}

/// Imports a DAT file and associates it with an emulator profile.
///
/// Activating a new DAT for a profile deactivates the previous active DAT for
/// that profile and clears match/route rows that depended on it.
pub async fn import_dat(
    pool: &SqlitePool,
    path: &str,
    emulator_profile_id: &str,
    display_name: Option<String>,
) -> AppResult<DatSource> {
    let profile = db::profiles::get(pool, emulator_profile_id)
        .await?
        .ok_or_else(|| {
            AppError::user(
                "Unknown emulator profile",
                format!("There is no emulator profile named “{emulator_profile_id}”."),
            )
        })?;

    let file_path = std::path::Path::new(path);
    if !file_path.is_file() {
        return Err(AppError::user(
            "DAT file not found",
            format!("Nothing exists at this location:\n{path}"),
        ));
    }

    let (mut parsed, sha256) = parser::parse_file(file_path)?;
    if let Some(name) = display_name {
        if !name.trim().is_empty() {
            parsed.display_name = name.trim().to_string();
        }
    }

    // Duplicate fingerprint for the same profile is refused rather than creating
    // two identical definition snapshots.
    if let Some(existing) = dats::find_by_sha256(pool, &profile.id, &sha256).await? {
        return Err(AppError::user(
            "DAT already imported",
            format!(
                "This exact file is already imported as “{}” (id {}).",
                existing.display_name, existing.id
            ),
        ));
    }

    let mut tx = pool.begin().await?;

    if let Some(previous) = dats::active_for_profile_tx(&mut tx, &profile.id).await? {
        dats::deactivate_tx(&mut tx, previous.id).await?;
        dats::clear_results_for_dat_tx(&mut tx, previous.id).await?;
        info!(
            previous_id = previous.id,
            profile = %profile.id,
            "deactivated previous DAT for profile"
        );
    }

    let source = dats::insert_tx(
        &mut tx,
        &dats::NewDatSource {
            emulator_profile_id: profile.id.clone(),
            display_name: parsed.display_name.clone(),
            source_type: "xml-dat".into(),
            version: parsed.version.clone(),
            path: path.to_string(),
            sha256: sha256.clone(),
            machine_count: parsed.machines.len() as i64,
            rom_entry_count: parsed.machines.iter().map(|m| m.roms.len() as i64).sum(),
            disk_entry_count: parsed.machines.iter().map(|m| m.disks.len() as i64).sum(),
            parser_version: parser::PARSER_VERSION,
        },
    )
    .await?;

    machines_db::insert_all_tx(&mut tx, source.id, &parsed.machines).await?;

    tx.commit().await?;

    info!(
        dat_id = source.id,
        profile = %profile.id,
        machines = source.machine_count,
        "DAT imported"
    );

    // Every import invalidates prior results for the profile; rematch immediately
    // so the library reflects the new definition without a separate command.
    matcher::rematch_library(pool).await?;

    Ok(source)
}
