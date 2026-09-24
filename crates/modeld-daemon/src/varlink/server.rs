//! Varlink Unix listener and message dispatch loop.
//!
//! Accepts client stream connections and executes NUL-delimited protocol exchanges.

use super::model1::{handle_model1_call, ModelServiceContext};
use super::protocol::{VarlinkCall, VarlinkReply};
use super::service::handle_service_call;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::watch;
use tracing::{debug, info, warn};

/// Maximum bytes accepted on a single client connection before it is dropped.
///
/// Caps memory growth so a hostile client that never sends a NUL terminator
/// cannot OOM the daemon. The Varlink protocol in this daemon carries only
/// JSON request envelopes for the methods listed in `docs/VARLINK_SPEC.md`,
/// none of which legitimately approach this size.
pub const MAX_MSG_BYTES: usize = 1024 * 1024;

/// Runs the primary Varlink request processing loop over an adopted or bound listener.
pub async fn run_varlink_server(
    listener: UnixListener,
    ctx: ModelServiceContext,
    shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let mut shutdown = shutdown;
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("Varlink listener shutting down on signal");
                    return Ok(());
                }
            }
            accept = listener.accept() => match accept {
                Ok((stream, _addr)) => {
                    let client_ctx = ctx.clone();
                    let sd = shutdown.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_varlink_client(stream, client_ctx, sd).await {
                            debug!("Client connection closed: {}", e);
                        }
                    });
                }
                Err(e) => {
                    warn!("Varlink listener accept failure: {}", e);
                }
            }
        }
    }
}

/// Reads NUL-delimited JSON requests from `stream` and dispatches each one.
///
/// On entry the buffer is empty. After every successful `read`, the buffer
/// is scanned for complete NUL-terminated messages; each complete message
/// is dispatched and removed from the buffer. If the buffer grows past
/// `MAX_MSG_BYTES` without a NUL, the connection is aborted with a
/// `ProtocolError` reply so the peer learns about the limit rather than
/// seeing a silent close.
///
/// Exposed publicly so the QA test suite can drive the connection loop
/// directly through a `UnixStream::pair` without needing a bound listener.
pub async fn handle_varlink_client(
    mut stream: UnixStream,
    ctx: ModelServiceContext,
    shutdown: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let mut buffer: Vec<u8> = Vec::with_capacity(4096);
    let mut read_chunk = [0u8; 1024];
    let mut shutdown = shutdown;

    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    return Ok(());
                }
            }
            read = stream.read(&mut read_chunk) => {
                let n = match read {
                    Ok(n) => n,
                    Err(e) => return Err(e.into()),
                };
                if n == 0 {
                    break;
                }

                buffer.extend_from_slice(&read_chunk[..n]);

                // Bound the buffer: a hostile client that never sends a NUL
                // must not be able to OOM the daemon.
                if buffer.len() > MAX_MSG_BYTES {
                    warn!(
                        "Varlink client exceeded {} bytes without NUL; closing",
                        MAX_MSG_BYTES
                    );
                    let err_reply = VarlinkReply::err(
                        "org.varlink.service.ProtocolError",
                        Some(json!({
                            "reason": format!("message exceeded {} bytes", MAX_MSG_BYTES)
                        })),
                    );
                    let _ = stream.write_all(&err_reply.to_bytes()).await;
                    let _ = stream.flush().await;
                    return Ok(());
                }

                // Process all complete NUL-delimited messages
                while let Some(nul_pos) = buffer.iter().position(|&b| b == 0x00) {
                    let message_bytes = buffer.drain(..=nul_pos).collect::<Vec<u8>>();
                    let payload = &message_bytes[..message_bytes.len() - 1]; // strip trailing NUL

                    let call: VarlinkCall = match serde_json::from_slice(payload) {
                        Ok(c) => c,
                        Err(e) => {
                            let err_reply = VarlinkReply::err(
                                "org.varlink.service.InvalidParameter",
                                Some(json!({ "reason": e.to_string() })),
                            );
                            stream.write_all(&err_reply.to_bytes()).await?;
                            continue;
                        }
                    };

                    let reply = dispatch_call(&call, &ctx);
                    stream.write_all(&reply.to_bytes()).await?;
                    stream.flush().await?;
                }
            }
        }
    }

    Ok(())
}

fn dispatch_call(call: &VarlinkCall, ctx: &ModelServiceContext) -> VarlinkReply {
    // 1. Try standard org.varlink.service methods
    if let Some(reply) = handle_service_call(&call.method, call.parameters.as_ref()) {
        return reply;
    }

    // 2. Try io.syntrop.Model1 methods
    if let Some(reply) = handle_model1_call(&call.method, call.parameters.as_ref(), ctx) {
        return reply;
    }

    // 3. Fallback: method not found
    VarlinkReply::err(
        "org.varlink.service.MethodNotFound",
        Some(json!({ "method": call.method })),
    )
}
