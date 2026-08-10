//! Synthetic fixture generation for integration tests.
//!
//! SPEC.md section 60.2 forbids copyrighted ROM data in fixtures. Every archive
//! used by the test suite is therefore built at test time from deterministic
//! pseudo-random bytes, and its expected CRC32 is computed independently of the
//! code under test so the assertions are a real cross-check rather than a
//! restatement of what the scanner produced.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// What a synthetic archive is expected to contain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedMember {
    pub name: String,
    pub size: u64,
    pub crc32: String,
}

/// Reproducible pseudo-random bytes (xorshift64).
///
/// Deterministic so a failing test can be replayed exactly.
pub fn deterministic_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(len + 8);

    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }

    out.truncate(len);
    out
}

fn crc32_of(bytes: &[u8]) -> String {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(bytes);
    format!("{:08x}", hasher.finalize())
}

/// Writes a ZIP whose members are stored uncompressed.
///
/// Stored entries keep the fixtures readable by any tool and match how the
/// scanner reads metadata: from the central directory, never by inflating.
pub fn write_zip(path: &Path, members: &[(&str, usize)], seed: u64) -> Vec<ExpectedMember> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }

    let file = std::fs::File::create(path).unwrap();
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let mut expected = Vec::with_capacity(members.len());

    for (index, (name, len)) in members.iter().enumerate() {
        let bytes = deterministic_bytes(seed.wrapping_add(index as u64 * 7919), *len);

        writer.start_file(*name, options).unwrap();
        writer.write_all(&bytes).unwrap();

        expected.push(ExpectedMember {
            name: (*name).to_string(),
            size: *len as u64,
            crc32: crc32_of(&bytes),
        });
    }

    writer.finish().unwrap();
    expected
}

/// A standard small ROM-set-shaped archive.
pub fn write_rom_set(dir: &Path, file_name: &str, chip_count: usize) -> Vec<ExpectedMember> {
    let names: Vec<String> = (0..chip_count).map(|i| format!("chip{i:02}.bin")).collect();
    let members: Vec<(&str, usize)> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), 512 + i * 16))
        .collect();

    write_zip(&dir.join(file_name), &members, file_name.len() as u64 * 31 + 1)
}

/// Bytes that are not a ZIP at all, to exercise the unreadable path.
pub fn write_damaged_zip(path: &Path) {
    std::fs::write(path, deterministic_bytes(4242, 2048)).unwrap();
}

/// A ZIP whose central directory is cut off partway through.
pub fn write_truncated_zip(path: &Path) {
    let intact = path.with_extension("intact");
    write_zip(&intact, &[("chip00.bin", 4096), ("chip01.bin", 4096)], 99);

    let bytes = std::fs::read(&intact).unwrap();
    std::fs::remove_file(&intact).unwrap();
    std::fs::write(path, &bytes[..bytes.len() / 2]).unwrap();
}

/// A ZIP containing member names that try to escape the archive.
pub fn write_zip_with_traversal_names(path: &Path) -> Vec<ExpectedMember> {
    write_zip(
        path,
        &[
            ("chip00.bin", 256),
            ("../escape.bin", 256),
            ("nested/../../escape2.bin", 256),
        ],
        7,
    )
}

/// A stand-in CHD. The scanner only records its path and size, so the contents
/// need no particular structure.
pub fn write_chd(path: &Path, len: usize) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, deterministic_bytes(31337, len)).unwrap();
}

/// SHA-256 of every file under a directory, keyed by path.
///
/// Used by the source-safety test: the collection is evidence, and a scan must
/// leave every byte of it untouched.
pub fn hash_tree(root: &Path) -> HashMap<PathBuf, String> {
    let mut hashes = HashMap::new();

    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }

        let bytes = std::fs::read(entry.path()).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);

        let digest = hasher.finalize();
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();

        hashes.insert(entry.path().to_path_buf(), hex);
    }

    hashes
}
