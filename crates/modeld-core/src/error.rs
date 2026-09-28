//! Error types for the modeld core engine.
//!
//! Follows standard POSIX and systemd error classification.

use thiserror::Error;

/// Primary error enumeration for modeld operations.
#[derive(Debug, Error)]
pub enum ModeldError {
    /// Input/output error on host filesystem.
    #[error("Filesystem I/O failure: {0}")]
    Io(#[from] std::io::Error),

    /// Checksum verification failure against expected digest.
    #[error("Cryptographic digest mismatch: expected {expected}, computed {computed}")]
    DigestMismatch {
        /// Expected SHA-256 hex string.
        expected: String,
        /// Computed SHA-256 hex string.
        computed: String,
    },

    /// Corrupted or invalid model format header.
    #[error("Invalid model format header: {0}")]
    InvalidFormat(String),

    /// Model format rejected for safety reasons (e.g. Python pickle).
    #[error("Insecure model format rejected by policy: {0}")]
    SecurityRejection(String),

    /// Specified model or tag not found in CAS store.
    #[error("Model not found: {0}")]
    NotFound(String),

    /// Storage quota exceeded and cannot reclaim sufficient space.
    #[error("Storage quota exceeded: required {required_bytes} bytes, free {free_bytes} bytes")]
    QuotaExceeded {
        /// Required storage capacity in bytes.
        required_bytes: u64,
        /// Available storage capacity in bytes.
        free_bytes: u64,
    },

    /// Operating system primitive or syscall error.
    #[error("Kernel syscall error: {0}")]
    Syscall(String),

    /// Configuration parsing failure.
    #[error("Configuration error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_messages_name_the_failure() {
        let cases: Vec<(ModeldError, &str)> = vec![
            (
                ModeldError::DigestMismatch {
                    expected: "aa".into(),
                    computed: "bb".into(),
                },
                "Cryptographic digest mismatch: expected aa, computed bb",
            ),
            (
                ModeldError::InvalidFormat("bad header".into()),
                "Invalid model format header: bad header",
            ),
            (
                ModeldError::SecurityRejection("pickle".into()),
                "Insecure model format rejected by policy: pickle",
            ),
            (
                ModeldError::NotFound("wisp".into()),
                "Model not found: wisp",
            ),
            (
                ModeldError::QuotaExceeded {
                    required_bytes: 10,
                    free_bytes: 4,
                },
                "Storage quota exceeded: required 10 bytes, free 4 bytes",
            ),
            (
                ModeldError::Syscall("EPERM".into()),
                "Kernel syscall error: EPERM",
            ),
            (
                ModeldError::Config("missing key".into()),
                "Configuration error: missing key",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn test_io_converts_with_filesystem_context() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing blob");
        let err = ModeldError::from(io_err);
        assert!(matches!(err, ModeldError::Io(_)));
        assert!(err.to_string().starts_with("Filesystem I/O failure: "));
    }
}
