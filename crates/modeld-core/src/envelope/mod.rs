//! Hardware envelope sizing engine and curated model family profiles.
//!
//! Provides hardware discovery over Varlink `Inference1.GetTopology`,
//! memory budget calculation, and model selection.

pub mod budget;
pub mod family;
pub mod plan;
pub mod topology;

pub use budget::MemoryBudget;
pub use family::BootstrapPlan;
pub use family::ModelFamily;
pub use family::ModelRole;
pub use family::ModelTarget;
pub use plan::plan_family;
pub use topology::query_or_fallback_topology;
pub use topology::query_varlink_topology;
pub use topology::read_system_ram_fallback;
pub use topology::GpuPlaneInfo;
pub use topology::HardwareTopology;
