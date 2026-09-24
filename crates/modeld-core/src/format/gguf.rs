//! Pure Rust GGUF metadata parser.
//!
//! Extracts architecture, context length, and quantization without loading weights.

use crate::error::ModeldError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek};

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

    for _ in 0..metadata_kv_count {
        let key = match read_string(&mut reader) {
            Ok(k) => k,
            Err(_) => break,
        };
        let val_type = match read_u32_le(&mut reader) {
            Ok(t) => t,
            Err(_) => break,
        };

        if let Ok(val_str) = skip_or_read_value(&mut reader, val_type) {
            if key == "general.architecture" {
                architecture = Some(val_str.clone());
            } else if key.ends_with(".context_length") {
                context_length = val_str.parse::<u64>().ok();
            } else if key == "general.file_type" {
                file_type = val_str.parse::<u32>().ok();
            }
            attributes.insert(key, val_str);
        }
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
    Ok(String::from_utf8_lossy(&buf).to_string())
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
            // Array: item_type (u32) + count (u64)
            let item_type = read_u32_le(reader)?;
            let count = read_u64_le(reader)? as usize;
            for _ in 0..count.min(1024) {
                let _ = skip_or_read_value(reader, item_type)?;
            }
            Ok(format!("[array: {} items]", count))
        }
        10 => Ok(format!("{}", read_u64_le(reader)?)),
        11 => Ok(format!("{}", read_i64_le(reader)?)),
        12 => Ok(format!("{:.4}", read_f64_le(reader)?)),
        _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown GGUF type")),
    }
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
