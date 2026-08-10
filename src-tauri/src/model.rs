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

string_enum! {
    HealthState {
        Unknown => "UNKNOWN",
        Healthy => "HEALTHY",
        NeedsDat => "NEEDS_DAT",
        MissingCore => "MISSING_CORE",
        MissingExecutable => "MISSING_EXECUTABLE",
        Unhealthy => "UNHEALTHY",
    }
}

string_enum! {
    MatchConfidence {
        Verified => "VERIFIED",
        Strong => "STRONG",
        Partial => "PARTIAL",
        Unknown => "UNKNOWN",
    }
}

string_enum! {
    /// Compatibility / readiness states (SPEC.md §14), produced by matching.
    CompatibilityState {
        VerifiedPlayable => "VERIFIED_PLAYABLE",
        VerifiedPlayableWithDependencies => "VERIFIED_PLAYABLE_WITH_DEPENDENCIES",
        MultipleValidRoutes => "MULTIPLE_VALID_ROUTES",
        MissingParent => "MISSING_PARENT",
        MissingBios => "MISSING_BIOS",
        MissingDevice => "MISSING_DEVICE",
        MissingChd => "MISSING_CHD",
        MissingSamplesOptional => "MISSING_SAMPLES_OPTIONAL",
        IncompleteSet => "INCOMPLETE_SET",
        WrongRomRevision => "WRONG_ROM_REVISION",
        KnownSetNameUnverifiedContent => "KNOWN_SET_NAME_UNVERIFIED_CONTENT",
        RecognizedRomContentAmbiguousSet => "RECOGNIZED_ROM_CONTENT_AMBIGUOUS_SET",
        ArchiveUnreadable => "ARCHIVE_UNREADABLE",
        Unidentified => "UNIDENTIFIED",
        EmulatorNotInstalled => "EMULATOR_NOT_INSTALLED",
        CoreNotInstalled => "CORE_NOT_INSTALLED",
        DatNotInstalled => "DAT_NOT_INSTALLED",
        RouteUnavailable => "ROUTE_UNAVAILABLE",
        UserDisabled => "USER_DISABLED",
        PlayableWithAudioSampleWarning => "PLAYABLE_WITH_AUDIO_SAMPLE_WARNING",
    }
}

string_enum! {
    RoutePreferenceMode {
        Balanced => "BALANCED",
        MaximumLegacy => "MAXIMUM_LEGACY",
        Preservation => "PRESERVATION",
        Performance => "PERFORMANCE",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorProfile {
    pub id: String,
    pub display_name: String,
    pub runner_type: String,
    pub executable_path: Option<String>,
    pub core_path: Option<String>,
    pub core_signature: Option<String>,
    pub enabled: bool,
    pub priority: i64,
    pub settings_json: String,
    pub last_health_check: Option<String>,
    pub health_state: HealthState,
    pub games_matched: i64,
    pub has_active_dat: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatSource {
    pub id: i64,
    pub emulator_profile_id: String,
    pub display_name: String,
    pub source_type: String,
    pub version: Option<String>,
    pub path: String,
    pub sha256: String,
    pub machine_count: i64,
    pub rom_entry_count: i64,
    pub disk_entry_count: i64,
    pub imported_at: String,
    pub active: bool,
    pub parser_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSummary {
    pub id: i64,
    pub dat_source_id: i64,
    pub set_name: String,
    pub description: Option<String>,
    pub year: Option<String>,
    pub manufacturer: Option<String>,
    pub clone_of: Option<String>,
    pub rom_of: Option<String>,
    pub is_bios: bool,
    pub runnable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineRomRow {
    pub name: String,
    pub size_bytes: Option<i64>,
    pub crc32: Option<String>,
    pub sha1: Option<String>,
    pub status: Option<String>,
    pub optional: bool,
    pub merge_name: Option<String>,
    pub bios_name: Option<String>,
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineDiskRow {
    pub name: String,
    pub sha1: Option<String>,
    pub status: Option<String>,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResultRow {
    pub id: i64,
    pub archive_id: i64,
    pub machine_id: i64,
    pub emulator_profile_id: String,
    pub dat_source_id: i64,
    pub state: CompatibilityState,
    pub confidence: MatchConfidence,
    pub matched_required: i64,
    pub missing_required: i64,
    pub wrong_required: i64,
    pub score: f64,
    pub evidence_json: String,
    pub created_at: String,
    pub machine: Option<MachineSummary>,
    pub profile_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRow {
    pub id: i64,
    pub archive_id: i64,
    pub machine_id: i64,
    pub emulator_profile_id: String,
    pub match_result_id: i64,
    pub is_selected: bool,
    pub selection_reason: Option<String>,
    pub user_override: bool,
    pub launchable: bool,
    pub profile_display_name: Option<String>,
    pub machine_set_name: Option<String>,
    pub state: Option<CompatibilityState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    pub archive: ArchiveRow,
    pub can_run: String,
    pub can_run_reason: String,
    pub selected_route: Option<RouteRow>,
    pub routes: Vec<RouteRow>,
    pub matches: Vec<MatchResultRow>,
    pub members: Vec<ArchiveMemberRow>,
    pub dependencies: Vec<DependencyStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub kind: String,
    pub name: String,
    pub present: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemSummary {
    pub missing_parent: i64,
    pub missing_bios: i64,
    pub missing_device: i64,
    pub missing_chd: i64,
    pub incomplete_set: i64,
    pub unidentified: i64,
    pub unreadable: i64,
    pub core_not_installed: i64,
    pub dat_not_installed: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetroArchDiscovery {
    pub executable_path: Option<String>,
    pub cores_dir: Option<String>,
    pub system_dir: Option<String>,
    pub config_path: Option<String>,
    pub detected_cores: Vec<DetectedCore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedCore {
    pub profile_id: String,
    pub display_name: String,
    pub core_path: String,
    pub matched_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub play_history_id: i64,
    pub pid: u32,
    pub started_at: String,
    pub core_path: String,
    pub content_path: String,
    pub log_path: Option<String>,
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
