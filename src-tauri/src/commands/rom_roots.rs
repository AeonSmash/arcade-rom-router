use std::path::Path;

use tauri::State;
use tracing::info;

use crate::db::rom_roots as repo;
use crate::error::{AppError, AppResult};
use crate::model::RomRoot;
use crate::state::AppState;

/// Trims trailing separators so `D:\Arcade` and `D:\Arcade\` are one root.
/// A bare drive root such as `D:\` keeps its separator.
fn normalize_root_path(input: &str) -> String {
    let trimmed = input.trim();
    let stripped = trimmed.trim_end_matches(['/', '\\']);

    if stripped.is_empty() || stripped.ends_with(':') {
        trimmed.to_string()
    } else {
        stripped.to_string()
    }
}

#[tauri::command]
pub async fn list_rom_roots(state: State<'_, AppState>) -> AppResult<Vec<RomRoot>> {
    repo::list(&state.pool).await
}

#[tauri::command]
pub async fn add_rom_root(
    state: State<'_, AppState>,
    path: String,
    label: Option<String>,
    recursive: Option<bool>,
) -> AppResult<RomRoot> {
    let path = normalize_root_path(&path);

    if path.is_empty() {
        return Err(AppError::user(
            "No folder chosen",
            "Select the folder that contains your arcade ROM archives.",
        ));
    }

    let candidate = Path::new(&path);
    if !candidate.exists() {
        return Err(AppError::user(
            "Folder not found",
            format!("Nothing exists at this location:\n{path}"),
        ));
    }
    if !candidate.is_dir() {
        return Err(AppError::user(
            "Not a folder",
            format!("A ROM root must be a folder, not a file:\n{path}"),
        ));
    }

    if repo::find_by_path(&state.pool, &path).await?.is_some() {
        return Err(AppError::user(
            "Folder already added",
            format!("This location is already one of your ROM folders:\n{path}"),
        ));
    }

    let root = repo::insert(
        &state.pool,
        &path,
        label.as_deref(),
        recursive.unwrap_or(true),
    )
    .await?;

    info!(path = %root.path, recursive = root.recursive, "ROM root added as read-only");

    Ok(root)
}

#[tauri::command]
pub async fn set_rom_root_enabled(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> AppResult<()> {
    repo::set_enabled(&state.pool, id, enabled).await
}

/// Forgets a ROM folder and its cached inventory.
///
/// Only the application's own records are affected; the folder and its files
/// are left exactly as they are.
#[tauri::command]
pub async fn remove_rom_root(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    if state.jobs.active_job_id().is_some() {
        return Err(AppError::user(
            "Scan in progress",
            "Wait for the current scan to finish, or cancel it, before removing a ROM folder.",
        ));
    }

    repo::delete(&state.pool, id).await?;
    info!(id, "ROM root removed from the library (files left untouched)");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_separators_do_not_create_duplicate_roots() {
        assert_eq!(normalize_root_path("D:\\Arcade\\"), "D:\\Arcade");
        assert_eq!(normalize_root_path("D:\\Arcade"), "D:\\Arcade");
        assert_eq!(normalize_root_path("  D:\\Arcade\\  "), "D:\\Arcade");
        assert_eq!(normalize_root_path("/mnt/roms/"), "/mnt/roms");
    }

    #[test]
    fn drive_roots_keep_their_separator() {
        assert_eq!(normalize_root_path("D:\\"), "D:\\");
    }

    #[test]
    fn empty_input_stays_empty() {
        assert_eq!(normalize_root_path("   "), "");
    }
}
