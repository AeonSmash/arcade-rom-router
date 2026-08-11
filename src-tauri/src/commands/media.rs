use tauri::{AppHandle, Manager, State};

use crate::db::settings;
use crate::error::{AppError, AppResult};
use crate::media::{self, emumovies};
use crate::model::{
    EmuMoviesStatus, EmuMoviesSyncRequest, EmuMoviesSyncSummary, GameMedia, MediaAsset, MediaKind,
};
use crate::state::AppState;

fn allow_media_dir(app: &AppHandle, path: &str) {
    let _ = app.asset_protocol_scope().allow_directory(path, true);
}

#[tauri::command]
pub async fn get_game_media(
    app: AppHandle,
    state: State<'_, AppState>,
    archive_id: i64,
) -> AppResult<GameMedia> {
    if let Some(folder) = media::get_media_folder(&state.pool).await {
        allow_media_dir(&app, &folder);
    }
    let cache = state.app_data_dir.join("media").join("emumovies");
    if cache.is_dir() {
        allow_media_dir(&app, &cache.display().to_string());
    }
    media::get_game_media(&state.pool, archive_id).await
}

#[tauri::command]
pub async fn set_media_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<()> {
    media::set_media_folder(&state.pool, &path).await?;
    allow_media_dir(&app, &path);
    Ok(())
}

#[tauri::command]
pub async fn get_media_folder(state: State<'_, AppState>) -> AppResult<Option<String>> {
    Ok(media::get_media_folder(&state.pool).await)
}

#[tauri::command]
pub async fn scan_local_media(state: State<'_, AppState>) -> AppResult<u64> {
    media::scan_local_media(&state.pool).await
}

#[tauri::command]
pub async fn clear_media_cache(state: State<'_, AppState>) -> AppResult<u64> {
    media::clear_media_cache(&state.pool).await
}

#[tauri::command]
pub async fn get_emumovies_status(state: State<'_, AppState>) -> AppResult<EmuMoviesStatus> {
    let enabled: bool = settings::get_or(&state.pool, "media.emumovies.enabled", false).await;
    let has_credentials = emumovies::EmuMoviesProvider::has_credentials();
    // App always supplies a product identifier (LaunchBox-style). Kept on the
    // status payload for older frontends; not a user-facing requirement.
    let has_product_key = !emumovies::EmuMoviesProvider::app_product_key().is_empty();

    let (api_ready, detail) = if !has_credentials {
        (
            false,
            "Save your EmuMovies site username and password (same login as emumovies.com / Sync), then enable the provider and Sync."
                .into(),
        )
    } else if !enabled {
        (
            false,
            "Credentials saved. Enable the provider, then use Sync to download media.".into(),
        )
    } else {
        match emumovies::EmuMoviesProvider::probe_session().await {
            Ok(_) => (
                true,
                "Connected to EmuMovies. Choose media types and scope, then Sync.".into(),
            ),
            Err(err) => (false, err.message()),
        }
    };

    Ok(EmuMoviesStatus {
        enabled,
        has_credentials,
        has_product_key,
        username: emumovies::EmuMoviesProvider::username(),
        api_ready,
        detail,
    })
}

#[tauri::command]
pub async fn set_emumovies_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    settings::set(&state.pool, "media.emumovies.enabled", &enabled).await
}

#[tauri::command]
pub async fn set_emumovies_credentials(
    username: String,
    password: Option<String>,
) -> AppResult<()> {
    let password = password.unwrap_or_default();
    emumovies::EmuMoviesProvider::store_credentials(&username, &password)
}

#[tauri::command]
pub async fn clear_emumovies_credentials() -> AppResult<()> {
    emumovies::EmuMoviesProvider::clear_credentials()
}

#[tauri::command]
pub async fn fetch_emumovies_media(
    app: AppHandle,
    state: State<'_, AppState>,
    archive_id: i64,
) -> AppResult<Vec<MediaAsset>> {
    let cache = emumovies::ensure_cache_dir(&state.app_data_dir)?;
    settings::set(
        &state.pool,
        "media.emumovies.cacheDir",
        &cache.display().to_string(),
    )
    .await?;
    allow_media_dir(&app, &cache.display().to_string());
    media::fetch_remote_media(&state.pool, archive_id).await
}

#[tauri::command]
pub async fn sync_emumovies_media(
    app: AppHandle,
    state: State<'_, AppState>,
    request: EmuMoviesSyncRequest,
) -> AppResult<EmuMoviesSyncSummary> {
    let mut kinds = Vec::new();
    for label in &request.kinds {
        let kind = MediaKind::parse(label).ok_or_else(|| {
            AppError::user(
                "Unknown media type",
                format!("“{label}” is not a supported media kind."),
            )
        })?;
        kinds.push(kind);
    }

    let favorites_only = match request.scope.as_str() {
        "favorites" => true,
        "all" => false,
        other => {
            return Err(AppError::user(
                "Invalid sync scope",
                format!("Scope must be “favorites” or “all”, not “{other}”."),
            ));
        }
    };

    let cache = emumovies::ensure_cache_dir(&state.app_data_dir)?;
    allow_media_dir(&app, &cache.display().to_string());

    emumovies::sync_library(
        &state.pool,
        &state.app_data_dir,
        &kinds,
        favorites_only,
    )
    .await
}
