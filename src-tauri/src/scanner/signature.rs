//! Quick signatures for the incremental scan cache.
//!
//! SPEC.md section 12.2 defines the cheap identity of a file as path + size +
//! modified time. Hashing the file itself is reserved for Deep Verify, so a
//! routine startup scan of a large collection reads directory metadata only.

use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::archive::fs_readonly::hex_lower;

/// Combines the three cheap facts into one comparable value.
///
/// The result is a hash rather than the raw triple so the stored value has a
/// fixed width regardless of path length.
pub fn quick_signature(path: &str, size_bytes: u64, modified: Option<SystemTime>) -> String {
    let modified_nanos = modified
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update([0u8]);
    hasher.update(size_bytes.to_le_bytes());
    hasher.update([0u8]);
    hasher.update(modified_nanos.to_le_bytes());

    hex_lower(&hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(secs: u64) -> Option<SystemTime> {
        Some(UNIX_EPOCH + Duration::from_secs(secs))
    }

    #[test]
    fn identical_facts_produce_identical_signatures() {
        assert_eq!(
            quick_signature("D:\\a.zip", 100, at(1_000)),
            quick_signature("D:\\a.zip", 100, at(1_000))
        );
    }

    #[test]
    fn any_changed_fact_changes_the_signature() {
        let baseline = quick_signature("D:\\a.zip", 100, at(1_000));

        assert_ne!(baseline, quick_signature("D:\\b.zip", 100, at(1_000)));
        assert_ne!(baseline, quick_signature("D:\\a.zip", 101, at(1_000)));
        assert_ne!(baseline, quick_signature("D:\\a.zip", 100, at(1_001)));
        assert_ne!(baseline, quick_signature("D:\\a.zip", 100, None));
    }

    #[test]
    fn field_boundaries_cannot_be_confused() {
        // Without a separator, ("ab", 1) and ("a", b1) could collide.
        assert_ne!(
            quick_signature("ab", 1, None),
            quick_signature("a", 1, None)
        );
    }

    #[test]
    fn signatures_have_a_fixed_width() {
        assert_eq!(quick_signature("D:\\a.zip", 1, None).len(), 64);
        assert_eq!(
            quick_signature(&"x".repeat(5_000), 1, None).len(),
            64
        );
    }
}
