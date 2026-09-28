//! 1:1 unit QA tests for cas::reflink::reflink_or_copy.

use modeld_core::cas::reflink_or_copy;
use tempfile::tempdir;

#[test]
fn test_reflink_or_copy_preserves_content() {
    let dir = tempdir().unwrap();
    let src = dir.path().join("source.gguf");
    let dest = dir.path().join("dest.gguf");
    let payload = b"reflink content preservation check payload";
    std::fs::write(&src, payload).unwrap();

    // On Btrfs/XFS this reflinks; elsewhere it falls back to a plain
    // copy. Either path must produce a byte-identical destination.
    let _reflinked = reflink_or_copy(&src, &dest).expect("reflink_or_copy failed");
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
}

#[test]
fn test_reflink_or_copy_missing_source_fails() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("nope.gguf");
    let dest = dir.path().join("dest.gguf");
    assert!(reflink_or_copy(&missing, &dest).is_err());
}
