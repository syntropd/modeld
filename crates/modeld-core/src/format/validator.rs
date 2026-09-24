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
///
/// The detector is intentionally strict: a binary blob is only accepted as
/// SafeTensors if the declared header length is plausible AND the next bytes
/// parse as a UTF-8 JSON object. The single-byte ONNX check is intentionally
/// left as a coarse protobuf sniff; callers that require stronger ONNX
/// validation should parse the file with a real protobuf decoder.
pub fn detect_safe_format<R: Read>(mut reader: R) -> Result<SafeFormat, ModeldError> {
    let mut header = [0u8; 16];
    let n = reader.read(&mut header)?;
    if n < 4 {
        return Err(ModeldError::InvalidFormat("Stream too short for header".into()));
    }

    // GGUF magic: "GGUF" (4 ASCII bytes).
    if &header[0..4] == b"GGUF" {
        return Ok(SafeFormat::Gguf);
    }

    // Python pickle protocol opcodes: 0x80 [0x02..0x05]. Reject early so
    // a renamed pickle never reaches the CAS store even if the extension
    // check passed.
    if header[0] == 0x80 && (header[1] >= 2 && header[1] <= 5) {
        return Err(ModeldError::SecurityRejection(
            "Detected Python pickle protocol header opcode".into(),
        ));
    }

    // SafeTensors: 8-byte little-endian header length, followed by a JSON
    // object. The previous heuristic accepted any blob whose byte 8 was '{'
    // or a space AND whose first 8 bytes formed a small u64 — that admits
    // ~2^-16 random blobs. Tighten by actually peeking the JSON head.
    if n >= 9 {
        let header_len = u64::from_le_bytes(header[0..8].try_into().unwrap()) as usize;
        let json_byte = header[8];
        if (8..=8 * 1024 * 1024).contains(&header_len) && json_byte == b'{' {
            // The JSON must start with '{' and not contain an interior NUL
            // within the first 8 header bytes. We do not require a full JSON
            // parse here — the dedicated parser enforces that.
            if looks_like_safetensors_json_head(&header[8..n]) {
                return Ok(SafeFormat::SafeTensors);
            }
        }
    }

    // ONNX: protobuf file where field 1 (ir_version) is the first wire-format
    // tag 0x08 (varint). A single-byte match is loose but the spec is
    // intentionally narrow; the per-field validation lives in the consumer.
    if header[0] == 0x08 {
        return Ok(SafeFormat::Onnx);
    }

    Err(ModeldError::InvalidFormat(
        "Unrecognized or unsupported model format header".into(),
    ))
}

/// Cheap structural check on the first JSON bytes of a SafeTensors header.
///
/// A real SafeTensors JSON header starts with `{` and contains an
/// `__metadata__` key plus tensor entries. We only check that the bytes
/// look like JSON (no NUL, no control characters other than space) — the
/// dedicated parser validates the structure.
fn looks_like_safetensors_json_head(bytes: &[u8]) -> bool {
    for &b in bytes {
        match b {
            b'{' | b'}' | b'"' | b':' | b',' | b' ' | b'\t' | b'\n' | b'\r'
            | b'[' | b']' | b'.' | b'-' | b'+' | b'_' | b'/' | b'\\' => {}
            0..=0x1F | 0x7F => return false,
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_safe_format_rejects_arbitrary_blob_misread_as_safetensors() {
        // 8 bytes form a small u64 (< 100 MiB) AND byte 8 is '{'. The
        // previous heuristic accepted this even though the rest of the
        // header is garbage; the strict check now requires the JSON head
        // to look like JSON, which a stray '{' followed by control bytes
        // does not.
        let mut bytes = vec![0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        bytes.push(b'{');
        bytes.push(0x01); // not a valid JSON byte
        bytes.extend_from_slice(&[0u8; 6]);
        let res = detect_safe_format(&bytes[..]);
        assert!(res.is_err(), "control byte in JSON head must reject");
    }

    #[test]
    fn test_detect_safe_format_accepts_real_safetensors_head() {
        // header_len = 17, JSON head: {"__metadata__":{...
        let json = br#"{"__metadata__":{"a":"b"#;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(json.len() as u64).to_le_bytes());
        bytes.extend_from_slice(json);
        let res = detect_safe_format(&bytes[..]);
        assert_eq!(res.unwrap(), SafeFormat::SafeTensors);
    }

    #[test]
    fn test_detect_safe_format_rejects_oversized_safetensors_header() {
        // header_len larger than the new 8 MiB cap must be rejected.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(16u64 * 1024 * 1024).to_le_bytes());
        bytes.push(b'{');
        let res = detect_safe_format(&bytes[..]);
        assert!(res.is_err());
    }
}
