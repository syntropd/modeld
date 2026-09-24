//! 1:1 unit QA tests for config::modeld_config.

use modeld_core::config::{ModeldConfig, DEFAULT_MAX_STORAGE_BYTES};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_config_defaults() {
    let cfg = ModeldConfig::default();
    assert_eq!(cfg.max_storage_bytes, DEFAULT_MAX_STORAGE_BYTES);
    assert!(cfg.allow_remote_pulls);
    assert!(cfg.enforce_safety_policy);
}

#[test]
fn test_config_load_nonexistent_returns_defaults() {
    let cfg = ModeldConfig::load_or_default("/nonexistent/syntrop/modeld.conf").unwrap();
    assert_eq!(cfg.max_storage_bytes, DEFAULT_MAX_STORAGE_BYTES);
}

#[test]
fn test_config_load_custom_toml() {
    let mut file = NamedTempFile::new().unwrap();
    let toml_str = r#"
storage_path = "/custom/models"
max_storage_bytes = 1073741824
socket_path = "/run/custom.sock"
allow_remote_pulls = false
enforce_safety_policy = true
"#;
    file.write_all(toml_str.as_bytes()).unwrap();
    file.flush().unwrap();

    let cfg = ModeldConfig::load_or_default(file.path()).unwrap();
    assert_eq!(cfg.storage_path.to_str().unwrap(), "/custom/models");
    assert_eq!(cfg.max_storage_bytes, 1073741824);
    assert!(!cfg.allow_remote_pulls);
}
