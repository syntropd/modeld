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
