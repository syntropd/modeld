//! Streaming CAS pull and commitment for visual LoRA delta adapters.

use crate::cmd::pull::run_pull;
use anyhow::Result;
use std::path::Path;

/// Pulls a visual LoRA adapter into CAS storage.
pub async fn run_lora_pull<P: AsRef<Path>>(
    storage_root: P,
    socket_path: P,
    lora_spec: &str,
    tag: Option<&str>,
    force: bool,
) -> Result<()> {
    println!("Pulling visual LoRA adapter: {lora_spec}");
    run_pull(
        storage_root.as_ref(),
        socket_path.as_ref(),
        lora_spec,
        "safetensors",
        None,
        tag,
        force,
    )
    .await?;

    println!("LoRA adapter committed to CAS successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lora_pull_signature() {
        let _ = run_lora_pull::<&std::path::Path>;
    }
}
