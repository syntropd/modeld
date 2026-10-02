//! CLI argument structures and parser definitions for modelctl.

use clap::{Parser, Subcommand};
use modeld_core::config::{DEFAULT_SOCKET_PATH, DEFAULT_STORAGE_PATH};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "modelctl", version, about = "Control modeld CAS storage")]
pub struct Cli {
    /// Varlink Unix domain socket path.
    #[arg(long, global = true, default_value = DEFAULT_SOCKET_PATH)]
    pub socket: PathBuf,

    /// CAS storage root path.
    #[arg(long, global = true, default_value = DEFAULT_STORAGE_PATH)]
    pub storage_path: PathBuf,

    /// Render machine-readable JSON output.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// List all registered models in CAS storage.
    #[command(alias = "ls")]
    List,

    /// Inspect detailed metadata for a model.
    Inspect {
        /// Model name or SHA-256 digest.
        id: String,
    },

    /// Pin a model to prevent LRU storage eviction.
    Pin {
        /// Model name or SHA-256 digest.
        id: String,
    },

    /// Unpin a model to allow standard storage reclamation.
    Unpin {
        /// Model name or SHA-256 digest.
        id: String,
    },

    /// Prune unpinned models to reclaim storage capacity.
    Prune {
        /// Target maximum storage quota in bytes.
        #[arg(long)]
        max_bytes: u64,
    },

    /// Import a local file directly into the CAS store.
    Import {
        /// Source file path.
        source: PathBuf,

        /// Optional tag in `name:variant` format.
        #[arg(long)]
        tag: Option<String>,
    },

    /// Pull a model from Hugging Face or registry.
    Pull {
        /// Model alias or Hugging Face repository.
        model: String,

        /// Target model format: gguf or safetensors.
        #[arg(long, default_value = "gguf")]
        format: String,

        /// Quantization filter (e.g. "Q4_K_M").
        #[arg(long)]
        quant: Option<String>,

        /// Optional tag in `name:variant` format.
        #[arg(long)]
        tag: Option<String>,

        /// Force re-download even if already present.
        #[arg(long, short)]
        force: bool,
    },

    /// Bootstrap a curated model family according to hardware envelope.
    Bootstrap {
        /// Curated model family: qwen, granite, phi, or gemma.
        #[arg(long, default_value = "qwen")]
        family: String,

        /// Print sizing calculation and planned downloads without downloading.
        #[arg(long)]
        dry_run: bool,
    },

    /// Visual diffusion model operations.
    Visual {
        #[command(subcommand)]
        command: VisualCommands,
    },

    /// LoRA delta operations.
    Lora {
        #[command(subcommand)]
        command: LoraCommands,
    },

    /// Generate shell completions.
    Completions {
        /// Target shell (bash, zsh, fish).
        shell: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum VisualCommands {
    /// Pull a visual diffusion model into CAS storage.
    Pull {
        /// Model spec or repo.
        model: String,
        /// Optional tag in name:variant format.
        #[arg(long)]
        tag: Option<String>,
        /// Force re-download even if already present.
        #[arg(long, short)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum LoraCommands {
    /// Pull a LoRA delta adapter into CAS storage.
    Pull {
        /// LoRA spec or repo.
        lora: String,
        /// Optional tag in name:variant format.
        #[arg(long)]
        tag: Option<String>,
        /// Force re-download even if already present.
        #[arg(long, short)]
        force: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("cli parse")
    }

    #[test]
    fn test_cli_list_defaults() {
        let cli = parse(&["modelctl", "list"]);
        assert!(!cli.json);
        assert!(matches!(cli.command, Commands::List));
    }

    #[test]
    fn test_cli_visual_and_lora_pull() {
        let cli_vis = parse(&["modelctl", "visual", "pull", "flux.1-schnell"]);
        assert!(matches!(cli_vis.command, Commands::Visual { .. }));

        let cli_lora = parse(&["modelctl", "lora", "pull", "org/adapter"]);
        assert!(matches!(cli_lora.command, Commands::Lora { .. }));
    }
}
