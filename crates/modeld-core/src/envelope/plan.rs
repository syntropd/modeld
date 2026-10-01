//! Sizing algorithms for family speculative pairings and quantization ladders.

use super::budget::MemoryBudget;
use super::family::{BootstrapPlan, ModelFamily, ModelRole, ModelTarget};

pub fn plan_family(family: ModelFamily, budget: &MemoryBudget) -> BootstrapPlan {
    match family {
        ModelFamily::Qwen => plan_qwen(budget),
        ModelFamily::Granite => plan_granite(budget),
        ModelFamily::Gemma => plan_gemma(budget),
    }
}

fn plan_qwen(budget: &MemoryBudget) -> BootstrapPlan {
    let q8_thresh = 650 * 1024 * 1024;
    let draft = if budget.ram_budget_bytes >= q8_thresh {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "qwen2.5".into(),
            tag: "0.5b".into(),
            repo: "Qwen/Qwen2.5-0.5B-Instruct-GGUF".into(),
            file: "qwen2.5-0.5b-instruct-q8_0.gguf".into(),
            quant: "Q8_0".into(),
            estimated_bytes: 650 * 1024 * 1024,
        }
    } else {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "qwen2.5".into(),
            tag: "0.5b".into(),
            repo: "Qwen/Qwen2.5-0.5B-Instruct-GGUF".into(),
            file: "qwen2.5-0.5b-instruct-q4_k_m.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: 398 * 1024 * 1024,
        }
    };

    let primary = if budget.has_gpu() && budget.vram_budget_bytes >= (5.0 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(ModelTarget {
            role: ModelRole::GpuPrimary,
            name: "qwen2.5".into(),
            tag: "7b".into(),
            repo: "Qwen/Qwen2.5-7B-Instruct-GGUF".into(),
            file: "qwen2.5-7b-instruct-q4_k_m.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (5.2 * 1024.0 * 1024.0 * 1024.0) as u64,
        })
    } else {
        None
    };

    let deep_reasoner = if budget.vram_budget_bytes >= (20.5 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(ModelTarget {
            role: ModelRole::DeepReasoner,
            name: "qwen2.5".into(),
            tag: "32b".into(),
            repo: "Qwen/Qwen2.5-32B-Instruct-GGUF".into(),
            file: "qwen2.5-32b-instruct-q4_k_m.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (20.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        })
    } else if budget.vram_budget_bytes >= (9.5 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(ModelTarget {
            role: ModelRole::DeepReasoner,
            name: "qwen2.5".into(),
            tag: "14b".into(),
            repo: "Qwen/Qwen2.5-14B-Instruct-GGUF".into(),
            file: "qwen2.5-14b-instruct-q4_k_m.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (9.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        })
    } else {
        None
    };

    BootstrapPlan {
        family: ModelFamily::Qwen,
        budget: budget.clone(),
        draft,
        primary,
        deep_reasoner,
        vision_tower: None,
    }
}

fn plan_granite(budget: &MemoryBudget) -> BootstrapPlan {
    let q8_thresh = 850 * 1024 * 1024;
    let draft = if budget.ram_budget_bytes >= q8_thresh {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "granite-3.0".into(),
            tag: "1b".into(),
            repo: "bartowski/granite-3.0-1b-a400m-instruct-GGUF".into(),
            file: "granite-3.0-1b-a400m-instruct-Q8_0.gguf".into(),
            quant: "Q8_0".into(),
            estimated_bytes: 850 * 1024 * 1024,
        }
    } else {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "granite-3.0".into(),
            tag: "1b".into(),
            repo: "bartowski/granite-3.0-1b-a400m-instruct-GGUF".into(),
            file: "granite-3.0-1b-a400m-instruct-Q4_K_M.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: 550 * 1024 * 1024,
        }
    };

    let primary = if budget.has_gpu() && budget.vram_budget_bytes >= (5.0 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(ModelTarget {
            role: ModelRole::GpuPrimary,
            name: "granite-3.0".into(),
            tag: "8b".into(),
            repo: "bartowski/granite-3.0-8b-instruct-GGUF".into(),
            file: "granite-3.0-8b-instruct-Q4_K_M.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (5.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        })
    } else {
        None
    };

    BootstrapPlan {
        family: ModelFamily::Granite,
        budget: budget.clone(),
        draft,
        primary,
        deep_reasoner: None,
        vision_tower: None,
    }
}

fn plan_gemma(budget: &MemoryBudget) -> BootstrapPlan {
    let q8_thresh = (2.0 * 1024.0 * 1024.0 * 1024.0) as u64;
    let draft = if budget.ram_budget_bytes >= q8_thresh {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "gemma".into(),
            tag: "2b".into(),
            repo: "bartowski/gemma-2-2b-it-GGUF".into(),
            file: "gemma-2-2b-it-Q8_0.gguf".into(),
            quant: "Q8_0".into(),
            estimated_bytes: (2.2 * 1024.0 * 1024.0 * 1024.0) as u64,
        }
    } else {
        ModelTarget {
            role: ModelRole::CpuDraft,
            name: "gemma".into(),
            tag: "2b".into(),
            repo: "bartowski/gemma-2-2b-it-GGUF".into(),
            file: "gemma-2-2b-it-Q4_K_M.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (1.6 * 1024.0 * 1024.0 * 1024.0) as u64,
        }
    };

    let (primary, vision_tower) = if budget.has_gpu() && budget.vram_budget_bytes >= (6.0 * 1024.0 * 1024.0 * 1024.0) as u64 {
        (
            Some(ModelTarget {
                role: ModelRole::GpuPrimary,
                name: "gemma".into(),
                tag: "9b".into(),
                repo: "bartowski/gemma-2-9b-it-GGUF".into(),
                file: "gemma-2-9b-it-Q4_K_M.gguf".into(),
                quant: "Q4_K_M".into(),
                estimated_bytes: (6.2 * 1024.0 * 1024.0 * 1024.0) as u64,
            }),
            Some(ModelTarget {
                role: ModelRole::VisionTower,
                name: "gemma".into(),
                tag: "mmproj-f16".into(),
                repo: "unsloth/gemma-4-E2B-it-GGUF".into(),
                file: "mmproj-F16.gguf".into(),
                quant: "F16".into(),
                estimated_bytes: 600 * 1024 * 1024,
            }),
        )
    } else {
        (None, None)
    };

    let deep_reasoner = if budget.vram_budget_bytes >= (17.0 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(ModelTarget {
            role: ModelRole::DeepReasoner,
            name: "gemma".into(),
            tag: "27b".into(),
            repo: "bartowski/gemma-2-27b-it-GGUF".into(),
            file: "gemma-2-27b-it-Q4_K_M.gguf".into(),
            quant: "Q4_K_M".into(),
            estimated_bytes: (17.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        })
    } else {
        None
    };

    BootstrapPlan {
        family: ModelFamily::Gemma,
        budget: budget.clone(),
        draft,
        primary,
        deep_reasoner,
        vision_tower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qwen_plan_small_ram() {
        let budget = MemoryBudget {
            vram_budget_bytes: 0,
            ram_budget_bytes: 300 * 1024 * 1024,
            total_gpu_vram_bytes: 0,
            available_ram_bytes: 600 * 1024 * 1024,
            cpu_cores: 4,
        };
        let plan = plan_family(ModelFamily::Qwen, &budget);
        assert_eq!(plan.draft.quant, "Q4_K_M");
        assert!(plan.primary.is_none());
        assert!(plan.deep_reasoner.is_none());
    }

    #[test]
    fn test_qwen_plan_with_gpu_24gb() {
        let budget = MemoryBudget {
            vram_budget_bytes: (20.6 * 1024.0 * 1024.0 * 1024.0) as u64,
            ram_budget_bytes: 16 * 1024 * 1024 * 1024,
            total_gpu_vram_bytes: 24 * 1024 * 1024 * 1024,
            available_ram_bytes: 32 * 1024 * 1024 * 1024,
            cpu_cores: 16,
        };
        let plan = plan_family(ModelFamily::Qwen, &budget);
        assert_eq!(plan.draft.quant, "Q8_0");
        assert_eq!(plan.primary.as_ref().unwrap().tag, "7b");
        assert_eq!(plan.deep_reasoner.as_ref().unwrap().tag, "32b");
    }

    #[test]
    fn test_gemma_plan_with_vision_and_reasoner() {
        let budget = MemoryBudget {
            vram_budget_bytes: (18.0 * 1024.0 * 1024.0 * 1024.0) as u64,
            ram_budget_bytes: 4 * 1024 * 1024 * 1024,
            total_gpu_vram_bytes: 24 * 1024 * 1024 * 1024,
            available_ram_bytes: 8 * 1024 * 1024 * 1024,
            cpu_cores: 8,
        };
        let plan = plan_family(ModelFamily::Gemma, &budget);
        assert_eq!(plan.draft.quant, "Q8_0");
        assert_eq!(plan.primary.as_ref().unwrap().tag, "9b");
        assert!(plan.vision_tower.is_some());
        assert_eq!(plan.deep_reasoner.as_ref().unwrap().tag, "27b");
    }
}
