//! Security validation and model format sanitization.
//!
//! Rejects unsafe serialized formats such as Python pickle payloads.

use crate::error::ModeldError;
use std::io::Read;
use std::path::Path;

/// Banned file extensions known to contain arbitrary executable bytecode.
const BANNED_EXTENSIONS: &[&str] = &["pt", "bin", "pkl", "pickle", "joblib"];

/// Recognized safe format headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeFormat {
    /// GGUF (GGML Universal Format) binary.
    Gguf,
    /// SafeTensors JSON-prefixed binary.
    SafeTensors,
    /// Open Neural Network Exchange protobuf.
    Onnx,
}

/// Validates that a file path or extension is permitted by safety policy.
pub fn validate_file_safety<P: AsRef<Path>>(path: P) -> Result<(), ModeldError> {
    if let Some(ext) = path.as_ref().extension().and_then(|s| s.to_str()) {
        let ext_lower = ext.to_lowercase();
        if BANNED_EXTENSIONS.contains(&ext_lower.as_str()) {
            return Err(ModeldError::SecurityRejection(format!(
                "File extension '.{}' may contain unsafe Python pickle bytecode",
                ext
            )));
        }
    }
    Ok(())
}

/// Identifies the format from the initial stream bytes and asserts safety.
pub fn detect_safe_format<R: Read>(mut reader: R) -> Result<SafeFormat, ModeldError> {
    let mut header = [0u8; 16];
    let n = reader.read(&mut header)?;
    if n < 4 {
        return Err(ModeldError::InvalidFormat("Stream too short for header".into()));
    }

    // Check for GGUF magic bytes: 'G', 'G', 'U', 'F' (0x46554747)
    if &header[0..4] == b"GGUF" {
        return Ok(SafeFormat::Gguf);
    }

    // Check for Python pickle protocol opcodes: 0x80 [0x02..0x05]
    if header[0] == 0x80 && (header[1] >= 2 && header[1] <= 5) {
        return Err(ModeldError::SecurityRejection(
            "Detected Python pickle protocol header opcode".into(),
        ));
    }

    // Check for SafeTensors: first 8 bytes is u64 little-endian header length,
    // and byte 8 should typically be '{' (0x7B) for the JSON header.
    if n >= 9 {
        let json_start = header[8];
        if json_start == b'{' || json_start == b' ' {
            let header_len = u64::from_le_bytes(header[0..8].try_into().unwrap());
            if header_len > 0 && header_len < 100 * 1024 * 1024 {
                return Ok(SafeFormat::SafeTensors);
            }
        }
    }

    // Check for ONNX: Protobuf wire format (starts with field 1: ir_version, tag 0x08)
    if header[0] == 0x08 {
        return Ok(SafeFormat::Onnx);
    }

    Err(ModeldError::InvalidFormat(
        "Unrecognized or unsupported model format header".into(),
    ))
}
