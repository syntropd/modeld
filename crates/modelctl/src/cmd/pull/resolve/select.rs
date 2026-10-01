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
        // Priority 1: strict suffix match (-QUANT.gguf or _QUANT.gguf or .QUANT.gguf)
        if let Some(found) = files
            .iter()
            .find(|f| matches_quant_strict(&f.to_ascii_uppercase(), &q_upper))
        {
            return Ok(found.clone());
        }
        // Priority 2: delimited boundary match (prevents BF16 matching F16)
        if let Some(found) = files
            .iter()
            .find(|f| matches_quant_delimited(&f.to_ascii_uppercase(), &q_upper))
        {
            return Ok(found.clone());
        }
        // Priority 3: fallback substring match with explicit BF16 guard
        if let Some(found) = files.iter().find(|f| {
            let upper = f.to_ascii_uppercase();
            if q_upper == "F16" && upper.contains("BF16") && !upper.contains("-F16") {
                return false;
            }
            upper.contains(&q_upper)
        }) {
            return Ok(found.clone());
        }
        return Err(anyhow!(
            "No file matching quantization '{}' found. Available: {}",
            q,
            files.join(", ")
        ));
    }

    for pref in &["Q4_K_M", "Q4_0", "Q8_0", "F16"] {
        if let Some(found) = files
            .iter()
            .find(|f| matches_quant_strict(&f.to_ascii_uppercase(), pref))
        {
            return Ok(found.clone());
        }
    }

    for pref in &["Q4_K_M", "Q4_0", "Q8_0", "F16"] {
        if let Some(found) = files
            .iter()
            .find(|f| matches_quant_delimited(&f.to_ascii_uppercase(), pref))
        {
            return Ok(found.clone());
        }
    }

    Ok(files[0].clone())
}

fn matches_quant_strict(file_upper: &str, q_upper: &str) -> bool {
    let suffix1 = format!("-{}.GGUF", q_upper);
    let suffix2 = format!("_{}.GGUF", q_upper);
    let suffix3 = format!(".{}.GGUF", q_upper);
    file_upper.ends_with(&suffix1)
        || file_upper.ends_with(&suffix2)
        || file_upper.ends_with(&suffix3)
}

fn matches_quant_delimited(file_upper: &str, q_upper: &str) -> bool {
    if q_upper == "F16" && file_upper.contains("BF16") && !file_upper.contains("-F16") {
        return false;
    }
    let delims = ['-', '_', '.'];
    let mut search_idx = 0;
    while let Some(pos) = file_upper[search_idx..].find(q_upper) {
        let abs_pos = search_idx + pos;
        let before_ok = if abs_pos == 0 {
            true
        } else {
            let prev_char = file_upper.as_bytes()[abs_pos - 1] as char;
            delims.contains(&prev_char)
        };
        let end_pos = abs_pos + q_upper.len();
        let after_ok = if end_pos == file_upper.len() {
            true
        } else {
            let next_char = file_upper.as_bytes()[end_pos] as char;
            delims.contains(&next_char)
        };
        if before_ok && after_ok {
            return true;
        }
        search_idx = abs_pos + 1;
    }
    false
}

pub fn select_gguf_file(files: &[String], quant: Option<&str>) -> Result<String> {
    select_file(files, "gguf", quant)
}
