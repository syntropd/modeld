//! Helpers for validating SHA-256 content digests used as CAS filenames.
//!
//! All digests are 64-character lowercase hex strings; rejecting anything
//! else at the type boundary keeps path-traversal payloads (`../../etc`)
//! and accidental binary data from ever reaching `Path::join`.

use crate::error::ModeldError;

/// Expected length of a SHA-256 hex digest (32 bytes -> 64 hex chars).
pub const DIGEST_LEN: usize = 64;

/// Returns true when `digest` is exactly 64 lowercase ASCII hex characters.
pub fn is_digest_format(digest: &str) -> bool {
    digest.len() == DIGEST_LEN
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Validates that `digest` is exactly 64 lowercase hex characters.
///
/// Returns `ModeldError::InvalidFormat` on mismatch. Callers that compose
/// filesystem paths from user-supplied digests must funnel them through
/// this check before joining onto a storage root.
pub fn validate_digest_format(digest: &str) -> Result<(), ModeldError> {
    if is_digest_format(digest) {
        Ok(())
    } else {
        Err(ModeldError::InvalidFormat(format!(
            "digest must be exactly {} lowercase hex characters, got {:?}",
            DIGEST_LEN, digest
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_digest_format_accepts_lowercase_hex() {
        assert!(is_digest_format(&"a".repeat(64)));
        assert!(is_digest_format(&"0123456789abcdef".repeat(4)));
    }

    #[test]
    fn test_is_digest_format_rejects_uppercase() {
        assert!(!is_digest_format(&"A".repeat(64)));
    }

    #[test]
    fn test_is_digest_format_rejects_wrong_length() {
        assert!(!is_digest_format(""));
        assert!(!is_digest_format(&"a".repeat(63)));
        assert!(!is_digest_format(&"a".repeat(65)));
    }

    #[test]
    fn test_is_digest_format_rejects_traversal_and_nul() {
        assert!(!is_digest_format(&"../".repeat(22)));
        assert!(!is_digest_format(&format!("{}a\0", "a".repeat(62))));
    }

    #[test]
    fn test_validate_digest_format_returns_err_for_traversal() {
        let bad = "../etc/passwd";
        let err = validate_digest_format(bad).unwrap_err();
        match err {
            ModeldError::InvalidFormat(msg) => {
                assert!(msg.contains("digest"), "error must mention digest format");
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }
}
