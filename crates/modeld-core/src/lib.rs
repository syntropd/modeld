//! Pure Rust core engine for modeld.
//!
//! Provides Content-Addressable Storage, format inspection, zero-copy
//! descriptor passing, and configuration parsing.

pub mod cas;
pub mod config;
pub mod descriptor;
pub mod error;
pub mod format;

pub use cas::{
    compute_file_digest, compute_stream_digest, verify_stream_digest, CasStore, EvictionManager,
    ModelTag, TagRegistry, reflink_or_copy,
};
pub use config::{
    ModeldConfig, DEFAULT_CONFIG_PATH, DEFAULT_MAX_STORAGE_BYTES, DEFAULT_SOCKET_PATH,
    DEFAULT_STORAGE_PATH,
};
pub use descriptor::{
    create_sealed_memfd, create_sealed_memfd_from_file, recv_fd_scm_rights, send_fd_scm_rights,
};
pub use error::ModeldError;
pub use format::{
    detect_safe_format, parse_gguf_header, parse_safetensors_header, validate_file_safety,
    GgufMetadata, SafeFormat, SafeTensorsMetadata,
};
