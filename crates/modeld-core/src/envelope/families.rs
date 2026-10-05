//! Curated model family catalog and metadata declarations.
//!
//! Declares vocabulary sizes, speculative pairings, VRAM envelope requirements,
//! and specialization targets for supported model families (Qwen, Granite, Phi, Gemma).

use super::budget::MemoryBudget;
use super::family::{BootstrapPlan, ModelFamily, ModelRole, ModelTarget};
use serde::{Deserialize, Serialize};

/// Catalog metadata describing a supported model family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyCatalogEntry {
    /// Family discriminator.
    pub family: ModelFamily,
    /// Canonical family name identifier.
    pub name: &'static str,
    /// Default hardware and workload profile summary.
    pub default_profile: &'static str,
    /// Designated CPU draft model name.
    pub cpu_draft_model: &'static str,
    /// Designated primary GPU target model name.
    pub primary_gpu_model: &'static str,
    /// Specialist optimization focus area.
    pub specialist_focus: &'static str,
    /// Tokenizer vocabulary size.
    pub vocab_size: usize,
    /// Minimum VRAM budget in megabytes required for GPU primary.
    pub min_vram_mb: usize,
    /// Recommended VRAM budget in megabytes.
    pub recommended_vram_mb: usize,
}

/// Retrieve static catalog entries for all supported model families.
pub fn family_catalog() -> &'static [FamilyCatalogEntry] {
    &[
        FamilyCatalogEntry {
            family: ModelFamily::Qwen,
            name: "qwen",
            default_profile: "Tier-1 Fleet Backbone",
            cpu_draft_model: "qwen2.5:0.5b",
            primary_gpu_model: "qwen2.5:7b",
            specialist_focus: "Balanced text, Qwen-Coder, Qwen-VL",
            vocab_size: 152064,
            min_vram_mb: 5324,
            recommended_vram_mb: 8192,
        },
        FamilyCatalogEntry {
            family: ModelFamily::Granite,
            name: "granite",
            default_profile: "Enterprise Linux & Sysadmin",
            cpu_draft_model: "granite-3.0:1b",
            primary_gpu_model: "granite-3.0:8b",
            specialist_focus: "Systemd units, C/Rust, kernel headers",
            vocab_size: 49152,
            min_vram_mb: 5632,
            recommended_vram_mb: 8192,
        },
        FamilyCatalogEntry {
            family: ModelFamily::Phi,
            name: "phi",
            default_profile: "Compact Laptop Density",
            cpu_draft_model: "phi-3.5-mini:3.8b",
            primary_gpu_model: "phi-4:14b",
            specialist_focus: "Phi-4-multimodal (audio+vision+text in 5.6B)",
            vocab_size: 100352,
            min_vram_mb: 9728,
            recommended_vram_mb: 16384,
        },
        FamilyCatalogEntry {
            family: ModelFamily::Gemma,
            name: "gemma",
            default_profile: "Multimodal Knowledge",
            cpu_draft_model: "gemma-2:2b",
            primary_gpu_model: "gemma-2:9b",
            specialist_focus: "PaliGemma 2 UI grounding, multilingual",
            vocab_size: 256000,
            min_vram_mb: 6348,
            recommended_vram_mb: 12288,
        },
        FamilyCatalogEntry {
            family: ModelFamily::BitNet,
            name: "bitnet",
            default_profile: "Ultra-Low Power CPU & Edge",
            cpu_draft_model: "bitnet:2b",
            primary_gpu_model: "bitnet:2b",
            specialist_focus: "Zero-VRAM CPU inference, 1.58-bit ternary math",
            vocab_size: 32000,
            min_vram_mb: 0,
            recommended_vram_mb: 0,
        },
    ]
}

/// Find family catalog entry by family enum.
pub fn lookup_family_entry(family: ModelFamily) -> Option<&'static FamilyCatalogEntry> {
    family_catalog().iter().find(|e| e.family == family)
}

/// Generate bootstrap plan for Phi family.
pub fn plan_phi(budget: &MemoryBudget) -> BootstrapPlan {
    let q8_thresh = (2.5 * 1024.0 * 1024.0 * 1024.0) as u64;
    let draft = if budget.ram_budget_bytes >= q8_thresh {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "phi-3.5-mini".into(),
            tag: "3.8b".into(),
            repo: "bartowski/Phi-3.5-mini-instruct-GGUF".into(),
            file: "Phi-3.5-mini-instruct-Q8_0.gguf".into(),
            quant: "Q8_0".into(),
            estimated_bytes: (2.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        }
    } else {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "phi-3.5-mini".into(),
            tag: "3.8b".into(),
            repo: "bartowski/Phi-3.5-mini-instruct-GGUF".into(),
            file: "Phi-3.5-mini-instruct-Q4_K_M.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (1.8 * 1024.0 * 1024.0 * 1024.0) as u64,
        }
    };

    let primary = if budget.has_gpu()
        && budget.vram_budget_bytes >= (9.0 * 1024.0 * 1024.0 * 1024.0) as u64
    {
        Some(ModelTarget {
            role: ModelRole::GpuPrimary,
            name: "phi-4".into(),
            tag: "14b".into(),
            repo: "bartowski/phi-4-GGUF".into(),
            file: "phi-4-Q4_K_M.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (9.1 * 1024.0 * 1024.0 * 1024.0) as u64,
        })
    } else {
        None
    };

    BootstrapPlan {
        family: ModelFamily::Phi,
        budget: budget.clone(),
        draft,
        primary,
        deep_reasoner: None,
        vision_tower: None,
    }
}

/// Generate bootstrap plan for BitNet family.
pub fn plan_bitnet(budget: &MemoryBudget) -> BootstrapPlan {
    let draft = ModelTarget {
        role: ModelRole::CpuDraft,
        name: "bitnet".into(),
        tag: "2b".into(),
        repo: "1bitLLM/bitnet_b1_58-2B4T".into(),
        file: "bitnet_b1_58-2B4T.gguf".into(),
        quant: "TL1".into(),
        estimated_bytes: (550.0 * 1024.0 * 1024.0) as u64,
    };

    BootstrapPlan {
        family: ModelFamily::BitNet,
        budget: budget.clone(),
        draft,
        primary: None,
        deep_reasoner: None,
        vision_tower: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_family_catalog_entries() {
        let catalog = family_catalog();
        assert_eq!(catalog.len(), 5);
        assert!(catalog.iter().any(|c| c.family == ModelFamily::Qwen));
        assert!(catalog.iter().any(|c| c.family == ModelFamily::Granite));
        assert!(catalog.iter().any(|c| c.family == ModelFamily::Phi));
        assert!(catalog.iter().any(|c| c.family == ModelFamily::Gemma));
        assert!(catalog.iter().any(|c| c.family == ModelFamily::BitNet));
    }

    #[test]
    fn test_lookup_family_entry() {
        let phi = lookup_family_entry(ModelFamily::Phi).unwrap();
        assert_eq!(phi.cpu_draft_model, "phi-3.5-mini:3.8b");
        assert_eq!(phi.primary_gpu_model, "phi-4:14b");
        assert_eq!(phi.vocab_size, 100352);

        let bitnet = lookup_family_entry(ModelFamily::BitNet).unwrap();
        assert_eq!(bitnet.cpu_draft_model, "bitnet:2b");
        assert_eq!(bitnet.min_vram_mb, 0);
    }
}
