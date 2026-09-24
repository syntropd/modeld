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
    let (d2, _) = cas.store_blob(Cursor::new(&b2), None).unwrap();
    let (d3, _) = cas.store_blob(Cursor::new(&b3), None).unwrap();

    // Pin blob 1 (important system emergency model)
    eviction.pin(&d1).unwrap();

    // Prune down to 2500 bytes. Total is 3000, pinned d1=1000 is
    // protected, so the loop must remove unpinned blobs until
    // current_total <= 2500. The oldest unpinned (d2 by mtime) is
    // evicted, bringing the total to 2000 <= 2500, then loop exits.
    // Exactly one unpinned blob must be reclaimed and exactly one
    // must survive.
    let reclaimed = eviction.prune(&cas, 2500).unwrap();
    assert_eq!(
        reclaimed, 1000,
        "exactly one unpinned blob must be reclaimed"
    );

    // Pinned model MUST survive.
    assert!(cas.has_blob(&d1));

    // Exactly one of {d2, d3} survives.
    let survivors: u64 = [&d2, &d3]
        .iter()
        .map(|d| if cas.has_blob(d) { 1000 } else { 0 })
        .sum();
    assert_eq!(
        survivors, 1000,
        "exactly one unpinned blob must survive after the prune"
    );
}
