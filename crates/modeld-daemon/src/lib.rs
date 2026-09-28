//! Daemon implementation for modeld.
//!
//! Provides systemd socket activation, sd_notify heartbeats,
//! Varlink IPC server, and SCM_RIGHTS descriptor handoff.

pub mod activation;
pub mod auth;
pub mod fd_server;
pub mod notify;
pub mod varlink;

pub use activation::{bind_standalone, parse_listen_fds, ActivatedSockets, SD_LISTEN_FDS_START};
pub use auth::TrustedGroupConfig;
pub use fd_server::run_fd_handoff_server;
pub use notify::{
    notify_ready, notify_status, notify_stopping, notify_watchdog, send_notify,
    watchdog_period_from_env,
};
pub use varlink::{
    handle_model1_call, handle_service_call, run_varlink_server, ModelServiceContext, VarlinkCall,
    VarlinkReply,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_facade_reexports_resolve() {
        // Every root re-export must keep resolving: the daemon binary
        // and downstream crates import through this facade.
        let _ = bind_standalone;
        let _ = parse_listen_fds;
        let _ = std::mem::size_of::<ActivatedSockets>();
        let _ = SD_LISTEN_FDS_START;
        let _ = TrustedGroupConfig::from_env_or_default;
        let _ = run_fd_handoff_server;
        let _ = notify_ready;
        let _ = notify_status;
        let _ = notify_stopping;
        let _ = notify_watchdog;
        let _ = send_notify;
        let _ = watchdog_period_from_env;
        let _ = handle_model1_call;
        let _ = handle_service_call;
        let _ = run_varlink_server;
        let _ = std::mem::size_of::<ModelServiceContext>();
        let _ = std::mem::size_of::<VarlinkCall>();
        let _ = VarlinkReply::ok;
    }
}
