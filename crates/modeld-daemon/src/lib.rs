//! Daemon implementation for modeld.
//!
//! Provides systemd socket activation, sd_notify heartbeats,
//! Varlink IPC server, and SCM_RIGHTS descriptor handoff.

pub mod activation;
pub mod auth;
pub mod fd_server;
pub mod notify;
pub mod varlink;

pub use activation::{parse_listen_fds, ActivatedSockets, SD_LISTEN_FDS_START};
pub use auth::TrustedGroupConfig;
pub use fd_server::run_fd_handoff_server;
pub use notify::{notify_ready, notify_status, notify_stopping, notify_watchdog, send_notify};
pub use varlink::{
    handle_model1_call, handle_service_call, run_varlink_server, ModelServiceContext,
    VarlinkCall, VarlinkReply,
};
