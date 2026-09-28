//! 1:1 unit QA tests for cas::digest functions.

use modeld_core::cas::digest::{compute_file_digest, compute_stream_digest, verify_stream_digest};
use std::io::Cursor;
use tempfile::NamedTempFile;
use std::io::Write;

#[test]
fn test_compute_stream_digest_empty() {
    let data = b"";
    let digest = compute_stream_digest(Cursor::new(data)).expect("Digest computation failed");
    // SHA-256 of empty string
    assert_eq!(digest, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn test_compute_stream_digest_known_vector() {
    let data = b"hello syntropd modeld";
    let digest = compute_stream_digest(Cursor::new(data)).expect("Digest computation failed");
    assert_eq!(digest.len(), 64);
}

#[test]
fn test_compute_file_digest_success() {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(b"test file payload for digest verification").unwrap();
    file.flush().unwrap();

    let digest = compute_file_digest(file.path()).expect("File digest failed");
    assert_eq!(digest.len(), 64);
}

#[test]
fn test_verify_stream_digest_match() {
    let data = b"verification test";
    let digest = compute_stream_digest(Cursor::new(data)).unwrap();
    let res = verify_stream_digest(Cursor::new(data), &digest);
    assert!(res.is_ok());
    assert!(res.unwrap());
}

#[test]
fn test_verify_stream_digest_mismatch() {
    let data = b"actual data";
    let wrong = "0000000000000000000000000000000000000000000000000000000000000000";
    let res = verify_stream_digest(Cursor::new(data), wrong);
    assert!(res.is_err());
}
