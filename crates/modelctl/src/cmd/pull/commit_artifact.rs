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
    if let Err(e) = fs::rename(commit.stage_path, &cas_dest) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(anyhow::anyhow!(
                "Permission denied committing CAS artifact to {}. \
                Ensure your user belongs to the 'syntrop' group (try: newgrp syntrop) or re-run with sudo: {}",
                cas_dest.display(),
                e
            ));
        }
        return Err(anyhow::anyhow!("Failed to commit CAS artifact: {}", e));
    }

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
    if let Err(e) = fs::create_dir_all(&tag_dir) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(anyhow::anyhow!(
                "Permission denied creating tags directory at {}. \
                Ensure your user belongs to the 'syntrop' group (try: newgrp syntrop) or re-run with sudo: {}",
                tag_dir.display(),
                e
            ));
        }
        return Err(anyhow::anyhow!("Failed to create tags directory: {}", e));
    }
    let tag_file = tag_dir.join(commit.tag);
    if let Err(e) = fs::write(&tag_file, commit.digest.as_bytes()) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(anyhow::anyhow!(
                "Permission denied writing tag file at {}. \
                Ensure your user belongs to the 'syntrop' group (try: newgrp syntrop) or re-run with sudo: {}",
                tag_file.display(),
                e
            ));
        }
        return Err(anyhow::anyhow!("Failed to write tag file: {}", e));
    }

    // 4. Create format-specific symlinks: /var/lib/models/<format>/<name>.<ext>
    // Also create tagged variants: <name>:<tag>.<ext> and <name>-<tag>.<ext>
    let fmt_dir = root.join(ext);
    if let Err(e) = fs::create_dir_all(&fmt_dir) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(anyhow::anyhow!(
                "Permission denied creating {} directory at {}. \
                Ensure your user belongs to the 'syntrop' group (try: newgrp syntrop) or re-run with sudo: {}",
                ext,
                fmt_dir.display(),
                e
            ));
        }
        return Err(anyhow::anyhow!("Failed to create {} directory: {}", ext, e));
    }
    let symlink_dest = fmt_dir.join(format!("{}.{}", commit.name, ext));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link_target = fs::canonicalize(&cas_dest).unwrap_or_else(|_| cas_dest.clone());
        if commit.tag == "latest" || !symlink_dest.exists() {
            let _ = fs::remove_file(&symlink_dest);
            if let Err(e) = symlink(&link_target, &symlink_dest) {
                warn!("Failed to create symlink at {:?}: {}", symlink_dest, e);
            }
        }
        if commit.tag != "latest" {
            let colon_dest = fmt_dir.join(format!("{}:{}.{}", commit.name, commit.tag, ext));
            let _ = fs::remove_file(&colon_dest);
            let _ = symlink(&link_target, &colon_dest);

            let dash_dest = fmt_dir.join(format!("{}-{}.{}", commit.name, commit.tag, ext));
            let _ = fs::remove_file(&dash_dest);
            let _ = symlink(&link_target, &dash_dest);
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
