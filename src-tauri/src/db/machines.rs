use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::dat::parser::{ParsedMachine, ParsedRom};
use crate::error::AppResult;
use crate::model::{MachineDiskRow, MachineRomRow, MachineSummary};

const ROM_CHUNK: usize = 80;

pub async fn insert_all_tx(
    tx: &mut Transaction<'_, Sqlite>,
    dat_source_id: i64,
    machines: &[ParsedMachine],
) -> AppResult<()> {
    for machine in machines {
        let machine_id: i64 = sqlx::query_scalar(
            "INSERT INTO machines (
                 dat_source_id, set_name, description, year, manufacturer,
                 clone_of, rom_of, is_bios, runnable, metadata_json
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             RETURNING id",
        )
        .bind(dat_source_id)
        .bind(&machine.set_name)
        .bind(&machine.description)
        .bind(&machine.year)
        .bind(&machine.manufacturer)
        .bind(&machine.clone_of)
        .bind(&machine.rom_of)
        .bind(i64::from(machine.is_bios))
        .bind(machine.runnable.map(i64::from))
        .bind(&machine.metadata_json)
        .fetch_one(&mut **tx)
        .await?;

        insert_roms_tx(tx, machine_id, &machine.roms).await?;

        for disk in &machine.disks {
            sqlx::query(
                "INSERT INTO machine_disks (machine_id, name, sha1, status, optional)
                 VALUES (?1,?2,?3,?4,?5)",
            )
            .bind(machine_id)
            .bind(&disk.name)
            .bind(&disk.sha1)
            .bind(&disk.status)
            .bind(i64::from(disk.optional))
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

async fn insert_roms_tx(
    tx: &mut Transaction<'_, Sqlite>,
    machine_id: i64,
    roms: &[ParsedRom],
) -> AppResult<()> {
    for chunk in roms.chunks(ROM_CHUNK) {
        let placeholders = (0..chunk.len())
            .map(|_| "(?,?,?,?,?,?,?,?,?,?)")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "INSERT INTO machine_roms (
                 machine_id, name, size_bytes, crc32, sha1, status,
                 optional, merge_name, bios_name, region
             ) VALUES {placeholders}"
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
        for rom in chunk {
            query = query
                .bind(machine_id)
                .bind(&rom.name)
                .bind(rom.size_bytes)
                .bind(&rom.crc32)
                .bind(&rom.sha1)
                .bind(&rom.status)
                .bind(i64::from(rom.optional))
                .bind(&rom.merge_name)
                .bind(&rom.bios_name)
                .bind(&rom.region);
        }
        query.execute(&mut **tx).await?;
    }
    Ok(())
}

pub async fn find_by_set_name(
    pool: &SqlitePool,
    dat_source_id: i64,
    set_name: &str,
) -> AppResult<Option<i64>> {
    let id = sqlx::query_scalar(
        "SELECT id FROM machines WHERE dat_source_id = ?1 AND set_name = ?2 COLLATE NOCASE",
    )
    .bind(dat_source_id)
    .bind(set_name)
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

pub async fn machine_ids_for_crc(
    pool: &SqlitePool,
    dat_source_id: i64,
    crc32: &str,
    size_bytes: Option<i64>,
) -> AppResult<Vec<i64>> {
    let rows: Vec<i64> = if let Some(size) = size_bytes {
        sqlx::query_scalar(
            "SELECT DISTINCT m.id
             FROM machines m
             JOIN machine_roms r ON r.machine_id = m.id
             WHERE m.dat_source_id = ?1 AND r.crc32 = ?2 AND r.size_bytes = ?3",
        )
        .bind(dat_source_id)
        .bind(crc32)
        .bind(size)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_scalar(
            "SELECT DISTINCT m.id
             FROM machines m
             JOIN machine_roms r ON r.machine_id = m.id
             WHERE m.dat_source_id = ?1 AND r.crc32 = ?2",
        )
        .bind(dat_source_id)
        .bind(crc32)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

pub async fn get_summary(pool: &SqlitePool, machine_id: i64) -> AppResult<Option<MachineSummary>> {
    let row = sqlx::query(
        "SELECT id, dat_source_id, set_name, description, year, manufacturer,
                clone_of, rom_of, is_bios, runnable
         FROM machines WHERE id = ?1",
    )
    .bind(machine_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| {
        use sqlx::Row;
        MachineSummary {
            id: row.get("id"),
            dat_source_id: row.get("dat_source_id"),
            set_name: row.get("set_name"),
            description: row.get("description"),
            year: row.get("year"),
            manufacturer: row.get("manufacturer"),
            clone_of: row.get("clone_of"),
            rom_of: row.get("rom_of"),
            is_bios: row.get::<i64, _>("is_bios") != 0,
            runnable: row.get::<Option<i64>, _>("runnable").map(|v| v != 0),
        }
    }))
}

pub async fn roms_for_machine(pool: &SqlitePool, machine_id: i64) -> AppResult<Vec<MachineRomRow>> {
    let rows = sqlx::query(
        "SELECT name, size_bytes, crc32, sha1, status, optional, merge_name, bios_name, region
         FROM machine_roms WHERE machine_id = ?1 ORDER BY name",
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            use sqlx::Row;
            MachineRomRow {
                name: row.get("name"),
                size_bytes: row.get("size_bytes"),
                crc32: row.get("crc32"),
                sha1: row.get("sha1"),
                status: row.get("status"),
                optional: row.get::<i64, _>("optional") != 0,
                merge_name: row.get("merge_name"),
                bios_name: row.get("bios_name"),
                region: row.get("region"),
            }
        })
        .collect())
}

pub async fn disks_for_machine(pool: &SqlitePool, machine_id: i64) -> AppResult<Vec<MachineDiskRow>> {
    let rows = sqlx::query(
        "SELECT name, sha1, status, optional FROM machine_disks WHERE machine_id = ?1 ORDER BY name",
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            use sqlx::Row;
            MachineDiskRow {
                name: row.get("name"),
                sha1: row.get("sha1"),
                status: row.get("status"),
                optional: row.get::<i64, _>("optional") != 0,
            }
        })
        .collect())
}

/// Expected ROM content for an archive: local required entries that are not
/// deferred to a parent via merge/romof inheritance.
pub async fn local_required_roms(
    pool: &SqlitePool,
    machine_id: i64,
) -> AppResult<Vec<MachineRomRow>> {
    let roms = roms_for_machine(pool, machine_id).await?;
    Ok(roms
        .into_iter()
        .filter(|r| !r.optional)
        .filter(|r| r.merge_name.is_none())
        .filter(|r| {
            // nodump entries cannot be matched from evidence we have.
            !matches!(r.status.as_deref(), Some("nodump"))
        })
        .collect())
}

/// Full required ROM list including chips that DAT merge/romof inheritance
/// places in a parent zip. Still skips optional and nodump entries.
pub async fn required_roms_full(
    pool: &SqlitePool,
    machine_id: i64,
) -> AppResult<Vec<MachineRomRow>> {
    let roms = roms_for_machine(pool, machine_id).await?;
    Ok(roms
        .into_iter()
        .filter(|r| !r.optional)
        .filter(|r| !matches!(r.status.as_deref(), Some("nodump")))
        .collect())
}

/// Walk `clone_of` then `rom_of` upward within the same DAT. The seed machine
/// is first; ancestors follow. Cycles are refused via a visited set.
pub async fn chain_of(pool: &SqlitePool, machine_id: i64) -> AppResult<Vec<MachineSummary>> {
    let mut out = Vec::new();
    let Some(seed) = get_summary(pool, machine_id).await? else {
        return Ok(out);
    };
    let dat_id = seed.dat_source_id;
    let mut visited = std::collections::HashSet::new();
    visited.insert(seed.set_name.to_ascii_lowercase());
    out.push(seed);

    let mut cursor = out[0].clone();
    loop {
        let parent_name = cursor
            .clone_of
            .as_ref()
            .or(cursor.rom_of.as_ref())
            .cloned();
        let Some(parent_name) = parent_name else {
            break;
        };
        let key = parent_name.to_ascii_lowercase();
        if !visited.insert(key) {
            break;
        }
        let Some(parent_id) = find_by_set_name(pool, dat_id, &parent_name).await? else {
            break;
        };
        let Some(parent) = get_summary(pool, parent_id).await? else {
            break;
        };
        out.push(parent.clone());
        cursor = parent;
    }
    Ok(out)
}
