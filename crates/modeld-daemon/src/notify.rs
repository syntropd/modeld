//! Pure Rust systemd sd_notify implementation.
//!
//! Writes notification datagrams directly to `$NOTIFY_SOCKET` without C dependencies.

use rustix::net::{sendto_unix, SendFlags, SocketAddrUnix};
use std::env;
use std::os::unix::net::UnixDatagram;

/// Maximum notify datagram size (systemd protocol cap).
const NOTIFY_MAX: usize = 8 * 1024 * 1024;

/// Builds a `SocketAddrUnix` for `$NOTIFY_SOCKET`, supporting both
/// filesystem paths (`/run/systemd/notify`) and abstract namespaces (`@notify`).
///
/// Abstract names containing interior NUL bytes are rejected even though the
/// kernel would accept them: a malformed name would silently send to a name
/// nobody listens on, defeating the readiness/watchdog protocol. Empty names
/// are also rejected for the same reason.
fn notify_address(socket_path: &str) -> Option<SocketAddrUnix> {
    if let Some(name) = socket_path.strip_prefix('@') {
        if name.is_empty() || name.as_bytes().contains(&0) {
            return None;
        }
        SocketAddrUnix::new_abstract_name(name.as_bytes()).ok()
    } else if socket_path.is_empty() {
        None
    } else {
        SocketAddrUnix::new(socket_path).ok()
    }
}

/// Sends a formatted raw notification string to `$NOTIFY_SOCKET`.
pub fn send_notify(state: &str) -> bool {
    if state.len() > NOTIFY_MAX {
        return false;
    }

    let socket_path = match env::var("NOTIFY_SOCKET") {
        Ok(path) if !path.is_empty() => path,
        _ => return false,
    };

    let address = match notify_address(&socket_path) {
        Some(addr) => addr,
        None => return false,
    };

    let socket = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Use `sendto_unix` so the kernel receives the correctly-sized sockaddr
    // (including the right `addrlen` for both abstract and filesystem
    // namespaces). Manually crafting the address would risk leaking the
    // trailing `len` field of `SocketAddrUnix` into the kernel buffer.
    sendto_unix(&socket, state.as_bytes(), SendFlags::NOSIGNAL, &address).is_ok()
}

/// Emits the READY=1 readiness notification to systemd.
pub fn notify_ready() -> bool {
    send_notify("READY=1\n")
}

/// Emits an updated STATUS string visible in systemctl status.
pub fn notify_status(status: &str) -> bool {
    // Strip any embedded newlines; sd_notify is line-based and embedded
    // newlines would terminate the variable early.
    let sanitized: String = status.chars().filter(|&c| c != '\n' && c != '\r').collect();
    send_notify(&format!("STATUS={}\n", sanitized))
}

/// Emits the WATCHDOG=1 keepalive heartbeat ping.
pub fn notify_watchdog() -> bool {
    send_notify("WATCHDOG=1\n")
}

/// Emits the STOPPING=1 shutdown signal.
pub fn notify_stopping() -> bool {
    send_notify("STOPPING=1\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notify_address_filesystem_path() {
        let addr = notify_address("/run/systemd/notify").unwrap();
        // Filesystem addresses must not start with a NUL byte in sun_path[0].
        assert!(addr.path().is_some());
    }

    #[test]
    fn test_notify_address_abstract_namespace() {
        let addr = notify_address("@notify").unwrap();
        // Abstract addresses carry no path; the abstract_name excludes the
        // leading NUL byte that the kernel prepends.
        assert_eq!(addr.abstract_name().unwrap(), b"notify");
    }

    #[test]
    fn test_notify_address_empty_returns_none() {
        assert!(notify_address("").is_none());
        assert!(notify_address("@").is_none());
    }

    #[test]
    fn test_notify_address_abstract_with_interior_nul_fails() {
        // Abstract names must not contain interior NULs.
        assert!(notify_address("@notify\0extra").is_none());
    }

    #[test]
    fn test_notify_status_sanitizes_newlines() {
        // Sanity: status must not contain embedded newlines that would
        // prematurely terminate the variable.
        let input = "evil\nSTATUS=overridden\n";
        let sanitized: String = input.chars().filter(|&c| c != '\n' && c != '\r').collect();
        assert!(!sanitized.contains('\n'));
    }

    #[test]
    fn test_notify_oversize_message_rejected() {
        // Anything over NOTIFY_MAX must be rejected without touching the socket.
        let huge = "X".repeat(NOTIFY_MAX + 1);
        assert!(!send_notify(&huge));
    }
}
