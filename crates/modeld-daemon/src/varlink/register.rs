//! Handler for io.syntrop.Model1.Register Varlink method.
//!
//! Registers model entries and extracts GGUF architecture metadata.

use super::model1::ModelServiceContext;
use super::protocol::VarlinkReply;
use modeld_core::cas::digest_format::validate_digest_format;
use modeld_core::cas::tags::validate_tag_segment;
use serde_json::json;
use tracing::warn;

/// Handles `io.syntrop.Model1.Register` method invocation.
pub fn handle_register(
    params: Option<&serde_json::Value>,
    ctx: &ModelServiceContext,
) -> VarlinkReply {
    let id = match params.and_then(|p| p.get("id")).and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return VarlinkReply::err("io.syntrop.Model1.InvalidIdentifier", None),
    };

    let tag_param = params.and_then(|p| p.get("tag")).and_then(|v| v.as_str());
    let digest_param = params
        .and_then(|p| p.get("digest"))
        .and_then(|v| v.as_str());

    let (name, tag) = match tag_param {
        Some(t) => {
            let n = id.split_once(':').map(|(n, _)| n).unwrap_or(id);
            (n, t.trim())
        }
        None => {
            if let Some((n, t)) = id.split_once(':') {
                (n, t)
            } else {
                (id, "latest")
            }
        }
    };

    if let Err(e) = validate_tag_segment(name) {
        return VarlinkReply::err(
            "io.syntrop.Model1.InvalidIdentifier",
            Some(json!({ "id": id, "reason": e.to_string() })),
        );
    }
    if let Err(e) = validate_tag_segment(tag) {
        return VarlinkReply::err(
            "io.syntrop.Model1.InvalidParameter",
            Some(json!({ "parameter": "tag", "reason": e.to_string() })),
        );
    }

    let resolve_id = format!("{}:{}", name, tag);
    let digest_candidate = match digest_param {
        Some(d) => d.trim().to_string(),
        None => match ctx.tags.resolve(&resolve_id) {
            Ok(Some(d)) => d,
            Ok(None) => {
                return VarlinkReply::err(
                    "io.syntrop.Model1.NoSuchModel",
                    Some(json!({ "id": resolve_id })),
                );
            }
            Err(_) => {
                return VarlinkReply::err(
                    "io.syntrop.Model1.InvalidIdentifier",
                    Some(json!({ "id": resolve_id })),
                );
            }
        },
    };

    let mut d_str = digest_candidate.as_str();
    if let Some(rest) = d_str.strip_prefix("sha256:") {
        d_str = rest;
    } else if let Some(rest) = d_str.strip_prefix("sha256-") {
        d_str = rest;
    }
    if let Some(rest) = d_str.strip_suffix(".gguf") {
        d_str = rest;
    }
    let clean_digest = d_str.to_ascii_lowercase();

    if let Err(e) = validate_digest_format(&clean_digest) {
        return VarlinkReply::err(
            "io.syntrop.Model1.InvalidParameter",
            Some(json!({ "parameter": "digest", "reason": e.to_string() })),
        );
    }

    if let Err(e) = ctx.tags.set_tag(name, tag, &clean_digest) {
        return VarlinkReply::err(
            "io.syntrop.Model1.OperationFailed",
            Some(json!({ "reason": e.to_string() })),
        );
    }

    let size = ctx.cas.blob_size(&clean_digest).unwrap_or_else(|e| {
        warn!("blob_size failed for digest {}: {}", clean_digest, e);
        0
    });
    let pinned = ctx.eviction.is_pinned(&clean_digest);

    let mut format_str = "gguf";
    let mut metadata_str = Some(format!("Digest: {}", clean_digest));

    if let Ok(mut blob_file) = ctx.cas.open_blob(&clean_digest) {
        match modeld_core::format::detect_safe_format(&mut blob_file) {
            Err(modeld_core::error::ModeldError::SecurityRejection(reason)) => {
                return VarlinkReply::err(
                    "io.syntrop.Model1.SecurityRejection",
                    Some(json!({ "reason": reason })),
                );
            }
            Ok(modeld_core::format::SafeFormat::SafeTensors) => {
                use std::io::Seek;
                let _ = blob_file.seek(std::io::SeekFrom::Start(0));
                let meta_str = match modeld_core::format::parse_safetensors_header(blob_file) {
                    Ok(meta) => {
                        let arch = meta.architecture.as_deref().unwrap_or("unknown");
                        Some(format!(
                            "architecture: {}, parameter_count: {}, tensor_count: {}",
                            arch, meta.parameter_count, meta.tensor_count
                        ))
                    }
                    Err(e) => {
                        warn!("SafeTensors header parse failed for {}: {}", clean_digest, e);
                        Some(format!("Digest: {}", clean_digest))
                    }
                };
                format_str = "safetensors";
                metadata_str = meta_str;
            }
            _ => {
                use std::io::Seek;
                let _ = blob_file.seek(std::io::SeekFrom::Start(0));
                let meta_str = match modeld_core::format::parse_gguf_header(blob_file) {
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
                        Some(format!(
                            "architecture: {}, parameter_count: {}, context_length: {}",
                            arch, param_count, context_len
                        ))
                    }
                    Err(e) => {
                        warn!("GGUF header parse failed for {}: {}", clean_digest, e);
                        Some(format!("Digest: {}", clean_digest))
                    }
                };
                format_str = "gguf";
                metadata_str = meta_str;
            }
        }
    }

    let entry = json!({
        "id": format!("{}:{}", name, tag),
        "digest": clean_digest,
        "name": name,
        "tag": tag,
        "size_bytes": size,
        "pinned": pinned,
        "format": format_str
    });

    VarlinkReply::ok(json!({
        "entry": entry,
        "metadata": metadata_str
    }))
}
