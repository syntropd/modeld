//! Handler for `modelctl list` subcommand.

use crate::client::VarlinkClient;
use anyhow::Result;

/// Lists all registered models in table or JSON format.
pub fn run_list(client: &mut VarlinkClient, json_output: bool) -> Result<()> {
    let params = client.call("io.syntrop.Model1.List", None)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&params)?);
        return Ok(());
    }

    let models = params
        .get("models")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if models.is_empty() {
        println!("No models registered in CAS store.");
        return Ok(());
    }

    println!("{:<24} {:<12} {:<10} {:<64}", "ID", "SIZE", "PINNED", "DIGEST");
    println!("{:-<24} {:-<12} {:-<10} {:-<64}", "", "", "", "");

    for m in models {
        let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("-");
        let size = m.get("size_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
        let pinned = m.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);
        let digest = m.get("digest").and_then(|v| v.as_str()).unwrap_or("-");

        println!(
            "{:<24} {:<12} {:<10} {:<64}",
            id,
            format_bytes(size),
            if pinned { "yes" } else { "no" },
            digest
        );
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;

    if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.0} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}
