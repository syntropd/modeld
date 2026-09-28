//! Configuration subsystem for modeld.
//!
//! Exposes declarative daemon configuration options and standard defaults.

pub mod modeld_config;

pub use modeld_config::ModeldConfig;
pub use modeld_config::DEFAULT_CONFIG_PATH;
pub use modeld_config::DEFAULT_MAX_STORAGE_BYTES;
pub use modeld_config::DEFAULT_SOCKET_PATH;
pub use modeld_config::DEFAULT_STORAGE_PATH;
