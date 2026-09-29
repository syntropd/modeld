//! Model resolution using built-in aliases and Hugging Face Hub tree API.

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;

/// Resolved model artifact metadata ready for streaming.
#[derive(Debug, Clone)]
pub struct ResolvedModel {
    /// Direct HTTPS URL for downloading the GGUF artifact.
    pub download_url: String,
    /// Canonical model filename.
    pub filename: String,
    /// Model name segment for tag registry.
    pub name: String,
    /// Model tag variant.
    pub tag: String,
}

struct ModelAlias {
    alias: &'static str,
    repo: &'static str,
    file: &'static str,
    name: &'static str,
    tag: &'static str,
}

const ALIASES: &[ModelAlias] = &[
    ModelAlias {
        alias: "qwen2.5:0.5b",
        repo: "Qwen/Qwen2.5-0.5B-Instruct-GGUF",
        file: "qwen2.5-0.5b-instruct-q4_k_m.gguf",
        name: "qwen2.5",
        tag: "0.5b",
    },
    ModelAlias {
        alias: "qwen2.5:1.5b",
        repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
        file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        name: "qwen2.5",
        tag: "1.5b",
    },
    ModelAlias {
        alias: "llama3.2:1b",
        repo: "bartowski/Llama-3.2-1B-Instruct-GGUF",
        file: "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
        name: "llama3.2",
        tag: "1b",
    },
    ModelAlias {
        alias: "smollm2:360m",
        repo: "HuggingFaceTB/SmolLM2-360M-Instruct-GGUF",
        file: "smollm2-360m-instruct-q4_k_m.gguf",
        name: "smollm2",
        tag: "360m",
    },
    ModelAlias {
        alias: "gemma4:e2b",
        repo: "bartowski/gemma-2-2b-it-GGUF",
        file: "gemma-2-2b-it-Q4_K_M.gguf",
        name: "gemma4",
        tag: "e2b",
    },
];

#[derive(Deserialize)]
struct HfTreeEntry {
    path: String,
    #[serde(default)]
    r#type: String,
}

/// Resolves a model alias or `org/repo` Hugging Face identifier.
pub async fn resolve_model(
    client: &Client,
    model_spec: &str,
    quant: Option<&str>,
    tag_override: Option<&str>,
) -> Result<ResolvedModel> {
    let spec = model_spec.trim();

    // Check alias table
    if let Some(alias) = ALIASES.iter().find(|a| a.alias.eq_ignore_ascii_case(spec)) {
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
            });
        }
        return resolve_hf_repo(client, alias.repo, quant, tag_override).await;
    }

    // Check for org/repo syntax
    if spec.contains('/') {
        return resolve_hf_repo(client, spec, quant, tag_override).await;
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

async fn resolve_hf_repo(
    client: &Client,
    repo: &str,
    quant: Option<&str>,
    tag_override: Option<&str>,
) -> Result<ResolvedModel> {
    let api_url = format!("https://huggingface.co/api/models/{}/tree/main", repo);
    let resp = client
        .get(&api_url)
        .header("User-Agent", "syntrop-modelctl")
        .send()
        .await
        .with_context(|| format!("Failed to query Hugging Face API for {}", repo))?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Hugging Face repository '{}' returned HTTP {}",
            repo,
            resp.status()
        ));
    }

    let entries: Vec<HfTreeEntry> = resp.json().await.context("Failed to parse HF tree API")?;
    let gguf_files: Vec<String> = entries
        .into_iter()
        .filter(|e| e.r#type == "file" && e.path.ends_with(".gguf"))
        .map(|e| e.path)
        .collect();

    if gguf_files.is_empty() {
        return Err(anyhow!("No .gguf files found in repository '{}'", repo));
    }

    let selected = select_gguf_file(&gguf_files, quant)?;
    let repo_name = repo.split('/').last().unwrap_or(repo);
    let default_name = repo_name.to_ascii_lowercase().replace('_', "-");
    let default_tag = quant.unwrap_or("latest").to_ascii_lowercase();
    let (name, tag) = parse_tag_override(&default_name, &default_tag, tag_override);

    Ok(ResolvedModel {
        download_url: format!("https://huggingface.co/{}/resolve/main/{}", repo, selected),
        filename: selected,
        name,
        tag,
    })
}

pub fn select_gguf_file(files: &[String], quant: Option<&str>) -> Result<String> {
    if files.is_empty() {
        return Err(anyhow!("No GGUF files found for selection"));
    }

    if let Some(q) = quant {
        let q_upper = q.to_ascii_uppercase();
        if let Some(found) = files
            .iter()
            .find(|f| f.to_ascii_uppercase().contains(&q_upper))
        {
            return Ok(found.clone());
        }
        return Err(anyhow!(
            "No file matching quantization '{}' found. Available: {}",
            q,
            files.join(", ")
        ));
    }

    // Default preference order
    for pref in &["Q4_K_M", "Q4_0", "Q8_0", "F16"] {
        if let Some(found) = files.iter().find(|f| f.to_ascii_uppercase().contains(pref)) {
            return Ok(found.clone());
        }
    }

    Ok(files[0].clone())
}

fn parse_tag_override(
    default_name: &str,
    default_tag: &str,
    override_str: Option<&str>,
) -> (String, String) {
    if let Some(t) = override_str {
        let t = t.trim();
        if let Some((n, v)) = t.split_once(':') {
            let name = if n.trim().is_empty() {
                default_name
            } else {
                n.trim()
            };
            let tag = if v.trim().is_empty() {
                default_tag
            } else {
                v.trim()
            };
            (name.to_string(), tag.to_string())
        } else if !t.is_empty() {
            (default_name.to_string(), t.to_string())
        } else {
            (default_name.to_string(), default_tag.to_string())
        }
    } else {
        (default_name.to_string(), default_tag.to_string())
    }
}

