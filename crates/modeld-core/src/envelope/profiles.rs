//! Curated model family profiles and quantization targets.

use super::budget::MemoryBudget;
use super::family::{BootstrapPlan, ModelFamily, ModelRole, ModelTarget};

pub(crate) fn mk_target(
    role: ModelRole,
    name: &str,
    tag: &str,
    repo: &str,
    file: &str,
    quant: &str,
    bytes: u64,
) -> ModelTarget {
    ModelTarget {
        role,
        name: name.into(),
        tag: tag.into(),
        repo: repo.into(),
        file: file.into(),
        quant: quant.into(),
        estimated_bytes: bytes,
    }
}

pub(crate) fn plan_qwen(budget: &MemoryBudget) -> BootstrapPlan {
    let q8_thresh = 650 * 1024 * 1024;
    let draft = if budget.ram_budget_bytes >= q8_thresh {
        mk_target(
            ModelRole::CpuDraft,
            "qwen2.5",
            "0.5b",
            "Qwen/Qwen2.5-0.5B-Instruct-GGUF",
            "qwen2.5-0.5b-instruct-q8_0.gguf",
            "Q8_0",
            650 * 1024 * 1024,
        )
    } else {
        mk_target(
            ModelRole::CpuDraft,
            "qwen2.5",
            "0.5b",
            "Qwen/Qwen2.5-0.5B-Instruct-GGUF",
            "qwen2.5-0.5b-instruct-q4_k_m.gguf",
            "Q4_K_M",
            398 * 1024 * 1024,
        )
    };

    let primary = if budget.has_gpu()
        && budget.vram_budget_bytes >= (5.0 * 1024.0 * 1024.0 * 1024.0) as u64
    {
        Some(mk_target(
            ModelRole::GpuPrimary,
            "qwen2.5",
            "7b",
            "Qwen/Qwen2.5-7B-Instruct-GGUF",
            "qwen2.5-7b-instruct-q4_k_m.gguf",
            "Q4_K_M",
            (5.2 * 1024.0 * 1024.0 * 1024.0) as u64,
        ))
    } else {
        None
    };

    let deep_reasoner = if budget.vram_budget_bytes >= (20.5 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(mk_target(
            ModelRole::DeepReasoner,
            "qwen2.5",
            "32b",
            "Qwen/Qwen2.5-32B-Instruct-GGUF",
            "qwen2.5-32b-instruct-q4_k_m.gguf",
            "Q4_K_M",
            (20.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        ))
    } else if budget.vram_budget_bytes >= (9.5 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(mk_target(
            ModelRole::DeepReasoner,
            "qwen2.5",
            "14b",
            "Qwen/Qwen2.5-14B-Instruct-GGUF",
            "qwen2.5-14b-instruct-q4_k_m.gguf",
            "Q4_K_M",
            (9.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        ))
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

pub(crate) fn plan_granite(budget: &MemoryBudget) -> BootstrapPlan {
    let q8_thresh = 850 * 1024 * 1024;
    let draft = if budget.ram_budget_bytes >= q8_thresh {
        mk_target(
            ModelRole::CpuDraft,
            "granite-3.0",
            "1b",
            "bartowski/granite-3.0-1b-a400m-instruct-GGUF",
            "granite-3.0-1b-a400m-instruct-Q8_0.gguf",
            "Q8_0",
            850 * 1024 * 1024,
        )
    } else {
        mk_target(
            ModelRole::CpuDraft,
            "granite-3.0",
            "1b",
            "bartowski/granite-3.0-1b-a400m-instruct-GGUF",
            "granite-3.0-1b-a400m-instruct-Q4_K_M.gguf",
            "Q4_K_M",
            550 * 1024 * 1024,
        )
    };

    let primary = if budget.has_gpu()
        && budget.vram_budget_bytes >= (5.0 * 1024.0 * 1024.0 * 1024.0) as u64
    {
        Some(mk_target(
            ModelRole::GpuPrimary,
            "granite-3.0",
            "8b",
            "bartowski/granite-3.0-8b-instruct-GGUF",
            "granite-3.0-8b-instruct-Q4_K_M.gguf",
            "Q4_K_M",
            (5.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        ))
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

pub(crate) fn plan_gemma(budget: &MemoryBudget) -> BootstrapPlan {
    let q8_thresh = (2.0 * 1024.0 * 1024.0 * 1024.0) as u64;
    let draft = if budget.ram_budget_bytes >= q8_thresh {
        mk_target(
            ModelRole::CpuDraft,
            "gemma",
            "2b",
            "bartowski/gemma-2-2b-it-GGUF",
            "gemma-2-2b-it-Q8_0.gguf",
            "Q8_0",
            (2.2 * 1024.0 * 1024.0 * 1024.0) as u64,
        )
    } else {
        mk_target(
            ModelRole::CpuDraft,
            "gemma",
            "2b",
            "bartowski/gemma-2-2b-it-GGUF",
            "gemma-2-2b-it-Q4_K_M.gguf",
            "Q4_K_M",
            (1.6 * 1024.0 * 1024.0 * 1024.0) as u64,
        )
    };

    let (primary, vision_tower) = if budget.has_gpu()
        && budget.vram_budget_bytes >= (6.0 * 1024.0 * 1024.0 * 1024.0) as u64
    {
        (
            Some(mk_target(
                ModelRole::GpuPrimary,
                "gemma",
                "9b",
                "bartowski/gemma-2-9b-it-GGUF",
                "gemma-2-9b-it-Q4_K_M.gguf",
                "Q4_K_M",
                (6.2 * 1024.0 * 1024.0 * 1024.0) as u64,
            )),
            Some(mk_target(
                ModelRole::VisionTower,
                "gemma",
                "mmproj-f16",
                "unsloth/gemma-4-E2B-it-GGUF",
                "mmproj-F16.gguf",
                "F16",
                600 * 1024 * 1024,
            )),
        )
    } else {
        (None, None)
    };

    let deep_reasoner = if budget.vram_budget_bytes >= (17.0 * 1024.0 * 1024.0 * 1024.0) as u64 {
        Some(mk_target(
            ModelRole::DeepReasoner,
            "gemma",
            "27b",
            "bartowski/gemma-2-27b-it-GGUF",
            "gemma-2-27b-it-Q4_K_M.gguf",
            "Q4_K_M",
            (17.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        ))
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
