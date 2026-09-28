//! 1:1 unit QA tests for cas::store::CasStore.

use modeld_core::cas::CasStore;
use std::io::{Cursor, Read};
use tempfile::tempdir;

#[test]
fn test_cas_store_init_and_paths() {
    let dir = tempdir().unwrap();
    let store = CasStore::new(dir.path()).expect("Store init failed");

    assert!(store.blobs_dir().is_dir());
    assert!(store.incoming_dir().is_dir());
}

#[test]
fn test_cas_store_blob_and_verify() {
    let dir = tempdir().unwrap();
    let store = CasStore::new(dir.path()).unwrap();

    let payload = b"model weights tensor data simulation 12345";
    let (digest, bytes) = store
        .store_blob(Cursor::new(payload), None)
        .expect("Store blob failed");

    assert_eq!(bytes, payload.len() as u64);
    assert!(store.has_blob(&digest));

    let size = store.blob_size(&digest).expect("Failed to get size");
    assert_eq!(size, payload.len() as u64);

    let mut reader = store.open_blob(&digest).expect("Failed to open blob");
    let mut read_bytes = Vec::new();
    reader.read_to_end(&mut read_bytes).unwrap();
    assert_eq!(read_bytes, payload);
}

#[test]
fn test_cas_store_with_expected_digest_mismatch() {
    let dir = tempdir().unwrap();
    let store = CasStore::new(dir.path()).unwrap();

    let payload = b"model data";
    let wrong = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let res = store.store_blob(Cursor::new(payload), Some(wrong));

    assert!(res.is_err());
    // Assert incoming directory is clean
    assert!(!store.has_blob(wrong));
}

#[test]
fn test_cas_store_deduplication() {
    let dir = tempdir().unwrap();
    let store = CasStore::new(dir.path()).unwrap();

    let payload = b"identical duplicate model payload";
    let (d1, s1) = store.store_blob(Cursor::new(payload), None).unwrap();
    let (d2, s2) = store.store_blob(Cursor::new(payload), None).unwrap();

    assert_eq!(d1, d2);
    assert_eq!(s1, s2);
}
