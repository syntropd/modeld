//! Handler for `modelctl prune` subcommand.

use crate::client::VarlinkClient;
use anyhow::Result;
use serde_json::json;

/// Triggers manual storage reclamation down to `max_bytes`.
pub fn run_prune(client: &mut VarlinkClient, max_bytes: Option<u64>) -> Result<()> {
    let limit = max_bytes.unwrap_or(0);
    let params = client.call("io.syntrop.Model1.Prune", Some(json!({ "max_bytes": limit })))?;

    let reclaimed = params
        .get("reclaimed_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    println!("Storage reclamation complete. Reclaimed {} bytes.", reclaimed);
    Ok(())
}
