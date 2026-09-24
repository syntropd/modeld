//! Varlink IPC server subsystem.
//!
//! Implements org.varlink.service and io.syntrop.Model1 over Unix domain sockets.

pub mod model1;
pub mod protocol;
pub mod server;
pub mod service;

pub use model1::{handle_model1_call, ModelServiceContext};
pub use protocol::{VarlinkCall, VarlinkReply};
pub use server::run_varlink_server;
pub use service::handle_service_call;
