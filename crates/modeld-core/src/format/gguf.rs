//! Pure Rust GGUF metadata parser.
//!
//! Extracts architecture, context length, and quantization without loading weights.

use crate::error::ModeldError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};

/// Extracted GGUF model metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufMetadata {
    /// GGUF file format version (typically 2 or 3).
    pub version: u32,
    /// Total number of tensor weights stored.
    pub tensor_count: u64,
    /// Architecture family (e.g. "llama", "qwen2", "phi3").
    pub architecture: Option<String>,
    /// Context window length in tokens.
    pub context_length: Option<u64>,
    /// Quantization or file type descriptor.
    pub file_type: Option<u32>,
    /// Key-value metadata summary.
    pub attributes: HashMap<String, String>,
}

/// Reads GGUF metadata from a seekable stream.
pub fn parse_gguf_header<R: Read + Seek>(mut reader: R) -> Result<GgufMetadata, ModeldError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if &magic != b"GGUF" {
        return Err(ModeldError::InvalidFormat("Missing GGUF magic bytes".into()));
    }

    let version = read_u32_le(&mut reader)?;
    if version < 2 || version > 3 {
        return Err(ModeldError::InvalidFormat(format!("Unsupported GGUF version: {}", version)));
    }

    let tensor_count = read_u64_le(&mut reader)?;
    let metadata_kv_count = read_u64_le(&mut reader)?;

    // Protect against pathological or malformed headers
    if metadata_kv_count > 4096 {
        return Err(ModeldError::InvalidFormat("GGUF metadata count exceeds safe threshold".into()));
    }

    let mut attributes = HashMap::new();
    let mut architecture = None;
    let mut context_length = None;
    let mut file_type = None;

    // Parse every declared KV. Truncation or unknown types must surface
    // as an error rather than silently returning partial metadata that
    // callers might treat as authoritative.
    for _ in 0..metadata_kv_count {
        let key = read_string(&mut reader)?;
        let val_type = read_u32_le(&mut reader)?;
        let val_str = skip_or_read_value(&mut reader, val_type)?;

        if key == "general.architecture" {
            architecture = Some(val_str.clone());
        } else if key.ends_with(".context_length") {
            context_length = val_str.parse::<u64>().ok();
        } else if key == "general.file_type" {
            file_type = val_str.parse::<u32>().ok();
        }
        attributes.insert(key, val_str);
    }

    Ok(GgufMetadata {
        version,
        tensor_count,
        architecture,
        context_length,
        file_type,
        attributes,
    })
}

fn read_u32_le<R: Read>(reader: &mut R) -> std::io::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64_le<R: Read>(reader: &mut R) -> std::io::Result<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_string<R: Read>(reader: &mut R) -> std::io::Result<String> {
    let len = read_u64_le(reader)? as usize;
    if len > 1024 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Key string exceeds limit"));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    // GGUF metadata keys are required to be valid UTF-8 per the spec.
    // Replacing invalid bytes with U+FFFD would silently collapse
    // distinct identifiers into a single key, so fail the parse instead.
    std::str::from_utf8(&buf)
        .map(|s| s.to_string())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn item_byte_size(item_type: u32) -> Option<u64> {
    // GGUF value type widths in bytes. Arrays use this to compute the
    // total payload size without parsing every element.
    match item_type {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4 | 5 | 6 => Some(4),
        10 | 11 | 12 => Some(8),
        // String and array items are variable-length and cannot be sized here.
        8 | 9 => None,
        _ => None,
    }
}

fn skip_or_read_value<R: Read + Seek>(reader: &mut R, val_type: u32) -> std::io::Result<String> {
    match val_type {
        0 => Ok(format!("{}", read_u8(reader)?)),
        1 => Ok(format!("{}", read_i8(reader)?)),
        2 => Ok(format!("{}", read_u16_le(reader)?)),
        3 => Ok(format!("{}", read_i16_le(reader)?)),
        4 => Ok(format!("{}", read_u32_le(reader)?)),
        5 => Ok(format!("{}", read_i32_le(reader)?)),
        6 => Ok(format!("{:.4}", read_f32_le(reader)?)),
        7 => Ok(format!("{}", read_u8(reader)? != 0)),
        8 => read_string(reader),
        9 => {
            // Array: item_type (u32) + count (u64) + items...
            //
            // The previous implementation parsed only the first 1024 items
            // and returned success, leaving the reader positioned mid-array
            // so the next KV pair read garbage. Compute the payload size
            // and seek over it instead.
            let item_type = read_u32_le(reader)?;
            let count = read_u64_le(reader)?;
            let payload_size = match item_byte_size(item_type) {
                Some(width) => count
                    .checked_mul(width)
                    .ok_or_else(|| invalid_data("GGUF array length overflows u64"))?,
                None => {
                    // Variable-size elements (strings or nested arrays):
                    // fall back to per-element skip, bounded by a safety cap.
                    let mut skipped: u64 = 0;
                    let max_iter = count.min(64 * 1024);
                    for _ in 0..max_iter {
                        skip_or_read_value(reader, item_type)?;
                        skipped += 1;
                    }
                    if count > skipped {
                        return Err(invalid_data(
                            "GGUF array exceeds safety skip cap; refusing to skip",
                        ));
                    }
                    return Ok(format!("[array: {} items]", count));
                }
            };
            // Seek forward over the array payload in a single syscall.
            let cur = reader.stream_position()?;
            let end = cur
                .checked_add(payload_size)
                .ok_or_else(|| invalid_data("GGUF array seek end overflows u64"))?;
            reader.seek(SeekFrom::Start(end))?;
            Ok(format!("[array: {} items]", count))
        }
        10 => Ok(format!("{}", read_u64_le(reader)?)),
        11 => Ok(format!("{}", read_i64_le(reader)?)),
        12 => Ok(format!("{:.4}", read_f64_le(reader)?)),
        _ => Err(invalid_data("Unknown GGUF type")),
    }
}

fn invalid_data(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg)
}

fn read_u8<R: Read>(reader: &mut R) -> std::io::Result<u8> {
    let mut b = [0u8; 1];
    reader.read_exact(&mut b)?;
    Ok(b[0])
}
fn read_i8<R: Read>(reader: &mut R) -> std::io::Result<i8> { Ok(read_u8(reader)? as i8) }
fn read_u16_le<R: Read>(reader: &mut R) -> std::io::Result<u16> {
    let mut b = [0u8; 2]; reader.read_exact(&mut b)?; Ok(u16::from_le_bytes(b))
}
fn read_i16_le<R: Read>(reader: &mut R) -> std::io::Result<i16> {
    let mut b = [0u8; 2]; reader.read_exact(&mut b)?; Ok(i16::from_le_bytes(b))
}
fn read_i32_le<R: Read>(reader: &mut R) -> std::io::Result<i32> {
    let mut b = [0u8; 4]; reader.read_exact(&mut b)?; Ok(i32::from_le_bytes(b))
}
fn read_f32_le<R: Read>(reader: &mut R) -> std::io::Result<f32> {
    let mut b = [0u8; 4]; reader.read_exact(&mut b)?; Ok(f32::from_le_bytes(b))
}
fn read_i64_le<R: Read>(reader: &mut R) -> std::io::Result<i64> {
    let mut b = [0u8; 8]; reader.read_exact(&mut b)?; Ok(i64::from_le_bytes(b))
}
fn read_f64_le<R: Read>(reader: &mut R) -> std::io::Result<f64> {
    let mut b = [0u8; 8]; reader.read_exact(&mut b)?; Ok(f64::from_le_bytes(b))
}
