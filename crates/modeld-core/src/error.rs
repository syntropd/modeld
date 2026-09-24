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
