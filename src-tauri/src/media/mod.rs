//! Local artwork and optional online media providers (Phases 11–12).

pub mod emumovies;
pub mod local;
pub mod provider;

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::db::{self, media as media_db};
use crate::error::{AppError, AppResult};
use crate::model::{GameMedia, MediaAsset, MediaKind};
use crate::media::provider::MediaProvider;

pub const MEDIA_KINDS: &[MediaKind] = &[
    MediaKind::Box,
    MediaKind::Screenshot,
    MediaKind::Title,
    MediaKind::Marquee,
    MediaKind::Cabinet,
];

pub async fn get_media_folder(pool: &SqlitePool) -> Option<String> {
    db::settings::get(pool, "media.localFolder").await.ok().flatten()
}

pub async fn set_media_folder(pool: &SqlitePool, path: &str) -> AppResult<()> {
    let p = Path::new(path);
    if !p.is_dir() {
        return Err(AppError::user(
            "Media folder not found",
            format!("Nothing exists at this location:\n{path}"),
        ));
    }
    db::settings::set(pool, "media.localFolder", &path.to_string()).await?;
    Ok(())
}

pub async fn get_game_media(pool: &SqlitePool, archive_id: i64) -> AppResult<GameMedia> {
    let cached = media_db::for_archive(pool, archive_id).await?;
    if !cached.is_empty() {
        return Ok(GameMedia {
            archive_id,
            assets: cached,
        });
    }

    // Try a live local resolve without requiring a prior full scan.
    if let Some(folder) = get_media_folder(pool).await {
        let names = resolve_lookup_names(pool, archive_id).await?;
        let mut assets = Vec::new();
        for kind in MEDIA_KINDS {
            if let Some(path) = local::find_asset(Path::new(&folder), kind, &names) {
                let asset = media_db::upsert(
                    pool,
                    archive_id,
                    names.first().map(|s| s.as_str()),
                    kind.as_str(),
                    &path.display().to_string(),
                    "local",
                )
                .await?;
                assets.push(asset);
            }
        }
        return Ok(GameMedia {
            archive_id,
            assets,
        });
    }

    Ok(GameMedia {
        archive_id,
        assets: Vec::new(),
    })
}

pub async fn scan_local_media(pool: &SqlitePool) -> AppResult<u64> {
    let folder = get_media_folder(pool).await.ok_or_else(|| {
        AppError::user(
            "No media folder configured",
            "Choose a local artwork folder before scanning.",
        )
    })?;
    let folder = PathBuf::from(folder);
    let archive_ids: Vec<(i64,)> =
        sqlx::query_as("SELECT id FROM archives WHERE archive_state = 'INDEXED'")
            .fetch_all(pool)
            .await?;

    let mut count = 0u64;
    for (archive_id,) in archive_ids {
        let names = resolve_lookup_names(pool, archive_id).await?;
        if names.is_empty() {
            continue;
        }
        for kind in MEDIA_KINDS {
            if let Some(path) = local::find_asset(&folder, kind, &names) {
                media_db::upsert(
                    pool,
                    archive_id,
                    names.first().map(|s| s.as_str()),
                    kind.as_str(),
                    &path.display().to_string(),
                    "local",
                )
                .await?;
                count += 1;
            }
        }
    }
    Ok(count)
}

pub async fn clear_media_cache(pool: &SqlitePool) -> AppResult<u64> {
    media_db::clear_all(pool).await
}

/// Lookup names: canonical set, parent/clone_of, then normalized file stem.
pub async fn resolve_lookup_names(pool: &SqlitePool, archive_id: i64) -> AppResult<Vec<String>> {
    let mut names = Vec::new();

    let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT m.set_name, m.clone_of, m.rom_of
         FROM match_results mr
         JOIN machines m ON m.id = mr.machine_id
         WHERE mr.archive_id = ?1
         ORDER BY mr.score DESC, mr.id
         LIMIT 1",
    )
    .bind(archive_id)
    .fetch_optional(pool)
    .await?;

    if let Some((set_name, clone_of, rom_of)) = row {
        names.push(set_name);
        if let Some(parent) = clone_of {
            if !names.contains(&parent) {
                names.push(parent);
            }
        }
        if let Some(parent) = rom_of {
            if !names.contains(&parent) {
                names.push(parent);
            }
        }
    }

    let file_name: Option<String> =
        sqlx::query_scalar("SELECT file_name FROM archives WHERE id = ?1")
            .bind(archive_id)
            .fetch_optional(pool)
            .await?;
    if let Some(file_name) = file_name {
        let stem = Path::new(&file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&file_name)
            .to_string();
        let normalized = local::normalize_title(&stem);
        if !names.iter().any(|n| n.eq_ignore_ascii_case(&stem)) {
            names.push(stem);
        }
        if !normalized.is_empty() && !names.iter().any(|n| local::normalize_title(n) == normalized)
        {
            names.push(normalized);
        }
    }

    Ok(names)
}

pub fn emumovies_provider() -> emumovies::EmuMoviesProvider {
    emumovies::EmuMoviesProvider::new()
}

pub async fn fetch_remote_media(
    pool: &SqlitePool,
    archive_id: i64,
) -> AppResult<Vec<MediaAsset>> {
    let enabled: bool = db::settings::get_or(pool, "media.emumovies.enabled", false).await;
    if !enabled {
        return Err(AppError::user(
            "EmuMovies is disabled",
            "Enable the EmuMovies provider in Media settings after configuring credentials.",
        ));
    }
    let provider = emumovies_provider();
    let names = resolve_lookup_names(pool, archive_id).await?;
    let set_name = names
        .first()
        .cloned()
        .ok_or_else(|| AppError::user("Unidentified game", "Match a DAT before scraping media."))?;
    provider.fetch(pool, archive_id, &set_name).await
}
