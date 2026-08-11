//! CatVer.ini `[Category]` parser (progetto-SNAPS / MAME frontend format).

use std::collections::HashMap;
use std::path::Path;

use crate::error::{AppError, AppResult};

/// One set → category mapping from the `[Category]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryEntry {
    pub set_name: String,
    pub category: String,
}

/// Parses the `[Category]` block from a CatVer.ini-style file.
///
/// Ignores other sections (e.g. `[VerAdded]`), blank lines, and `;` / `#` comments.
pub fn parse_category_section(text: &str) -> Vec<CategoryEntry> {
    let mut in_category = false;
    let mut out = Vec::new();
    // Last write wins for duplicate set names within the file.
    let mut seen: HashMap<String, usize> = HashMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let section = line[1..line.len() - 1].trim();
            in_category = section.eq_ignore_ascii_case("Category");
            continue;
        }
        if !in_category {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let set_name = key.trim().to_ascii_lowercase();
        let category = value.trim().to_string();
        if set_name.is_empty() || category.is_empty() {
            continue;
        }

        if let Some(idx) = seen.get(&set_name).copied() {
            out[idx] = CategoryEntry {
                set_name: set_name.clone(),
                category,
            };
        } else {
            seen.insert(set_name.clone(), out.len());
            out.push(CategoryEntry { set_name, category });
        }
    }

    out
}

pub fn parse_file(path: &Path) -> AppResult<Vec<CategoryEntry>> {
    let text = std::fs::read_to_string(path).map_err(|source| AppError::Filesystem {
        path: path.display().to_string(),
        source,
    })?;
    let entries = parse_category_section(&text);
    if entries.is_empty() {
        return Err(AppError::user(
            "No categories found",
            "That file has no usable [Category] entries. Use a CatVer.ini from progetto-SNAPS or a libretro MAME metadata pack.",
        ));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_category_and_skips_ver_added() {
        let text = r#"
; comment
[Category]
1942 = Shooter / Flying Vertical
mspacman=Maze / Collect
[VerAdded]
1942 = 0.37b16
"#;
        let entries = parse_category_section(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].set_name, "1942");
        assert_eq!(entries[0].category, "Shooter / Flying Vertical");
        assert_eq!(entries[1].set_name, "mspacman");
        assert_eq!(entries[1].category, "Maze / Collect");
    }

    #[test]
    fn duplicate_set_name_last_wins() {
        let text = "[Category]\nfoo = First\nfoo = Second\n";
        let entries = parse_category_section(text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, "Second");
    }
}
