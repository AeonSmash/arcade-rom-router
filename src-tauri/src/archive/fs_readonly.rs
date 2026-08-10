//! The only module permitted to touch files inside a user ROM root.
//!
//! SPEC.md principles 1 and 3 make the user's collection read-only evidence.
//! That guarantee is enforced structurally rather than by convention: every
//! function below opens handles with write access explicitly disabled, and this
//! module deliberately exposes no way to create, write, rename, move, truncate,
//! or delete anything. Code elsewhere in the backend has no other entry point
//! into a ROM root, so there is no code path that can mutate one.

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

/// Metadata read from a candidate file without opening its contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
}

/// Opens a file for reading only.
///
/// `write(false)`, `create(false)`, `truncate(false)`, and `append(false)` are
/// stated explicitly rather than relied upon as defaults, so that the intent
/// survives future edits.
pub fn open_read(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(false)
        .append(false)
        .create(false)
        .create_new(false)
        .truncate(false)
        .open(path)
}

pub fn read_facts(path: &Path) -> io::Result<FileFacts> {
    let metadata = std::fs::metadata(path)?;
    Ok(FileFacts {
        size_bytes: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

/// Streams a file through SHA-256 without loading it into memory.
///
/// Only invoked for Deep Verify or explicit duplicate detection; SPEC.md
/// section 12.2 forbids hashing large files during a normal scan.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = open_read(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex_lower(&hasher.finalize()))
}

pub fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut acc, byte| {
        let _ = write!(acc, "{byte:02x}");
        acc
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn opened_handles_reject_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evidence.bin");
        std::fs::write(&path, b"original").unwrap();

        let mut handle = open_read(&path).unwrap();
        assert!(handle.write_all(b"tampered").is_err());

        assert_eq!(std::fs::read(&path).unwrap(), b"original");
    }

    #[test]
    fn sha256_matches_known_vector() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abc.bin");
        std::fs::write(&path, b"abc").unwrap();

        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_handles_files_larger_than_the_read_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.bin");
        std::fs::write(&path, vec![0x5au8; (1 << 20) + 1234]).unwrap();

        assert_eq!(sha256_file(&path).unwrap().len(), 64);
    }

    #[test]
    fn facts_report_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sized.bin");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        assert_eq!(read_facts(&path).unwrap().size_bytes, 4096);
    }
}
