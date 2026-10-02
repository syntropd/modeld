//! Safetensors metadata inspection for visual diffusion models.
//!
//! Detects UNet vs. DiT architectures, latent channel counts, and LoRA rank deltas
//! directly from SafeTensors headers with zero non-Rust dependencies.

use crate::error::ModeldError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffusionArchitecture {
    Unet,
    Dit,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VisualModelKind {
    BaseModel {
        architecture: DiffusionArchitecture,
        latent_channels: usize,
    },
    Lora {
        rank: usize,
        alpha: f64,
        target_architecture: DiffusionArchitecture,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualModelMetadata {
    pub kind: VisualModelKind,
    pub tensor_count: usize,
    pub parameter_count: u64,
}

/// Inspects SafeTensors header stream to identify diffusion architecture and LoRA ranks.
pub fn inspect_visual_metadata<R: Read>(reader: R) -> Result<VisualModelMetadata, ModeldError> {
    let raw = crate::format::safetensors::parse_safetensors_header(reader)?;
    Ok(analyze_visual_attributes(&raw.attributes, raw.tensor_count, raw.parameter_count))
}

/// Analyzes tensor keys and metadata attributes to categorize visual model artifacts.
pub fn analyze_visual_attributes(
    attrs: &HashMap<String, String>,
    tensor_count: usize,
    parameter_count: u64,
) -> VisualModelMetadata {
    let mut is_lora = false;
    let mut detected_rank = 0usize;
    let mut detected_alpha = 1.0f64;
    let mut is_dit = false;
    let mut is_unet = false;
    let mut latent_channels = 4usize;

    for (k, v) in attrs {
        let lk = k.to_ascii_lowercase();
        if lk.contains("double_blocks") || lk.contains("single_blocks") || lk.contains("transformer_blocks") || lk.contains("joint_blocks") {
            is_dit = true;
            latent_channels = 16;
        } else if lk.contains("down_blocks") || lk.contains("up_blocks") || lk.contains("input_blocks") || lk.contains("middle_block") {
            is_unet = true;
        }

        if lk.contains("lora_down") || lk.contains("lora_up") || lk.contains("lora_a") || lk.contains("lora_b") {
            is_lora = true;
            if lk.ends_with(".alpha") {
                if let Ok(a) = v.parse::<f64>() {
                    detected_alpha = a;
                }
            }
        }
        if lk.contains("rank") || lk.contains("network_dim") {
            if let Ok(r) = v.parse::<usize>() {
                detected_rank = r;
            }
        }
        if lk.contains("network_alpha") {
            if let Ok(a) = v.parse::<f64>() {
                detected_alpha = a;
            }
        }
    }

    let arch = if is_dit {
        DiffusionArchitecture::Dit
    } else if is_unet {
        DiffusionArchitecture::Unet
    } else {
        DiffusionArchitecture::Unknown
    };

    let kind = if is_lora {
        VisualModelKind::Lora {
            rank: if detected_rank == 0 { 16 } else { detected_rank },
            alpha: detected_alpha,
            target_architecture: arch,
        }
    } else if arch != DiffusionArchitecture::Unknown {
        VisualModelKind::BaseModel {
            architecture: arch,
            latent_channels,
        }
    } else {
        VisualModelKind::Unknown
    };

    VisualModelMetadata {
        kind,
        tensor_count,
        parameter_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_unet_base_model() {
        let mut attrs = HashMap::new();
        attrs.insert("model.diffusion_model.input_blocks.0.0.weight".into(), "tensor".into());
        let meta = analyze_visual_attributes(&attrs, 120, 800_000_000);
        assert_eq!(
            meta.kind,
            VisualModelKind::BaseModel {
                architecture: DiffusionArchitecture::Unet,
                latent_channels: 4,
            }
        );
    }

    #[test]
    fn test_detect_flux_dit_model() {
        let mut attrs = HashMap::new();
        attrs.insert("double_blocks.0.img_attn.qkv.weight".into(), "tensor".into());
        let meta = analyze_visual_attributes(&attrs, 350, 12_000_000_000);
        assert_eq!(
            meta.kind,
            VisualModelKind::BaseModel {
                architecture: DiffusionArchitecture::Dit,
                latent_channels: 16,
            }
        );
    }

    #[test]
    fn test_detect_lora_rank_and_alpha() {
        let mut attrs = HashMap::new();
        attrs.insert("lora_unet_down_blocks_0_attentions_0_proj.lora_down.weight".into(), "tensor".into());
        attrs.insert("network_dim".into(), "32".into());
        attrs.insert("network_alpha".into(), "16.0".into());
        let meta = analyze_visual_attributes(&attrs, 48, 15_000_000);
        assert_eq!(
            meta.kind,
            VisualModelKind::Lora {
                rank: 32,
                alpha: 16.0,
                target_architecture: DiffusionArchitecture::Unet,
            }
        );
    }
}
