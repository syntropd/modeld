//! Configuration subsystem for modeld.
//!
//! Exposes declarative daemon configuration options and standard defaults.

pub mod modeld_config;

pub use modeld_config::{
    ModeldConfig, DEFAULT_CONFIG_PATH, DEFAULT_MAX_STORAGE_BYTES, DEFAULT_SOCKET_PATH,
    DEFAULT_STORAGE_PATH,
};
