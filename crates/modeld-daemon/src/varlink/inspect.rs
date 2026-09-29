//! Handler for io.syntrop.Model1.Inspect Varlink method.
//!
//! Resolves model identifiers and returns full metadata and pinning status.

use super::model1::ModelServiceContext;
use super::protocol::VarlinkReply;
use modeld_core::cas::TagRegistry;
use serde_json::json;
use tracing::warn;

/// Distinguishes the three outcomes of resolving a user-supplied identifier.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolveOutcome {
    Ok(String),
    Invalid,
    NotFound,
}

/// Resolves an identifier to a digest, distinguishing "rejected by segment
/// validation" from "no such model" so the daemon can return the correct
/// Varlink error code.
pub(crate) fn resolve_digest(tags: &TagRegistry, id: &str) -> ResolveOutcome {
    match tags.resolve(id) {
        Ok(Some(d)) => ResolveOutcome::Ok(d),
        Ok(None) => ResolveOutcome::NotFound,
        Err(_) => ResolveOutcome::Invalid,
    }
}

/// Handles `io.syntrop.Model1.Inspect` method invocation.
pub fn handle_inspect(params: Option<&serde_json::Value>, ctx: &ModelServiceContext) -> VarlinkReply {
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

    let metadata_str = match ctx.cas.open_blob(&digest) {
        Ok(file) => match modeld_core::format::parse_gguf_header(file) {
            Ok(meta) => {
                let arch = meta.architecture.as_deref().unwrap_or("unknown");
                let param_count = meta
                    .attributes
                    .get("general.parameter_count")
                    .or_else(|| {
                        meta.architecture
                            .as_ref()
                            .and_then(|a| meta.attributes.get(&format!("{}.parameter_count", a)))
                    })
                    .cloned()
                    .unwrap_or_else(|| meta.tensor_count.to_string());
                let context_len = meta
                    .context_length
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                format!(
                    "architecture: {}, parameter_count: {}, context_length: {}",
                    arch, param_count, context_len
                )
            }
            Err(_) => format!("Digest: {}", digest),
        },
        Err(_) => format!("Digest: {}", digest),
    };

    VarlinkReply::ok(json!({
        "info": {
            "id": id,
            "digest": digest,
            "size_bytes": size,
            "pinned": pinned,
            "format": "gguf"
        },
        "metadata": metadata_str
    }))
}
