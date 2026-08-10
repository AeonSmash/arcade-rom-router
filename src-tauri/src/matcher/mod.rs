//! Content matching and dependency resolution (Phases 3–4).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::json;
use sqlx::SqlitePool;
use tracing::info;

use crate::db::{archives, dats, machines, matches as matches_db, profiles};
use crate::error::AppResult;
use crate::model::{
    ArchiveRow, ArchiveState, CompatibilityState, DependencyStatus, MatchConfidence, MachineRomRow,
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

fn evidence_from_members(
    members: &[crate::model::ArchiveMemberRow],
) -> MemberEvidence {
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
            by_crc.entry(crc.clone()).or_default().push(member.member_name.clone());
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
            // No CRC to prove — treat as missing evidence for required ROM.
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

async fn archive_exists_by_set_name(pool: &SqlitePool, set_name: &str) -> AppResult<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM archives
         WHERE lower(replace(file_name, '.zip', '')) = lower(?1)
            OR lower(file_name) = lower(?2)",
    )
    .bind(set_name)
    .bind(format!("{set_name}.zip"))
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

async fn chd_present_for_set(pool: &SqlitePool, set_name: &str, disk_name: &str) -> AppResult<bool> {
    // Convention: romroot\setname\disk.chd or romroot\disk.chd / setname.chd
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

async fn resolve_dependencies(
    pool: &SqlitePool,
    machine_id: i64,
) -> AppResult<DepOutcome> {
    let summary = machines::get_summary(pool, machine_id)
        .await?
        .expect("machine exists");

    let mut deps = Vec::new();
    let mut events = Vec::new();
    let mut parent_missing = false;
    let mut bios_missing = false;
    let mut chd_missing = false;

    if let Some(parent) = summary
        .clone_of
        .as_ref()
        .or(summary.rom_of.as_ref())
        .filter(|name| name.as_str() != summary.set_name)
    {
        // If parent is a BIOS set, classify as BIOS; else parent.
        let parent_id = machines::find_by_set_name(pool, summary.dat_source_id, parent).await?;
        let parent_is_bios = if let Some(pid) = parent_id {
            machines::get_summary(pool, pid)
                .await?
                .map(|m| m.is_bios)
                .unwrap_or(false)
        } else {
            false
        };

        let present = archive_exists_by_set_name(pool, parent).await?;
        let kind = if parent_is_bios { "bios" } else { "parent" };
        deps.push(DependencyStatus {
            kind: kind.into(),
            name: parent.clone(),
            present,
            detail: if present {
                format!("{kind} set found in library")
            } else {
                format!("Required {kind} set “{parent}” was not found among indexed archives")
            },
        });
        if !present {
            if parent_is_bios {
                bios_missing = true;
            } else {
                parent_missing = true;
            }
            events.push(json!({
                "type": if parent_is_bios { "bios-not-found" } else { "parent-not-found" },
                "name": parent,
            }));
        } else {
            events.push(json!({
                "type": if parent_is_bios { "bios-present" } else { "parent-present" },
                "name": parent,
            }));
        }
    }

    let disks = machines::disks_for_machine(pool, machine_id).await?;
    for disk in disks.iter().filter(|d| !d.optional) {
        let present = chd_present_for_set(pool, &summary.set_name, &disk.name).await?;
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

    Ok(DepOutcome {
        parent_missing,
        bios_missing,
        chd_missing,
        deps,
        events,
    })
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

    let mut score = 0.0;
    if required_total > 0 && missing == 0 && wrong == 0 {
        score += 70.0;
    }
    if name_match {
        score += 15.0;
    }
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
    } else if name_match {
        (
            CompatibilityState::KnownSetNameUnverifiedContent,
            MatchConfidence::Unknown,
        )
    } else {
        (CompatibilityState::Unidentified, MatchConfidence::Unknown)
    };

    (state, confidence, f64::max(score, 0.0))
}

async fn match_archive_against_dat(
    pool: &SqlitePool,
    archive: &ArchiveRow,
    members: &[crate::model::ArchiveMemberRow],
    dat: &crate::model::DatSource,
) -> AppResult<Option<matches_db::NewMatchResult>> {
    if archive.archive_state == ArchiveState::ArchiveUnreadable {
        return Ok(None);
    }
    if archive.extension == "chd" {
        // CHDs are dependency evidence, not primary match targets in Phase 3.
        return Ok(None);
    }

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

    let set_name = normalize_set_name(&archive.file_name);
    let evidence = evidence_from_members(members);

    let mut candidate_ids: HashSet<i64> = HashSet::new();
    if let Some(id) = machines::find_by_set_name(pool, dat.id, &set_name).await? {
        candidate_ids.insert(id);
    }
    for ((crc, size), _) in &evidence.by_crc_size {
        for id in machines::machine_ids_for_crc(pool, dat.id, crc, Some(*size)).await? {
            candidate_ids.insert(id);
        }
    }

    if candidate_ids.is_empty() {
        return Ok(None);
    }

    let mut best: Option<matches_db::NewMatchResult> = None;

    for machine_id in candidate_ids {
        let summary = machines::get_summary(pool, machine_id).await?.unwrap();
        let required = machines::local_required_roms(pool, machine_id).await?;
        let required_total = required.len() as i64;
        let (matched, missing, wrong, mut events) = compare_roms(&required, &evidence);
        let name_match = summary.set_name.eq_ignore_ascii_case(&set_name);
        if name_match {
            events.push(json!({"type": "archive-name-match", "setName": summary.set_name}));
        }

        let deps = resolve_dependencies(pool, machine_id).await?;
        events.extend(deps.events.clone());

        if !profile_enabled {
            // Still record the content match, but mark unavailable.
        }

        let (mut state, confidence, score) = classify(
            matched,
            missing,
            wrong,
            required_total,
            name_match,
            &deps,
            profile_healthy,
            core_installed && exe_ok,
            true,
        );

        if !profile_enabled {
            state = CompatibilityState::UserDisabled;
        }

        let candidate = matches_db::NewMatchResult {
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
        };

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
            ..Default::default()
        },
    )
    .await?;

    let mut matched_archives = 0u64;

    for archive in &page.rows {
        let members = archives::members(pool, archive.id).await?;
        let mut any = false;

        for dat in &dats {
            if let Some(result) = match_archive_against_dat(pool, archive, &members, dat).await? {
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
    Ok(resolve_dependencies(pool, best.machine_id).await?.deps)
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
    fn complete_match_classifies_as_verified() {
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
}
