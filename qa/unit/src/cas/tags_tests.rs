//! 1:1 unit QA tests for cas::tags::TagRegistry.

use modeld_core::cas::TagRegistry;
use tempfile::tempdir;

#[test]
fn test_tag_set_get_remove_lifecycle() {
    let dir = tempdir().unwrap();
    let registry = TagRegistry::new(dir.path()).unwrap();

    let digest = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    registry.set_tag("qwen2.5", "7b", digest).expect("Failed to set tag");

    let retrieved = registry.get_tag("qwen2.5", "7b").expect("Failed to get tag");
    assert_eq!(retrieved, Some(digest.to_string()));

    let removed = registry.remove_tag("qwen2.5", "7b").expect("Failed to remove tag");
    assert!(removed);

    let retrieved_after = registry.get_tag("qwen2.5", "7b").unwrap();
    assert_eq!(retrieved_after, None);
}

#[test]
fn test_tag_list_sorted() {
    let dir = tempdir().unwrap();
    let registry = TagRegistry::new(dir.path()).unwrap();

    let d1 = "1111111111111111111111111111111111111111111111111111111111111111";
    let d2 = "2222222222222222222222222222222222222222222222222222222222222222";

    registry.set_tag("llama3.2", "1b", d1).unwrap();
    registry.set_tag("gemma2", "2b", d2).unwrap();

    let list = registry.list_tags().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name, "gemma2");
    assert_eq!(list[1].name, "llama3.2");
}

#[test]
fn test_tag_resolve_name_variant_and_raw_digest() {
    let dir = tempdir().unwrap();
    let registry = TagRegistry::new(dir.path()).unwrap();

    let digest = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    registry.set_tag("phi3", "mini", digest).unwrap();

    // Resolves explicit variant
    assert_eq!(registry.resolve("phi3:mini").unwrap(), Some(digest.to_string()));

    // Resolves raw 64-char hex digest directly
    assert_eq!(registry.resolve(digest).unwrap(), Some(digest.to_string()));

    // Non-existent returns None
    assert_eq!(registry.resolve("nonexistent:latest").unwrap(), None);
}
