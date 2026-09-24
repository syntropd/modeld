//! Entrypoint for modeld: unprivileged system model storage broker.

use modeld_core::cas::{CasStore, EvictionManager, TagRegistry};
use modeld_core::config::{ModeldConfig, DEFAULT_CONFIG_PATH};
use modeld_daemon::activation::parse_listen_fds;
use modeld_daemon::fd_server::run_fd_handoff_server;
use modeld_daemon::notify::{notify_ready, notify_status, notify_stopping, notify_watchdog};
use modeld_daemon::varlink::{run_varlink_server, ModelServiceContext};
use std::fs;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging to standard error / journald
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!("Starting syntropd modeld daemon v0.1.0");

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
            if path.exists() {
                let _ = fs::remove_file(path);
            }
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let l = UnixListener::bind(path)?;
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
            if fd_socket_path.exists() {
                let _ = fs::remove_file(&fd_socket_path);
            }
            let l = UnixListener::bind(&fd_socket_path)?;
            info!("Bound standalone FD handoff socket at {:?}", fd_socket_path);
            l
        }
    };

    // Spawn Varlink protocol listener
    tokio::spawn(async move {
        if let Err(e) = run_varlink_server(varlink_listener, ctx).await {
            error!("Varlink server exited with error: {}", e);
        }
    });

    // Spawn FD handoff listener
    let cas_fd = cas.clone();
    let tags_fd = tags.clone();
    tokio::spawn(async move {
        if let Err(e) = run_fd_handoff_server(fd_listener, cas_fd, tags_fd).await {
            error!("FD handoff server exited with error: {}", e);
        }
    });

    // Spawn periodic systemd watchdog heartbeat
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            notify_watchdog();
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

    notify_status("Stopping modeld daemon");
    notify_stopping();
    info!("modeld terminated cleanly");

    Ok(())
}
