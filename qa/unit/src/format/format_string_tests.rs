//! 1:1 unit QA tests for format-level UTF-8 enforcement.
//!
//! Pins the contract that `parse_gguf_header` rejects metadata keys whose
//! bytes are not valid UTF-8 instead of silently substituting U+FFFD.

use modeld_core::format::parse_gguf_header;
use std::io::Cursor;

#[test]
fn test_parse_gguf_header_rejects_non_utf8_key() {
    // Build a GGUF v3 header with a single KV whose key contains a stray
    // 0xFF byte. The previous `from_utf8_lossy` implementation turned
    // this into a U+FFFD-replaced string; the strict implementation must
    // surface an error instead.
    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes()); // tensor count
    buf.extend_from_slice(&1u64.to_le_bytes()); // kv count

    // Key: "good\xFF" — 5 bytes, second half invalid UTF-8.
    let key: &[u8] = b"good\xFF";
    buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
    buf.extend_from_slice(key);
    // Value type = u64 (10), value = 1
    buf.extend_from_slice(&10u32.to_le_bytes());
    buf.extend_from_slice(&1u64.to_le_bytes());

    let res = parse_gguf_header(Cursor::new(buf));
    assert!(
        res.is_err(),
        "non-UTF-8 metadata key must be a parse error"
    );
}

#[test]
fn test_parse_gguf_header_accepts_clean_ascii_keys() {
    // Sanity check that valid ASCII keys still parse correctly.
    let mut buf = Vec::new();
    buf.extend_from_slice(b"GGUF");
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend_from_slice(&0u64.to_le_bytes());
    buf.extend_from_slice(&1u64.to_le_bytes());

    let key = b"general.architecture";
    buf.extend_from_slice(&(key.len() as u64).to_le_bytes());
    buf.extend_from_slice(key);
    buf.extend_from_slice(&8u32.to_le_bytes()); // string
    let val = b"llama";
    buf.extend_from_slice(&(val.len() as u64).to_le_bytes());
    buf.extend_from_slice(val);

    let meta = parse_gguf_header(Cursor::new(buf)).expect("valid GGUF must parse");
    assert_eq!(meta.architecture.as_deref(), Some("llama"));
}
