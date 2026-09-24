//! Model pinning and Least-Recently-Used (LRU) storage reclamation.
//!
//! Enforces storage quotas while strictly protecting pinned system models.

use crate::cas::digest_format::validate_digest_format;
use crate::cas::store::CasStore;
use crate::error::ModeldError;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Manages pinned model status and quota eviction.
#[derive(Debug, Clone)]
pub struct EvictionManager {
    pinned_dir: PathBuf,
}

impl EvictionManager {
    /// Initializes the eviction manager under the given root path.
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self, ModeldError> {
        let pinned_dir = root.as_ref().join("pinned");
        fs::create_dir_all(&pinned_dir)?;
        Ok(Self { pinned_dir })
    }

    /// Marks a model digest as permanently pinned against LRU eviction.
    pub fn pin(&self, digest: &str) -> Result<(), ModeldError> {
        validate_digest_format(digest.trim())?;
        let pin_file = self.pinned_dir.join(digest.trim());
        File::create(pin_file)?;
        Ok(())
    }

    /// Removes the pinned protection from a model digest.
    pub fn unpin(&self, digest: &str) -> Result<bool, ModeldError> {
        validate_digest_format(digest.trim())?;
        let pin_file = self.pinned_dir.join(digest.trim());
        if pin_file.is_file() {
            fs::remove_file(pin_file)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Checks if a model digest is currently pinned.
    pub fn is_pinned(&self, digest: &str) -> bool {
        // Validate first so a crafted digest like `../../etc/passwd` can
        // never be reported as pinned.
        if validate_digest_format(digest.trim()).is_err() {
            return false;
        }
        self.pinned_dir.join(digest.trim()).is_file()
    }

    /// Lists all currently pinned model digests.
    pub fn list_pinned(&self) -> Result<Vec<String>, ModeldError> {
        let mut pinned = Vec::new();
        if !self.pinned_dir.is_dir() {
            return Ok(pinned);
        }
        for entry in fs::read_dir(&self.pinned_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                pinned.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        pinned.sort();
        Ok(pinned)
    }

    /// Prunes unpinned blobs until the total CAS storage falls below `max_bytes`.
    ///
    /// Returns the total number of bytes reclaimed.
    pub fn prune(&self, cas: &CasStore, max_bytes: u64) -> Result<u64, ModeldError> {
        let blobs_dir = cas.blobs_dir();
        if !blobs_dir.is_dir() {
            return Ok(0);
        }

        let mut blobs: Vec<(String, u64, SystemTime)> = Vec::new();
        let mut current_total: u64 = 0;

        for entry in fs::read_dir(&blobs_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let digest = entry.file_name().to_string_lossy().to_string();
                let meta = entry.metadata()?;
                let size = meta.len();
                let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                current_total += size;
                blobs.push((digest, size, modified));
            }
        }

        if current_total <= max_bytes {
            return Ok(0);
        }

        // Sort unpinned blobs by modification time ascending (oldest first)
        blobs.sort_by_key(|b| b.2);

        let mut reclaimed_bytes: u64 = 0;
        for (digest, size, _) in blobs {
            if current_total <= max_bytes {
                break;
            }
            if self.is_pinned(&digest) {
                continue;
            }

            let path = cas.blob_path(&digest);
            if fs::remove_file(path).is_ok() {
                current_total = current_total.saturating_sub(size);
                reclaimed_bytes += size;
            }
        }

        Ok(reclaimed_bytes)
    }
}
