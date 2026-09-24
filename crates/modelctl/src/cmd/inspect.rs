//! Handler for `modelctl inspect` subcommand.

use crate::client::VarlinkClient;
use anyhow::Result;
use serde_json::json;

/// Displays detailed metadata and configuration for a specific model.
pub fn run_inspect(client: &mut VarlinkClient, id: &str, json_output: bool) -> Result<()> {
    let params = client.call("io.syntrop.Model1.Inspect", Some(json!({ "id": id })))?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&params)?);
        return Ok(());
    }

    if let Some(info) = params.get("info") {
        let digest = info.get("digest").and_then(|v| v.as_str()).unwrap_or("-");
        let size = info.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        let pinned = info.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);
        let format = info.get("format").and_then(|v| v.as_str()).unwrap_or("-");

        println!("Model: {}", id);
        println!("  Digest:   {}", digest);
        println!("  Size:     {} bytes", size);
        println!("  Format:   {}", format);
        println!("  Pinned:   {}", pinned);
    }

    Ok(())
}
