//! Entrypoint for modelctl: operator CLI utility for modeld.

use clap::{CommandFactory, Parser, Subcommand};
use modelctl::client::VarlinkClient;
use modelctl::cmd;
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
        /// Target maximum storage quota in bytes. Required to prevent
        /// accidental deletion of every unpinned model.
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

    /// Generate shell completions.
    Completions {
        /// Target shell (bash, zsh, fish).
        shell: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List => {
            let mut client = VarlinkClient::connect(&cli.socket)?;
            cmd::run_list(&mut client, cli.json)?;
        }
        Commands::Inspect { id } => {
            let mut client = VarlinkClient::connect(&cli.socket)?;
            cmd::run_inspect(&mut client, &id, cli.json)?;
        }
        Commands::Pin { id } => {
            let mut client = VarlinkClient::connect(&cli.socket)?;
            cmd::run_pin(&mut client, &id)?;
        }
        Commands::Unpin { id } => {
            let mut client = VarlinkClient::connect(&cli.socket)?;
            cmd::run_unpin(&mut client, &id)?;
        }
        Commands::Prune { max_bytes } => {
            let mut client = VarlinkClient::connect(&cli.socket)?;
            cmd::run_prune(&mut client, max_bytes)?;
        }
        Commands::Import { source, tag } => {
            cmd::run_import(&cli.storage_path, &source, tag.as_deref())?;
        }
        Commands::Completions { shell } => {
            cmd::run_completions(&shell, Cli::command());
        }
    }

    Ok(())
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
        assert_eq!(cli.socket, PathBuf::from(DEFAULT_SOCKET_PATH));
        assert_eq!(cli.storage_path, PathBuf::from(DEFAULT_STORAGE_PATH));
    }

    #[test]
    fn test_cli_ls_alias_and_global_flags() {
        let cli = parse(&["modelctl", "--json", "--socket", "/tmp/t.sock", "ls"]);
        assert!(cli.json);
        assert!(matches!(cli.command, Commands::List));
        assert_eq!(cli.socket, PathBuf::from("/tmp/t.sock"));
    }

    #[test]
    fn test_cli_inspect_pin_unpin_take_id() {
        let cli = parse(&["modelctl", "inspect", "wisp:1b"]);
        assert!(matches!(cli.command, Commands::Inspect { .. }));
        let cli = parse(&["modelctl", "pin", "wisp:1b"]);
        assert!(matches!(cli.command, Commands::Pin { .. }));
        let cli = parse(&["modelctl", "unpin", "wisp:1b"]);
        assert!(matches!(cli.command, Commands::Unpin { .. }));
    }

    #[test]
    fn test_cli_prune_requires_max_bytes() {
        assert!(Cli::try_parse_from(["modelctl", "prune"]).is_err());
        let cli = parse(&["modelctl", "prune", "--max-bytes", "1024"]);
        assert!(matches!(cli.command, Commands::Prune { max_bytes: 1024 }));
    }

    #[test]
    fn test_cli_import_tag_is_optional() {
        let cli = parse(&["modelctl", "import", "/tmp/m.gguf"]);
        assert!(matches!(cli.command, Commands::Import { tag: None, .. }));
        let cli = parse(&["modelctl", "import", "/tmp/m.gguf", "--tag", "wisp:1b"]);
        assert!(matches!(cli.command, Commands::Import { tag: Some(_), .. }));
    }

    #[test]
    fn test_cli_completions_takes_shell() {
        let cli = parse(&["modelctl", "completions", "bash"]);
        assert!(matches!(cli.command, Commands::Completions { .. }));
    }
}
