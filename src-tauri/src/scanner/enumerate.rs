//! Candidate discovery inside a ROM root.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

/// Content types the MVP inventories (SPEC.md section 11).
pub const SUPPORTED_EXTENSIONS: &[&str] = &["zip", "chd"];

/// Extensions the spec calls out as ignored. The scanner uses an allowlist, so
/// this list is not what does the filtering; it exists so the documented policy
/// stays visible and testable.
pub const IGNORED_EXTENSIONS: &[&str] = &[
    "txt", "nfo", "jpg", "png", "ini", "cfg", "exe", "dll", "bat", "cmd",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Default)]
pub struct Enumeration {
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
}

/// Lowercased extension without the dot, or an empty string when absent.
pub fn normalized_extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn is_supported_extension(extension: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&extension)
}

/// Walks a root and returns the files worth inspecting.
///
/// Symlinks are not followed, which prevents both directory cycles and reads
/// that would escape the configured root. Unreadable directory entries become
/// warnings rather than terminating the walk.
pub fn enumerate(root: &Path, recursive: bool) -> Enumeration {
    let mut result = Enumeration::default();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .max_depth(if recursive { usize::MAX } else { 1 });

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                result.warnings.push(error.to_string());
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let extension = normalized_extension(path);
        if !is_supported_extension(&extension) {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            result
                .warnings
                .push(format!("skipped file with unreadable name: {}", path.display()));
            continue;
        };

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                result
                    .warnings
                    .push(format!("{}: {error}", path.display()));
                continue;
            }
        };

        result.candidates.push(Candidate {
            path: path.to_path_buf(),
            file_name: file_name.to_string(),
            extension,
            size_bytes: metadata.len(),
            modified: metadata.modified().ok(),
        });
    }

    result.candidates.sort_by(|a, b| a.path.cmp(&b.path));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn only_supported_extensions_are_candidates() {
        for extension in SUPPORTED_EXTENSIONS {
            assert!(is_supported_extension(extension));
        }
        for extension in IGNORED_EXTENSIONS {
            assert!(!is_supported_extension(extension));
        }
        assert!(!is_supported_extension("7z"), "7z is Phase 2");
        assert!(!is_supported_extension(""));
    }

    #[test]
    fn extension_matching_ignores_case() {
        assert_eq!(normalized_extension(Path::new("A:\\SF2.ZIP")), "zip");
        assert!(is_supported_extension(&normalized_extension(Path::new(
            "1942.Zip"
        ))));
    }

    #[test]
    fn executables_and_notes_are_never_enumerated() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("sf2.zip"));
        touch(&dir.path().join("readme.txt"));
        touch(&dir.path().join("setup.exe"));
        touch(&dir.path().join("launch.bat"));
        touch(&dir.path().join("art.png"));

        let found = enumerate(dir.path(), true);
        let names: Vec<&str> = found
            .candidates
            .iter()
            .map(|c| c.file_name.as_str())
            .collect();

        assert_eq!(names, vec!["sf2.zip"]);
    }

    #[test]
    fn recursion_can_be_switched_off() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("top.zip"));
        touch(&dir.path().join("nested").join("deep.zip"));

        assert_eq!(enumerate(dir.path(), true).candidates.len(), 2);
        assert_eq!(enumerate(dir.path(), false).candidates.len(), 1);
    }

    #[test]
    fn chd_files_are_inventoried_alongside_zips() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("game.zip"));
        touch(&dir.path().join("game").join("game.chd"));

        let found = enumerate(dir.path(), true);
        assert_eq!(found.candidates.len(), 2);
        assert!(found.candidates.iter().any(|c| c.extension == "chd"));
    }

    #[test]
    fn a_missing_root_yields_a_warning_rather_than_a_panic() {
        let found = enumerate(Path::new("Z:\\does-not-exist-arr"), true);
        assert!(found.candidates.is_empty());
        assert!(!found.warnings.is_empty());
    }
}
