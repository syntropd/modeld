//! Systemd socket activation implementation in 100% pure Rust.
//!
//! Adopts pre-bound file descriptors passed via environment variables
//! `LISTEN_FDS` and `LISTEN_PID` per systemd socket activation specification.

use std::env;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::{FromRawFd, RawFd};
use tokio::net::UnixListener;

/// The starting file descriptor index passed by systemd (FD 3).
pub const SD_LISTEN_FDS_START: RawFd = 3;

/// Socket file permissions applied to standalone-bound sockets so that
/// unprivileged members of the `modeld` group can connect when the daemon
/// is launched outside systemd.
const SOCKET_MODE: u32 = 0o660;

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
        Ok(pid_str) => pid_str
            .parse::<u32>()
            .map(|p| p == std::process::id())
            .unwrap_or(false),
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

/// Binds a Unix domain socket for the standalone (non-systemd) launch path.
///
/// Drops the historical `exists()`/`remove_file()` race in favor of letting
/// `bind()` fail if a stale socket file is already present (the operator
/// can clean it up). After a successful bind the socket is chmod'd to
/// `SOCKET_MODE` so that group `modeld` can connect.
///
/// If `chmod` fails after the bind (e.g. EROFS after a remount), the
/// socket inode is removed before returning the error so a retry
/// doesn't hit `EADDRINUSE` with the wrong mode.
pub fn bind_standalone(path: &std::path::Path) -> anyhow::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(path)?;
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE)) {
        // Dropping `listener` closes the FD, but the inode remains on disk.
        // Remove it explicitly so the next start can re-bind cleanly.
        drop(listener);
        let _ = std::fs::remove_file(path);
        return Err(e.into());
    }
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_listen_fds_without_activation_returns_none() {
        // Sequential sub-cases in one test: LISTEN_* is process-global
        // and parallel tests must not observe a half-set pair.
        let saved_pid = env::var("LISTEN_PID").ok();
        let saved_fds = env::var("LISTEN_FDS").ok();

        // No activation variables at all.
        env::remove_var("LISTEN_PID");
        env::remove_var("LISTEN_FDS");
        let sockets = parse_listen_fds();
        assert!(sockets.varlink_listener.is_none());
        assert!(sockets.fd_handoff_listener.is_none());

        // PID mismatch (PID 0 never exists) and unparsable values.
        for pid in ["0", "not-a-number"] {
            env::set_var("LISTEN_PID", pid);
            env::set_var("LISTEN_FDS", "2");
            let sockets = parse_listen_fds();
            assert!(sockets.varlink_listener.is_none());
            assert!(sockets.fd_handoff_listener.is_none());
        }

        // Own PID but zero FDs: nothing adopted, no FD touched.
        env::set_var("LISTEN_PID", std::process::id().to_string());
        env::set_var("LISTEN_FDS", "0");
        let sockets = parse_listen_fds();
        assert!(sockets.varlink_listener.is_none());
        assert!(sockets.fd_handoff_listener.is_none());

        match saved_pid {
            Some(v) => env::set_var("LISTEN_PID", v),
            None => env::remove_var("LISTEN_PID"),
        }
        match saved_fds {
            Some(v) => env::set_var("LISTEN_FDS", v),
            None => env::remove_var("LISTEN_FDS"),
        }
    }

    #[tokio::test]
    async fn test_bind_standalone_creates_group_socket() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("sub").join("modeld.sock");
        let _listener = bind_standalone(&sock).expect("bind");
        let mode = std::fs::metadata(&sock).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, SOCKET_MODE);
    }

    #[tokio::test]
    async fn test_bind_standalone_existing_path_fails() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("busy.sock");
        let _first = bind_standalone(&sock).expect("first bind");
        assert!(bind_standalone(&sock).is_err());
    }
}
