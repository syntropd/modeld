//! Streaming CAS pull and commitment for visual diffusion models.

use crate::cmd::pull::run_pull;
use anyhow::Result;
use std::path::Path;

/// Pulls a visual diffusion model (in SafeTensors format) into CAS storage.
pub async fn run_visual_pull<P: AsRef<Path>>(
    storage_root: P,
    socket_path: P,
    model_spec: &str,
    tag: Option<&str>,
    force: bool,
) -> Result<()> {
    println!("Pulling visual diffusion model: {model_spec}");
    run_pull(
        storage_root.as_ref(),
        socket_path.as_ref(),
        model_spec,
        "safetensors",
        None,
        tag,
        force,
    )
    .await?;

    println!("Visual model committed to CAS successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_pull_signature() {
        let _ = run_visual_pull::<&std::path::Path>;
    }
}
