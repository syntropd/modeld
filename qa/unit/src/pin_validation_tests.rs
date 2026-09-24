//! 1:1 unit QA tests for `EvictionManager` digest-format validation.
//!
//! The pin / unpin paths accept user-supplied digests; without validation
//! a hostile CLI could pass `../etc/passwd` and have the daemon create or
//! remove an arbitrary file under the pinned directory. These tests pin
//! the new validation contract.

use modeld_core::cas::EvictionManager;
use modeld_core::error::ModeldError;
use tempfile::tempdir;

#[test]
fn test_pin_rejects_path_traversal_digest() {
    let dir = tempdir().unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    let bad = "../etc/passwd";
    let err = eviction.pin(bad).expect_err("pin must reject traversal");
    match err {
        ModeldError::InvalidFormat(msg) => {
            assert!(msg.contains("digest"), "error must mention digest format");
        }
        other => panic!("unexpected error variant: {:?}", other),
    }
    // The pin file must NOT have been created.
    assert!(!eviction.is_pinned(bad));
}

#[test]
fn test_pin_rejects_empty_and_short_digests() {
    let dir = tempdir().unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    for bad in ["", "abc", &"a".repeat(63), &"a".repeat(65)] {
        assert!(
            eviction.pin(bad).is_err(),
            "pin must reject {:?}",
            bad
        );
        assert!(!eviction.is_pinned(bad));
    }
}

#[test]
fn test_pin_rejects_uppercase_hex() {
    let dir = tempdir().unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    let bad = "A".repeat(64);
    let err = eviction.pin(&bad).expect_err("pin must reject uppercase hex");
    assert!(matches!(err, ModeldError::InvalidFormat(_)));
    assert!(!eviction.is_pinned(&bad));
}

#[test]
fn test_pin_accepts_lowercase_hex_digest() {
    let dir = tempdir().unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    let good = "0123456789abcdef".repeat(4);
    eviction.pin(&good).expect("valid digest must be accepted");
    assert!(eviction.is_pinned(&good));
}

#[test]
fn test_unpin_rejects_invalid_digest() {
    let dir = tempdir().unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    assert!(eviction.unpin("../escape").is_err());
    assert!(eviction.unpin("not-hex").is_err());
}

#[test]
fn test_is_pinned_returns_false_for_invalid_digest() {
    let dir = tempdir().unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    // Even if the named file exists inside pinned_dir, an invalid digest
    // must never be reported as pinned.
    let pinned_dir = dir.path().join("pinned");
    std::fs::create_dir_all(&pinned_dir).unwrap();
    std::fs::write(pinned_dir.join("../etc"), b"x").unwrap();

    assert!(!eviction.is_pinned("../etc"));
    assert!(!eviction.is_pinned(""));
}
