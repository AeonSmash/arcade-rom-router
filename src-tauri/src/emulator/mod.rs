//! RetroArch discovery, core scan, and profile health (Phase 5).

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use tracing::info;

use crate::db::{profiles, settings};
use crate::error::{AppError, AppResult};
use crate::model::{DetectedCore, HealthState, RetroArchDiscovery};

const SETTINGS_EXE: &str = "emulator.retroarchExecutable";
const SETTINGS_CORES: &str = "emulator.retroarchCoresDir";
const SETTINGS_SYSTEM: &str = "emulator.retroarchSystemDir";
const SETTINGS_CONFIG: &str = "emulator.retroarchConfigPath";

/// Maps core DLL filename fragments to built-in profile ids.
///
/// More specific prefixes are checked before the bare `mame` catch-all.
const CORE_MAP: &[(&str, &str, &str)] = &[
    ("mame2003_plus", "mame2003plus", "MAME 2003-Plus"),
    ("mame2003plus", "mame2003plus", "MAME 2003-Plus"),
    ("mame2003", "mame2003", "MAME 2003"),
    ("mame2010", "mame2010", "MAME 2010"),
    ("mame2015", "mame2015", "MAME 2015"),
    ("mame2016", "mame2016", "MAME 2016"),
    ("fbneo", "fbneo", "FinalBurn Neo"),
    ("fbalpha", "fbneo", "FinalBurn Neo"),
    ("mame", "mame_current", "MAME Current"),
];

fn common_retroarch_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    // Official / installer layouts
    if let Ok(pf) = std::env::var("ProgramFiles") {
        out.push(PathBuf::from(pf).join("RetroArch").join("retroarch.exe"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        out.push(PathBuf::from(pf86).join("RetroArch").join("retroarch.exe"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let local = PathBuf::from(local);
        out.push(local.join("RetroArch").join("retroarch.exe"));
        out.push(
            local
                .join("Programs")
                .join("RetroArch")
                .join("retroarch.exe"),
        );
    }

    // Common portable / zip extracts (Windows nightly builds often use RetroArch-Win64)
    for root in [
        r"C:\RetroArch-Win64",
        r"C:\RetroArch",
        r"D:\RetroArch-Win64",
        r"D:\RetroArch",
        r"E:\RetroArch-Win64",
        r"E:\RetroArch",
        r"F:\RetroArch-Win64",
        r"F:\RetroArch",
        r"F:\Arcade Emulation\RetroArch",
        r"F:\Arcade Emulation\RetroArch-Win64",
    ] {
        out.push(PathBuf::from(root).join("retroarch.exe"));
    }

    out
}

fn infer_dirs(exe: &Path) -> (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) {
    let root = exe.parent().map(|p| p.to_path_buf());
    let cores = root.as_ref().map(|r| r.join("cores"));
    let system = root.as_ref().map(|r| r.join("system"));
    let config = root.as_ref().map(|r| r.join("retroarch.cfg"));
    (
        cores.filter(|p| p.is_dir()),
        system.filter(|p| p.is_dir()),
        config.filter(|p| p.is_file()),
    )
}

fn core_signature(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(format!("{}:{}:{}", path.display(), meta.len(), modified))
}

pub fn match_core_filename(file_name: &str) -> Option<(&'static str, &'static str)> {
    let lower = file_name.to_ascii_lowercase();
    if !lower.ends_with(".dll") && !lower.ends_with(".so") && !lower.ends_with(".dylib") {
        return None;
    }
    for (needle, profile_id, display) in CORE_MAP {
        // Bare "mame" must not match hbmame / mame2000 / etc. Prefer the
        // current libretro core filename shape: mame_libretro.*
        if *needle == "mame" {
            if lower.starts_with("mame_libretro.") || lower == "mame_libretro.dll" {
                return Some((*profile_id, *display));
            }
            continue;
        }
        // FB Alpha split cores (cps1/cps2/neogeo/…) are not the full FBNeo
        // arcade core. Only map the generic FB Alpha DLL, and prefer FBNeo.
        if *needle == "fbalpha" {
            if lower.starts_with("fbalpha_libretro.")
                || lower.starts_with("fbalpha2012_libretro.")
            {
                return Some((*profile_id, *display));
            }
            continue;
        }
        // Midway-only 2003 fork is not the main MAME 2003 core.
        if *needle == "mame2003" && lower.contains("mame2003_midway") {
            continue;
        }
        if lower.contains(needle) {
            return Some((*profile_id, *display));
        }
    }
    None
}

/// Higher is better when several DLLs map to the same profile.
fn core_preference(file_name: &str) -> i32 {
    let lower = file_name.to_ascii_lowercase();
    if lower.contains("fbneo") {
        return 100;
    }
    if lower.starts_with("fbalpha_libretro.") || lower.starts_with("fbalpha2012_libretro.") {
        return 40;
    }
    if lower.contains("mame2003_plus") || lower.contains("mame2003plus") {
        return 100;
    }
    50
}

pub fn scan_cores_dir(cores_dir: &Path) -> Vec<DetectedCore> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(cores_dir) else {
        return found;
    };

    // Best match per profile (FBNeo must win over FB Alpha CPS split cores).
    let mut best: std::collections::HashMap<&str, DetectedCore> = std::collections::HashMap::new();

    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();

    for path in files {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some((profile_id, display)) = match_core_filename(name) else {
            continue;
        };
        let pref = core_preference(name);
        let candidate = DetectedCore {
            profile_id: profile_id.into(),
            display_name: display.into(),
            core_path: path.display().to_string(),
            matched_filename: name.into(),
        };
        match best.get(profile_id) {
            Some(existing) if core_preference(&existing.matched_filename) >= pref => {}
            _ => {
                best.insert(profile_id, candidate);
            }
        }
    }

    found.extend(best.into_values());
    found.sort_by(|a, b| a.profile_id.cmp(&b.profile_id));
    found
}

/// Discovers RetroArch from settings, an optional user path, or common locations.
pub async fn discover_retroarch(
    pool: &SqlitePool,
    explicit_exe: Option<String>,
) -> AppResult<RetroArchDiscovery> {
    let saved: Option<String> = settings::get(pool, SETTINGS_EXE).await?;
    let candidate = explicit_exe
        .or(saved)
        .map(PathBuf::from)
        .or_else(|| common_retroarch_candidates().into_iter().find(|p| p.is_file()));

    let Some(exe) = candidate.filter(|p| p.is_file()) else {
        return Ok(RetroArchDiscovery {
            executable_path: None,
            cores_dir: None,
            system_dir: None,
            config_path: None,
            detected_cores: Vec::new(),
        });
    };

    let saved_cores: Option<String> = settings::get(pool, SETTINGS_CORES).await?;
    let saved_system: Option<String> = settings::get(pool, SETTINGS_SYSTEM).await?;
    let saved_config: Option<String> = settings::get(pool, SETTINGS_CONFIG).await?;

    let (inferred_cores, inferred_system, inferred_config) = infer_dirs(&exe);
    let cores_dir = saved_cores
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or(inferred_cores);
    let system_dir = saved_system
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or(inferred_system);
    let config_path = saved_config
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or(inferred_config);

    let detected_cores = cores_dir
        .as_ref()
        .map(|d| scan_cores_dir(d))
        .unwrap_or_default();

    settings::set(pool, SETTINGS_EXE, &exe.display().to_string()).await?;
    if let Some(dir) = &cores_dir {
        settings::set(pool, SETTINGS_CORES, &dir.display().to_string()).await?;
    }
    if let Some(dir) = &system_dir {
        settings::set(pool, SETTINGS_SYSTEM, &dir.display().to_string()).await?;
    }
    if let Some(cfg) = &config_path {
        settings::set(pool, SETTINGS_CONFIG, &cfg.display().to_string()).await?;
    }

    // Apply discovered cores onto profiles without rewriting RetroArch config.
    for core in &detected_cores {
        let sig = core_signature(Path::new(&core.core_path));
        profiles::update_paths(
            pool,
            &core.profile_id,
            Some(&exe.display().to_string()),
            Some(&core.core_path),
            sig.as_deref(),
        )
        .await?;
    }

    // Profiles without a matching core still get the exe path for health messaging.
    for profile in profiles::list(pool).await? {
        if profile.executable_path.is_none() {
            profiles::update_paths(pool, &profile.id, Some(&exe.display().to_string()), None, None)
                .await?;
        }
    }

    info!(
        exe = %exe.display(),
        cores = detected_cores.len(),
        "RetroArch discovery complete"
    );

    Ok(RetroArchDiscovery {
        executable_path: Some(exe.display().to_string()),
        cores_dir: cores_dir.map(|p| p.display().to_string()),
        system_dir: system_dir.map(|p| p.display().to_string()),
        config_path: config_path.map(|p| p.display().to_string()),
        detected_cores,
    })
}

pub async fn validate_profile(pool: &SqlitePool, profile_id: &str) -> AppResult<HealthState> {
    let profile = profiles::get(pool, profile_id)
        .await?
        .ok_or_else(|| {
            AppError::user(
                "Unknown emulator profile",
                format!("No profile named “{profile_id}”."),
            )
        })?;

    let exe_ok = profile
        .executable_path
        .as_ref()
        .map(|p| Path::new(p).is_file())
        .unwrap_or(false);
    let core_ok = profile
        .core_path
        .as_ref()
        .map(|p| Path::new(p).is_file())
        .unwrap_or(false);
    let has_dat = profile.has_active_dat
        || crate::db::dats::active_for_profile(pool, profile_id)
            .await?
            .is_some();

    let state = if !exe_ok {
        HealthState::MissingExecutable
    } else if !core_ok {
        HealthState::MissingCore
    } else if !has_dat {
        HealthState::NeedsDat
    } else {
        // Harmless help probe — never rewrite config.
        let help_ok = std::process::Command::new(profile.executable_path.as_ref().unwrap())
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success() || s.code().is_some())
            .unwrap_or(false);
        if help_ok {
            HealthState::Healthy
        } else {
            HealthState::Unhealthy
        }
    };

    profiles::set_health(pool, profile_id, state).await?;
    Ok(state)
}

pub async fn validate_all_profiles(pool: &SqlitePool) -> AppResult<Vec<(String, HealthState)>> {
    let mut out = Vec::new();
    for profile in profiles::list(pool).await? {
        let state = validate_profile(pool, &profile.id).await?;
        out.push((profile.id, state));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_filename_mapping_prefers_specific_mame_versions() {
        assert_eq!(
            match_core_filename("mame2003_plus_libretro.dll").unwrap().0,
            "mame2003plus"
        );
        assert_eq!(
            match_core_filename("mame2010_libretro.dll").unwrap().0,
            "mame2010"
        );
        assert_eq!(
            match_core_filename("mame_libretro.dll").unwrap().0,
            "mame_current"
        );
        assert_eq!(match_core_filename("fbneo_libretro.dll").unwrap().0, "fbneo");
        assert_eq!(
            match_core_filename("fbalpha2012_libretro.dll").unwrap().0,
            "fbneo"
        );
        assert!(match_core_filename("fbalpha2012_cps1_libretro.dll").is_none());
        assert!(match_core_filename("fbalpha2012_neogeo_libretro.dll").is_none());
        assert!(match_core_filename("mame2003_midway_libretro.dll").is_none());
        assert!(match_core_filename("snes9x_libretro.dll").is_none());
        assert!(match_core_filename("hbmame_libretro.dll").is_none());
        assert!(match_core_filename("mame2000_libretro.dll").is_none());
    }

    #[test]
    fn scan_prefers_fbneo_over_fbalpha_split_cores() {
        let dir = tempfile_cores(&[
            "fbalpha2012_cps1_libretro.dll",
            "fbalpha2012_libretro.dll",
            "fbneo_libretro.dll",
        ]);
        let found = scan_cores_dir(&dir);
        let fbneo = found.iter().find(|c| c.profile_id == "fbneo").unwrap();
        assert!(
            fbneo.matched_filename.contains("fbneo"),
            "expected fbneo DLL, got {}",
            fbneo.matched_filename
        );
    }

    fn tempfile_cores(names: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "arr-cores-{}-{}",
            std::process::id(),
            names.len()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in names {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        dir
    }
}
