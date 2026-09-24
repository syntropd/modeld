//! Edge-case testing for corrupted headers and malicious byte streams.

use modeld_core::format::{detect_safe_format, parse_gguf_header, parse_safetensors_header};
use std::io::Cursor;

#[test]
fn test_edge_gguf_truncated_at_all_lengths() {
    let raw = b"GGUF\x03\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\x00";
    for i in 0..raw.len() {
        let truncated = &raw[..i];
        let res = parse_gguf_header(Cursor::new(truncated));
        assert!(res.is_err(), "Truncation at length {} did not fail safely", i);
    }
}

#[test]
fn test_edge_safetensors_astronomical_header_length_rejection() {
    // 1 GiB header length declared in first 8 bytes
    let mut buf = Vec::new();
    buf.extend_from_slice(&(1024u64 * 1024 * 1024).to_le_bytes());
    buf.extend_from_slice(b"{}");

    let res = parse_safetensors_header(Cursor::new(buf));
    assert!(res.is_err());
}

#[test]
fn test_edge_detect_format_random_garbage() {
    let garbage = [0xFF, 0xFE, 0x00, 0x42, 0x11, 0x99];
    let res = detect_safe_format(Cursor::new(garbage));
    assert!(res.is_err());
}
