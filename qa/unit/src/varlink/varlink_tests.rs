//! 1:1 unit QA tests for varlink protocol and method handlers.

use modeld_core::cas::{CasStore, EvictionManager, TagRegistry};
use modeld_daemon::varlink::{
    handle_model1_call, handle_service_call, ModelServiceContext, VarlinkReply,
};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_varlink_reply_nul_termination() {
    let reply = VarlinkReply::ok(json!({ "status": "active" }));
    let bytes = reply.to_bytes();
    assert_eq!(bytes.last(), Some(&0x00));
}

#[test]
fn test_varlink_service_get_info() {
    let reply = handle_service_call("org.varlink.service.GetInfo", None);
    assert!(reply.is_some());
    let params = reply.unwrap().parameters.unwrap();
    assert_eq!(params.get("product").and_then(|v| v.as_str()), Some("modeld"));
}

#[test]
fn test_varlink_service_get_interface_description() {
    let req = json!({ "interface": "io.syntrop.Model1" });
    let reply = handle_service_call("org.varlink.service.GetInterfaceDescription", Some(&req));
    assert!(reply.is_some());
    let params = reply.unwrap().parameters.unwrap();
    assert!(params.get("description").unwrap().as_str().unwrap().contains("interface io.syntrop.Model1"));
}

#[test]
fn test_varlink_model1_get_storage_stats() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());

    let ctx = ModelServiceContext { cas, tags, eviction };

    let reply = handle_model1_call("io.syntrop.Model1.GetStorageStats", None, &ctx);
    assert!(reply.is_some());
    let params = reply.unwrap().parameters.unwrap();
    assert_eq!(params.get("total_bytes").and_then(|v| v.as_u64()), Some(0));
}

#[test]
fn test_varlink_model1_rejects_unsafe_identifier_as_invalid_identifier() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());
    let ctx = ModelServiceContext { cas, tags, eviction };

    for method in [
        "io.syntrop.Model1.Inspect",
        "io.syntrop.Model1.Pin",
        "io.syntrop.Model1.Unpin",
    ] {
        // Identifier with shell metacharacters that must not be silently
        // swallowed as "not found" — the daemon must return the dedicated
        // `InvalidIdentifier` error.
        let params = json!({ "id": "foo$bar" });
        let reply = handle_model1_call(method, Some(&params), &ctx).expect("dispatcher returned None");
        assert_eq!(
            reply.error.as_deref(),
            Some("io.syntrop.Model1.InvalidIdentifier"),
            "{method} must reject unsafe identifier"
        );
    }
}

#[test]
fn test_varlink_model1_unknown_identifier_is_no_such_model() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());
    let ctx = ModelServiceContext { cas, tags, eviction };

    let params = json!({ "id": "nonexistent:latest" });
    let reply = handle_model1_call("io.syntrop.Model1.Inspect", Some(&params), &ctx).unwrap();
    assert_eq!(
        reply.error.as_deref(),
        Some("io.syntrop.Model1.NoSuchModel")
    );
}

#[test]
fn test_varlink_model1_register_success_and_errors() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());
    let ctx = ModelServiceContext { cas, tags, eviction };

    let digest = "ab".repeat(32);
    let params = json!({
        "id": "qwen2.5:0.5b",
        "digest": digest
    });
    let reply = handle_model1_call("io.syntrop.Model1.Register", Some(&params), &ctx).unwrap();
    assert!(reply.error.is_none());
    let out = reply.parameters.unwrap();
    let entry = out.get("entry").unwrap();
    assert_eq!(entry.get("name").and_then(|v| v.as_str()), Some("qwen2.5"));
    assert_eq!(entry.get("tag").and_then(|v| v.as_str()), Some("0.5b"));
    assert_eq!(entry.get("digest").and_then(|v| v.as_str()), Some(digest.as_str()));

    // Invalid identifier
    let invalid_id_params = json!({
        "id": "bad/model:tag",
        "digest": digest
    });
    let reply = handle_model1_call("io.syntrop.Model1.Register", Some(&invalid_id_params), &ctx).unwrap();
    assert_eq!(reply.error.as_deref(), Some("io.syntrop.Model1.InvalidIdentifier"));

    // Invalid digest
    let invalid_digest_params = json!({
        "id": "qwen2.5:0.5b",
        "digest": "invalid-digest"
    });
    let reply = handle_model1_call("io.syntrop.Model1.Register", Some(&invalid_digest_params), &ctx).unwrap();
    assert_eq!(reply.error.as_deref(), Some("io.syntrop.Model1.InvalidParameter"));
}

