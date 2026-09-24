//! Systemd socket activation implementation in 100% pure Rust.
//!
//! Adopts pre-bound file descriptors passed via environment variables
//! `LISTEN_FDS` and `LISTEN_PID` per systemd socket activation specification.

use std::env;
use std::os::unix::io::{FromRawFd, RawFd};
use tokio::net::UnixListener;

/// The starting file descriptor index passed by systemd (FD 3).
pub const SD_LISTEN_FDS_START: RawFd = 3;

/// Adopted systemd sockets for modeld.
#[derive(Debug)]
pub struct ActivatedSockets {
    /// Primary Varlink IPC Unix listener (FD 3).
    pub varlink_listener: Option<UnixListener>,
    /// SCM_RIGHTS file descriptor handoff Unix listener (FD 4).
    pub fd_handoff_listener: Option<UnixListener>,
}

/// Parses systemd environment variables and adopts pre-bound Unix sockets.
pub fn parse_listen_fds() -> ActivatedSockets {
    let pid_matches = match env::var("LISTEN_PID") {
        Ok(pid_str) => pid_str.parse::<u32>().map(|p| p == std::process::id()).unwrap_or(false),
        Err(_) => false,
    };

    if !pid_matches {
        return ActivatedSockets {
            varlink_listener: None,
            fd_handoff_listener: None,
        };
    }

    let count: usize = env::var("LISTEN_FDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let varlink_listener = if count >= 1 {
        adopt_unix_listener(SD_LISTEN_FDS_START)
    } else {
        None
    };

    let fd_handoff_listener = if count >= 2 {
        adopt_unix_listener(SD_LISTEN_FDS_START + 1)
    } else {
        None
    };

    ActivatedSockets {
        varlink_listener,
        fd_handoff_listener,
    }
}

fn adopt_unix_listener(fd: RawFd) -> Option<UnixListener> {
    unsafe {
        let std_listener = std::os::unix::net::UnixListener::from_raw_fd(fd);
        let _ = std_listener.set_nonblocking(true);
        UnixListener::from_std(std_listener).ok()
    }
}
