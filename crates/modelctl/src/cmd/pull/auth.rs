//! Hugging Face authentication token resolution.
//!
//! Detects Hugging Face API tokens from standard environment variables
//! (`HF_TOKEN`, `HUGGING_FACE_HUB_TOKEN`) or the user's Hugging Face CLI
//! cache (`~/.cache/huggingface/token`).

use std::env;
use std::fs;
use std::path::PathBuf;

/// Attempts to resolve a Hugging Face API token from environment or disk.
pub fn resolve_hf_token() -> Option<String> {
    if let Ok(tok) = env::var("HF_TOKEN") {
        let trimmed = tok.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    if let Ok(tok) = env::var("HUGGING_FACE_HUB_TOKEN") {
        let trimmed = tok.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    token_from_cache()
}

/// Reads token from `~/.cache/huggingface/token` if accessible.
fn token_from_cache() -> Option<String> {
    let home = env::var("HOME").ok().map(PathBuf::from)?;
    let token_path = home.join(".cache/huggingface/token");
    if let Ok(content) = fs::read_to_string(token_path) {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

/// Attaches bearer authorization header to the request builder if a token exists
/// and the destination URL targets Hugging Face (`huggingface.co`).
pub fn apply_hf_auth(builder: reqwest::RequestBuilder, url: &str) -> reqwest::RequestBuilder {
    if url.contains("huggingface.co") {
        if let Some(token) = resolve_hf_token() {
            return builder.bearer_auth(token);
        }
    }
    builder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_from_cache_or_env() {
        let _ = resolve_hf_token();
    }

    #[test]
    fn test_apply_hf_auth_non_hf_url() {
        let client = reqwest::Client::new();
        let rb = client.get("https://example.com/file");
        let modified = apply_hf_auth(rb, "https://example.com/file");
        let req = modified.build().unwrap();
        assert!(!req.headers().contains_key(reqwest::header::AUTHORIZATION));
    }
}
