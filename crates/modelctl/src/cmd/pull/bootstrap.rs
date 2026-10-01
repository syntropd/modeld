//! Model family bootstrapping and automated envelope sizing CLI command.

use super::stream::run_pull;
use anyhow::{anyhow, Result};
use modeld_core::envelope::{
    query_or_fallback_topology, BootstrapPlan, MemoryBudget, ModelFamily,
};
use std::path::Path;

/// Executes the model family bootstrap workflow.
pub async fn run_bootstrap<P: AsRef<Path>>(
    storage_root: P,
    socket_path: P,
    family_str: &str,
    dry_run: bool,
    json_output: bool,
) -> Result<()> {
    let family = family_str.parse::<ModelFamily>()?;
    let root = storage_root.as_ref().to_path_buf();
    let socket = socket_path.as_ref().to_path_buf();

    let topo = query_or_fallback_topology(&socket);
    let budget = MemoryBudget::from_topology(&topo);
    let plan = BootstrapPlan::plan(family, &budget);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&plan)?);
        if dry_run {
            return Ok(());
        }
    } else {
        render_human_plan(&plan, dry_run);
        if dry_run {
            println!("\n[dry-run] Envelope sizing plan generated. Downloads skipped.");
            return Ok(());
        }
    }

    println!("\nBeginning parallel streaming download and CAS commitment...");
    let targets = plan.all_targets();
    let mut tasks = tokio::task::JoinSet::new();

    for target in targets {
        let root_c = root.clone();
        let sock_c = socket.clone();
        let repo = target.repo.clone();
        let quant = target.quant.clone();
        let tag_override = format!("{}:{}", target.name, target.tag);

        tasks.spawn(async move {
            run_pull(
                root_c,
                sock_c,
                &repo,
                "gguf",
                Some(&quant),
                Some(&tag_override),
                false,
            )
            .await
        });
    }

    let mut errors = Vec::new();
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => errors.push(e.to_string()),
            Err(e) => errors.push(format!("Task execution failed: {}", e)),
        }
    }

    if !errors.is_empty() {
        return Err(anyhow!(
            "Bootstrap encountered download errors:\n{}",
            errors.join("\n")
        ));
    }

    println!("\nBootstrap completed successfully for family: {}", family);
    Ok(())
}

fn render_human_plan(plan: &BootstrapPlan, dry_run: bool) {
    let b = &plan.budget;
    println!("=== Hardware Envelope & Usable Memory Budgets ===");
    println!("  CPU Cores:        {}", b.cpu_cores);
    println!(
        "  Host RAM:         {:.1} GB avail -> {:.1} GB usable draft budget (50%)",
        b.available_ram_bytes as f64 / 1_073_741_824.0,
        b.ram_budget_gb()
    );
    if b.has_gpu() {
        println!(
            "  GPU VRAM:         {:.1} GB total -> {:.1} GB usable GPU budget (85%)",
            b.total_gpu_vram_bytes as f64 / 1_073_741_824.0,
            b.vram_budget_gb()
        );
    } else {
        println!("  GPU VRAM:         None detected (CPU draft only)");
    }

    println!("\n=== Selected Model Family: {} ===", plan.family);
    println!(
        "  CPU Draft:        {}:{} [{} - {}] (~{:.0} MB)",
        plan.draft.name,
        plan.draft.tag,
        plan.draft.quant,
        plan.draft.file,
        plan.draft.estimated_bytes as f64 / (1024.0 * 1024.0)
    );

    if let Some(p) = &plan.primary {
        println!(
            "  GPU Primary:      {}:{} [{} - {}] (~{:.1} GB)",
            p.name,
            p.tag,
            p.quant,
            p.file,
            p.estimated_bytes as f64 / 1_073_741_824.0
        );
    } else {
        println!("  GPU Primary:      (Skipped: insufficient VRAM)");
    }

    if let Some(v) = &plan.vision_tower {
        println!(
            "  Vision Tower:     {}:{} [{} - {}] (~{:.0} MB)",
            v.name,
            v.tag,
            v.quant,
            v.file,
            v.estimated_bytes as f64 / (1024.0 * 1024.0)
        );
    }

    if let Some(r) = &plan.deep_reasoner {
        println!(
            "  Deep Reasoner:    {}:{} [{} - {}] (~{:.1} GB)",
            r.name,
            r.tag,
            r.quant,
            r.file,
            r.estimated_bytes as f64 / 1_073_741_824.0
        );
    }

    if dry_run {
        println!("\n  Mode:             DRY-RUN (no models will be fetched)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_bootstrap_dry_run_success() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("mock.sock");
        let res = run_bootstrap(dir.path(), &sock, "qwen", true, false).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_run_bootstrap_dry_run_json_output() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("mock.sock");
        let res = run_bootstrap(dir.path(), &sock, "granite", true, true).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_run_bootstrap_invalid_family() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("mock.sock");
        let res = run_bootstrap(dir.path(), &sock, "invalid-family-xyz", true, false).await;
        assert!(res.is_err());
    }
}
