//! XML DAT parser for Logiqx-style and MAME `-listxml`-style definition files.
//!
//! Supported constructs (Phase 2 MVP):
//! - `<datafile>` / `<mame>` roots
//! - `<header>` name/description/version
//! - `<game>` / `<machine>` with name, cloneof, romof, isbios, runnable
//! - `<rom>` name/size/crc/sha1/status/optional/merge/bios/region
//! - `<disk>` name/sha1/status/optional
//!
//! Unsupported (recorded as skipped, not fatal): softlist, device_ref trees as
//! first-class tables (hints go into metadata_json), Logiqx-only extensions.

use std::io::BufRead;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};

use crate::archive::fs_readonly;
use crate::error::{AppError, AppResult};

pub const PARSER_VERSION: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDat {
    pub display_name: String,
    pub version: Option<String>,
    pub machines: Vec<ParsedMachine>,
    pub unsupported_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedMachine {
    pub set_name: String,
    pub description: Option<String>,
    pub year: Option<String>,
    pub manufacturer: Option<String>,
    pub clone_of: Option<String>,
    pub rom_of: Option<String>,
    pub is_bios: bool,
    pub runnable: Option<bool>,
    pub roms: Vec<ParsedRom>,
    pub disks: Vec<ParsedDisk>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRom {
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
pub struct ParsedDisk {
    pub name: String,
    pub sha1: Option<String>,
    pub status: Option<String>,
    pub optional: bool,
}

fn attr_map(
    e: &quick_xml::events::BytesStart<'_>,
) -> AppResult<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    for attr in e.attributes().with_checks(false) {
        let attr = attr.map_err(|err| {
            AppError::user(
                "DAT could not be read",
                format!("Invalid attribute in DAT XML: {err}"),
            )
        })?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        // DAT attribute values are plain ASCII in practice (names, hex CRCs).
        let value = String::from_utf8_lossy(&attr.value).into_owned();
        map.insert(key.to_ascii_lowercase(), value);
    }
    Ok(map)
}

fn normalize_crc(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches("0x");
    format!("{:0>8}", trimmed.to_ascii_lowercase())
}

fn normalize_sha1(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn truthy(value: Option<&String>) -> bool {
    matches!(
        value.map(|s| s.as_str()),
        Some("yes") | Some("true") | Some("1")
    )
}

/// Parses a DAT file from disk (read-only open).
pub fn parse_file(path: &Path) -> AppResult<(ParsedDat, String)> {
    let file = fs_readonly::open_read(path).map_err(|source| AppError::Filesystem {
        path: path.display().to_string(),
        source,
    })?;
    let sha256 = fs_readonly::sha256_file(path).map_err(|source| AppError::Filesystem {
        path: path.display().to_string(),
        source,
    })?;

    let mut reader = Reader::from_reader(std::io::BufReader::new(file));
    reader.config_mut().trim_text(true);

    let parsed = parse_reader(&mut reader)?;
    Ok((parsed, sha256))
}

pub fn parse_reader<R: BufRead>(reader: &mut Reader<R>) -> AppResult<ParsedDat> {
    let mut buf = Vec::new();
    let mut display_name = String::new();
    let mut version = None;
    let mut machines = Vec::new();
    let mut unsupported_notes = Vec::new();

    let mut in_header = false;
    let mut header_field: Option<String> = None;
    let mut current: Option<ParsedMachine> = None;
    let mut text_field: Option<&'static str> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                match name.as_str() {
                    "header" => in_header = true,
                    "name" if in_header => header_field = Some("name".into()),
                    "description" if in_header => header_field = Some("description".into()),
                    "version" if in_header => header_field = Some("version".into()),
                    "game" | "machine" => {
                        let attrs = attr_map(&e)?;
                        let set_name = attrs.get("name").cloned().unwrap_or_default();
                        if set_name.is_empty() {
                            unsupported_notes
                                .push("Skipped a machine/game element without a name".into());
                            current = None;
                            continue;
                        }
                        current = Some(ParsedMachine {
                            set_name,
                            description: None,
                            year: None,
                            manufacturer: None,
                            clone_of: attrs.get("cloneof").cloned(),
                            rom_of: attrs.get("romof").cloned(),
                            is_bios: truthy(attrs.get("isbios")),
                            runnable: attrs.get("runnable").map(|v| !matches!(v.as_str(), "no" | "false" | "0")),
                            roms: Vec::new(),
                            disks: Vec::new(),
                            metadata_json: None,
                        });
                    }
                    "description" if current.is_some() && !in_header => {
                        text_field = Some("description");
                    }
                    "year" if current.is_some() => text_field = Some("year"),
                    "manufacturer" if current.is_some() => text_field = Some("manufacturer"),
                    "rom" => {
                        if let Some(machine) = current.as_mut() {
                            let attrs = attr_map(&e)?;
                            if let Some(rom_name) = attrs.get("name").cloned() {
                                machine.roms.push(ParsedRom {
                                    name: rom_name,
                                    size_bytes: attrs
                                        .get("size")
                                        .and_then(|s| s.parse::<i64>().ok()),
                                    crc32: attrs.get("crc").map(|s| normalize_crc(s)),
                                    sha1: attrs.get("sha1").map(|s| normalize_sha1(s)),
                                    status: attrs.get("status").cloned(),
                                    optional: truthy(attrs.get("optional"))
                                        || matches!(
                                            attrs.get("status").map(|s| s.as_str()),
                                            Some("nodump")
                                        ),
                                    merge_name: attrs.get("merge").cloned(),
                                    bios_name: attrs.get("bios").cloned(),
                                    region: attrs.get("region").cloned(),
                                });
                            }
                        }
                    }
                    "disk" => {
                        if let Some(machine) = current.as_mut() {
                            let attrs = attr_map(&e)?;
                            if let Some(disk_name) = attrs.get("name").cloned() {
                                machine.disks.push(ParsedDisk {
                                    name: disk_name,
                                    sha1: attrs.get("sha1").map(|s| normalize_sha1(s)),
                                    status: attrs.get("status").cloned(),
                                    optional: truthy(attrs.get("optional")),
                                });
                            }
                        }
                    }
                    "softlist" | "device" | "chip" | "display" | "sound" | "input"
                    | "dipswitch" | "driver" | "sample" => {
                        // Known but not modelled as first-class rows in Phase 2.
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                match name.as_str() {
                    "rom" => {
                        if let Some(machine) = current.as_mut() {
                            let attrs = attr_map(&e)?;
                            if let Some(rom_name) = attrs.get("name").cloned() {
                                machine.roms.push(ParsedRom {
                                    name: rom_name,
                                    size_bytes: attrs
                                        .get("size")
                                        .and_then(|s| s.parse::<i64>().ok()),
                                    crc32: attrs.get("crc").map(|s| normalize_crc(s)),
                                    sha1: attrs.get("sha1").map(|s| normalize_sha1(s)),
                                    status: attrs.get("status").cloned(),
                                    optional: truthy(attrs.get("optional"))
                                        || matches!(
                                            attrs.get("status").map(|s| s.as_str()),
                                            Some("nodump")
                                        ),
                                    merge_name: attrs.get("merge").cloned(),
                                    bios_name: attrs.get("bios").cloned(),
                                    region: attrs.get("region").cloned(),
                                });
                            }
                        }
                    }
                    "disk" => {
                        if let Some(machine) = current.as_mut() {
                            let attrs = attr_map(&e)?;
                            if let Some(disk_name) = attrs.get("name").cloned() {
                                machine.disks.push(ParsedDisk {
                                    name: disk_name,
                                    sha1: attrs.get("sha1").map(|s| normalize_sha1(s)),
                                    status: attrs.get("status").cloned(),
                                    optional: truthy(attrs.get("optional")),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let text = t.decode().unwrap_or_default().into_owned();
                if let Some(field) = header_field.take() {
                    match field.as_str() {
                        "name" | "description" if display_name.is_empty() => {
                            display_name = text;
                        }
                        "version" => version = Some(text),
                        _ => {}
                    }
                } else if let (Some(field), Some(machine)) = (text_field.take(), current.as_mut()) {
                    match field {
                        "description" => machine.description = Some(text),
                        "year" => machine.year = Some(text),
                        "manufacturer" => machine.manufacturer = Some(text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                match name.as_str() {
                    "header" => in_header = false,
                    "game" | "machine" => {
                        if let Some(machine) = current.take() {
                            machines.push(machine);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(AppError::user(
                    "DAT could not be read",
                    format!("The DAT XML is malformed or incomplete.\n{err}"),
                ));
            }
            _ => {}
        }
        buf.clear();
    }

    if machines.is_empty() {
        return Err(AppError::user(
            "DAT has no machines",
            "The file was read, but no game or machine definitions were found.",
        ));
    }

    if display_name.is_empty() {
        display_name = "Imported DAT".into();
    }

    Ok(ParsedDat {
        display_name,
        version,
        machines,
        unsupported_notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SAMPLE: &str = r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>Test DAT</name>
    <version>0.1</version>
  </header>
  <game name="pacman" romof="puckman">
    <description>Pac-Man</description>
    <year>1980</year>
    <manufacturer>Namco</manufacturer>
    <rom name="pacman.6e" size="4096" crc="c1e6ab10" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/>
    <rom name="pacman.6f" size="4096" crc="1a6fb2d4" optional="yes"/>
    <disk name="pacman" sha1="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"/>
  </game>
  <machine name="neogeo" isbios="yes">
    <description>Neo-Geo BIOS</description>
    <rom name="sp-s2.sp1" size="131072" crc="9036d879"/>
  </machine>
</datafile>
"#;

    #[test]
    fn parses_logiqx_style_dat() {
        let mut reader = Reader::from_reader(Cursor::new(SAMPLE.as_bytes()));
        reader.config_mut().trim_text(true);
        let parsed = parse_reader(&mut reader).unwrap();

        assert_eq!(parsed.display_name, "Test DAT");
        assert_eq!(parsed.version.as_deref(), Some("0.1"));
        assert_eq!(parsed.machines.len(), 2);

        let pac = &parsed.machines[0];
        assert_eq!(pac.set_name, "pacman");
        assert_eq!(pac.rom_of.as_deref(), Some("puckman"));
        assert_eq!(pac.roms.len(), 2);
        assert_eq!(pac.roms[0].crc32.as_deref(), Some("c1e6ab10"));
        assert!(pac.roms[1].optional);
        assert_eq!(pac.disks.len(), 1);
        assert!(parsed.machines[1].is_bios);
    }

    #[test]
    fn crc_is_normalized_to_eight_hex_digits() {
        assert_eq!(normalize_crc("C1E6AB10"), "c1e6ab10");
        assert_eq!(normalize_crc("ab10"), "0000ab10");
    }
}
