//! Shell completion generator for modelctl.

use clap::Command;
use std::io::{self, Write};

/// Outputs shell completion scripts for supported shells.
pub fn run_completions(shell: &str, mut cmd: Command) {
    match shell.to_lowercase().as_str() {
        "bash" => {
            clap_complete::generate(
                clap_complete::Shell::Bash,
                &mut cmd,
                "modelctl",
                &mut io::stdout(),
            );
        }
        "zsh" => {
            clap_complete::generate(
                clap_complete::Shell::Zsh,
                &mut cmd,
                "modelctl",
                &mut io::stdout(),
            );
        }
        "fish" => {
            clap_complete::generate(
                clap_complete::Shell::Fish,
                &mut cmd,
                "modelctl",
                &mut io::stdout(),
            );
        }
        _ => {
            let _ = writeln!(
                io::stderr(),
                "Unsupported shell '{}'. Supported: bash, zsh, fish",
                shell
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_completions_supported_shells_do_not_panic() {
        for shell in ["bash", "zsh", "fish", "BASH"] {
            run_completions(shell, Command::new("modelctl"));
        }
    }

    #[test]
    fn test_run_completions_unsupported_shell_is_ignored() {
        run_completions("powershell", Command::new("modelctl"));
    }
}
