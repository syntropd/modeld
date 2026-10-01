//! Hardware topology discovery via Varlink or local system fallback.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

const RPC_TIMEOUT: Duration = Duration::from_millis(500);

/// Status of an accelerator GPU plane reported by inferenced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuPlaneInfo {
    pub id: String,
    pub name: String,
    pub total_memory: u64,
    pub available_memory: u64,
}

/// Consolidated hardware topology used for envelope budget sizing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardwareTopology {
    pub gpu_planes: Vec<GpuPlaneInfo>,
    pub total_ram: u64,
    pub available_ram: u64,
    pub cpu_cores: usize,
}

impl HardwareTopology {
    /// Sum of total GPU VRAM across all physical GPU planes.
    pub fn sum_gpu_vram(&self) -> u64 {
        self.gpu_planes.iter().map(|g| g.total_memory).sum()
    }
}

/// Query hardware topology over the inferenced Varlink domain socket.
pub fn query_varlink_topology(socket_path: &Path) -> Result<HardwareTopology> {
    if !socket_path.exists() {
        return Err(anyhow!("Varlink socket {:?} not present", socket_path));
    }
    let stream = UnixStream::connect(socket_path)?;
    stream.set_read_timeout(Some(RPC_TIMEOUT))?;
    stream.set_write_timeout(Some(RPC_TIMEOUT))?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    let req = serde_json::json!({
        "method": "io.syntrop.Inference1.GetTopology",
        "parameters": {}
    });
    let mut req_bytes = serde_json::to_vec(&req)?;
    req_bytes.push(0);
    writer.write_all(&req_bytes)?;
    writer.flush()?;

    let mut buf = Vec::with_capacity(1024);
    reader.read_until(0, &mut buf)?;
    if buf.last() == Some(&0) {
        buf.pop();
    }
    if buf.is_empty() {
        return Err(anyhow!("Empty Varlink response"));
    }
    let resp: serde_json::Value = serde_json::from_slice(&buf)?;
    if let Some(err) = resp.get("error").and_then(|e| e.as_str()) {
        return Err(anyhow!("Varlink error: {}", err));
    }
    let params = resp
        .get("parameters")
        .ok_or_else(|| anyhow!("Missing parameters in GetTopology reply"))?;

    let mut gpu_planes = Vec::new();
    if let Some(planes) = params.get("planes").and_then(|p| p.as_array()) {
        for p in planes {
            let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let kind = p.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            if kind == "CpuMatrixExtension" || id == "cpu-host" {
                continue;
            }
            let total = p.get("total_memory").and_then(|v| v.as_u64()).unwrap_or(0);
            let avail = p.get("available_memory").and_then(|v| v.as_u64()).unwrap_or(0);
            gpu_planes.push(GpuPlaneInfo {
                id: id.to_string(),
                name: name.to_string(),
                total_memory: total,
                available_memory: avail,
            });
        }
    }

    let total_ram = params.get("total_ram").and_then(|v| v.as_u64()).unwrap_or(0);
    let available_ram = params.get("available_ram").and_then(|v| v.as_u64()).unwrap_or(0);
    let cpu_cores = params.get("cpu_cores").and_then(|v| v.as_u64()).unwrap_or(1) as usize;

    Ok(HardwareTopology {
        gpu_planes,
        total_ram,
        available_ram,
        cpu_cores,
    })
}

/// Fallback to read total and available system memory from /proc/meminfo.
pub fn read_system_ram_fallback() -> (u64, u64, usize) {
    let mut total_kb: u64 = 0;
    let mut avail_kb: u64 = 0;
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = parse_meminfo_kb(line);
            } else if line.starts_with("MemAvailable:") {
                avail_kb = parse_meminfo_kb(line);
            }
        }
    }
    let cores = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1);
    let total = total_kb.saturating_mul(1024);
    let avail = if avail_kb > 0 {
        avail_kb.saturating_mul(1024)
    } else {
        total / 2
    };
    (total, avail, cores)
}

fn parse_meminfo_kb(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

/// Query topology over Varlink or safely fall back to host system resources.
pub fn query_or_fallback_topology(socket_path: &Path) -> HardwareTopology {
    if let Ok(topo) = query_varlink_topology(socket_path) {
        if topo.total_ram > 0 {
            return topo;
        }
    }
    let (total_ram, available_ram, cpu_cores) = read_system_ram_fallback();
    HardwareTopology {
        gpu_planes: Vec::new(),
        total_ram,
        available_ram,
        cpu_cores,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_meminfo_line() {
        assert_eq!(parse_meminfo_kb("MemTotal:       32654324 kB"), 32654324);
        assert_eq!(parse_meminfo_kb("MemAvailable:   16327162 kB"), 16327162);
        assert_eq!(parse_meminfo_kb("InvalidLine"), 0);
    }

    #[test]
    fn test_fallback_reads_sensible_values() {
        let (total, avail, cores) = read_system_ram_fallback();
        assert!(total > 0);
        assert!(avail > 0);
        assert!(cores > 0);
    }

    #[test]
    fn test_sum_gpu_vram() {
        let topo = HardwareTopology {
            gpu_planes: vec![
                GpuPlaneInfo {
                    id: "gpu0".into(),
                    name: "NVIDIA RTX 4090".into(),
                    total_memory: 24 * 1024 * 1024 * 1024,
                    available_memory: 22 * 1024 * 1024 * 1024,
                },
                GpuPlaneInfo {
                    id: "gpu1".into(),
                    name: "NVIDIA RTX 4090".into(),
                    total_memory: 24 * 1024 * 1024 * 1024,
                    available_memory: 22 * 1024 * 1024 * 1024,
                },
            ],
            total_ram: 64 * 1024 * 1024 * 1024,
            available_ram: 48 * 1024 * 1024 * 1024,
            cpu_cores: 32,
        };
        assert_eq!(topo.sum_gpu_vram(), 48 * 1024 * 1024 * 1024);
    }
}
