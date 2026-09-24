//! Varlink Unix listener and message dispatch loop.
//!
//! Accepts client stream connections and executes NUL-delimited protocol exchanges.

use super::model1::{handle_model1_call, ModelServiceContext};
use super::protocol::{VarlinkCall, VarlinkReply};
use super::service::handle_service_call;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, warn};

/// Runs the primary Varlink request processing loop over an adopted or bound listener.
pub async fn run_varlink_server(
    listener: UnixListener,
    ctx: ModelServiceContext,
) -> anyhow::Result<()> {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let client_ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_varlink_client(stream, client_ctx).await {
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

async fn handle_varlink_client(
    mut stream: UnixStream,
    ctx: ModelServiceContext,
) -> anyhow::Result<()> {
    let mut buffer = Vec::with_capacity(4096);
    let mut read_chunk = [0u8; 1024];

    loop {
        let n = stream.read(&mut read_chunk).await?;
        if n == 0 {
            break;
        }

        buffer.extend_from_slice(&read_chunk[..n]);

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
