//! EmuMovies media provider (Phase 12).
//!
//! Disabled by default. Credentials and the developer Product/API key live in
//! the OS credential store (`keyring`), never in SQLite. Talks to
//! `api.gamesdbase.com` (same surface as EmuMovies Sync / LaunchBox).

use std::path::{Path, PathBuf};
use std::time::Duration;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use sqlx::SqlitePool;

use crate::db::{self, media as media_db};
use crate::error::{AppError, AppResult};
use crate::media::local;
use crate::media::provider::MediaProvider;
use crate::model::{EmuMoviesSyncSummary, MediaAsset, MediaKind};

const SERVICE: &str = "aeonic-arcadia";
const USER_KEY: &str = "emumovies-username";
const PASS_KEY: &str = "emumovies-password";
/// Legacy keyring slot (older builds asked users to paste a product key).
const PRODUCT_KEY: &str = "emumovies-product-key";
/// Previous product name before the Aeonic Arcadia rename (credential store migration).
const LEGACY_SERVICE: &str = "arcade-rom-router";

const LOGIN_URL: &str = "https://api.gamesdbase.com/login.aspx";
const SEARCH_URL: &str = "https://api.gamesdbase.com/search.aspx";
/// Arcade ROM sets are indexed under MAME in the EmuMovies catalog.
const ARCADE_SYSTEM: &str = "MAME";
/// App-side product identifier (LaunchBox/Sync embed theirs; members only enter user/pass).
/// Override with env `AEONIC_ARCADIA_EMUMOVIES_PRODUCT` once EmuMovies grants a key.
const DEFAULT_APP_PRODUCT: &str = "AeonicArcadia";

pub struct EmuMoviesProvider;

impl EmuMoviesProvider {
    pub fn new() -> Self {
        Self
    }

    pub fn store_credentials(username: &str, password: &str) -> AppResult<()> {
        set_secret(USER_KEY, username)?;
        // Empty password means “keep the previously saved password”.
        if !password.is_empty() {
            set_secret(PASS_KEY, password)?;
        } else if !Self::has_credentials() {
            return Err(AppError::user(
                "Password required",
                "Enter your EmuMovies password the first time you save credentials.",
            ));
        }
        Ok(())
    }

    pub fn clear_credentials() -> AppResult<()> {
        for service in [SERVICE, LEGACY_SERVICE] {
            for key in [USER_KEY, PASS_KEY, PRODUCT_KEY] {
                if let Ok(entry) = keyring::Entry::new(service, key) {
                    let _ = entry.delete_credential();
                }
            }
        }
        Ok(())
    }

    pub fn has_credentials() -> bool {
        !read_secret(USER_KEY).unwrap_or_default().is_empty()
            && !read_secret(PASS_KEY).unwrap_or_default().is_empty()
    }

    pub fn username() -> Option<String> {
        read_secret(USER_KEY).filter(|p| !p.is_empty())
    }

    /// App-owned product string used as the `product=` login parameter.
    ///
    /// Members never enter this (same model as LaunchBox / EmuMovies Sync).
    pub fn app_product_key() -> String {
        for env_name in [
            "AEONIC_ARCADIA_EMUMOVIES_PRODUCT",
            "ARCADE_ROM_ROUTER_EMUMOVIES_PRODUCT",
        ] {
            if let Ok(from_env) = std::env::var(env_name) {
                let trimmed = from_env.trim().to_string();
                if !trimmed.is_empty() {
                    return trimmed;
                }
            }
        }
        if let Some(compiled) = option_env!("AEONIC_ARCADIA_EMUMOVIES_PRODUCT")
            .or(option_env!("ARCADE_ROM_ROUTER_EMUMOVIES_PRODUCT"))
        {
            let trimmed = compiled.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        // Prefer a legacy user-pasted key if one remains in the keyring.
        if let Some(legacy) = read_secret(PRODUCT_KEY).filter(|p| !p.is_empty()) {
            return legacy;
        }
        DEFAULT_APP_PRODUCT.to_string()
    }

    /// Probe login; returns Ok(session) when the API accepts the credentials.
    pub async fn probe_session() -> AppResult<String> {
        login_session().await
    }
}

impl MediaProvider for EmuMoviesProvider {
    fn id(&self) -> &'static str {
        "emumovies"
    }

    fn display_name(&self) -> &'static str {
        "EmuMovies"
    }

    fn is_configured(&self) -> bool {
        Self::has_credentials() && !Self::app_product_key().is_empty()
    }

    async fn fetch(
        &self,
        pool: &SqlitePool,
        archive_id: i64,
        set_name: &str,
    ) -> AppResult<Vec<MediaAsset>> {
        let cache_root = emumovies_cache_dir_from_pool(pool).await?;
        let session = login_session().await?;
        let kinds = [
            MediaKind::Box,
            MediaKind::Screenshot,
            MediaKind::Title,
            MediaKind::Marquee,
            MediaKind::Cabinet,
        ];
        let mut out = Vec::new();
        for kind in kinds {
            match fetch_one_kind(&session, &cache_root, pool, archive_id, set_name, kind).await {
                Ok(Some(asset)) => out.push(asset),
                Ok(None) => {}
                Err(err) => tracing::warn!(%err, set_name, kind = kind.as_str(), "emumovies fetch kind failed"),
            }
        }
        Ok(out)
    }
}

fn set_secret(key: &str, value: &str) -> AppResult<()> {
    let entry = keyring::Entry::new(SERVICE, key).map_err(|e| {
        AppError::config(
            "Credential store unavailable",
            format!("Could not open the OS credential store: {e}"),
        )
    })?;
    entry.set_password(value).map_err(|e| {
        AppError::config(
            "Could not save credential",
            format!("Windows Credential Manager rejected the write: {e}"),
        )
    })?;
    Ok(())
}

fn read_secret(key: &str) -> Option<String> {
    if let Some(value) = keyring::Entry::new(SERVICE, key)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|p| !p.is_empty())
    {
        return Some(value);
    }
    // One-time style fallback: credentials saved under the pre-rename service name.
    let legacy = keyring::Entry::new(LEGACY_SERVICE, key)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|p| !p.is_empty())?;
    let _ = set_secret(key, &legacy);
    Some(legacy)
}

fn http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent("AeonicArcadia/0.2")
        .build()
        .map_err(|e| AppError::internal(format!("Could not create HTTP client: {e}")))
}

async fn login_session() -> AppResult<String> {
    let username = read_secret(USER_KEY).filter(|s| !s.is_empty()).ok_or_else(|| {
        AppError::user(
            "EmuMovies credentials missing",
            "Save your EmuMovies username and password in Media settings first.",
        )
    })?;
    let password = read_secret(PASS_KEY).filter(|s| !s.is_empty()).ok_or_else(|| {
        AppError::user(
            "EmuMovies credentials missing",
            "Save your EmuMovies password in Media settings first.",
        )
    })?;
    let product = EmuMoviesProvider::app_product_key();

    let url = format!(
        "{LOGIN_URL}?user={}&api={}&product={}",
        urlencoding::encode(&username),
        urlencoding::encode(&password),
        urlencoding::encode(&product)
    );

    let client = http_client()?;
    let body = client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            AppError::user(
                "EmuMovies login failed",
                format!("Could not reach api.gamesdbase.com: {e}"),
            )
        })?
        .error_for_status()
        .map_err(|e| {
            AppError::user(
                "EmuMovies login rejected",
                format!("HTTP error from login endpoint: {e}"),
            )
        })?
        .text()
        .await
        .map_err(|e| {
            AppError::user(
                "EmuMovies login failed",
                format!("Could not read login response: {e}"),
            )
        })?;

    match parse_login_result(&body) {
        LoginResult::Session(session) => Ok(session),
        LoginResult::Failed { message } => {
            let lower = message.to_ascii_lowercase();
            // Unregistered app product ids fail before credential checks.
            if lower.contains("first pass validation") || lower.contains("product") {
                Err(AppError::user(
                    "EmuMovies app not registered yet",
                    "Your site password is fine — this is not a wrong-password error. \
                     Aeonic Arcadia is not registered with EmuMovies as an API client yet \
                     (LaunchBox and Sync each ship with their own app key). \
                     Workaround: download artwork with the official EmuMovies Sync utility, then \
                     choose that folder under Local artwork and Scan. \
                     To enable in-app Sync later, request developer API access for Aeonic Arcadia \
                     from EmuMovies Member Support.",
                ))
            } else if !message.is_empty() {
                Err(AppError::user(
                    "EmuMovies login failed",
                    format!(
                        "{message}. If the password has special characters, try an alphanumeric \
                         password in your EmuMovies account settings (LaunchBox has the same quirk)."
                    ),
                ))
            } else {
                Err(AppError::user(
                    "EmuMovies login failed",
                    "Login was rejected with no details. Check username/password, or use EmuMovies Sync \
                     and point Local artwork at its output folder.",
                ))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum LoginResult {
    Session(String),
    Failed { message: String },
}

/// Parse `<Results><Result Session="…"/></Results>` (attribute name is case-insensitive).
pub fn parse_session_id(xml: &str) -> Option<String> {
    match parse_login_result(xml) {
        LoginResult::Session(s) => Some(s),
        LoginResult::Failed { .. } => None,
    }
}

fn parse_login_result(xml: &str) -> LoginResult {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                if name == "result" {
                    let mut session = String::new();
                    let mut message = String::new();
                    let mut success = false;
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_ascii_lowercase();
                        let value = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
                        match key.as_str() {
                            "session" => session = value,
                            "msg" | "message" => message = value,
                            "success" => success = value.eq_ignore_ascii_case("true"),
                            _ => {}
                        }
                    }
                    if success && !session.is_empty() {
                        return LoginResult::Session(session);
                    }
                    if !session.is_empty() {
                        return LoginResult::Session(session);
                    }
                    return LoginResult::Failed { message };
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    LoginResult::Failed {
        message: String::new(),
    }
}

/// Parse search result URL from `<Result Found="True" URL="…"/>`.
pub fn parse_search_url(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                if name == "result" {
                    let mut found = false;
                    let mut url = None;
                    let mut error = false;
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_ascii_lowercase();
                        let value = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
                        match key.as_str() {
                            "found" => found = value.eq_ignore_ascii_case("true"),
                            "url" => url = Some(value),
                            "error" => error = value.eq_ignore_ascii_case("true"),
                            _ => {}
                        }
                    }
                    if found && !error {
                        return url.filter(|u| !u.is_empty());
                    }
                    return None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

pub fn emu_media_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Box => "Box",
        MediaKind::Screenshot => "Snap",
        MediaKind::Title => "Title",
        MediaKind::Marquee => "Marquee",
        MediaKind::Cabinet => "Cabinet",
        MediaKind::Video => "Video",
        MediaKind::Manual => "Manual",
    }
}

pub fn parse_kind(label: &str) -> Option<MediaKind> {
    MediaKind::parse(label)
}

async fn search_media_url(session: &str, set_name: &str, kind: MediaKind) -> AppResult<Option<String>> {
    let media = emu_media_label(kind);
    let url = format!(
        "{SEARCH_URL}?search={}&system={}&media={}&sessionid={}",
        urlencoding::encode(set_name),
        urlencoding::encode(ARCADE_SYSTEM),
        urlencoding::encode(media),
        urlencoding::encode(session)
    );
    let client = http_client()?;
    let body = client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            AppError::user(
                "EmuMovies search failed",
                format!("Could not search for “{set_name}” ({media}): {e}"),
            )
        })?
        .text()
        .await
        .map_err(|e| {
            AppError::user(
                "EmuMovies search failed",
                format!("Could not read search response: {e}"),
            )
        })?;
    Ok(parse_search_url(&body))
}

async fn download_to(path: &Path, url: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| AppError::Filesystem {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let client = http_client()?;
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| {
            AppError::user(
                "EmuMovies download failed",
                format!("Could not download media: {e}"),
            )
        })?
        .error_for_status()
        .map_err(|e| {
            AppError::user(
                "EmuMovies download rejected",
                format!("HTTP error downloading media (membership may be required): {e}"),
            )
        })?
        .bytes()
        .await
        .map_err(|e| {
            AppError::user(
                "EmuMovies download failed",
                format!("Could not read media bytes: {e}"),
            )
        })?;
    std::fs::write(path, &bytes).map_err(|source| AppError::Filesystem {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}

fn extension_from_url(url: &str) -> &str {
    Path::new(url)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.split('?').next().unwrap_or(e))
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("bin")
}

async fn fetch_one_kind(
    session: &str,
    cache_root: &Path,
    pool: &SqlitePool,
    archive_id: i64,
    set_name: &str,
    kind: MediaKind,
) -> AppResult<Option<MediaAsset>> {
    let Some(url) = search_media_url(session, set_name, kind).await? else {
        return Ok(None);
    };
    let ext = extension_from_url(&url);
    let dest = cache_root
        .join(local::normalize_title(set_name))
        .join(format!("{}.{}", kind.as_str().to_ascii_lowercase(), ext));
    download_to(&dest, &url).await?;
    let asset = media_db::upsert(
        pool,
        archive_id,
        Some(set_name),
        kind.as_str(),
        &dest.display().to_string(),
        "emumovies",
    )
    .await?;
    Ok(Some(asset))
}

async fn emumovies_cache_dir_from_pool(pool: &SqlitePool) -> AppResult<PathBuf> {
    // Prefer an explicit setting; otherwise use a sibling of the library DB via settings fallback.
    if let Ok(Some(folder)) = db::settings::get::<String>(pool, "media.emumovies.cacheDir").await {
        let path = PathBuf::from(folder);
        std::fs::create_dir_all(&path).map_err(|source| AppError::Filesystem {
            path: path.display().to_string(),
            source,
        })?;
        return Ok(path);
    }
    Err(AppError::config(
        "EmuMovies cache not configured",
        "Internal error: media cache directory was not set for this sync.",
    ))
}

pub fn ensure_cache_dir(app_data_dir: &Path) -> AppResult<PathBuf> {
    let path = app_data_dir.join("media").join("emumovies");
    std::fs::create_dir_all(&path).map_err(|source| AppError::Filesystem {
        path: path.display().to_string(),
        source,
    })?;
    Ok(path)
}

pub async fn sync_library(
    pool: &SqlitePool,
    app_data_dir: &Path,
    kinds: &[MediaKind],
    favorites_only: bool,
) -> AppResult<EmuMoviesSyncSummary> {
    if kinds.is_empty() {
        return Err(AppError::user(
            "No media types selected",
            "Choose at least one media type (box, screenshot, …) before syncing.",
        ));
    }

    let enabled: bool = db::settings::get_or(pool, "media.emumovies.enabled", false).await;
    if !enabled {
        return Err(AppError::user(
            "EmuMovies is disabled",
            "Enable the EmuMovies provider in Media settings first.",
        ));
    }

    let cache = ensure_cache_dir(app_data_dir)?;
    db::settings::set(
        pool,
        "media.emumovies.cacheDir",
        &cache.display().to_string(),
    )
    .await?;

    let session = login_session().await?;

    let archive_ids: Vec<i64> = if favorites_only {
        sqlx::query_scalar(
            "SELECT a.id FROM archives a
             INNER JOIN favorites f ON f.archive_id = a.id
             WHERE a.archive_state IN ('INDEXED', 'DISK_IMAGE_INDEXED')
             ORDER BY a.file_name COLLATE NOCASE",
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_scalar(
            "SELECT id FROM archives
             WHERE archive_state IN ('INDEXED', 'DISK_IMAGE_INDEXED')
             ORDER BY file_name COLLATE NOCASE",
        )
        .fetch_all(pool)
        .await?
    };

    if favorites_only && archive_ids.is_empty() {
        return Err(AppError::user(
            "No favorites to sync",
            "Mark some games as favorites first, or choose Entire library.",
        ));
    }

    let mut summary = EmuMoviesSyncSummary {
        processed: 0,
        downloaded: 0,
        skipped: 0,
        failed: 0,
        errors: Vec::new(),
    };

    for archive_id in archive_ids {
        summary.processed += 1;
        let names = crate::media::resolve_lookup_names(pool, archive_id).await?;
        let Some(set_name) = names.first().cloned() else {
            summary.skipped += 1;
            if summary.errors.len() < 12 {
                summary.errors.push(format!(
                    "archive {archive_id}: no set name (match a DAT first)"
                ));
            }
            continue;
        };

        for &kind in kinds {
            match fetch_one_kind(&session, &cache, pool, archive_id, &set_name, kind).await {
                Ok(Some(_)) => summary.downloaded += 1,
                Ok(None) => summary.skipped += 1,
                Err(err) => {
                    summary.failed += 1;
                    if summary.errors.len() < 12 {
                        summary.errors.push(format!(
                            "{set_name} / {}: {}",
                            kind.as_str(),
                            err.message()
                        ));
                    }
                }
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_from_login_xml() {
        let xml = r#"<?xml version="1.0"?>
<Results>
  <Result Success="True" Session="ABC123SESSION" TimeTaken="0.1 seconds" />
</Results>"#;
        assert_eq!(parse_session_id(xml).as_deref(), Some("ABC123SESSION"));
    }

    #[test]
    fn first_pass_validation_is_detected() {
        let xml = r#"<Results>
  <Result Success="False" Session="" Error="True" MSG="First pass validation error" />
</Results>"#;
        assert_eq!(
            parse_login_result(xml),
            LoginResult::Failed {
                message: "First pass validation error".into()
            }
        );
    }

    #[test]
    fn parses_search_url_when_found() {
        let xml = r#"<Results>
  <Result Found="True" Cached="True" URL="http://api.gamesdbase.com/foo.png" CRC="abcd" />
</Results>"#;
        assert_eq!(
            parse_search_url(xml).as_deref(),
            Some("http://api.gamesdbase.com/foo.png")
        );
    }

    #[test]
    fn search_miss_returns_none() {
        let xml = r#"<Results>
  <Result Found="False" Error="True" MSG="not found" />
</Results>"#;
        assert!(parse_search_url(xml).is_none());
    }

    #[test]
    fn media_labels_cover_artwork_kinds() {
        assert_eq!(emu_media_label(MediaKind::Screenshot), "Snap");
        assert_eq!(emu_media_label(MediaKind::Box), "Box");
    }
}
