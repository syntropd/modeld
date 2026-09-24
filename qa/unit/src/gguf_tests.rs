//! 1:1 unit QA tests for format::gguf parser.

use modeld_core::format::parse_gguf_header;
use std::io::Cursor;

fn build_synthetic_gguf_header() -> Vec<u8> {
    let mut buf = Vec::new();
    // Magic: "GGUF"
    buf.extend_from_slice(b"GGUF");
    // Version: 3 (u32 le)
    buf.extend_from_slice(&3u32.to_le_bytes());
    // Tensor count: 128 (u64 le)
    buf.extend_from_slice(&128u64.to_le_bytes());
    // Metadata KV count: 2 (u64 le)
    buf.extend_from_slice(&2u64.to_le_bytes());

    // KV 1: "general.architecture" -> "llama" (val_type: 8 = string)
    let k1 = "general.architecture";
    buf.extend_from_slice(&(k1.len() as u64).to_le_bytes());
    buf.extend_from_slice(k1.as_bytes());
    buf.extend_from_slice(&8u32.to_le_bytes()); // type string
    let v1 = "llama";
    buf.extend_from_slice(&(v1.len() as u64).to_le_bytes());
    buf.extend_from_slice(v1.as_bytes());

    // KV 2: "llama.context_length" -> 8192 (val_type: 10 = u64)
    let k2 = "llama.context_length";
    buf.extend_from_slice(&(k2.len() as u64).to_le_bytes());
    buf.extend_from_slice(k2.as_bytes());
    buf.extend_from_slice(&10u32.to_le_bytes()); // type u64
    buf.extend_from_slice(&8192u64.to_le_bytes());

    buf
}

#[test]
fn test_parse_gguf_header_synthetic_success() {
    let raw = build_synthetic_gguf_header();
    let meta = parse_gguf_header(Cursor::new(raw)).expect("GGUF parsing failed");

    assert_eq!(meta.version, 3);
    assert_eq!(meta.tensor_count, 128);
    assert_eq!(meta.architecture.as_deref(), Some("llama"));
    assert_eq!(meta.context_length, Some(8192));
}

#[test]
fn test_parse_gguf_header_invalid_magic() {
    let raw = b"NOT_GGUF_HEADER_BYTES";
    let res = parse_gguf_header(Cursor::new(raw));
    assert!(res.is_err());
}

/// Build a GGUF v3 header with 3 KV pairs:
///   - general.architecture (string)
///   - llama.context_length (u64)
///   - tokenizer.ggml.bos_token_id (array of u32, count = 5000)
///
/// The array length is intentionally larger than the 1024-item cap that
/// the previous buggy implementation used, so the test would have parsed
/// the wrong bytes for the third KV before this fix.
fn build_gguf_header_with_large_array() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend_from_slice(&128u64.to_le_bytes());
    buf.extend_from_slice(&3u64.to_le_bytes()); // 3 KV pairs

    // KV 1
    let k1 = "general.architecture";
    buf.extend_from_slice(&(k1.len() as u64).to_le_bytes());
    buf.extend_from_slice(k1.as_bytes());
    buf.extend_from_slice(&8u32.to_le_bytes());
    let v1 = "llama";
    buf.extend_from_slice(&(v1.len() as u64).to_le_bytes());
    buf.extend_from_slice(v1.as_bytes());

    // KV 2
    let k2 = "llama.context_length";
    buf.extend_from_slice(&(k2.len() as u64).to_le_bytes());
    buf.extend_from_slice(k2.as_bytes());
    buf.extend_from_slice(&10u32.to_le_bytes());
    buf.extend_from_slice(&8192u64.to_le_bytes());

    // KV 3: array of 5000 u32 items (20 KiB payload).
    let k3 = "tokenizer.ggml.bos_token_id";
    buf.extend_from_slice(&(k3.len() as u64).to_le_bytes());
    buf.extend_from_slice(k3.as_bytes());
    buf.extend_from_slice(&9u32.to_le_bytes()); // value type: array
    buf.extend_from_slice(&4u32.to_le_bytes()); // array element type: u32
    buf.extend_from_slice(&5000u64.to_le_bytes()); // element count
    buf.extend(std::iter::repeat(0u32).take(5000).flat_map(|n| n.to_le_bytes()));

    buf
}

#[test]
fn test_parse_gguf_header_array_parsing_does_not_corrupt_subsequent_kv() {
    let raw = build_gguf_header_with_large_array();
    let meta = parse_gguf_header(Cursor::new(raw)).expect("GGUF parse failed");

    // If the array skip is wrong, the third KV would either fail or report
    // garbage, and the early KVs would also be misread.
    assert_eq!(meta.architecture.as_deref(), Some("llama"));
    assert_eq!(meta.context_length, Some(8192));
    assert_eq!(meta.attributes.len(), 3);
    assert!(meta.attributes.contains_key("tokenizer.ggml.bos_token_id"));
    assert_eq!(
        meta.attributes.get("tokenizer.ggml.bos_token_id").unwrap(),
        "[array: 5000 items]"
    );
}

#[test]
fn test_parse_gguf_header_truncated_at_kv_reports_error() {
    // KV count says 2 but only the first KV is present.
    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes()); // tensor count
    buf.extend_from_slice(&2u64.to_le_bytes()); // KV count = 2

    // First KV is complete.
    let k1 = "general.architecture";
    buf.extend_from_slice(&(k1.len() as u64).to_le_bytes());
    buf.extend_from_slice(k1.as_bytes());
    buf.extend_from_slice(&8u32.to_le_bytes());
    let v1 = "llama";
    buf.extend_from_slice(&(v1.len() as u64).to_le_bytes());
    buf.extend_from_slice(v1.as_bytes());

    // Second KV is missing; the parser must surface this as an error.
    let res = parse_gguf_header(Cursor::new(buf));
    assert!(res.is_err(), "truncated header must not be silently accepted");
}
