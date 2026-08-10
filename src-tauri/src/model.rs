//! Domain types shared across the backend and serialized at the Tauri boundary.
//!
//! Backend code uses enums; the boundary uses the stable string values defined
//! here, per SPEC.md section 59.

use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $name:ident { $($variant:ident => $text:literal),+ $(,)? }
    ) => {
        // `derive` must precede the serde helper attributes it introduces.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        $(#[$meta])*
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            pub fn parse(value: &str) -> Option<Self> {
                match value { $($text => Some(Self::$variant),)+ _ => None }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

string_enum! {
    /// Outcome of inspecting one file in a ROM root.
    ///
    /// Phase 1 only distinguishes readable from unreadable content. Compatibility
    /// states (SPEC.md section 14) belong to the Phase 3 matching engine and are
    /// deliberately absent here.
    ArchiveState {
        Indexed => "INDEXED",
        DiskImageIndexed => "DISK_IMAGE_INDEXED",
        ArchiveUnreadable => "ARCHIVE_UNREADABLE",
    }
}

string_enum! {
    JobType {
        FullScan => "FULL_SCAN",
        IncrementalScan => "INCREMENTAL_SCAN",
        DeepVerify => "DEEP_VERIFY",
    }
}

string_enum! {
    JobState {
        Queued => "QUEUED",
        Running => "RUNNING",
        Paused => "PAUSED",
        Cancelling => "CANCELLING",
        Cancelled => "CANCELLED",
        Completed => "COMPLETED",
        Failed => "FAILED",
    }
}

impl JobState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed | Self::Failed)
    }
}

string_enum! {
    /// Progress phases surfaced to the UI (SPEC.md section 30.4). Later phases
    /// add matching, dependency resolution, and route selection.
    ScanPhase {
        EnumeratingFiles => "ENUMERATING_FILES",
        InspectingArchives => "INSPECTING_ARCHIVES",
        Finalizing => "FINALIZING",
    }
}

/// How much work a scan should redo. Maps onto the three rescan modes in
/// SPEC.md section 68.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScanMode {
    /// Reuse cached results for archives whose quick signature is unchanged.
    Quick,
    /// Re-inspect every archive, ignoring the cache.
    Full,
    /// Re-inspect everything and additionally compute a SHA-256 of each file.
    DeepVerify,
}

impl ScanMode {
    pub fn job_type(&self) -> JobType {
        match self {
            Self::Quick => JobType::IncrementalScan,
            Self::Full => JobType::FullScan,
            Self::DeepVerify => JobType::DeepVerify,
        }
    }

    pub fn uses_cache(&self) -> bool {
        matches!(self, Self::Quick)
    }

    pub fn hashes_whole_file(&self) -> bool {
        matches!(self, Self::DeepVerify)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RomRoot {
    pub id: i64,
    pub path: String,
    pub label: Option<String>,
    pub recursive: bool,
    pub enabled: bool,
    pub read_only: bool,
    pub watch_changes: bool,
    pub created_at: String,
    pub last_scan_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveRow {
    pub id: i64,
    pub rom_root_id: i64,
    pub path: String,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: i64,
    pub modified_at: Option<String>,
    pub sha256: Option<String>,
    pub archive_state: ArchiveState,
    pub member_count: i64,
    pub unsafe_member_count: i64,
    pub error_detail: Option<String>,
    pub last_scanned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveMemberRow {
    pub member_name: String,
    pub size_bytes: Option<i64>,
    pub compressed_size_bytes: Option<i64>,
    pub crc32: Option<String>,
    pub sha1: Option<String>,
    pub compression_method: Option<String>,
    pub is_directory: bool,
    pub name_is_safe: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySummary {
    pub total: i64,
    pub indexed: i64,
    pub disk_images: i64,
    pub unreadable: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivePage {
    pub rows: Vec<ArchiveRow>,
    pub total_matching: i64,
    pub summary: LibrarySummary,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trips_through_its_stored_text() {
        for state in [
            ArchiveState::Indexed,
            ArchiveState::DiskImageIndexed,
            ArchiveState::ArchiveUnreadable,
        ] {
            assert_eq!(ArchiveState::parse(state.as_str()), Some(state));
        }
    }

    #[test]
    fn unknown_stored_text_is_rejected_rather_than_defaulted() {
        assert_eq!(ArchiveState::parse("VERIFIED_PLAYABLE"), None);
        assert_eq!(JobState::parse(""), None);
    }

    #[test]
    fn scan_modes_map_to_spec_job_types() {
        assert_eq!(ScanMode::Quick.job_type(), JobType::IncrementalScan);
        assert_eq!(ScanMode::Full.job_type(), JobType::FullScan);
        assert_eq!(ScanMode::DeepVerify.job_type(), JobType::DeepVerify);

        assert!(ScanMode::Quick.uses_cache());
        assert!(!ScanMode::Full.uses_cache());
        assert!(!ScanMode::Full.hashes_whole_file());
        assert!(ScanMode::DeepVerify.hashes_whole_file());
    }
}
