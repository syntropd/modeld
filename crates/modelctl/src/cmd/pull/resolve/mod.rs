//! Model resolution using built-in aliases and Hugging Face Hub tree API.

pub mod alias;
pub mod hf_tree;
pub mod select;

pub use alias::{find_alias, ModelAlias, ALIASES};
pub use hf_tree::resolve_hf_repo;
pub use select::{parse_tag_override, select_file, select_gguf_file};

use anyhow::{anyhow, Result};
use reqwest::Client;

/// Resolved model artifact metadata ready for streaming.
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    /// Direct HTTPS URL for downloading the model artifact.
    pub download_url: String,
    /// Canonical model filename.
    pub filename: String,
    /// Model name segment for tag registry.
    pub name: String,
    /// Model tag variant.
    pub tag: String,
    /// Model format: "gguf" or "safetensors".
    pub format: String,
    /// Optional companion tokenizer URL (for safetensors).
    pub tokenizer_url: Option<String>,
}

/// Resolves a model alias or `org/repo` Hugging Face identifier defaulting to GGUF.
pub async fn resolve_model(
    client: &Client,
    model_spec: &str,
    quant: Option<&str>,
    tag_override: Option<&str>,
) -> Result<ResolvedModel> {
    resolve_model_with_format(client, model_spec, "gguf", quant, tag_override).await
}

/// Resolves a model alias or `org/repo` Hugging Face identifier with specified format.
pub async fn resolve_model_with_format(
    client: &Client,
    model_spec: &str,
    format: &str,
    quant: Option<&str>,
    tag_override: Option<&str>,
) -> Result<ResolvedModel> {
    let spec = model_spec.trim();
    let fmt = format.to_ascii_lowercase();

    if fmt == "gguf" {
        if let Some(alias) = find_alias(spec) {
            if quant.is_none() {
                let (name, tag) = parse_tag_override(alias.name, alias.tag, tag_override);
                return Ok(ResolvedModel {
                    download_url: format!(
                        "https://huggingface.co/{}/resolve/main/{}",
                        alias.repo, alias.file
                    ),
                    filename: alias.file.to_string(),
                    name,
                    tag,
                    format: "gguf".to_string(),
                    tokenizer_url: None,
                });
            }
            return resolve_hf_repo(client, alias.repo, &fmt, quant, tag_override).await;
        }
    }

    if spec.contains('/') {
        return resolve_hf_repo(client, spec, &fmt, quant, tag_override).await;
    }

    Err(anyhow!(
        "Unknown model alias '{}'. Known aliases: {}",
        spec,
        ALIASES
            .iter()
            .map(|a| a.alias)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}
