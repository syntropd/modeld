//! Modeld configuration file parser and default values.
//!
//! Loads declarative settings from `/etc/syntrop/modeld.conf`.

use crate::error::ModeldError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Default location of the modeld configuration file.
pub const DEFAULT_CONFIG_PATH: &str = "/etc/syntrop/modeld.conf";

/// Default storage directory for Content-Addressable Storage.
pub const DEFAULT_STORAGE_PATH: &str = "/var/lib/models";

/// Default Varlink Unix domain socket path.
pub const DEFAULT_SOCKET_PATH: &str = "/run/syntrop/io.syntrop.Model1";

/// Default maximum storage capacity in bytes (64 GiB).
pub const DEFAULT_MAX_STORAGE_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// Root configuration structure for the modeld daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeldConfig {
    /// Content-addressable storage root directory.
    pub storage_path: PathBuf,
    /// Maximum storage quota allocated for model storage.
    pub max_storage_bytes: u64,
    /// IPC Varlink socket path.
    pub socket_path: PathBuf,
    /// Whether unprivileged remote downloads are permitted.
    pub allow_remote_pulls: bool,
    /// Whether unsafe formats (e.g. pickle) are rejected.
    pub enforce_safety_policy: bool,
}

impl Default for ModeldConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from(DEFAULT_STORAGE_PATH),
            max_storage_bytes: DEFAULT_MAX_STORAGE_BYTES,
            socket_path: PathBuf::from(DEFAULT_SOCKET_PATH),
            allow_remote_pulls: true,
            enforce_safety_policy: true,
        }
    }
}

impl ModeldConfig {
    /// Loads configuration from a specified filesystem path, falling back to defaults.
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Result<Self, ModeldError> {
        let p = path.as_ref();
        if !p.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(p)?;
        toml::from_str(&content).map_err(|e| ModeldError::Config(e.to_string()))
    }
}
