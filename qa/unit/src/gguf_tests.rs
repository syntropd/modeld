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
