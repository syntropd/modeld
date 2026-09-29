//! Pure Rust SafeTensors header parser.
//!
//! Parses the 8-byte length prefix and JSON metadata without reading tensor data.

use crate::error::ModeldError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

/// Extracted SafeTensors metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeTensorsMetadata {
    /// Total number of individual tensors declared in the header.
    pub tensor_count: usize,
    /// Total parameter count summed from tensor shapes.
    pub parameter_count: u64,
    /// Model architecture detected or specified in metadata.
    pub architecture: Option<String>,
    /// Model format metadata attributes.
    pub attributes: HashMap<String, String>,
}

/// Maximum SafeTensors JSON header size in bytes. Real SafeTensors headers
/// are typically well under 1 MiB; the cap here is a DoS guard against
/// attacker-controlled `header_len` values that would otherwise force a
/// multi-hundred-megabyte allocation per import.
const MAX_SAFETENSORS_HEADER: usize = 8 * 1024 * 1024;

/// Reads SafeTensors metadata from a readable stream.
pub fn parse_safetensors_header<R: Read>(
    mut reader: R,
) -> Result<SafeTensorsMetadata, ModeldError> {
    let mut len_buf = [0u8; 8];
    reader.read_exact(&mut len_buf)?;
    let header_len = u64::from_le_bytes(len_buf) as usize;

    // Safety guard: reject empty or unreasonably large header lengths.
    // A 33 MiB allocation per import is a reliable OOM vector against
    // memory-constrained modeld hosts; tighten to 8 MiB which still
    // accommodates every legitimate SafeTensors file shipped today.
    if header_len == 0 {
        return Err(ModeldError::InvalidFormat(
            "SafeTensors header length is zero".into(),
        ));
    }
    if header_len > MAX_SAFETENSORS_HEADER {
        return Err(ModeldError::InvalidFormat(format!(
            "SafeTensors header length {} exceeds {} byte safety cap",
            header_len, MAX_SAFETENSORS_HEADER
        )));
    }

    let mut json_buf = vec![0u8; header_len];
    reader.read_exact(&mut json_buf)?;

    let parsed_json: serde_json::Value = serde_json::from_slice(&json_buf)
        .map_err(|e| ModeldError::InvalidFormat(format!("Malformed SafeTensors JSON: {}", e)))?;

    let obj = parsed_json.as_object().ok_or_else(|| {
        ModeldError::InvalidFormat("SafeTensors header is not a JSON object".into())
    })?;

    let mut tensor_count = 0;
    let mut parameter_count: u64 = 0;
    let mut attributes = HashMap::new();
    let mut architecture = None;

    for (k, v) in obj {
        if k == "__metadata__" {
            if let Some(meta_obj) = v.as_object() {
                for (mk, mv) in meta_obj {
                    let val_str = match mv {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        other => other.to_string(),
                    };
                    attributes.insert(mk.clone(), val_str);
                }
            }
        } else {
            tensor_count += 1;
            if let Some(tensor_obj) = v.as_object() {
                if let Some(shape_arr) = tensor_obj.get("shape").and_then(|s| s.as_array()) {
                    let mut prod: u64 = 1;
                    for dim in shape_arr {
                        if let Some(d) = dim.as_u64() {
                            prod = prod.saturating_mul(d);
                        }
                    }
                    parameter_count = parameter_count.saturating_add(prod);
                }
            }
        }
    }

    if let Some(arch) = attributes
        .get("architecture")
        .or_else(|| attributes.get("models.model_type"))
        .or_else(|| attributes.get("model_type"))
        .or_else(|| attributes.get("general.architecture"))
    {
        architecture = Some(arch.clone());
    }

    Ok(SafeTensorsMetadata {
        tensor_count,
        parameter_count,
        architecture,
        attributes,
    })
}
