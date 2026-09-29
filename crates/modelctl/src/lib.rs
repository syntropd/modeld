//! Library interface for modelctl command-line tooling.

pub mod client;
pub mod cmd;

pub use client::VarlinkClient;
pub use cmd::run_completions;
pub use cmd::run_import;
pub use cmd::run_inspect;
pub use cmd::run_list;
pub use cmd::run_pin;
pub use cmd::run_prune;
pub use cmd::run_pull;
pub use cmd::run_unpin;
pub use reqwest;
