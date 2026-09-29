//! Selection logic for quantization variants and tag overrides.

use anyhow::{anyhow, Result};

pub fn parse_tag_override(
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

pub fn select_file(files: &[String], format: &str, quant: Option<&str>) -> Result<String> {
    if files.is_empty() {
        return Err(anyhow!("No matching files found for selection"));
    }

    if format == "safetensors" {
        if let Some(f) = files.iter().find(|f| *f == "model.safetensors") {
            return Ok(f.clone());
        }
        return Ok(files[0].clone());
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

    for pref in &["Q4_K_M", "Q4_0", "Q8_0", "F16"] {
        if let Some(found) = files.iter().find(|f| f.to_ascii_uppercase().contains(pref)) {
            return Ok(found.clone());
        }
    }

    Ok(files[0].clone())
}

pub fn select_gguf_file(files: &[String], quant: Option<&str>) -> Result<String> {
    select_file(files, "gguf", quant)
}
