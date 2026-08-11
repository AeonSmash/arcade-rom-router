//! Local artwork folder resolution (SPEC.md §42.1).

use std::path::{Path, PathBuf};

use crate::model::MediaKind;

const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

pub fn kind_folder(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Box => "box",
        MediaKind::Screenshot => "screenshot",
        MediaKind::Title => "title",
        MediaKind::Marquee => "marquee",
        MediaKind::Cabinet => "cabinet",
        MediaKind::Video => "video",
        MediaKind::Manual => "manual",
    }
}

pub fn normalize_title(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '-' || ch == '_' {
            if !out.ends_with('_') {
                out.push('_');
            }
        }
    }
    out.trim_matches('_').to_string()
}

/// Finds the first matching asset for any of the lookup names under root/kind/.
pub fn find_asset(root: &Path, kind: &MediaKind, names: &[String]) -> Option<PathBuf> {
    let folder = root.join(kind_folder(kind));
    if !folder.is_dir() {
        // Also allow flat layout: root/name.png
        return find_in_dir(root, names);
    }
    find_in_dir(&folder, names).or_else(|| find_in_dir(root, names))
}

fn find_in_dir(dir: &Path, names: &[String]) -> Option<PathBuf> {
    for name in names {
        for ext in EXTENSIONS {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
            let candidate = dir.join(format!("{}.{ext}", normalize_title(name)));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Case-insensitive scan for small folders.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };
    let wanted: Vec<String> = names
        .iter()
        .flat_map(|n| {
            EXTENSIONS
                .iter()
                .map(|ext| format!("{}.{}", normalize_title(n), ext))
                .collect::<Vec<_>>()
        })
        .collect();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(fname) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let normalized = normalize_title(fname);
        // compare stem+ext normalized loosely
        for w in &wanted {
            if normalize_title(w) == normalized || fname.eq_ignore_ascii_case(w) {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_punctuation() {
        assert_eq!(normalize_title("Ms. Pac-Man"), "ms_pac_man");
        assert_eq!(normalize_title("1942"), "1942");
    }

    #[test]
    fn finds_asset_by_set_name() {
        let dir = tempfile::tempdir().unwrap();
        let box_dir = dir.path().join("box");
        std::fs::create_dir_all(&box_dir).unwrap();
        let file = box_dir.join("galaga.png");
        std::fs::write(&file, b"png").unwrap();
        let found = find_asset(dir.path(), &MediaKind::Box, &["galaga".into()]);
        assert_eq!(found.unwrap(), file);
    }
}
