//! Memory budget computation based on hardware envelope.

use super::topology::HardwareTopology;
use serde::{Deserialize, Serialize};

/// Calculated memory budgets for model allocation and quantization sizing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryBudget {
    /// Usable VRAM across all GPUs (sum(VRAM_GPU) * 0.85).
    pub vram_budget_bytes: u64,
    /// Usable host RAM for CPU draft models (available_ram * 0.50).
    pub ram_budget_bytes: u64,
    /// Raw total GPU VRAM discovered.
    pub total_gpu_vram_bytes: u64,
    /// Raw available host RAM discovered.
    pub available_ram_bytes: u64,
    /// Detected CPU core count.
    pub cpu_cores: usize,
}

impl MemoryBudget {
    /// Compute memory budget from detected hardware topology.
    ///
    /// Rules:
    /// - `VRAM_budget = sum(VRAM_GPU) * 0.85`
    /// - `RAM_budget = available_ram * 0.50`
    pub fn from_topology(topo: &HardwareTopology) -> Self {
        let total_gpu_vram = topo.sum_gpu_vram();
        let vram_budget = (total_gpu_vram as f64 * 0.85) as u64;
        let ram_budget = (topo.available_ram as f64 * 0.50) as u64;

        Self {
            vram_budget_bytes: vram_budget,
            ram_budget_bytes: ram_budget,
            total_gpu_vram_bytes: total_gpu_vram,
            available_ram_bytes: topo.available_ram,
            cpu_cores: topo.cpu_cores,
        }
    }

    /// Whether this system has dedicated GPU acceleration available.
    pub fn has_gpu(&self) -> bool {
        self.vram_budget_bytes > 0
    }

    /// Usable VRAM budget in gigabytes.
    pub fn vram_budget_gb(&self) -> f64 {
        self.vram_budget_bytes as f64 / 1_073_741_824.0
    }

    /// Usable RAM budget in gigabytes.
    pub fn ram_budget_gb(&self) -> f64 {
        self.ram_budget_bytes as f64 / 1_073_741_824.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::topology::GpuPlaneInfo;

    #[test]
    fn test_budget_with_single_gpu() {
        let topo = HardwareTopology {
            gpu_planes: vec![GpuPlaneInfo {
                id: "gpu0".into(),
                name: "RTX 4090".into(),
                kind: "DiscreteGpu".into(),
                total_memory: 24 * 1024 * 1024 * 1024,
                available_memory: 24 * 1024 * 1024 * 1024,
            }],
            total_ram: 32 * 1024 * 1024 * 1024,
            available_ram: 16 * 1024 * 1024 * 1024,
            cpu_cores: 16,
        };

        let budget = MemoryBudget::from_topology(&topo);
        assert!(budget.has_gpu());
        let expected_vram = (24.0 * 1024.0 * 1024.0 * 1024.0 * 0.85) as u64;
        let expected_ram = (16.0 * 1024.0 * 1024.0 * 1024.0 * 0.50) as u64;
        assert_eq!(budget.vram_budget_bytes, expected_vram);
        assert_eq!(budget.ram_budget_bytes, expected_ram);
        assert!((budget.vram_budget_gb() - 20.4).abs() < 1e-2);
        assert!((budget.ram_budget_gb() - 8.0).abs() < 1e-2);
    }

    #[test]
    fn test_budget_cpu_only() {
        let topo = HardwareTopology {
            gpu_planes: Vec::new(),
            total_ram: 16 * 1024 * 1024 * 1024,
            available_ram: 8 * 1024 * 1024 * 1024,
            cpu_cores: 8,
        };

        let budget = MemoryBudget::from_topology(&topo);
        assert!(!budget.has_gpu());
        assert_eq!(budget.vram_budget_bytes, 0);
        assert_eq!(budget.ram_budget_bytes, 4 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_budget_multi_gpu() {
        let topo = HardwareTopology {
            gpu_planes: vec![
                GpuPlaneInfo {
                    id: "gpu0".into(),
                    name: "RTX 3090".into(),
                    kind: "DiscreteGpu".into(),
                    total_memory: 24 * 1024 * 1024 * 1024,
                    available_memory: 24 * 1024 * 1024 * 1024,
                },
                GpuPlaneInfo {
                    id: "gpu1".into(),
                    name: "RTX 3090".into(),
                    kind: "DiscreteGpu".into(),
                    total_memory: 24 * 1024 * 1024 * 1024,
                    available_memory: 24 * 1024 * 1024 * 1024,
                },
            ],
            total_ram: 128 * 1024 * 1024 * 1024,
            available_ram: 96 * 1024 * 1024 * 1024,
            cpu_cores: 64,
        };

        let budget = MemoryBudget::from_topology(&topo);
        let expected_vram = (48.0 * 1024.0 * 1024.0 * 1024.0 * 0.85) as u64;
        assert_eq!(budget.vram_budget_bytes, expected_vram);
        assert_eq!(budget.ram_budget_bytes, 48 * 1024 * 1024 * 1024);
    }
}
