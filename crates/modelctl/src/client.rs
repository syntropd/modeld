//! Minimal pure Rust Varlink IPC client over Unix domain socket.
//!
//! Transmits NUL-delimited JSON call envelopes and awaits framed replies.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

/// Synchronous Varlink client for modelctl command execution.
pub struct VarlinkClient {
    stream: UnixStream,
}

impl VarlinkClient {
    /// Connects to a Varlink Unix domain socket path.
    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Self> {
        let stream = UnixStream::connect(path)?;
        Ok(Self { stream })
    }

    /// Invokes a remote Varlink method and parses the reply.
    pub fn call(&mut self, method: &str, parameters: Option<Value>) -> Result<Value> {
        let call_payload = json!({
            "method": method,
            "parameters": parameters
        });

        let mut bytes = serde_json::to_vec(&call_payload)?;
        bytes.push(0x00); // NUL-byte framing
        self.stream.write_all(&bytes)?;
        self.stream.flush()?;

        let mut reply_buf = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            let n = self.stream.read(&mut byte)?;
            if n == 0 {
                return Err(anyhow!("Unexpected EOF reading Varlink reply"));
            }
            if byte[0] == 0x00 {
                break;
            }
            reply_buf.push(byte[0]);
        }

        let reply: Value = serde_json::from_slice(&reply_buf)?;

        if let Some(err) = reply.get("error").and_then(|v| v.as_str()) {
            return Err(anyhow!("Varlink error: {}", err));
        }

        Ok(reply.get("parameters").cloned().unwrap_or(Value::Null))
    }
}
