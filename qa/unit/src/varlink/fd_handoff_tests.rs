//! 1:1 unit QA tests for the SCM_RIGHTS FD handoff server.
//!
//! Spins a real `run_fd_handoff_server` on a temporary socket, stores
//! one blob, and proves an authorized peer receives a sealed FD whose
//! content matches while unauthorized and unknown-model peers get
//! protocol errors instead.

use modeld_core::cas::{CasStore, TagRegistry};
use modeld_core::descriptor::recv_fd_scm_rights;
use modeld_daemon::auth::TrustedGroupConfig;
use modeld_daemon::fd_server::run_fd_handoff_server;
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::net::UnixListener;
use tokio::sync::watch;

const PAYLOAD: &[u8] = b"fd handoff sealed content check";

/// Stored blob plus server handles for one handoff test.
struct Harness {
    sock: std::path::PathBuf,
    digest: String,
    shutdown_tx: watch::Sender<bool>,
    server: tokio::task::JoinHandle<anyhow::Result<()>>,
    _dir: tempfile::TempDir,
}

async fn start_server(policy: TrustedGroupConfig) -> Harness {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let (digest, _) = cas.store_blob(Cursor::new(PAYLOAD), None).unwrap();
    tags.set_tag("wisp", "1b", &digest).unwrap();

    let sock = dir.path().join("modeld-fd.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let (shutdown_tx, shutdown_rx) = watch::channel::<bool>(false);
    let server = tokio::spawn(run_fd_handoff_server(
        listener,
        cas,
        tags,
        policy,
        shutdown_rx,
    ));
    Harness {
        sock,
        digest,
        shutdown_tx,
        server,
        _dir: dir,
    }
}

impl Harness {
    async fn stop(self) {
        let _ = self.shutdown_tx.send(true);
        tokio::time::timeout(Duration::from_secs(5), self.server)
            .await
            .expect("server shutdown timeout")
            .expect("server task join")
            .expect("server run");
    }
}

fn connect(sock: &std::path::Path) -> UnixStream {
    let stream = UnixStream::connect(sock).expect("fd client connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    stream
}

fn request_id(stream: &mut UnixStream, id: &str) {
    let mut req = serde_json::to_vec(&serde_json::json!({ "id": id })).unwrap();
    req.push(0x00);
    stream.write_all(&req).expect("fd request write");
    stream.flush().expect("fd request flush");
}

// Multi-threaded runtime: the test thread performs blocking client
// I/O while the spawned server task must keep running. On a
// current-thread runtime the two would deadlock.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fd_handoff_delivers_sealed_content() {
    let egid = rustix::process::getegid().as_raw();
    let policy = TrustedGroupConfig {
        trusted_group: "test".to_string(),
        trusted_gid_override: Some(egid),
    };
    let harness = start_server(policy).await;

    let mut stream = connect(&harness.sock);
    request_id(&mut stream, "wisp:1b");

    let mut buf = [0u8; 4096];
    let (n, fd) = recv_fd_scm_rights(&stream, &mut buf).expect("fd recv");
    let reply: serde_json::Value = serde_json::from_slice(&buf[..n]).expect("reply json");
    assert_eq!(reply.get("status").and_then(|v| v.as_str()), Some("ok"));
    assert_eq!(
        reply.get("digest").and_then(|v| v.as_str()),
        Some(harness.digest.as_str())
    );

    let mut file = File::from(fd.expect("server must send an FD"));
    let mut content = Vec::new();
    file.read_to_end(&mut content).expect("fd read");
    assert_eq!(content, PAYLOAD);

    harness.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fd_handoff_unknown_model_has_no_fd() {
    let egid = rustix::process::getegid().as_raw();
    let policy = TrustedGroupConfig {
        trusted_group: "test".to_string(),
        trusted_gid_override: Some(egid),
    };
    let harness = start_server(policy).await;

    let mut stream = connect(&harness.sock);
    request_id(&mut stream, "ghost:latest");

    let mut buf = [0u8; 4096];
    let (n, fd) = recv_fd_scm_rights(&stream, &mut buf).expect("fd recv");
    assert!(fd.is_none(), "unknown model must not yield an FD");
    let reply: serde_json::Value = serde_json::from_slice(&buf[..n]).expect("reply json");
    assert!(
        reply.get("error").is_some(),
        "unknown model must be an error reply: {reply}"
    );

    harness.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_fd_handoff_rejects_untrusted_gid() {
    // Root bypasses the GID check by policy; on a root test runner the
    // same request must succeed instead of being rejected.
    let is_root = rustix::process::geteuid().is_root();
    let egid = rustix::process::getegid().as_raw();
    let policy = TrustedGroupConfig {
        trusted_group: "test".to_string(),
        trusted_gid_override: Some(egid.wrapping_add(1)),
    };
    let harness = start_server(policy).await;

    let mut stream = connect(&harness.sock);
    request_id(&mut stream, "wisp:1b");

    let mut buf = [0u8; 4096];
    let (n, fd) = recv_fd_scm_rights(&stream, &mut buf).expect("fd recv");
    let reply: serde_json::Value = serde_json::from_slice(&buf[..n]).expect("reply json");
    if is_root {
        assert_eq!(reply.get("status").and_then(|v| v.as_str()), Some("ok"));
        assert!(fd.is_some(), "root must receive the FD");
    } else {
        assert!(fd.is_none(), "untrusted gid must not yield an FD");
        assert_eq!(
            reply.get("error").and_then(|v| v.as_str()),
            Some("io.syntrop.Model1.PermissionDenied")
        );
    }

    harness.stop().await;
}
