//! Sizing algorithms for family speculative pairings and quantization ladders.

use super::budget::MemoryBudget;
use super::families::{plan_bitnet, plan_phi};
use super::family::{BootstrapPlan, ModelFamily};
use super::profiles::{plan_gemma, plan_granite, plan_qwen};

pub fn plan_family(family: ModelFamily, budget: &MemoryBudget) -> BootstrapPlan {
    match family {
        ModelFamily::Qwen => plan_qwen(budget),
        ModelFamily::Granite => plan_granite(budget),
        ModelFamily::Phi => plan_phi(budget),
        ModelFamily::Gemma => plan_gemma(budget),
        ModelFamily::BitNet => plan_bitnet(budget),
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
        assert!(plan.embedder.is_some());
        assert_eq!(plan.embedder.as_ref().unwrap().name, "embeddinggemma");
        assert_eq!(plan.deep_reasoner.as_ref().unwrap().tag, "27b");
    }

    #[test]
    fn test_phi_plan_with_gpu() {
        let budget = MemoryBudget {
            vram_budget_bytes: (10.0 * 1024.0 * 1024.0 * 1024.0) as u64,
            ram_budget_bytes: 3 * 1024 * 1024 * 1024,
            total_gpu_vram_bytes: 16 * 1024 * 1024 * 1024,
            available_ram_bytes: 16 * 1024 * 1024 * 1024,
            cpu_cores: 8,
        };
        let plan = plan_family(ModelFamily::Phi, &budget);
        assert_eq!(plan.draft.name, "phi-3.5-mini");
        assert_eq!(plan.primary.as_ref().unwrap().name, "phi-4");
    }

    #[test]
    fn test_bitnet_plan_cpu_only() {
        let budget = MemoryBudget {
            vram_budget_bytes: 0,
            ram_budget_bytes: 2 * 1024 * 1024 * 1024,
            total_gpu_vram_bytes: 0,
            available_ram_bytes: 4 * 1024 * 1024 * 1024,
            cpu_cores: 4,
        };
        let plan = plan_family(ModelFamily::BitNet, &budget);
        assert_eq!(plan.draft.name, "bitnet");
        assert_eq!(plan.draft.tag, "2b");
        assert_eq!(plan.draft.quant, "TL1");
        assert!(plan.primary.is_none());
    }
}
