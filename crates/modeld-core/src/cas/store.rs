//! Content-Addressable Storage (CAS) layout and blob persistence.
//!
//! Stores immutable model blobs keyed by SHA-256 with atomic tempfile commit.

use crate::error::ModeldError;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Primary manager for Content-Addressable Storage blobs on host filesystem.
#[derive(Debug, Clone)]
pub struct CasStore {
    root_dir: PathBuf,
}

impl CasStore {
    /// Initializes a new CAS store with the designated root path.
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self, ModeldError> {
        let store = Self {
            root_dir: root.as_ref().to_path_buf(),
        };
        fs::create_dir_all(store.blobs_dir())?;
        fs::create_dir_all(store.incoming_dir())?;
        Ok(store)
    }

    /// Returns the directory path where finalized blobs reside.
    pub fn blobs_dir(&self) -> PathBuf {
        self.root_dir.join("cas").join("blobs").join("sha256")
    }

    /// Returns the directory path for staging incoming writes.
    pub fn incoming_dir(&self) -> PathBuf {
        self.root_dir.join("cas").join("incoming")
    }

    /// Returns the absolute filesystem path for a specific SHA-256 digest.
    pub fn blob_path(&self, digest: &str) -> PathBuf {
        self.blobs_dir().join(digest)
    }

    /// Checks if a blob with the specified digest already exists.
    pub fn has_blob(&self, digest: &str) -> bool {
        self.blob_path(digest).is_file()
    }

    /// Opens an existing blob for read-only access.
    pub fn open_blob(&self, digest: &str) -> Result<File, ModeldError> {
        let path = self.blob_path(digest);
        if !path.is_file() {
            return Err(ModeldError::NotFound(digest.to_string()));
        }
        Ok(File::open(path)?)
    }

    /// Returns the size in bytes of an existing blob.
    pub fn blob_size(&self, digest: &str) -> Result<u64, ModeldError> {
        let path = self.blob_path(digest);
        let meta = fs::metadata(&path)
            .map_err(|_| ModeldError::NotFound(digest.to_string()))?;
        Ok(meta.len())
    }

    /// Stores data from a reader into the CAS store atomically.
    ///
    /// Writes to a temporary staging file while streaming SHA-256, verifies
    /// the computed digest against the optional expected digest, and renames
    /// the staged file into the final content-addressed location. The hash
    /// is computed inline so a multi-gigabyte import does not require a
    /// second full pass over the staged file.
    pub fn store_blob<R: Read>(
        &self,
        mut reader: R,
        expected_digest: Option<&str>,
    ) -> Result<(String, u64), ModeldError> {
        let temp_path = self.incoming_dir().join(format!("stage-{}", stage_nonce()));
        let mut temp_file = File::create(&temp_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut total_bytes: u64 = 0;

        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            temp_file.write_all(&buffer[..n])?;
            total_bytes += n as u64;
        }
        temp_file.flush()?;
        drop(temp_file);

        let computed_digest = format!("{:x}", hasher.finalize());

        // Validate against optional expected digest
        if let Some(expected) = expected_digest {
            if !computed_digest.eq_ignore_ascii_case(expected.trim()) {
                let _ = fs::remove_file(&temp_path);
                return Err(ModeldError::DigestMismatch {
                    expected: expected.trim().to_string(),
                    computed: computed_digest,
                });
            }
        }

        let target_path = self.blob_path(&computed_digest);
        if target_path.exists() {
            // Deduplication: blob already present, clean up temporary file
            let _ = fs::remove_file(&temp_path);
        } else {
            // Atomic commit via rename
            fs::rename(&temp_path, &target_path)?;
        }

        Ok((computed_digest, total_bytes))
    }
}

fn stage_nonce() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
