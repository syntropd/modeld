//! Cryptographic digest computation using fixed stack buffers.
//!
//! Provides zero-allocation streaming SHA-256 computation over readers.

use crate::error::ModeldError;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Fixed chunk buffer size for I/O efficiency (64 KiB).
const BUFFER_SIZE: usize = 65536;

/// Computes the SHA-256 hexadecimal digest for a given readable stream.
pub fn compute_stream_digest<R: Read>(mut reader: R) -> Result<String, ModeldError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Computes the SHA-256 hexadecimal digest for a filesystem path.
pub fn compute_file_digest<P: AsRef<Path>>(path: P) -> Result<String, ModeldError> {
    let file = File::open(path)?;
    compute_stream_digest(file)
}

/// Verifies whether the stream matches an expected SHA-256 hexadecimal digest.
pub fn verify_stream_digest<R: Read>(reader: R, expected: &str) -> Result<bool, ModeldError> {
    let computed = compute_stream_digest(reader)?;
    if computed.eq_ignore_ascii_case(expected.trim()) {
        Ok(true)
    } else {
        Err(ModeldError::DigestMismatch {
            expected: expected.trim().to_string(),
            computed,
        })
    }
}
