//! Dedicated Unix domain socket server for SCM_RIGHTS descriptor handoff.
//!
//! Delivers verified, sealed model file descriptors to authorized clients.

use modeld_core::cas::{CasStore, TagRegistry};
use modeld_core::descriptor::{create_sealed_memfd_from_file, send_fd_scm_rights};
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

/// Reads NUL-framed JSON requests from `stream` and dispatches each one.
///
/// The previous implementation did a single `read(&mut [u8; 1024])` and
/// dropped any bytes past the first JSON value. Clients sending multiple
/// requests on the same connection — or oversized requests — silently
/// lost data.
async fn read_framed_request(
    stream: &mut UnixStream,
    scratch: &mut Vec<u8>,
) -> anyhow::Result<Option<Value>> {
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(None);
        }
        scratch.extend_from_slice(&chunk[..n]);
        if let Some(nul_pos) = scratch.iter().position(|&b| b == 0x00) {
            let payload: Vec<u8> = scratch.drain(..=nul_pos).collect();
            let payload = &payload[..payload.len() - 1];
            let v: Value = serde_json::from_slice(payload)?;
            return Ok(Some(v));
        }
        // Bound memory growth so a hostile client cannot OOM the daemon.
        if scratch.len() > 64 * 1024 {
            return Err(anyhow::anyhow!(
                "FD handoff request exceeded 64 KiB without NUL terminator"
            ));
        }
    }
}

async fn handle_fd_request(
    mut stream: UnixStream,
    cas: Arc<CasStore>,
    tags: Arc<TagRegistry>,
) -> anyhow::Result<()> {
    let mut scratch = Vec::with_capacity(4096);
    let request_json = match read_framed_request(&mut stream, &mut scratch).await? {
        Some(v) => v,
        None => return Ok(()),
    };

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

    let size = cas.blob_size(&digest)?;

    // Stream the CAS blob into a sealed memfd and hand that to the consumer
    // via SCM_RIGHTS. The seal blocks write/shrink/grow/further-seal so
    // the recipient cannot mutate the view, matching ARCHITECTURE.md §2.3.
    let mut blob_file = cas.open_blob(&digest)?;
    let memfd_name = format!("modeld-{}", &digest[..16]);
    let sealed = create_sealed_memfd_from_file(&memfd_name, &mut blob_file)?;

    let payload = serde_json::json!({
        "status": "ok",
        "id": id,
        "digest": digest,
        "size_bytes": size,
        "sealed": true,
    });
    let payload_bytes = serde_json::to_vec(&payload)?;

    // Send FD using standard blocking Unix stream method via std converter.
    let std_stream = stream.into_std()?;
    std_stream.set_nonblocking(false)?;

    // `send_fd_scm_rights` consumes the sealed memfd on success. On
    // failure the FD is dropped and closed here, but we still log the
    // event so an operator can correlate a missing client reply with
    // the model they asked for. Without this log, a broken peer would
    // be invisible: the sealed memfd would have already been built
    // (one full source read pass of cost), the stream consumed, and no
    // journalctl record of the failure.
    if let Err(e) = send_fd_scm_rights(&std_stream, &sealed, &payload_bytes) {
        warn!(
            "SCM_RIGHTS send failed for model {} ({} bytes): {}",
            id, size, e
        );
        return Err(e.into());
    }
    info!(
        "Transferred sealed SCM_RIGHTS descriptor for model {} ({} bytes)",
        id, size
    );

    Ok(())
}
