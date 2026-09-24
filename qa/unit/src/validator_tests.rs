//! 1:1 unit QA tests for format::validator.

use modeld_core::format::validator::{detect_safe_format, validate_file_safety, SafeFormat};
use std::io::Cursor;
use std::path::Path;

#[test]
fn test_validate_file_safety_allows_safe_extensions() {
    assert!(validate_file_safety(Path::new("model.gguf")).is_ok());
    assert!(validate_file_safety(Path::new("weights.safetensors")).is_ok());
    assert!(validate_file_safety(Path::new("classifier.onnx")).is_ok());
}

#[test]
fn test_validate_file_safety_rejects_pickle_extensions() {
    assert!(validate_file_safety(Path::new("pytorch_model.bin")).is_err());
    assert!(validate_file_safety(Path::new("weights.pt")).is_err());
    assert!(validate_file_safety(Path::new("payload.pkl")).is_err());
}

#[test]
fn test_detect_safe_format_gguf_magic() {
    let mut header = vec![b'G', b'G', b'U', b'F'];
    header.extend_from_slice(&[2, 0, 0, 0]); // version 2
    let format = detect_safe_format(Cursor::new(header)).unwrap();
    assert_eq!(format, SafeFormat::Gguf);
}

#[test]
fn test_detect_safe_format_rejects_pickle_magic() {
    let header = vec![0x80, 0x04, 0x95, 0x00];
    let res = detect_safe_format(Cursor::new(header));
    assert!(res.is_err());
}
