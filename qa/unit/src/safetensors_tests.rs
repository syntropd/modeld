//! 1:1 unit QA tests for format::safetensors parser.

use modeld_core::format::parse_safetensors_header;
use std::io::Cursor;

fn build_synthetic_safetensors_header() -> Vec<u8> {
    let json_str = r#"{"weight_1":{"dtype":"F16","shape":[1024,1024]},"__metadata__":{"format":"pt"}}"#;
    let json_bytes = json_str.as_bytes();
    let header_len = json_bytes.len() as u64;

    let mut buf = Vec::new();
    buf.extend_from_slice(&header_len.to_le_bytes());
    buf.extend_from_slice(json_bytes);
    buf
}

#[test]
fn test_parse_safetensors_synthetic_success() {
    let raw = build_synthetic_safetensors_header();
    let meta = parse_safetensors_header(Cursor::new(raw)).expect("SafeTensors parsing failed");

    assert_eq!(meta.tensor_count, 1);
    assert_eq!(meta.attributes.get("format").map(String::as_str), Some("pt"));
}

#[test]
fn test_parse_safetensors_invalid_length() {
    let raw = vec![0u8; 8]; // header length 0 is invalid
    let res = parse_safetensors_header(Cursor::new(raw));
    assert!(res.is_err());
}
