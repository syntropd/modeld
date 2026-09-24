//! Subcommand dispatchers for modelctl CLI.

pub mod completions;
pub mod import;
pub mod inspect;
pub mod list;
pub mod pin;
pub mod prune;

pub use completions::run_completions;
pub use import::run_import;
pub use inspect::run_inspect;
pub use list::run_list;
pub use pin::{run_pin, run_unpin};
pub use prune::run_prune;
