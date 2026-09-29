//! Alias mapping and known model definitions for fast pulls.

pub struct ModelAlias {
    pub alias: &'static str,
    pub repo: &'static str,
    pub file: &'static str,
    pub name: &'static str,
    pub tag: &'static str,
}

pub const ALIASES: &[ModelAlias] = &[
    ModelAlias {
        alias: "qwen2.5:0.5b",
        repo: "Qwen/Qwen2.5-0.5B-Instruct-GGUF",
        file: "qwen2.5-0.5b-instruct-q4_k_m.gguf",
        name: "qwen2.5",
        tag: "0.5b",
    },
    ModelAlias {
        alias: "qwen2.5:1.5b",
        repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF",
        file: "qwen2.5-1.5b-instruct-q4_k_m.gguf",
        name: "qwen2.5",
        tag: "1.5b",
    },
    ModelAlias {
        alias: "llama3.2:1b",
        repo: "bartowski/Llama-3.2-1B-Instruct-GGUF",
        file: "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
        name: "llama3.2",
        tag: "1b",
    },
    ModelAlias {
        alias: "smollm2:360m",
        repo: "HuggingFaceTB/SmolLM2-360M-Instruct-GGUF",
        file: "smollm2-360m-instruct-q4_k_m.gguf",
        name: "smollm2",
        tag: "360m",
    },
    ModelAlias {
        alias: "gemma4:e2b",
        repo: "bartowski/gemma-2-2b-it-GGUF",
        file: "gemma-2-2b-it-Q4_K_M.gguf",
        name: "gemma4",
        tag: "e2b",
    },
];

pub fn find_alias(spec: &str) -> Option<&'static ModelAlias> {
    ALIASES.iter().find(|a| a.alias.eq_ignore_ascii_case(spec))
}
