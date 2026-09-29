//! Streaming model downloader with in-flight SHA-256 calculation and CAS commitment.

use super::progress::create_download_progress;
use super::resolve::resolve_model;
use crate::client::VarlinkClient;
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tracing::{info, warn};

const ROUTER_SOCKET: &str = "/run/syntrop/io.syntrop.Router1";

/// Executes the full pull workflow: resolution, streaming download, CAS commit, and daemon notifications.
pub async fn run_pull<P: AsRef<Path>>(
    storage_root: P,
    socket_path: P,
    model_spec: &str,
    quant: Option<&str>,
    tag_override: Option<&str>,
    force: bool,
) -> Result<()> {
    let client = Client::builder()
        .user_agent("syntrop-modelctl")
        .build()
        .context("Failed to build HTTP client")?;

    println!("Resolving model: {}", model_spec);
    let resolved = resolve_model(&client, model_spec, quant, tag_override).await?;
    println!("  Target: {}:{}", resolved.name, resolved.tag);
    println!("  Source: {}", resolved.download_url);

    let root = storage_root.as_ref();
    let cas_dir = root.join("cas");
    let tag_file = root.join("tags").join(&resolved.name).join(&resolved.tag);

    if tag_file.is_file() && !force {
        let existing_digest = fs::read_to_string(&tag_file).unwrap_or_default();
        let digest_trim = existing_digest.trim();
        let flat_dest = cas_dir.join(format!("sha256-{}.gguf", digest_trim));
        let blob_dest = cas_dir.join("blobs").join("sha256").join(digest_trim);
        if flat_dest.is_file() || blob_dest.is_file() {
            println!(
                "Model {}:{} is already present (digest: {}). Use --force to re-download.",
                resolved.name, resolved.tag, digest_trim
            );
            return Ok(());
        }
    }

    let mut resp = client
        .get(&resolved.download_url)
        .send()
        .await
        .with_context(|| format!("Failed to connect to {}", resolved.download_url))?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Download failed with HTTP status {}",
            resp.status()
        ));
    }

    let total_bytes = resp.content_length();
    let pb = create_download_progress(total_bytes, &resolved.name);

    let incoming_dir = cas_dir.join("incoming");
    fs::create_dir_all(&incoming_dir).context("Failed to create incoming directory")?;

    let stage_filename = format!("pull-{}-{}", std::process::id(), resolved.name);
    let stage_path = incoming_dir.join(stage_filename);
    let mut stage_file = File::create(&stage_path).context("Failed to create staging file")?;

    struct StagingGuard<'a> {
        path: &'a Path,
        active: bool,
    }
    impl<'a> Drop for StagingGuard<'a> {
        fn drop(&mut self) {
            if self.active {
                let _ = fs::remove_file(self.path);
            }
        }
    }
    let mut guard = StagingGuard {
        path: &stage_path,
        active: true,
    };

    let mut hasher = Sha256::new();
    let mut downloaded_bytes: u64 = 0;

    while let Some(chunk) = resp
        .chunk()
        .await
        .context("Network error during streaming download")?
    {
        hasher.update(&chunk);
        if let Err(e) = stage_file.write_all(&chunk) {
            pb.abandon();
            return Err(anyhow!("Disk write failure: {}", e));
        }
        downloaded_bytes += chunk.len() as u64;
        pb.set_position(downloaded_bytes);
    }

    stage_file.flush().context("Disk flush failure")?;
    drop(stage_file);
    pb.finish_with_message("Download complete");

    let digest = format!("{:x}", hasher.finalize());
    println!("  SHA-256: {}", digest);

    // Commit to CAS: /var/lib/models/cas/sha256-<digest>.gguf
    let cas_filename = format!("sha256-{}.gguf", digest);
    let cas_dest = cas_dir.join(&cas_filename);
    guard.active = false;
    fs::rename(&stage_path, &cas_dest).context("Failed to commit CAS artifact")?;

    // Maintain standard CAS blob path compatibility: cas/blobs/sha256/<digest>
    let blob_dir = cas_dir.join("blobs").join("sha256");
    if fs::create_dir_all(&blob_dir).is_ok() {
        let blob_path = blob_dir.join(&digest);
        let _ = fs::remove_file(&blob_path);
        if fs::hard_link(&cas_dest, &blob_path).is_err() {
            let _ = fs::copy(&cas_dest, &blob_path);
        }
    }

    // Write plain text tag: /var/lib/models/tags/<name>/<tag>
    let tag_dir = root.join("tags").join(&resolved.name);
    fs::create_dir_all(&tag_dir).context("Failed to create tags directory")?;
    fs::write(&tag_file, digest.as_bytes()).context("Failed to write tag file")?;

    // Create symlink: /var/lib/models/gguf/<name>.gguf
    let gguf_dir = root.join("gguf");
    fs::create_dir_all(&gguf_dir).context("Failed to create gguf directory")?;
    let symlink_dest = gguf_dir.join(format!("{}.gguf", resolved.name));
    let _ = fs::remove_file(&symlink_dest);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let link_target = fs::canonicalize(&cas_dest).unwrap_or_else(|_| cas_dest.clone());
        if let Err(e) = symlink(&link_target, &symlink_dest) {
            warn!("Failed to create symlink at {:?}: {}", symlink_dest, e);
        }
    }

    // Register with modeld via Varlink if daemon is listening
    register_with_modeld(socket_path.as_ref(), &resolved.name, &resolved.tag, &digest);

    // Notify routerd of new model via Reload Varlink method
    let router_socket =
        std::env::var("SYNTROP_ROUTER_SOCKET").unwrap_or_else(|_| ROUTER_SOCKET.to_string());
    notify_router_reload(Path::new(&router_socket));

    println!(
        "Successfully pulled and registered {}:{}",
        resolved.name, resolved.tag
    );
    Ok(())
}

fn register_with_modeld(socket_path: &Path, name: &str, tag: &str, digest: &str) {
    if !socket_path.exists() {
        return;
    }
    match VarlinkClient::connect(socket_path) {
        Ok(mut client) => {
            let params = json!({ "id": name, "tag": tag, "digest": digest });
            if let Err(e) = client.call("io.syntrop.Model1.Register", Some(params)) {
                warn!("modeld Register IPC notification failed: {}", e);
            } else {
                info!("Registered {}:{} with modeld", name, tag);
            }
        }
        Err(e) => {
            warn!("Could not connect to modeld at {:?}: {}", socket_path, e);
        }
    }
}

fn notify_router_reload(socket_path: &Path) {
    if !socket_path.exists() {
        return;
    }
    match VarlinkClient::connect(socket_path) {
        Ok(mut client) => {
            if let Err(e) = client.call("io.syntrop.Router1.Reload", None) {
                warn!("routerd Reload IPC notification failed: {}", e);
            } else {
                info!("Notified routerd of model reload");
            }
        }
        Err(e) => {
            warn!("Could not connect to routerd at {:?}: {}", socket_path, e);
        }
    }
}
