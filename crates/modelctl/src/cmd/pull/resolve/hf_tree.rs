//! Hugging Face Hub tree API walker and format filter.

use super::{parse_tag_override, select_file, ResolvedModel};
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct HfTreeEntry {
    pub path: String,
    #[serde(default)]
    pub r#type: String,
}

pub async fn resolve_hf_repo(
    client: &Client,
    repo: &str,
    format: &str,
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
    let is_safetensors = format.eq_ignore_ascii_case("safetensors");

    let matched_files: Vec<String> = if is_safetensors {
        let st_files: Vec<String> = entries
            .iter()
            .filter(|e| e.r#type == "file" && e.path.ends_with(".safetensors"))
            .map(|e| e.path.clone())
            .collect();

        let has_shards = st_files
            .iter()
            .any(|f| f.contains("-of-") || f.contains(".part"));
        if has_shards || (st_files.len() > 1 && !st_files.iter().any(|f| f == "model.safetensors"))
        {
            return Err(anyhow!(
                "Sharded Safetensors model detected in '{}' (found {} shards: {}). \
                Sharded weights are not supported for single-stream pull. \
                Please choose an unsharded model or a GGUF quant.",
                repo,
                st_files.len(),
                st_files.join(", ")
            ));
        }

        if st_files.is_empty() {
            return Err(anyhow!(
                "No .safetensors files found in repository '{}'",
                repo
            ));
        }
        st_files
    } else {
        let gguf_files: Vec<String> = entries
            .iter()
            .filter(|e| e.r#type == "file" && e.path.ends_with(".gguf"))
            .map(|e| e.path.clone())
            .collect();

        if gguf_files.is_empty() {
            return Err(anyhow!("No .gguf files found in repository '{}'", repo));
        }
        gguf_files
    };

    let selected = select_file(&matched_files, format, quant)?;
    let repo_name = repo.split('/').last().unwrap_or(repo);
    let default_name = repo_name.to_ascii_lowercase().replace('_', "-");
    let default_tag = if is_safetensors {
        "latest".to_string()
    } else {
        quant.unwrap_or("latest").to_ascii_lowercase()
    };
    let (name, tag) = parse_tag_override(&default_name, &default_tag, tag_override);

    let tokenizer_url = if is_safetensors {
        entries
            .iter()
            .find(|e| e.r#type == "file" && e.path == "tokenizer.json")
            .map(|_| {
                format!(
                    "https://huggingface.co/{}/resolve/main/tokenizer.json",
                    repo
                )
            })
    } else {
        None
    };

    Ok(ResolvedModel {
        download_url: format!("https://huggingface.co/{}/resolve/main/{}", repo, selected),
        filename: selected,
        name,
        tag,
        format: if is_safetensors {
            "safetensors".to_string()
        } else {
            "gguf".to_string()
        },
        tokenizer_url,
    })
}
