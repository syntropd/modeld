//! Content-Addressable Storage commitment and filesystem linking for pulled models.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use tracing::warn;

pub struct ArtifactCommit<'a> {
    pub storage_root: &'a Path,
    pub stage_path: &'a Path,
    pub digest: &'a str,
    pub format: &'a str,
    pub name: &'a str,
    pub tag: &'a str,
}

/// Commits a staged model file to CAS, creates the tag, and symlinks under /var/lib/models/<format>/
pub fn commit_artifact(commit: &ArtifactCommit) -> Result<()> {
    let root = commit.storage_root;
    let cas_dir = root.join("cas");
    let ext = if commit.format.eq_ignore_ascii_case("safetensors") {
        "safetensors"
    } else {
        "gguf"
    };

    // 1. Commit to CAS: /var/lib/models/cas/sha256-<digest>.<ext>
    let cas_filename = format!("sha256-{}.{}", commit.digest, ext);
    let cas_dest = cas_dir.join(&cas_filename);
    fs::rename(commit.stage_path, &cas_dest).context("Failed to commit CAS artifact")?;

    // 2. Maintain standard CAS blob path compatibility: cas/blobs/sha256/<digest>
    let blob_dir = cas_dir.join("blobs").join("sha256");
    if fs::create_dir_all(&blob_dir).is_ok() {
        let blob_path = blob_dir.join(commit.digest);
        let _ = fs::remove_file(&blob_path);
        if fs::hard_link(&cas_dest, &blob_path).is_err() {
            let _ = fs::copy(&cas_dest, &blob_path);
        }
    }

    // 3. Write plain text tag: /var/lib/models/tags/<name>/<tag>
    let tag_dir = root.join("tags").join(commit.name);
    fs::create_dir_all(&tag_dir).context("Failed to create tags directory")?;
    let tag_file = tag_dir.join(commit.tag);
    fs::write(&tag_file, commit.digest.as_bytes()).context("Failed to write tag file")?;

    // 4. Create format-specific symlink: /var/lib/models/<format>/<name>.<ext>
    let fmt_dir = root.join(ext);
    fs::create_dir_all(&fmt_dir).context(format!("Failed to create {} directory", ext))?;
    let symlink_dest = fmt_dir.join(format!("{}.{}", commit.name, ext));
    let _ = fs::remove_file(&symlink_dest);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link_target = fs::canonicalize(&cas_dest).unwrap_or_else(|_| cas_dest.clone());
        if let Err(e) = symlink(&link_target, &symlink_dest) {
            warn!("Failed to create symlink at {:?}: {}", symlink_dest, e);
        }
    }

    Ok(())
}

/// Stores companion tokenizer.json next to Safetensors model.
pub fn commit_tokenizer(storage_root: &Path, name: &str, data: &[u8]) -> Result<()> {
    let st_dir = storage_root.join("safetensors");
    fs::create_dir_all(&st_dir).context("Failed to create safetensors directory")?;
    let tok_dest = st_dir.join(format!("{}.tokenizer.json", name));
    fs::write(&tok_dest, data).context("Failed to write companion tokenizer.json")?;
    Ok(())
}
