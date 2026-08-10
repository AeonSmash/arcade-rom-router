//! Read-only inspection of files found inside a ROM root.

pub mod fs_readonly;
pub mod zip_reader;

use std::path::Path;

use crate::model::ArchiveState;
use zip_reader::ZipMember;

/// What a single candidate file turned out to be.
#[derive(Debug, Clone)]
pub struct InspectedArchive {
    pub state: ArchiveState,
    pub members: Vec<ZipMember>,
    pub error_detail: Option<String>,
}

impl InspectedArchive {
    pub fn unsafe_member_count(&self) -> i64 {
        self.members.iter().filter(|m| !m.name_is_safe).count() as i64
    }

    pub fn member_count(&self) -> i64 {
        self.members.len() as i64
    }
}

/// Inspects one candidate file.
///
/// This never returns `Err`: an archive that cannot be parsed is itself a
/// finding, recorded as `ARCHIVE_UNREADABLE` with the exact parse error kept
/// for diagnostics so a single damaged file cannot abort a scan
/// (SPEC.md section 12.4).
pub fn inspect(path: &Path, extension: &str) -> InspectedArchive {
    match extension {
        // CHDs are indexed by path and size only. Hashing multi-gigabyte disk
        // images during a normal scan is forbidden by SPEC.md section 41.
        "chd" => InspectedArchive {
            state: ArchiveState::DiskImageIndexed,
            members: Vec::new(),
            error_detail: None,
        },
        _ => match zip_reader::inspect(path) {
            Ok(inspection) => InspectedArchive {
                state: ArchiveState::Indexed,
                error_detail: if inspection.warnings.is_empty() {
                    None
                } else {
                    Some(inspection.warnings.join("; "))
                },
                members: inspection.members,
            },
            Err(error) => InspectedArchive {
                state: ArchiveState::ArchiveUnreadable,
                members: Vec::new(),
                error_detail: Some(error.to_string()),
            },
        },
    }
}
