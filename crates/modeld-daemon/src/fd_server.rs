//! Dedicated Unix domain socket server for SCM_RIGHTS descriptor handoff.
//!
//! Delivers verified, read-only model file descriptors to authorized clients.

use modeld_core::cas::{CasStore, TagRegistry};
use modeld_core::descriptor::send_fd_scm_rights;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, info, warn};

/// Runs the SCM_RIGHTS descriptor handoff server over the designated listener.
pub async fn run_fd_handoff_server(
    listener: UnixListener,
    cas: Arc<CasStore>,
    tags: Arc<TagRegistry>,
) -> anyhow::Result<()> {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let cas_clone = cas.clone();
                let tags_clone = tags.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_fd_request(stream, cas_clone, tags_clone).await {
                        debug!("FD handoff client error: {}", e);
                    }
                });
            }
            Err(e) => {
                warn!("FD handoff accept failure: {}", e);
            }
        }
    }
}

async fn handle_fd_request(
    mut stream: UnixStream,
    cas: Arc<CasStore>,
    tags: Arc<TagRegistry>,
) -> anyhow::Result<()> {
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }

    let request_json: Value = serde_json::from_slice(&buf[..n])
        .unwrap_or_else(|_| serde_json::json!({ "id": String::from_utf8_lossy(&buf[..n]).trim() }));

    let id = request_json
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let digest = match tags.resolve(id)? {
        Some(d) => d,
        None => {
            let err_reply = serde_json::json!({ "error": "Model not found", "id": id });
            stream.write_all(&serde_json::to_vec(&err_reply)?).await?;
            return Ok(());
        }
    };

    let file = cas.open_blob(&digest)?;
    let size = cas.blob_size(&digest)?;

    let payload = serde_json::json!({
        "status": "ok",
        "id": id,
        "digest": digest,
        "size_bytes": size
    });

    let payload_bytes = serde_json::to_vec(&payload)?;

    // Send FD using standard blocking Unix stream method via std converter
    let std_stream = stream.into_std()?;
    std_stream.set_nonblocking(false)?;

    send_fd_scm_rights(&std_stream, &file, &payload_bytes)?;
    info!("Transferred SCM_RIGHTS descriptor for model {} ({} bytes)", id, size);

    Ok(())
}
