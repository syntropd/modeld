//! Implementation of io.syntrop.Model1 Varlink interface.
//!
//! Exposes model catalog listing, inspection, pinning, and storage pruning.

use super::protocol::VarlinkReply;
use modeld_core::cas::{CasStore, EvictionManager, TagRegistry};
use serde_json::json;
use std::fs;
use std::sync::Arc;
use tracing::warn;

/// Context holding shared storage references for Varlink method execution.
#[derive(Clone)]
pub struct ModelServiceContext {
    /// Content-addressable storage store.
    pub cas: Arc<CasStore>,
    /// Human-readable tag registry.
    pub tags: Arc<TagRegistry>,
    /// Pinning and LRU eviction manager.
    pub eviction: Arc<EvictionManager>,
}

/// Dispatches calls matching the `io.syntrop.Model1` namespace.
pub fn handle_model1_call(
    method: &str,
    params: Option<&serde_json::Value>,
    ctx: &ModelServiceContext,
) -> Option<VarlinkReply> {
    match method {
        "io.syntrop.Model1.List" => Some(handle_list(ctx)),
        "io.syntrop.Model1.Inspect" => Some(handle_inspect(params, ctx)),
        "io.syntrop.Model1.Pin" => Some(handle_pin(params, ctx)),
        "io.syntrop.Model1.Unpin" => Some(handle_unpin(params, ctx)),
        "io.syntrop.Model1.Prune" => Some(handle_prune(params, ctx)),
        "io.syntrop.Model1.GetStorageStats" => Some(handle_get_storage_stats(ctx)),
        _ => None,
    }
}

fn handle_list(ctx: &ModelServiceContext) -> VarlinkReply {
    let mut entries = Vec::new();
    let tags = ctx.tags.list_tags().unwrap_or_default();

    // Map tagged models
    for tag in tags {
        let size = ctx.cas.blob_size(&tag.digest).unwrap_or_else(|e| {
            warn!("blob_size failed for tag {}:{}: {}", tag.name, tag.tag, e);
            0
        });
        let pinned = ctx.eviction.is_pinned(&tag.digest);
        entries.push(json!({
            "id": format!("{}:{}", tag.name, tag.tag),
            "digest": tag.digest,
            "name": tag.name,
            "tag": tag.tag,
            "size_bytes": size,
            "pinned": pinned,
            "format": "gguf"
        }));
    }

    VarlinkReply::ok(json!({ "models": entries }))
}

fn handle_inspect(params: Option<&serde_json::Value>, ctx: &ModelServiceContext) -> VarlinkReply {
    let id = match params.and_then(|p| p.get("id")).and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return VarlinkReply::err("io.syntrop.Model1.InvalidIdentifier", None),
    };

    let digest = match resolve_digest(&ctx.tags, id) {
        ResolveOutcome::Ok(d) => d,
        ResolveOutcome::Invalid => {
            return VarlinkReply::err(
                "io.syntrop.Model1.InvalidIdentifier",
                Some(json!({ "id": id })),
            );
        }
        ResolveOutcome::NotFound => {
            return VarlinkReply::err(
                "io.syntrop.Model1.NoSuchModel",
                Some(json!({ "id": id })),
            );
        }
    };

    let size = ctx.cas.blob_size(&digest).unwrap_or_else(|e| {
        warn!("blob_size failed for digest {}: {}", digest, e);
        0
    });
    let pinned = ctx.eviction.is_pinned(&digest);

    VarlinkReply::ok(json!({
        "info": {
            "id": id,
            "digest": digest,
            "size_bytes": size,
            "pinned": pinned,
            "format": "gguf"
        },
        "metadata": format!("Digest: {}", digest)
    }))
}

fn handle_pin(params: Option<&serde_json::Value>, ctx: &ModelServiceContext) -> VarlinkReply {
    let id = match params.and_then(|p| p.get("id")).and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return VarlinkReply::err("io.syntrop.Model1.InvalidIdentifier", None),
    };

    let digest = match resolve_digest(&ctx.tags, id) {
        ResolveOutcome::Ok(d) => d,
        ResolveOutcome::Invalid => {
            return VarlinkReply::err(
                "io.syntrop.Model1.InvalidIdentifier",
                Some(json!({ "id": id })),
            );
        }
        ResolveOutcome::NotFound => {
            return VarlinkReply::err(
                "io.syntrop.Model1.NoSuchModel",
                Some(json!({ "id": id })),
            );
        }
    };

    match ctx.eviction.pin(&digest) {
        Ok(_) => VarlinkReply::ok(json!({})),
        Err(e) => VarlinkReply::err(
            "io.syntrop.Model1.OperationFailed",
            Some(json!({ "reason": e.to_string() })),
        ),
    }
}

fn handle_unpin(params: Option<&serde_json::Value>, ctx: &ModelServiceContext) -> VarlinkReply {
    let id = match params.and_then(|p| p.get("id")).and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return VarlinkReply::err("io.syntrop.Model1.InvalidIdentifier", None),
    };

    let digest = match resolve_digest(&ctx.tags, id) {
        ResolveOutcome::Ok(d) => d,
        ResolveOutcome::Invalid => {
            return VarlinkReply::err(
                "io.syntrop.Model1.InvalidIdentifier",
                Some(json!({ "id": id })),
            );
        }
        ResolveOutcome::NotFound => {
            return VarlinkReply::err(
                "io.syntrop.Model1.NoSuchModel",
                Some(json!({ "id": id })),
            );
        }
    };

    match ctx.eviction.unpin(&digest) {
        Ok(_) => VarlinkReply::ok(json!({})),
        Err(e) => VarlinkReply::err(
            "io.syntrop.Model1.OperationFailed",
            Some(json!({ "reason": e.to_string() })),
        ),
    }
}

/// Distinguishes the three outcomes of resolving a user-supplied identifier:
/// found, well-formed-but-unknown, and rejected as unsafe.
enum ResolveOutcome {
    Ok(String),
    Invalid,
    NotFound,
}

/// Resolves an identifier to a digest, distinguishing "rejected by segment
/// validation" from "no such model" so the daemon can return the correct
/// Varlink error code.
fn resolve_digest(tags: &TagRegistry, id: &str) -> ResolveOutcome {
    match tags.resolve(id) {
        Ok(Some(d)) => ResolveOutcome::Ok(d),
        Ok(None) => ResolveOutcome::NotFound,
        Err(_) => ResolveOutcome::Invalid,
    }
}

fn handle_prune(params: Option<&serde_json::Value>, ctx: &ModelServiceContext) -> VarlinkReply {
    let max_bytes = params
        .and_then(|p| p.get("max_bytes"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if max_bytes == 0 {
        return VarlinkReply::err(
            "io.syntrop.Model1.InvalidParameter",
            Some(json!({
                "parameter": "max_bytes",
                "reason": "max_bytes must be greater than zero"
            })),
        );
    }

    match ctx.eviction.prune(&ctx.cas, max_bytes) {
        Ok(reclaimed) => VarlinkReply::ok(json!({ "reclaimed_bytes": reclaimed })),
        Err(e) => VarlinkReply::err(
            "io.syntrop.Model1.OperationFailed",
            Some(json!({ "reason": e.to_string() })),
        ),
    }
}

fn handle_get_storage_stats(ctx: &ModelServiceContext) -> VarlinkReply {
    let blobs_dir = ctx.cas.blobs_dir();
    let mut total_bytes: u64 = 0;
    let mut model_count: usize = 0;

    if let Ok(entries) = fs::read_dir(blobs_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total_bytes += meta.len();
                    model_count += 1;
                }
            }
        }
    }

    let pinned_count = ctx.eviction.list_pinned().map(|v| v.len()).unwrap_or(0);

    VarlinkReply::ok(json!({
        "total_bytes": total_bytes,
        "model_count": model_count,
        "pinned_count": pinned_count
    }))
}
