//! Read-only RetroArch configuration parsing (Phase 10).
//!
//! Never writes `retroarch.cfg`. Used to locate savestate directories.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Default)]
pub struct RetroArchConfig {
    pub values: HashMap<String, String>,
    pub path: Option<PathBuf>,
}

impl RetroArchConfig {
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.values
            .get(key)
            .map(|v| matches!(v.as_str(), "true" | "1"))
            .unwrap_or(default)
    }

    pub fn get_path(&self, key: &str) -> Option<PathBuf> {
        self.values.get(key).and_then(|v| {
            let trimmed = v.trim().trim_matches('"');
            if trimmed.is_empty() || trimmed == "nul" || trimmed == "null" {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        })
    }
}

/// Parses a RetroArch `.cfg` file (key = "value" lines).
pub fn parse_file(path: &Path) -> AppResult<RetroArchConfig> {
    let text = std::fs::read_to_string(path).map_err(|source| AppError::Filesystem {
        path: path.display().to_string(),
        source,
    })?;
    let mut cfg = parse_text(&text);
    cfg.path = Some(path.to_path_buf());
    Ok(cfg)
}

pub fn parse_text(text: &str) -> RetroArchConfig {
    let mut values = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, rest)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let mut value = rest.trim().to_string();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }
        values.insert(key, value);
    }
    RetroArchConfig {
        values,
        path: None,
    }
}

/// Resolves the directory that may contain save states for a given content file.
pub fn resolve_savestate_dirs(cfg: &RetroArchConfig, content_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let in_content = cfg.get_bool("savestates_in_content_dir", false);
    if in_content {
        if let Some(parent) = content_path.parent() {
            dirs.push(parent.to_path_buf());
        }
    }
    if let Some(base) = cfg.get_path("savestate_directory") {
        let sort = cfg.get_bool("sort_savestates_enable", false);
        let by_content = cfg.get_bool("sort_savestates_by_content_enable", false);
        if sort {
            // Common RetroArch layouts: <savestate_dir>/<core>/ and optionally content name.
            dirs.push(base.clone());
            if by_content {
                if let Some(stem) = content_path.file_stem().and_then(|s| s.to_str()) {
                    dirs.push(base.join(stem));
                }
            }
        } else {
            dirs.push(base);
        }
    }
    // Fallback: beside the content.
    if dirs.is_empty() {
        if let Some(parent) = content_path.parent() {
            dirs.push(parent.to_path_buf());
        }
    }
    dirs
}

/// Discovers slot files for a content basename under candidate directories.
pub fn discover_slots(
    dirs: &[PathBuf],
    content_stem: &str,
) -> Vec<(i64, PathBuf, Option<PathBuf>, bool)> {
    let mut found = Vec::new();
    let prefixes = [
        format!("{content_stem}.state"),
        format!("{content_stem}.state"),
    ];
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let is_entry = name.ends_with(".entry");
            let base_name = name.strip_suffix(".entry").unwrap_or(name);
            for prefix in &prefixes {
                if let Some(rest) = base_name.strip_prefix(prefix.as_str()) {
                    // "".state, ".state1", ".state42", or "state" already stripped via prefix
                    let slot_str = if rest.is_empty() {
                        "0"
                    } else if rest.starts_with('.') {
                        // unexpected
                        continue;
                    } else {
                        rest
                    };
                    // prefix is "{stem}.state" so rest is "" or "1" or "42"
                    let slot: i64 = if slot_str.is_empty() {
                        0
                    } else {
                        match slot_str.parse() {
                            Ok(n) => n,
                            Err(_) => continue,
                        }
                    };
                    let thumb = {
                        let png = path.with_extension(format!(
                            "{}png",
                            path.extension()
                                .and_then(|e| e.to_str())
                                .map(|e| format!("{e}."))
                                .unwrap_or_default()
                        ));
                        // RetroArch names thumbnails as file.stateN.png
                        let alt = PathBuf::from(format!("{}.png", path.display()));
                        if alt.is_file() {
                            Some(alt)
                        } else if png.is_file() {
                            Some(png)
                        } else {
                            let sibling = dir.join(format!("{name}.png"));
                            if sibling.is_file() {
                                Some(sibling)
                            } else {
                                None
                            }
                        }
                    };
                    found.push((slot, path.clone(), thumb, is_entry));
                    break;
                }
            }
        }
    }
    found.sort_by_key(|(slot, _, _, is_entry)| (*is_entry as u8, *slot));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_keys() {
        let cfg = parse_text(
            r#"
# comment
savestate_directory = "C:\Saves"
config_save_on_exit = "true"
sort_savestates_enable = "false"
"#,
        );
        assert_eq!(
            cfg.get_path("savestate_directory").unwrap(),
            PathBuf::from(r"C:\Saves")
        );
        assert!(cfg.get_bool("config_save_on_exit", false));
        assert!(!cfg.get_bool("sort_savestates_enable", true));
    }

    #[test]
    fn discover_slots_finds_numbered_states() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("galaga.state"), b"x").unwrap();
        std::fs::write(dir.path().join("galaga.state1"), b"x").unwrap();
        std::fs::write(dir.path().join("galaga.state2.entry"), b"x").unwrap();
        std::fs::write(dir.path().join("galaga.state1.png"), b"png").unwrap();
        let found = discover_slots(&[dir.path().to_path_buf()], "galaga");
        assert!(found.iter().any(|(s, _, _, e)| *s == 0 && !*e));
        assert!(found.iter().any(|(s, _, thumb, e)| *s == 1 && !*e && thumb.is_some()));
        assert!(found.iter().any(|(s, _, _, e)| *s == 2 && *e));
    }
}
