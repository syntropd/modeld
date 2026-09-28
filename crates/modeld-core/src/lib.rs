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
    compute_file_digest, compute_stream_digest, reflink_or_copy, verify_stream_digest, CasStore,
    EvictionManager, ModelTag, TagRegistry,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::Path;

    #[test]
    fn test_facade_reexports_resolve() {
        // Every root re-export must keep resolving: downstream crates
        // import the public surface through this facade.
        let _ = compute_stream_digest::<Cursor<&[u8]>>;
        let _ = compute_file_digest::<&Path>;
        let _ = verify_stream_digest::<Cursor<&[u8]>>;
        let _ = CasStore::new::<&Path>;
        let _ = EvictionManager::new::<&Path>;
        let _ = TagRegistry::new::<&Path>;
        let _ = std::mem::size_of::<ModelTag>();
        let _ = reflink_or_copy::<&Path, &Path>;
        let _ = ModeldConfig::default;
        let _ = DEFAULT_CONFIG_PATH;
        let _ = DEFAULT_MAX_STORAGE_BYTES;
        let _ = DEFAULT_SOCKET_PATH;
        let _ = DEFAULT_STORAGE_PATH;
        let _ = create_sealed_memfd;
        let _ = create_sealed_memfd_from_file;
        let _ = recv_fd_scm_rights::<&std::fs::File>;
        let _ = send_fd_scm_rights::<&std::fs::File, &std::fs::File>;
        let _ = std::mem::size_of::<ModeldError>();
        let _ = detect_safe_format::<Cursor<&[u8]>>;
        let _ = parse_gguf_header::<Cursor<&[u8]>>;
        let _ = parse_safetensors_header::<Cursor<&[u8]>>;
        let _ = validate_file_safety::<&Path>;
        let _ = std::mem::size_of::<GgufMetadata>();
        let _ = std::mem::size_of::<SafeFormat>();
        let _ = std::mem::size_of::<SafeTensorsMetadata>();
    }
}
