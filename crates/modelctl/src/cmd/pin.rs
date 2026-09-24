//! Handlers for `modelctl pin` and `modelctl unpin` subcommands.

use crate::client::VarlinkClient;
use anyhow::Result;
use serde_json::json;

/// Pins a model to protect it from automatic storage eviction.
pub fn run_pin(client: &mut VarlinkClient, id: &str) -> Result<()> {
    client.call("io.syntrop.Model1.Pin", Some(json!({ "id": id })))?;
    println!("Pinned model '{}' against storage eviction.", id);
    Ok(())
}

/// Unpins a model to allow standard LRU storage reclamation.
pub fn run_unpin(client: &mut VarlinkClient, id: &str) -> Result<()> {
    client.call("io.syntrop.Model1.Unpin", Some(json!({ "id": id })))?;
    println!("Unpinned model '{}'.", id);
    Ok(())
}
