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
    assert_eq!(
        params.get("product").and_then(|v| v.as_str()),
        Some("modeld")
    );
}

#[test]
fn test_varlink_service_get_interface_description() {
    let req = json!({ "interface": "io.syntrop.Model1" });
    let reply = handle_service_call("org.varlink.service.GetInterfaceDescription", Some(&req));
    assert!(reply.is_some());
    let params = reply.unwrap().parameters.unwrap();
    assert!(params
        .get("description")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("interface io.syntrop.Model1"));
}

#[test]
fn test_varlink_model1_get_storage_stats() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());

    let ctx = ModelServiceContext {
        cas,
        tags,
        eviction,
    };

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
    let ctx = ModelServiceContext {
        cas,
        tags,
        eviction,
    };

    for method in [
        "io.syntrop.Model1.Inspect",
        "io.syntrop.Model1.Pin",
        "io.syntrop.Model1.Unpin",
    ] {
        // Identifier with shell metacharacters that must not be silently
        // swallowed as "not found" — the daemon must return the dedicated
        // `InvalidIdentifier` error.
        let params = json!({ "id": "foo$bar" });
        let reply =
            handle_model1_call(method, Some(&params), &ctx).expect("dispatcher returned None");
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
    let ctx = ModelServiceContext {
        cas,
        tags,
        eviction,
    };

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
    let ctx = ModelServiceContext {
        cas,
        tags,
        eviction,
    };

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
    assert_eq!(
        entry.get("digest").and_then(|v| v.as_str()),
        Some(digest.as_str())
    );

    // Invalid identifier
    let invalid_id_params = json!({
        "id": "bad/model:tag",
        "digest": digest
    });
    let reply =
        handle_model1_call("io.syntrop.Model1.Register", Some(&invalid_id_params), &ctx).unwrap();
    assert_eq!(
        reply.error.as_deref(),
        Some("io.syntrop.Model1.InvalidIdentifier")
    );

    // Invalid digest
    let invalid_digest_params = json!({
        "id": "qwen2.5:0.5b",
        "digest": "invalid-digest"
    });
    let reply = handle_model1_call(
        "io.syntrop.Model1.Register",
        Some(&invalid_digest_params),
        &ctx,
    )
    .unwrap();
    assert_eq!(
        reply.error.as_deref(),
        Some("io.syntrop.Model1.InvalidParameter")
    );
}

#[test]
fn test_varlink_model1_register_gguf_metadata() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());
    let ctx = ModelServiceContext {
        cas: cas.clone(),
        tags,
        eviction,
    };

    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend_from_slice(&128u64.to_le_bytes());
    buf.extend_from_slice(&2u64.to_le_bytes());

    let k1 = "general.architecture";
    buf.extend_from_slice(&(k1.len() as u64).to_le_bytes());
    buf.extend_from_slice(k1.as_bytes());
    buf.extend_from_slice(&8u32.to_le_bytes());
    let v1 = "qwen2";
    buf.extend_from_slice(&(v1.len() as u64).to_le_bytes());
    buf.extend_from_slice(v1.as_bytes());

    let k2 = "qwen2.context_length";
    buf.extend_from_slice(&(k2.len() as u64).to_le_bytes());
    buf.extend_from_slice(k2.as_bytes());
    buf.extend_from_slice(&10u32.to_le_bytes());
    buf.extend_from_slice(&32768u64.to_le_bytes());

    let (digest, _) = cas.store_blob(std::io::Cursor::new(buf), None).unwrap();

    let params = json!({
        "id": "qwen2.5",
        "tag": "0.5b",
        "digest": format!("sha256-{}", digest)
    });
    let reply = handle_model1_call("io.syntrop.Model1.Register", Some(&params), &ctx).unwrap();
    assert!(reply.error.is_none(), "reply: {:?}", reply);
    let out = reply.parameters.unwrap();
    let meta = out.get("metadata").and_then(|v| v.as_str()).unwrap();
    assert!(meta.contains("architecture: qwen2"), "meta: {meta}");
    assert!(meta.contains("context_length: 32768"), "meta: {meta}");

    let stats_reply = handle_model1_call("io.syntrop.Model1.GetStorageStats", None, &ctx).unwrap();
    let stats_out = stats_reply.parameters.unwrap();
    assert_eq!(stats_out["model_count"], 1);
    assert!(stats_out["total_bytes"].as_u64().unwrap() > 0);
}
