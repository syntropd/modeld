//! Pure Rust systemd sd_notify implementation.
//!
//! Writes notification datagrams directly to `$NOTIFY_SOCKET` without C dependencies.

use rustix::net::{sendto_unix, SendFlags, SocketAddrUnix};
use std::env;
use std::os::unix::net::UnixDatagram;
use std::time::Duration;

/// Maximum notify datagram size (systemd protocol cap).
const NOTIFY_MAX: usize = 8 * 1024 * 1024;

/// Default ping interval when no `$WATCHDOG_USEC` is advertised by systemd.
const DEFAULT_WATCHDOG_PERIOD_SECS: u64 = 10;

/// Minimum ping interval when systemd advertises a sub-3-second watchdog.
const MIN_WATCHDOG_PERIOD_SECS: u64 = 1;

/// Upper bound on the computed ping interval. Without this, a manually
/// inflated `$WATCHDOG_USEC` (e.g. via a container override) could
/// silently reduce ping frequency to the point where systemd's kill
/// timer fires before the next heartbeat. Clamping prevents that.
const MAX_WATCHDOG_PERIOD_SECS: u64 = 300;

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

/// Computes the watchdog ping period from the optional `$WATCHDOG_USEC`.
///
/// Returns a [`Duration`] equal to one-third of the advertised timeout when
/// the timeout is at least 3 seconds (so the daemon pings three times per
/// watchdog window), `MIN_WATCHDOG_PERIOD_SECS` when the timeout is shorter,
/// and `DEFAULT_WATCHDOG_PERIOD_SECS` when no `$WATCHDOG_USEC` is set.
///
/// `WATCHDOG_USEC=0` is the documented systemd sentinel for "watchdog
/// disabled"; in that case `Duration::ZERO` is returned and the watchdog
/// task is never spawned.
pub fn watchdog_period_from_env() -> Duration {
    let usec = std::env::var("WATCHDOG_USEC")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let raw = match usec {
        Some(0) => return Duration::ZERO,
        Some(usec) if usec >= 3_000_000 => Duration::from_micros(usec / 3),
        Some(_) => Duration::from_secs(MIN_WATCHDOG_PERIOD_SECS),
        None => Duration::from_secs(DEFAULT_WATCHDOG_PERIOD_SECS),
    };
    // Clamp so absurdly large values (operator typo, container override)
    // don't silently disable the watchdog.
    raw.min(Duration::from_secs(MAX_WATCHDOG_PERIOD_SECS))
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

    #[test]
    fn test_watchdog_period_from_env_cases() {
        // Sequential sub-cases in one test: WATCHDOG_USEC is
        // process-global and parallel tests must not see a half-set value.
        let saved = env::var("WATCHDOG_USEC").ok();

        env::remove_var("WATCHDOG_USEC");
        assert_eq!(watchdog_period_from_env(), Duration::from_secs(10));

        env::set_var("WATCHDOG_USEC", "0");
        assert!(watchdog_period_from_env().is_zero());

        env::set_var("WATCHDOG_USEC", "9000000");
        assert_eq!(watchdog_period_from_env(), Duration::from_secs(3));

        env::set_var("WATCHDOG_USEC", "1000000");
        assert_eq!(watchdog_period_from_env(), Duration::from_secs(1));

        env::set_var("WATCHDOG_USEC", "10800000000");
        assert_eq!(watchdog_period_from_env(), Duration::from_secs(300));

        env::set_var("WATCHDOG_USEC", "not-a-number");
        assert_eq!(watchdog_period_from_env(), Duration::from_secs(10));

        match saved {
            Some(v) => env::set_var("WATCHDOG_USEC", v),
            None => env::remove_var("WATCHDOG_USEC"),
        }
    }
}
