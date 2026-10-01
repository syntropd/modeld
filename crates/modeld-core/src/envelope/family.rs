//! Curated model family profiles and speculative roles.

use super::budget::MemoryBudget;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Supported curated model families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModelFamily {
    #[default]
    Qwen,
    Granite,
    Gemma,
}

impl FromStr for ModelFamily {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "qwen" | "qwen2.5" => Ok(Self::Qwen),
            "granite" | "granite-3.0" | "granite3" => Ok(Self::Granite),
            "gemma" | "gemma2" | "gemma3" | "gemma4" => Ok(Self::Gemma),
            other => Err(anyhow!("Unknown model family '{}'. Supported: qwen, granite, gemma", other)),
        }
    }
}

impl fmt::Display for ModelFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Qwen => write!(f, "qwen"),
            Self::Granite => write!(f, "granite"),
            Self::Gemma => write!(f, "gemma"),
        }
    }
}

/// The role a model plays within the family's speculative pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelRole {
    CpuDraft,
    GpuPrimary,
    DeepReasoner,
    VisionTower,
}

/// Specific artifact target to download and commit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelTarget {
    pub role: ModelRole,
    pub name: String,
    pub tag: String,
    pub repo: String,
    pub file: String,
    pub quant: String,
    pub estimated_bytes: u64,
}

impl ModelTarget {
    pub fn download_url(&self) -> String {
        format!("https://huggingface.co/{}/resolve/main/{}", self.repo, self.file)
    }
}

/// Comprehensive plan for a bootstrapped family given hardware memory budgets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BootstrapPlan {
    pub family: ModelFamily,
    pub budget: MemoryBudget,
    pub draft: ModelTarget,
    pub primary: Option<ModelTarget>,
    pub deep_reasoner: Option<ModelTarget>,
    pub vision_tower: Option<ModelTarget>,
}

impl BootstrapPlan {
    pub fn plan(family: ModelFamily, budget: &MemoryBudget) -> Self {
        super::plan::plan_family(family, budget)
    }

    pub fn all_targets(&self) -> Vec<&ModelTarget> {
        let mut list = vec![&self.draft];
        if let Some(p) = &self.primary {
            list.push(p);
        }
        if let Some(v) = &self.vision_tower {
            list.push(v);
        }
        if let Some(r) = &self.deep_reasoner {
            list.push(r);
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_family_from_str() {
        assert_eq!("qwen".parse::<ModelFamily>().unwrap(), ModelFamily::Qwen);
        assert_eq!("granite".parse::<ModelFamily>().unwrap(), ModelFamily::Granite);
        assert_eq!("gemma".parse::<ModelFamily>().unwrap(), ModelFamily::Gemma);
        assert!("unknown".parse::<ModelFamily>().is_err());
    }

    #[test]
    fn test_target_download_url() {
        let target = ModelTarget {
            role: ModelRole::CpuDraft,
            name: "qwen2.5".into(),
            tag: "0.5b".into(),
            repo: "Qwen/Qwen2.5-0.5B-Instruct-GGUF".into(),
            file: "qwen2.5-0.5b-instruct-q8_0.gguf".into(),
            quant: "Q8_0".into(),
            estimated_bytes: 650 * 1024 * 1024,
        };
        assert_eq!(
            target.download_url(),
            "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q8_0.gguf"
        );
    }
}
