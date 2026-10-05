//! Streaming model downloader with in-flight SHA-256 calculation and CAS commitment.

use super::commit_artifact::{commit_artifact, commit_tokenizer, ArtifactCommit};
use super::progress::create_download_progress;
use super::resolve::resolve_model_with_format;
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
    format: &str,
    quant: Option<&str>,
    tag_override: Option<&str>,
    force: bool,
) -> Result<()> {
    let client = Client::builder()
        .user_agent("syntrop-modelctl")
        .build()
        .context("Failed to build HTTP client")?;

    println!("Resolving model: {} (format: {})", model_spec, format);
    let resolved =
        resolve_model_with_format(&client, model_spec, format, quant, tag_override).await?;
    println!("  Target: {}:{}", resolved.name, resolved.tag);
    println!("  Source: {}", resolved.download_url);

    let root = storage_root.as_ref();
    let cas_dir = root.join("cas");
    let tag_file = root.join("tags").join(&resolved.name).join(&resolved.tag);

    if tag_file.is_file() && !force {
        let existing_digest = fs::read_to_string(&tag_file).unwrap_or_default();
        let digest_trim = existing_digest.trim();
        let ext = if resolved.format.eq_ignore_ascii_case("safetensors") {
            "safetensors"
        } else {
            "gguf"
        };
        let flat_dest = cas_dir.join(format!("sha256-{}.{}", digest_trim, ext));
        let blob_dest = cas_dir.join("blobs").join("sha256").join(digest_trim);
        if flat_dest.is_file() || blob_dest.is_file() {
            println!(
                "Model {}:{} is already present (digest: {}). Use --force to re-download.",
                resolved.name, resolved.tag, digest_trim
            );
            return Ok(());
        }
    }

    let req = client.get(&resolved.download_url);
    let req = super::auth::apply_hf_auth(req, &resolved.download_url);
    let mut resp = req
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
    if let Err(e) = fs::create_dir_all(&incoming_dir) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(anyhow!(
                "Permission denied creating incoming directory at {}. \
                Ensure your user belongs to the 'syntrop' group or re-run with sudo: {}",
                incoming_dir.display(),
                e
            ));
        }
        return Err(anyhow!(
            "Failed to create incoming directory at {}: {}",
            incoming_dir.display(),
            e
        ));
    }

    use std::sync::atomic::{AtomicU64, Ordering};
    static DOWNLOAD_COUNTER: AtomicU64 = AtomicU64::new(1);
    let seq = DOWNLOAD_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let safe_name = resolved.name.replace('/', "-");
    let safe_tag = resolved.tag.replace(':', "-");
    let stage_filename = format!(
        "pull-{}-{}-{}-{}-{}",
        std::process::id(),
        safe_name,
        safe_tag,
        seq,
        nanos
    );
    let stage_path = incoming_dir.join(stage_filename);
    let mut stage_file = match File::create(&stage_path) {
        Ok(f) => f,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return Err(anyhow!(
                    "Permission denied creating staging file at {}. \
                    Ensure your user belongs to the 'syntrop' group or re-run with sudo: {}",
                    stage_path.display(),
                    e
                ));
            }
            return Err(anyhow!(
                "Failed to create staging file at {}: {}",
                stage_path.display(),
                e
            ));
        }
    };

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

    let commit = ArtifactCommit {
        storage_root: root,
        stage_path: &stage_path,
        digest: &digest,
        format: &resolved.format,
        name: &resolved.name,
        tag: &resolved.tag,
    };
    guard.active = false;
    commit_artifact(&commit)?;

    if let Some(tok_url) = &resolved.tokenizer_url {
        println!("  Downloading companion tokenizer.json...");
        let tok_req = client.get(tok_url);
        let tok_req = super::auth::apply_hf_auth(tok_req, tok_url);
        if let Ok(tok_resp) = tok_req.send().await {
            if tok_resp.status().is_success() {
                if let Ok(bytes) = tok_resp.bytes().await {
                    let _ = commit_tokenizer(root, &resolved.name, &bytes);
                    println!("  Companion tokenizer.json saved");
                }
            }
        }
    }

    register_with_modeld(socket_path.as_ref(), &resolved.name, &resolved.tag, &digest);

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
