//! Varlink IPC server subsystem.
//!
//! Implements org.varlink.service and io.syntrop.Model1 over Unix domain sockets.

pub mod model1;
pub mod protocol;
pub mod server;
pub mod service;

pub use model1::{handle_model1_call, ModelServiceContext};
pub use protocol::{VarlinkCall, VarlinkReply};
pub use server::{handle_varlink_client, run_varlink_server, MAX_MSG_BYTES};
pub use service::handle_service_call;
