//! Metadata-only ZIP inspection.
//!
//! Arcade ROM sets store one chip dump per archive member, and the CRC32 of
//! each member is the primary matching evidence (SPEC.md section 12.3). That
//! CRC is already recorded in the ZIP central directory, so the scanner reads
//! it directly and never inflates a single byte of member data. The `zip`
//! dependency is compiled without any compression codecs to make that
//! structural rather than incidental.

use std::io::BufReader;
use std::path::Path;

use crate::archive::fs_readonly;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipMember {
    pub name: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: String,
    pub compression_method: String,
    pub is_directory: bool,
    /// False when the stored name tries to escape its archive. Such members are
    /// recorded as evidence but are never treated as extractable paths.
    pub name_is_safe: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZipInspection {
    pub members: Vec<ZipMember>,
    /// Per-entry problems that did not invalidate the archive as a whole.
    pub warnings: Vec<String>,
}

impl ZipInspection {
    pub fn unsafe_member_count(&self) -> usize {
        self.members.iter().filter(|m| !m.name_is_safe).count()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ZipReadError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Rejects member names that would escape the archive if they were ever
/// written to disk (SPEC.md section 43.1).
///
/// The scanner does not extract anything, so an unsafe name cannot cause harm
/// today. It is still refused here so that the rule is already in force before
/// any future feature gains the ability to write files.
pub fn is_safe_member_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') {
        return false;
    }

    // Absolute POSIX paths, Windows roots, and UNC shares.
    if name.starts_with('/') || name.starts_with('\\') {
        return false;
    }

    let bytes = name.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        return false;
    }

    !name
        .split(['/', '\\'])
        .any(|component| component == ".." || component == "...")
}

/// Reads every member's metadata from the central directory.
///
/// Returns `Err` only when the archive itself cannot be parsed; problems with
/// individual entries become warnings so that one bad member never discards an
/// otherwise usable inventory.
pub fn inspect(path: &Path) -> Result<ZipInspection, ZipReadError> {
    let file = fs_readonly::open_read(path)?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file))?;

    let mut inspection = ZipInspection {
        members: Vec::with_capacity(archive.len()),
        warnings: Vec::new(),
    };

    for index in 0..archive.len() {
        // `by_index_raw` hands back the entry without constructing a
        // decompressor, so no member data is ever inflated.
        let entry = match archive.by_index_raw(index) {
            Ok(entry) => entry,
            Err(error) => {
                inspection
                    .warnings
                    .push(format!("entry {index} could not be read: {error}"));
                continue;
            }
        };

        let name = entry.name().to_string();
        let declared_directory = entry.is_dir();

        inspection.members.push(ZipMember {
            name_is_safe: is_safe_member_name(&name),
            is_directory: declared_directory,
            uncompressed_size: entry.size(),
            compressed_size: entry.compressed_size(),
            crc32: format!("{:08x}", entry.crc32()),
            compression_method: entry.compression().to_string(),
            name,
        });
    }

    Ok(inspection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_rom_chip_names_are_accepted() {
        assert!(is_safe_member_name("abc123.bin"));
        assert!(is_safe_member_name("sub/abc123.bin"));
        assert!(is_safe_member_name("a.b.c"));
        assert!(is_safe_member_name("..leading-dots.bin"));
    }

    #[test]
    fn traversal_and_absolute_names_are_rejected() {
        assert!(!is_safe_member_name("../escape.bin"));
        assert!(!is_safe_member_name("sub/../../escape.bin"));
        assert!(!is_safe_member_name("..\\escape.bin"));
        assert!(!is_safe_member_name("/etc/passwd"));
        assert!(!is_safe_member_name("\\\\server\\share\\rom.bin"));
        assert!(!is_safe_member_name("C:\\Windows\\system32\\rom.bin"));
        assert!(!is_safe_member_name(""));
        assert!(!is_safe_member_name("nul\0byte.bin"));
    }
}
