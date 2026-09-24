//! 1:1 unit QA tests for cas::eviction::EvictionManager.

use modeld_core::cas::{CasStore, EvictionManager};
use std::io::Cursor;
use tempfile::tempdir;

#[test]
fn test_pin_and_unpin_lifecycle() {
    let dir = tempdir().unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    let digest = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890";
    assert!(!eviction.is_pinned(digest));

    eviction.pin(digest).expect("Failed to pin model");
    assert!(eviction.is_pinned(digest));

    let pinned_list = eviction.list_pinned().unwrap();
    assert_eq!(pinned_list, vec![digest.to_string()]);

    let unpinned = eviction.unpin(digest).expect("Failed to unpin");
    assert!(unpinned);
    assert!(!eviction.is_pinned(digest));
}

#[test]
fn test_lru_prune_preserves_pinned_models() {
    let dir = tempdir().unwrap();
    let cas = CasStore::new(dir.path()).unwrap();
    let eviction = EvictionManager::new(dir.path()).unwrap();

    // Store three 1000-byte blobs
    let b1 = vec![1u8; 1000];
    let b2 = vec![2u8; 1000];
    let b3 = vec![3u8; 1000];

    let (d1, _) = cas.store_blob(Cursor::new(&b1), None).unwrap();
    let (_d2, _) = cas.store_blob(Cursor::new(&b2), None).unwrap();
    let (_d3, _) = cas.store_blob(Cursor::new(&b3), None).unwrap();

    // Pin blob 1 (important system emergency model)
    eviction.pin(&d1).unwrap();

    // Prune down to 1500 bytes (should evict unpinned d2 or d3, but NEVER d1)
    let reclaimed = eviction.prune(&cas, 1500).unwrap();
    assert!(reclaimed >= 1000);

    // Pinned model MUST survive
    assert!(cas.has_blob(&d1));
}
