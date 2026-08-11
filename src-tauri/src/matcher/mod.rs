//! Content matching and dependency resolution (Phases 3–4).
//!
//! Launch identity is the archive filename: a match against a differently-named
//! clone can never be a valid RetroArch/libretro route. Required chips include
//! DAT merge/romof entries, and evidence is the union of the archive's members
//! with any ancestor-named parent zips (split sets).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::json;
use sqlx::SqlitePool;
use tracing::info;

use crate::db::{archives, dats, machines, matches as matches_db, profiles};
use crate::error::AppResult;
use crate::model::{
    ArchiveMemberRow, ArchiveRow, ArchiveState, CompatibilityState, DependencyStatus,
    MatchConfidence, MachineRomRow, MachineSummary,
};
use crate::routing;

/// Stem of an archive filename without extension, lowercased.
pub fn normalize_set_name(file_name: &str) -> String {
    let path = Path::new(file_name);
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name)
        .to_ascii_lowercase()
}

struct MemberEvidence {
    by_crc_size: HashMap<(String, i64), String>,
    by_crc: HashMap<String, Vec<String>>,
    by_name: HashSet<String>,
}

fn evidence_from_members(members: &[ArchiveMemberRow]) -> MemberEvidence {
    let mut by_crc_size = HashMap::new();
    let mut by_crc: HashMap<String, Vec<String>> = HashMap::new();
    let mut by_name = HashSet::new();

    for member in members {
        if member.is_directory {
            continue;
        }
        by_name.insert(member.member_name.to_ascii_lowercase());
        if let Some(crc) = member.crc32.as_ref() {
            let crc = crc.to_ascii_lowercase();
            by_crc
                .entry(crc.clone())
                .or_default()
                .push(member.member_name.clone());
            if let Some(size) = member.size_bytes {
                by_crc_size.insert((crc, size), member.member_name.clone());
            }
        }
    }

    MemberEvidence {
        by_crc_size,
        by_crc,
        by_name,
    }
}

fn merge_evidence(into: &mut MemberEvidence, from: &MemberEvidence) {
    for (k, v) in &from.by_crc_size {
        into.by_crc_size.entry(k.clone()).or_insert_with(|| v.clone());
    }
    for (crc, names) in &from.by_crc {
        into.by_crc.entry(crc.clone()).or_default().extend(names.iter().cloned());
    }
    into.by_name.extend(from.by_name.iter().cloned());
}

fn compare_roms(
    required: &[MachineRomRow],
    evidence: &MemberEvidence,
) -> (i64, i64, i64, Vec<serde_json::Value>) {
    let mut matched = 0i64;
    let mut missing = 0i64;
    let mut wrong = 0i64;
    let mut events = Vec::new();

    for rom in required {
        let Some(crc) = rom.crc32.as_ref() else {
            missing += 1;
            events.push(json!({
                "type": "rom-missing-crc",
                "name": rom.name,
            }));
            continue;
        };
        let crc = crc.to_ascii_lowercase();

        let hit = if let Some(size) = rom.size_bytes {
            evidence.by_crc_size.contains_key(&(crc.clone(), size))
        } else {
            evidence.by_crc.contains_key(&crc)
        };

        if hit {
            matched += 1;
            events.push(json!({
                "type": "rom-checksum-match",
                "name": rom.name,
                "crc32": crc,
            }));
        } else if evidence.by_name.contains(&rom.name.to_ascii_lowercase()) {
            wrong += 1;
            events.push(json!({
                "type": "rom-wrong-revision",
                "name": rom.name,
            }));
        } else {
            missing += 1;
            events.push(json!({
                "type": "rom-missing",
                "name": rom.name,
                "crc32": crc,
            }));
        }
    }

    (matched, missing, wrong, events)
}

async fn chd_present_for_set(pool: &SqlitePool, set_name: &str, disk_name: &str) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM archives WHERE extension = 'chd' AND (
             lower(file_name) = lower(?1)
             OR lower(file_name) = lower(?2)
             OR lower(path) LIKE lower(?3)
         )",
    )
    .bind(format!("{disk_name}.chd"))
    .bind(format!("{set_name}.chd"))
    .bind(format!("%{set_name}%{disk_name}.chd"))
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

struct DepOutcome {
    parent_missing: bool,
    bios_missing: bool,
    chd_missing: bool,
    deps: Vec<DependencyStatus>,
    events: Vec<serde_json::Value>,
}

/// Dependency resolution using preloaded stem→archive presence. Parent presence
/// means an ancestor-named zip exists; chip completeness is checked separately
/// via unioned MemberEvidence.
fn resolve_dependencies_with_index(
    summary: &MachineSummary,
    chain: &[MachineSummary],
    stem_to_archive: &HashMap<String, i64>,
    disks: &[crate::model::MachineDiskRow],
    chd_present: &HashMap<(String, String), bool>,
) -> DepOutcome {
    let mut deps = Vec::new();
    let mut events = Vec::new();
    let mut parent_missing = false;
    let mut bios_missing = false;
    let mut chd_missing = false;

    for ancestor in chain.iter().skip(1) {
        let present = stem_to_archive.contains_key(&ancestor.set_name.to_ascii_lowercase());
        let kind = if ancestor.is_bios { "bios" } else { "parent" };
        deps.push(DependencyStatus {
            kind: kind.into(),
            name: ancestor.set_name.clone(),
            present,
            detail: if present {
                format!("{kind} set found in library")
            } else {
                format!(
                    "Required {kind} set “{}” was not found among indexed archives",
                    ancestor.set_name
                )
            },
        });
        if !present {
            if ancestor.is_bios {
                bios_missing = true;
            } else {
                parent_missing = true;
            }
            events.push(json!({
                "type": if ancestor.is_bios { "bios-not-found" } else { "parent-not-found" },
                "name": ancestor.set_name,
            }));
        } else {
            events.push(json!({
                "type": if ancestor.is_bios { "bios-present" } else { "parent-present" },
                "name": ancestor.set_name,
            }));
        }
    }

    for disk in disks.iter().filter(|d| !d.optional) {
        let key = (summary.set_name.to_ascii_lowercase(), disk.name.to_ascii_lowercase());
        let present = chd_present.get(&key).copied().unwrap_or(false);
        deps.push(DependencyStatus {
            kind: "chd".into(),
            name: disk.name.clone(),
            present,
            detail: if present {
                "CHD found near the ROM set".into()
            } else {
                format!("Required disk image “{}.chd” was not found", disk.name)
            },
        });
        if !present {
            chd_missing = true;
            events.push(json!({"type": "chd-not-found", "name": disk.name}));
        }
    }

    DepOutcome {
        parent_missing,
        bios_missing,
        chd_missing,
        deps,
        events,
    }
}

fn classify(
    matched: i64,
    missing: i64,
    wrong: i64,
    required_total: i64,
    name_match: bool,
    deps: &DepOutcome,
    profile_healthy: bool,
    core_installed: bool,
    has_dat: bool,
) -> (CompatibilityState, MatchConfidence, f64) {
    if !has_dat {
        return (CompatibilityState::DatNotInstalled, MatchConfidence::Unknown, 0.0);
    }
    if !core_installed {
        return (
            CompatibilityState::CoreNotInstalled,
            MatchConfidence::Partial,
            if matched > 0 { 40.0 } else { 0.0 },
        );
    }

    // Content that matches a differently-named machine cannot be a launch route.
    if !name_match {
        let score = if matched > 0 { 45.0 } else { 10.0 };
        return (
            CompatibilityState::RecognizedRomContentAmbiguousSet,
            MatchConfidence::Partial,
            score,
        );
    }

    let mut score = 0.0;
    if required_total > 0 && missing == 0 && wrong == 0 {
        score += 70.0;
    }
    score += 15.0; // name_match guaranteed above
    if !deps.parent_missing && !deps.bios_missing {
        score += 5.0;
    }
    if !deps.chd_missing {
        score += 5.0;
    }
    if profile_healthy {
        score += 5.0;
    }
    if wrong > 0 {
        score -= 50.0;
    }
    if deps.parent_missing {
        score -= 40.0;
    }
    if deps.bios_missing {
        score -= 40.0;
    }
    if deps.chd_missing {
        score -= 40.0;
    }

    let complete = required_total > 0 && missing == 0 && wrong == 0;
    let deps_ok = !deps.parent_missing && !deps.bios_missing && !deps.chd_missing;

    let (state, confidence) = if complete && deps_ok {
        (
            CompatibilityState::VerifiedPlayable,
            MatchConfidence::Verified,
        )
    } else if complete && (deps.parent_missing || deps.bios_missing) {
        let state = if deps.bios_missing {
            CompatibilityState::MissingBios
        } else {
            CompatibilityState::MissingParent
        };
        (state, MatchConfidence::Strong)
    } else if complete && deps.chd_missing {
        (CompatibilityState::MissingChd, MatchConfidence::Strong)
    } else if complete {
        (
            CompatibilityState::VerifiedPlayableWithDependencies,
            MatchConfidence::Strong,
        )
    } else if wrong > 0 && matched > 0 {
        (CompatibilityState::WrongRomRevision, MatchConfidence::Partial)
    } else if matched > 0 {
        (CompatibilityState::IncompleteSet, MatchConfidence::Partial)
    } else {
        (
            CompatibilityState::KnownSetNameUnverifiedContent,
            MatchConfidence::Unknown,
        )
    };

    (state, confidence, f64::max(score, 0.0))
}

/// Preloaded indexes for a full-library rematch.
struct RematchIndex {
    members_by_archive: HashMap<i64, Vec<ArchiveMemberRow>>,
    stem_to_archive: HashMap<String, i64>,
    /// (set_name_lower, disk_name_lower) → present
    chd_present: HashMap<(String, String), bool>,
}

async fn build_rematch_index(pool: &SqlitePool, archives_list: &[ArchiveRow]) -> AppResult<RematchIndex> {
    let mut members_by_archive = HashMap::new();
    let mut stem_to_archive = HashMap::new();

    for archive in archives_list {
        let stem = normalize_set_name(&archive.file_name);
        stem_to_archive.insert(stem, archive.id);
        let members = archives::members(pool, archive.id).await?;
        members_by_archive.insert(archive.id, members);
    }

    // CHD presence keyed for any disk name we might look up later is expensive
    // to precompute exhaustively; resolve_dependencies fills via query when
    // needed. For rematch we batch-load CHD filenames.
    let mut chd_present = HashMap::new();
    let chd_rows = sqlx::query(
        "SELECT lower(file_name) AS file_name, lower(path) AS path
         FROM archives WHERE extension = 'chd'",
    )
    .fetch_all(pool)
    .await?;
    for row in chd_rows {
        use sqlx::Row;
        let file_name: String = row.get("file_name");
        let path: String = row.get("path");
        // Mark as present under both bare disk name and path-substring keys
        // that resolve_dependencies_with_index may consult. We store a sentinel
        // keyed by ("*", disk) and ("*", set) for cheap lookup helpers.
        if let Some(stem) = Path::new(&file_name).file_stem().and_then(|s| s.to_str()) {
            chd_present.insert(("*".into(), stem.to_string()), true);
        }
        let _ = path; // path used by LIKE in the live query path; keep for future
    }

    Ok(RematchIndex {
        members_by_archive,
        stem_to_archive,
        chd_present,
    })
}

fn union_evidence_for_chain(
    archive_id: i64,
    chain: &[MachineSummary],
    index: &RematchIndex,
) -> MemberEvidence {
    let empty = Vec::new();
    let own = index
        .members_by_archive
        .get(&archive_id)
        .unwrap_or(&empty);
    let mut evidence = evidence_from_members(own);

    for ancestor in chain.iter().skip(1) {
        let stem = ancestor.set_name.to_ascii_lowercase();
        if let Some(&parent_id) = index.stem_to_archive.get(&stem) {
            if parent_id == archive_id {
                continue;
            }
            let parent_members = index
                .members_by_archive
                .get(&parent_id)
                .unwrap_or(&empty);
            let parent_ev = evidence_from_members(parent_members);
            merge_evidence(&mut evidence, &parent_ev);
        }
    }
    evidence
}

async fn match_named_machine(
    pool: &SqlitePool,
    archive: &ArchiveRow,
    dat: &crate::model::DatSource,
    machine_id: i64,
    index: &RematchIndex,
    content_hints: &[serde_json::Value],
) -> AppResult<matches_db::NewMatchResult> {
    let profile = profiles::get(pool, &dat.emulator_profile_id).await?;
    let profile_enabled = profile.as_ref().map(|p| p.enabled).unwrap_or(false);
    let core_installed = profile
        .as_ref()
        .and_then(|p| p.core_path.as_ref())
        .map(|p| Path::new(p).is_file())
        .unwrap_or(false);
    let exe_ok = profile
        .as_ref()
        .and_then(|p| p.executable_path.as_ref())
        .map(|p| Path::new(p).is_file())
        .unwrap_or(false);
    let profile_healthy = profile
        .as_ref()
        .map(|p| p.health_state == crate::model::HealthState::Healthy)
        .unwrap_or(false)
        || (core_installed && exe_ok);

    let chain = machines::chain_of(pool, machine_id).await?;
    let summary = chain.first().cloned().expect("chain includes seed");
    let required = machines::required_roms_full(pool, machine_id).await?;
    let required_total = required.len() as i64;

    let evidence = union_evidence_for_chain(archive.id, &chain, index);
    let (matched, missing, wrong, mut events) = compare_roms(&required, &evidence);
    events.push(json!({"type": "archive-name-match", "setName": summary.set_name}));
    events.extend(content_hints.iter().cloned());

    let disks = machines::disks_for_machine(pool, machine_id).await?;
    // Resolve CHD presence: check preloaded stems, fall back to live query.
    let mut chd_map = index.chd_present.clone();
    for disk in disks.iter().filter(|d| !d.optional) {
        let key = (
            summary.set_name.to_ascii_lowercase(),
            disk.name.to_ascii_lowercase(),
        );
        if !chd_map.contains_key(&key) {
            let present = chd_present_for_set(pool, &summary.set_name, &disk.name).await?;
            chd_map.insert(key, present);
        }
    }

    let deps = resolve_dependencies_with_index(
        &summary,
        &chain,
        &index.stem_to_archive,
        &disks,
        &chd_map,
    );
    events.extend(deps.events.clone());

    let (mut state, confidence, score) = classify(
        matched,
        missing,
        wrong,
        required_total,
        true, // filename-anchored
        &deps,
        profile_healthy,
        core_installed && exe_ok,
        true,
    );

    if !profile_enabled {
        state = CompatibilityState::UserDisabled;
    }

    Ok(matches_db::NewMatchResult {
        archive_id: archive.id,
        machine_id,
        emulator_profile_id: dat.emulator_profile_id.clone(),
        dat_source_id: dat.id,
        state,
        confidence,
        matched_required: matched,
        missing_required: missing,
        wrong_required: wrong,
        score,
        evidence_json: serde_json::to_string(&events).unwrap_or_else(|_| "[]".into()),
    })
}

/// Content-only hint when CRCs match a differently-named machine. Never launchable.
async fn content_hint_match(
    pool: &SqlitePool,
    archive: &ArchiveRow,
    dat: &crate::model::DatSource,
    machine_id: i64,
    index: &RematchIndex,
) -> AppResult<matches_db::NewMatchResult> {
    let profile = profiles::get(pool, &dat.emulator_profile_id).await?;
    let core_installed = profile
        .as_ref()
        .and_then(|p| p.core_path.as_ref())
        .map(|p| Path::new(p).is_file())
        .unwrap_or(false);
    let exe_ok = profile
        .as_ref()
        .and_then(|p| p.executable_path.as_ref())
        .map(|p| Path::new(p).is_file())
        .unwrap_or(false);

    let required = machines::required_roms_full(pool, machine_id).await?;
    let summary = machines::get_summary(pool, machine_id)
        .await?
        .expect("machine exists");
    let empty_chain = [summary.clone()];
    let evidence = union_evidence_for_chain(archive.id, &empty_chain, index);
    let (matched, missing, wrong, mut events) = compare_roms(&required, &evidence);
    events.push(json!({
        "type": "content-suggests-set",
        "setName": summary.set_name,
        "note": "CRC overlap with a differently-named machine; not a launch identity",
    }));

    let deps = DepOutcome {
        parent_missing: false,
        bios_missing: false,
        chd_missing: false,
        deps: vec![],
        events: vec![],
    };

    let (state, confidence, score) = classify(
        matched,
        missing,
        wrong,
        required.len() as i64,
        false, // no name match
        &deps,
        false,
        core_installed && exe_ok,
        true,
    );

    Ok(matches_db::NewMatchResult {
        archive_id: archive.id,
        machine_id,
        emulator_profile_id: dat.emulator_profile_id.clone(),
        dat_source_id: dat.id,
        state,
        confidence,
        matched_required: matched,
        missing_required: missing,
        wrong_required: wrong,
        score,
        evidence_json: serde_json::to_string(&events).unwrap_or_else(|_| "[]".into()),
    })
}

async fn match_archive_against_dat(
    pool: &SqlitePool,
    archive: &ArchiveRow,
    dat: &crate::model::DatSource,
    index: &RematchIndex,
) -> AppResult<Option<matches_db::NewMatchResult>> {
    if archive.archive_state == ArchiveState::ArchiveUnreadable {
        return Ok(None);
    }
    if archive.extension == "chd" {
        return Ok(None);
    }

    let set_name = normalize_set_name(&archive.file_name);
    let empty = Vec::new();
    let members = index
        .members_by_archive
        .get(&archive.id)
        .unwrap_or(&empty);
    let own_evidence = evidence_from_members(members);

    // CRC-derived hints (informational only when a name match exists).
    let mut content_ids: HashSet<i64> = HashSet::new();
    for ((crc, size), _) in &own_evidence.by_crc_size {
        for id in machines::machine_ids_for_crc(pool, dat.id, crc, Some(*size)).await? {
            content_ids.insert(id);
        }
    }

    if let Some(named_id) = machines::find_by_set_name(pool, dat.id, &set_name).await? {
        let mut hints = Vec::new();
        for id in content_ids.iter().filter(|id| **id != named_id).take(5) {
            if let Some(s) = machines::get_summary(pool, *id).await? {
                hints.push(json!({
                    "type": "content-suggests-set",
                    "setName": s.set_name,
                }));
            }
        }
        return Ok(Some(
            match_named_machine(pool, archive, dat, named_id, index, &hints).await?,
        ));
    }

    // No filename match: best content hint only (never VerifiedPlayable).
    if content_ids.is_empty() {
        return Ok(None);
    }

    let mut best: Option<matches_db::NewMatchResult> = None;
    for machine_id in content_ids {
        let candidate = content_hint_match(pool, archive, dat, machine_id, index).await?;
        let replace = match &best {
            None => true,
            Some(prev) => candidate.score > prev.score,
        };
        if replace {
            best = Some(candidate);
        }
    }
    Ok(best)
}

/// Rematch every indexed archive against all active DATs and rebuild routes.
pub async fn rematch_library(pool: &SqlitePool) -> AppResult<u64> {
    let dats = dats::list_active(pool).await?;
    if dats.is_empty() {
        info!("no active DATs; skipping rematch");
        return Ok(0);
    }

    matches_db::clear_all(pool).await?;

    let page = archives::page(
        pool,
        &archives::ArchiveQuery {
            limit: 100_000,
            favorites_only: false,
            ..Default::default()
        },
    )
    .await?;

    let index = build_rematch_index(pool, &page.rows).await?;
    let mut matched_archives = 0u64;

    for archive in &page.rows {
        let mut any = false;
        for dat in &dats {
            if let Some(result) = match_archive_against_dat(pool, archive, dat, &index).await? {
                matches_db::insert(pool, &result).await?;
                any = true;
            }
        }

        if any {
            matched_archives += 1;
        }

        routing::rebuild_routes_for_archive(pool, archive.id).await?;
    }

    info!(matched_archives, dats = dats.len(), "library rematch complete");
    Ok(matched_archives)
}

pub async fn dependencies_for_archive(
    pool: &SqlitePool,
    archive_id: i64,
) -> AppResult<Vec<DependencyStatus>> {
    let results = matches_db::for_archive(pool, archive_id).await?;
    let Some(best) = results.first() else {
        return Ok(Vec::new());
    };

    let chain = machines::chain_of(pool, best.machine_id).await?;
    let Some(summary) = chain.first() else {
        return Ok(Vec::new());
    };

    // Build a minimal stem index for ancestor presence.
    let page = archives::page(
        pool,
        &archives::ArchiveQuery {
            limit: 100_000,
            favorites_only: false,
            ..Default::default()
        },
    )
    .await?;
    let mut stem_to_archive = HashMap::new();
    for a in &page.rows {
        stem_to_archive.insert(normalize_set_name(&a.file_name), a.id);
    }

    let disks = machines::disks_for_machine(pool, best.machine_id).await?;
    let mut chd_map = HashMap::new();
    for disk in disks.iter().filter(|d| !d.optional) {
        let present = chd_present_for_set(pool, &summary.set_name, &disk.name).await?;
        chd_map.insert(
            (
                summary.set_name.to_ascii_lowercase(),
                disk.name.to_ascii_lowercase(),
            ),
            present,
        );
    }

    Ok(resolve_dependencies_with_index(summary, &chain, &stem_to_archive, &disks, &chd_map).deps)
}

/// Path used only by tests to locate a sibling CHD.
#[allow(dead_code)]
pub fn expected_chd_paths(rom_path: &Path, set_name: &str, disk_name: &str) -> Vec<PathBuf> {
    let parent = rom_path.parent().unwrap_or_else(|| Path::new("."));
    vec![
        parent.join(set_name).join(format!("{disk_name}.chd")),
        parent.join(format!("{disk_name}.chd")),
        parent.join(format!("{set_name}.chd")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_name_strips_extension_and_case() {
        assert_eq!(normalize_set_name("PACMAN.ZIP"), "pacman");
        assert_eq!(normalize_set_name("sf2.zip"), "sf2");
    }

    #[test]
    fn complete_named_match_classifies_as_verified() {
        let deps = DepOutcome {
            parent_missing: false,
            bios_missing: false,
            chd_missing: false,
            deps: vec![],
            events: vec![],
        };
        let (state, confidence, score) =
            classify(14, 0, 0, 14, true, &deps, true, true, true);
        assert_eq!(state, CompatibilityState::VerifiedPlayable);
        assert_eq!(confidence, MatchConfidence::Verified);
        assert!(score >= 70.0);
    }

    #[test]
    fn missing_parent_is_strong_but_not_launchable_state() {
        let deps = DepOutcome {
            parent_missing: true,
            bios_missing: false,
            chd_missing: false,
            deps: vec![],
            events: vec![],
        };
        let (state, confidence, _) = classify(10, 0, 0, 10, true, &deps, true, true, true);
        assert_eq!(state, CompatibilityState::MissingParent);
        assert_eq!(confidence, MatchConfidence::Strong);
    }

    #[test]
    fn content_match_without_name_never_verified() {
        let deps = DepOutcome {
            parent_missing: false,
            bios_missing: false,
            chd_missing: false,
            deps: vec![],
            events: vec![],
        };
        // 1-of-1 "complete" on the wrong clone — the sf2ceb → sf2ceub bug.
        let (state, confidence, _) = classify(1, 0, 0, 1, false, &deps, true, true, true);
        assert_eq!(
            state,
            CompatibilityState::RecognizedRomContentAmbiguousSet
        );
        assert_ne!(confidence, MatchConfidence::Verified);
        assert!(!matches!(
            state,
            CompatibilityState::VerifiedPlayable
                | CompatibilityState::VerifiedPlayableWithDependencies
        ));
    }

    #[test]
    fn incomplete_named_set_stays_incomplete() {
        let deps = DepOutcome {
            parent_missing: false,
            bios_missing: false,
            chd_missing: false,
            deps: vec![],
            events: vec![],
        };
        let (state, _, _) = classify(0, 11, 0, 11, true, &deps, true, true, true);
        assert_eq!(
            state,
            CompatibilityState::KnownSetNameUnverifiedContent
        );
        let (state2, _, _) = classify(3, 8, 0, 11, true, &deps, true, true, true);
        assert_eq!(state2, CompatibilityState::IncompleteSet);
    }

    #[test]
    fn compare_roms_reports_missing_chip_names() {
        let required = vec![MachineRomRow {
            name: "3.ic171".into(),
            size_bytes: Some(524288),
            crc32: Some("a2355d90".into()),
            sha1: None,
            status: None,
            optional: false,
            merge_name: None,
            bios_name: None,
            region: None,
        }];
        let evidence = MemberEvidence {
            by_crc_size: HashMap::new(),
            by_crc: HashMap::new(),
            by_name: HashSet::new(),
        };
        let (matched, missing, wrong, events) = compare_roms(&required, &evidence);
        assert_eq!(matched, 0);
        assert_eq!(missing, 1);
        assert_eq!(wrong, 0);
        assert!(events.iter().any(|e| e["type"] == "rom-missing"
            && e["name"] == "3.ic171"
            && e["crc32"] == "a2355d90"));
    }
}
