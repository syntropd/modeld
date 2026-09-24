//! Entrypoint for modeld: unprivileged system model storage broker.

use modeld_core::cas::{CasStore, EvictionManager, TagRegistry};
use modeld_core::config::{ModeldConfig, DEFAULT_CONFIG_PATH};
use modeld_daemon::activation::parse_listen_fds;
use modeld_daemon::auth::TrustedGroupConfig;
use modeld_daemon::fd_server::run_fd_handoff_server;
use modeld_daemon::notify::{notify_ready, notify_status, notify_stopping, notify_watchdog};
use modeld_daemon::varlink::{run_varlink_server, ModelServiceContext};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Default ping interval when no `$WATCHDOG_USEC` is advertised by systemd.
const DEFAULT_WATCHDOG_PERIOD_SECS: u64 = 10;

/// Minimum ping interval when systemd advertises a sub-3-second watchdog.
const MIN_WATCHDOG_PERIOD_SECS: u64 = 1;

/// Upper bound on the computed ping interval. Without this, a manually
/// inflated `$WATCHDOG_USEC` (e.g. via a container override) could
/// silently reduce ping frequency to the point where systemd's kill
/// timer fires before the next heartbeat. Clamping prevents that.
const MAX_WATCHDOG_PERIOD_SECS: u64 = 300;

/// Socket file permissions applied to standalone-bound sockets so that
/// unprivileged members of the `modeld` group can connect when the daemon
/// is launched outside systemd.
const SOCKET_MODE: u32 = 0o660;

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
fn watchdog_period_from_env() -> Duration {
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
fn bind_standalone(path: &std::path::Path) -> anyhow::Result<UnixListener> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let listener = UnixListener::bind(path)?;
    if let Err(e) =
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))
    {
        // Dropping `listener` closes the FD, but the inode remains on disk.
        // Remove it explicitly so the next start can re-bind cleanly.
        drop(listener);
        let _ = std::fs::remove_file(path);
        return Err(e.into());
    }
    Ok(listener)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging to standard error / journald
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Starting syntropd modeld daemon v{}", env!("CARGO_PKG_VERSION"));

    let config = ModeldConfig::load_or_default(DEFAULT_CONFIG_PATH)
        .unwrap_or_else(|_| ModeldConfig::default());

    // Initialize core storage subsystems
    let cas = Arc::new(CasStore::new(&config.storage_path)?);
    let tags = Arc::new(TagRegistry::new(&config.storage_path)?);
    let eviction = Arc::new(EvictionManager::new(&config.storage_path)?);

    let ctx = ModelServiceContext {
        cas: cas.clone(),
        tags: tags.clone(),
        eviction: eviction.clone(),
    };

    // Check for systemd socket activation or bind standalone sockets
    let activated = parse_listen_fds();

    let varlink_listener = match activated.varlink_listener {
        Some(l) => {
            info!("Adopted pre-bound Varlink socket from systemd (FD 3)");
            l
        }
        None => {
            let path = &config.socket_path;
            let l = bind_standalone(path)?;
            info!("Bound standalone Varlink socket at {:?}", path);
            l
        }
    };

    let fd_socket_path = config
        .socket_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/run/syntrop"))
        .join("modeld-fd.sock");

    let fd_listener = match activated.fd_handoff_listener {
        Some(l) => {
            info!("Adopted pre-bound FD handoff socket from systemd (FD 4)");
            l
        }
        None => {
            let l = bind_standalone(&fd_socket_path)?;
            info!("Bound standalone FD handoff socket at {:?}", fd_socket_path);
            l
        }
    };

    // Shared shutdown signal propagated to every spawned listener task so
    // SIGTERM drains in-flight connections cleanly instead of dropping
    // them mid-`accept`.
    let (shutdown_tx, shutdown_rx) = watch::channel::<bool>(false);

    // Spawn Varlink protocol listener. Capture the JoinHandle so main can
    // await it before notifying systemd of STOPPING=1.
    let varlink_handle = {
        let ctx = ctx.clone();
        let shutdown_rx = shutdown_rx.clone();
        tokio::spawn(async move {
            if let Err(e) = run_varlink_server(varlink_listener, ctx, shutdown_rx).await {
                error!("Varlink server exited with error: {}", e);
            }
        })
    };

    // Spawn FD handoff listener.
    let cas_fd = cas.clone();
    let tags_fd = tags.clone();
    let trusted_group = TrustedGroupConfig::from_env_or_default();
    let fd_shutdown_rx = shutdown_rx.clone();
    let fd_handle = tokio::spawn(async move {
        if let Err(e) =
            run_fd_handoff_server(fd_listener, cas_fd, tags_fd, trusted_group, fd_shutdown_rx).await
        {
            error!("FD handoff server exited with error: {}", e);
        }
    });

    // Spawn periodic systemd watchdog heartbeat honoring $WATCHDOG_USEC.
    // A zero period (from `WATCHDOG_USEC=0`) disables pinging entirely.
    let period = watchdog_period_from_env();
    let watchdog_shutdown_rx = shutdown_rx.clone();
    let watchdog_handle = tokio::spawn(async move {
        if period.is_zero() {
            return;
        }
        let mut interval = tokio::time::interval(period);
        let mut shutdown = watchdog_shutdown_rx;
        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        return;
                    }
                }
                _ = interval.tick() => {
                    notify_watchdog();
                }
            }
        }
    });

    // Report readiness to systemd
    notify_status("Operating normally: CAS store active");
    notify_ready();
    info!("modeld initialized and ready for requests");

    // Wait for termination signal
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    tokio::select! {
        _ = sigterm.recv() => info!("Received SIGTERM, initiating clean shutdown"),
        _ = sigint.recv() => info!("Received SIGINT, initiating clean shutdown"),
    }

    // Tell the listener tasks to drain; await each JoinHandle with a
    // bounded timeout so a stuck client cannot prevent shutdown.
    let _ = shutdown_tx.send(true);

    const SHUTDOWN_DRAIN: Duration = Duration::from_secs(2);
    if let Err(_e) = tokio::time::timeout(SHUTDOWN_DRAIN, async {
        let _ = varlink_handle.await;
        let _ = fd_handle.await;
        let _ = watchdog_handle.await;
    })
    .await
    {
        warn!(
            "Listener drain exceeded {:?}; proceeding with STOPPING=1 anyway",
            SHUTDOWN_DRAIN
        );
    }

    notify_status("Stopping modeld daemon");
    notify_stopping();
    info!("modeld terminated cleanly");

    Ok(())
}
