//! Entrypoint for modelctl: operator CLI utility for modeld.

use clap::{CommandFactory, Parser};
use modelctl::cli::{Cli, Commands, LoraCommands, VisualCommands};
use modelctl::client::VarlinkClient;
use modelctl::cmd;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
        Commands::Pull {
            model,
            format,
            quant,
            tag,
            force,
        } => {
            cmd::run_pull(
                &cli.storage_path,
                &cli.socket,
                &model,
                &format,
                quant.as_deref(),
                tag.as_deref(),
                force,
            )
            .await?;
        }
        Commands::Bootstrap { family, dry_run } => {
            cmd::run_bootstrap(
                &cli.storage_path,
                &cli.socket,
                &cli.inference_socket,
                &family,
                dry_run,
                cli.json,
            )
            .await?;
        }
        Commands::Visual {
            command: VisualCommands::Pull { model, tag, force },
        } => {
            cmd::run_visual_pull(
                &cli.storage_path,
                &cli.socket,
                &model,
                tag.as_deref(),
                force,
            )
            .await?;
        }
        Commands::Lora {
            command: LoraCommands::Pull { lora, tag, force },
        } => {
            cmd::run_lora_pull(&cli.storage_path, &cli.socket, &lora, tag.as_deref(), force)
                .await?;
        }
        Commands::Completions { shell } => {
            cmd::run_completions(&shell, Cli::command());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_main_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
