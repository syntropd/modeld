//! Handler for `modelctl prune` subcommand.

use crate::client::VarlinkClient;
use anyhow::{bail, Result};
use serde_json::json;

/// Triggers manual storage reclamation down to `max_bytes`.
///
/// `max_bytes` must be > 0. The CLI requires `--max-bytes`; this check is
/// also enforced at the daemon boundary as defense in depth.
pub fn run_prune(client: &mut VarlinkClient, max_bytes: u64) -> Result<()> {
    if max_bytes == 0 {
        bail!("--max-bytes must be greater than zero to prevent deletion of all unpinned models");
    }

    let params = client.call(
        "io.syntrop.Model1.Prune",
        Some(json!({ "max_bytes": max_bytes })),
    )?;

    let reclaimed = params
        .get("reclaimed_bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    println!(
        "Storage reclamation complete. Reclaimed {} bytes.",
        reclaimed
    );
    Ok(())
}
