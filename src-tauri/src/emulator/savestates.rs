//! Save state discovery and resume helpers (Phase 10).

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sqlx::SqlitePool;

use crate::db::{self, save_states as save_states_db};
use crate::emulator::retro_cfg;
use crate::error::{AppError, AppResult};
use crate::model::SaveStateRow;

pub async fn list_for_archive(pool: &SqlitePool, archive_id: i64) -> AppResult<Vec<SaveStateRow>> {
    refresh_index(pool, archive_id).await?;
    save_states_db::for_archive(pool, archive_id).await
}

pub async fn refresh_index(pool: &SqlitePool, archive_id: i64) -> AppResult<()> {
    let content_path: String = sqlx::query_scalar("SELECT path FROM archives WHERE id = ?1")
        .bind(archive_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| {
            AppError::user(
                "Archive not found",
                "That archive is no longer in the library.",
            )
        })?;

    let cfg_path: Option<String> =
        db::settings::get(pool, "emulator.retroarchConfigPath").await?;
    let cfg = if let Some(path) = cfg_path.as_deref().map(Path::new).filter(|p| p.is_file()) {
        retro_cfg::parse_file(path)?
    } else {
        retro_cfg::RetroArchConfig::default()
    };

    let content = PathBuf::from(&content_path);
    let stem = content
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");
    let dirs = retro_cfg::resolve_savestate_dirs(&cfg, &content);
    let discovered = retro_cfg::discover_slots(&dirs, stem);

    // Preserve labels for matching paths.
    let previous = save_states_db::for_archive(pool, archive_id).await?;
    let label_by_path: std::collections::HashMap<String, Option<String>> = previous
        .into_iter()
        .map(|r| (r.path, r.label))
        .collect();

    let mut rows = Vec::new();
    for (slot, path, thumb, is_entry) in discovered {
        let meta = std::fs::metadata(&path).ok();
        let size = meta.as_ref().map(|m| m.len() as i64).unwrap_or(0);
        let modified = meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| {
                chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            });
        let path_str = path.display().to_string();
        rows.push(SaveStateRow {
            id: 0,
            archive_id,
            slot,
            path: path_str.clone(),
            size_bytes: size,
            modified_at: modified,
            label: label_by_path.get(&path_str).cloned().flatten(),
            thumbnail_path: thumb.map(|p| p.display().to_string()),
            is_entry,
            thumbnail_url: None,
        });
    }

    save_states_db::replace_for_archive(pool, archive_id, &rows).await?;
    Ok(())
}

pub async fn label(pool: &SqlitePool, id: i64, label: Option<String>) -> AppResult<()> {
    save_states_db::set_label(pool, id, label.as_deref()).await
}

pub async fn delete_state(pool: &SqlitePool, id: i64) -> AppResult<()> {
    let Some(row) = save_states_db::delete(pool, id).await? else {
        return Err(AppError::user(
            "Save state not found",
            "That save state is no longer indexed.",
        ));
    };
    let path = Path::new(&row.path);
    if path.is_file() {
        std::fs::remove_file(path).map_err(|source| AppError::Filesystem {
            path: row.path.clone(),
            source,
        })?;
    }
    if let Some(thumb) = row.thumbnail_path.as_deref() {
        let thumb_path = Path::new(thumb);
        if thumb_path.is_file() {
            let _ = std::fs::remove_file(thumb_path);
        }
    }
    Ok(())
}

/// Promotes a normal state file to an entry-state name RetroArch's `-e N` can load.
pub fn promote_to_entry(state_path: &Path, slot: i64) -> AppResult<PathBuf> {
    let parent = state_path.parent().ok_or_else(|| {
        AppError::user(
            "Invalid save state path",
            "The save state has no parent directory.",
        )
    })?;
    let stem = state_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::user("Invalid save state path", "Could not read the file name."))?;
    // Strip trailing digits after .state to get base, then write .state{N}.entry
    let base = if let Some(idx) = stem.find(".state") {
        &stem[..idx + ".state".len()]
    } else {
        stem
    };
    let entry_name = if slot == 0 {
        format!("{base}.entry")
    } else {
        // If base already ends with .state, produce name.stateN.entry
        // base is like "galaga.state" — for slot 1 → galaga.state1.entry
        let content_stem = base.trim_end_matches(".state");
        format!("{content_stem}.state{slot}.entry")
    };
    let dest = parent.join(entry_name);
    std::fs::copy(state_path, &dest).map_err(|source| AppError::Filesystem {
        path: dest.display().to_string(),
        source,
    })?;
    Ok(dest)
}
