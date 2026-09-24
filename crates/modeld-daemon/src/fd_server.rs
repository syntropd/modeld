//! Dedicated Unix domain socket server for SCM_RIGHTS descriptor handoff.
//!
//! Delivers verified, sealed model file descriptors to authorized clients.

use crate::auth::TrustedGroupConfig;
use modeld_core::cas::{CasStore, TagRegistry};
use modeld_core::descriptor::{create_sealed_memfd_from_file, send_fd_scm_rights};
use rustix::net::sockopt::get_socket_peercred;
use serde_json::Value;
use std::os::unix::io::AsFd;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tracing::{debug, info, warn};

/// Runs the SCM_RIGHTS descriptor handoff server over the designated listener.
pub async fn run_fd_handoff_server(
    listener: UnixListener,
    cas: Arc<CasStore>,
    tags: Arc<TagRegistry>,
    trusted_group: TrustedGroupConfig,
    shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let mut shutdown = shutdown;
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("FD handoff listener shutting down on signal");
                    return Ok(());
                }
            }
            accept = listener.accept() => match accept {
                Ok((stream, _addr)) => {
                    let cas_clone = cas.clone();
                    let tags_clone = tags.clone();
                    let policy = trusted_group.clone();
                    let sd = shutdown.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_fd_request(stream, cas_clone, tags_clone, policy, sd).await {
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
    stream: UnixStream,
    cas: Arc<CasStore>,
    tags: Arc<TagRegistry>,
    policy: TrustedGroupConfig,
    shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    // Authorize the peer before reading any request data. SO_PEERCRED is
    // filled in by the kernel on `accept()` and remains valid for the
    // lifetime of the connection; this closes the gap where anyone able
    // to open the socket could ask for any registered model.
    let peer = get_socket_peercred(stream.as_fd())
        .map_err(|e| anyhow::anyhow!("SO_PEERCRED read failed: {}", e))?;
    if !policy.is_authorized(&peer) {
        warn!(
            "FD handoff rejected peer uid={} gid={} (not in trusted group {})",
            peer.uid.as_raw(),
            peer.gid.as_raw(),
            policy.trusted_group
        );
        // Reply with a protocol-level error so the peer sees a clear
        // "permission denied" rather than an ambiguous EPIPE/EOF on
        // read.
        let mut stream = stream;
        let reply = serde_json::json!({
            "error": "io.syntrop.Model1.PermissionDenied",
            "trust_group": policy.trusted_group,
        });
        let _ = stream.write_all(&serde_json::to_vec(&reply)?).await;
        let _ = stream.flush().await;
        return Ok(());
    }

    let mut stream = stream;
    let mut scratch = Vec::with_capacity(4096);
    let mut shutdown = shutdown;
    let request_json = tokio::select! {
        biased;
        _ = shutdown.changed() => {
            if *shutdown.borrow() {
                return Ok(());
            }
            None
        }
        read = read_framed_request(&mut stream, &mut scratch) => Some(match read? {
            Some(v) => v,
            None => return Ok(()),
        })
    };
    let request_json = match request_json {
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

    // Hand the FD across on the blocking Tokio pool. The kernel
    // `sendmsg(SCM_RIGHTS)` call can block while the peer drains; running
    // it inline would stall the Tokio worker for the duration.
    let id_owned = id.to_string();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        // `send_fd_scm_rights` consumes the sealed memfd on success. On
        // failure the FD is dropped and closed here, but we still log the
        // event so an operator can correlate a missing client reply with
        // the model they asked for. Without this log, a broken peer would
        // be invisible: the sealed memfd would have already been built
        // (one full source read pass of cost), the stream consumed, and no
        // journalctl record of the failure.
        if let Err(e) = send_fd_scm_rights(stream.as_fd(), &sealed, &payload_bytes) {
            warn!(
                "SCM_RIGHTS send failed for model {} ({} bytes): {}",
                id_owned, size, e
            );
            return Err(e.into());
        }
        info!(
            "Transferred sealed SCM_RIGHTS descriptor for model {} ({} bytes)",
            id_owned, size
        );
        Ok(())
    })
    .await??;

    Ok(())
}
