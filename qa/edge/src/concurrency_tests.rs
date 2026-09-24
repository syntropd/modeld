//! Edge-case testing for concurrent tag writes and storage races.

use modeld_core::cas::{CasStore, TagRegistry};
use std::io::Cursor;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_edge_concurrent_tag_churn() {
    let dir = tempdir().unwrap();
    let registry = Arc::new(TagRegistry::new(dir.path()).unwrap());

    let mut handles = Vec::new();

    for i in 0..10 {
        let reg = registry.clone();
        handles.push(tokio::spawn(async move {
            let digest = format!("{:064x}", i);
            reg.set_tag("churn_model", "latest", &digest).unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let final_tag = registry.get_tag("churn_model", "latest").unwrap();
    assert!(final_tag.is_some());
}

#[tokio::test]
async fn test_edge_concurrent_duplicate_blob_stores() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());

    let mut handles = Vec::new();
    let payload = Arc::new(b"concurrent duplicate blob payload 999".to_vec());

    for _ in 0..8 {
        let store = cas.clone();
        let data = payload.clone();
        handles.push(tokio::spawn(async move {
            store.store_blob(Cursor::new(&*data), None).unwrap()
        }));
    }

    for h in handles {
        let (digest, bytes) = h.await.unwrap();
        assert_eq!(bytes, payload.len() as u64);
        assert!(cas.has_blob(&digest));
    }
}
