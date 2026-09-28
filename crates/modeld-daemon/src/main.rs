//! Entrypoint for modeld: unprivileged system model storage broker.

use modeld_core::cas::{CasStore, EvictionManager, TagRegistry};
use modeld_core::config::{ModeldConfig, DEFAULT_CONFIG_PATH};
use modeld_daemon::activation::{bind_standalone, parse_listen_fds};
use modeld_daemon::auth::TrustedGroupConfig;
use modeld_daemon::fd_server::run_fd_handoff_server;
use modeld_daemon::notify::{
    notify_ready, notify_status, notify_stopping, notify_watchdog, watchdog_period_from_env,
};
use modeld_daemon::varlink::{run_varlink_server, ModelServiceContext};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

/// Derives the FD handoff socket path from the Varlink socket path.
///
/// The handoff socket always lives next to the Varlink socket so both
/// inherit the same directory permissions and mount namespace.
fn fd_socket_path(socket_path: &std::path::Path) -> std::path::PathBuf {
    socket_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/run/syntrop"))
        .join("modeld-fd.sock")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging to standard error / journald
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!(
        "Starting syntropd modeld daemon v{}",
        env!("CARGO_PKG_VERSION")
    );

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

    let fd_socket_path = fd_socket_path(&config.socket_path);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fd_socket_path_sits_next_to_varlink_socket() {
        let derived = fd_socket_path(std::path::Path::new("/run/syntrop/io.syntrop.Model1"));
        assert_eq!(
            derived,
            std::path::PathBuf::from("/run/syntrop/modeld-fd.sock")
        );
    }

    #[test]
    fn test_fd_socket_path_bare_name_stays_relative() {
        let derived = fd_socket_path(std::path::Path::new("modeld.sock"));
        assert_eq!(derived, std::path::PathBuf::from("modeld-fd.sock"));
    }
}
