//! Library interface for modelctl command-line tooling.

pub mod client;
pub mod cmd;

pub use client::VarlinkClient;
pub use cmd::{run_completions, run_import, run_inspect, run_list, run_pin, run_prune, run_unpin};
