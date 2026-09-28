//! 1:1 unit QA tests for the Varlink connection loop and buffer cap.
//!
//! Drives `MAX_MSG_BYTES` overflow directly through a `UnixStream::pair`
//! so the test does not depend on a daemon listener being bound. The peer
//! sends more than the cap without a NUL terminator; the daemon is
//! expected to reply with a `org.varlink.service.ProtocolError` and close.

use modeld_core::cas::{CasStore, EvictionManager, TagRegistry};
use modeld_daemon::varlink::{handle_varlink_client, ModelServiceContext, MAX_MSG_BYTES};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::watch;

#[tokio::test]
async fn test_handle_varlink_client_overflows_with_protocol_error() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());
    let ctx = ModelServiceContext { cas, tags, eviction };

    let (server, peer) = UnixStream::pair().expect("socketpair");
    server.set_nonblocking(true).expect("set nonblocking");
    let server = tokio::net::UnixStream::from_std(server).expect("from_std");

    let (_tx, rx) = watch::channel::<bool>(false);
    let handle = tokio::spawn(async move {
        let _ = handle_varlink_client(server, ctx, rx).await;
    });

    // Clone the peer so the reader thread can read the reply while the
    // writer thread keeps pushing bytes past MAX_MSG_BYTES.
    let mut peer_writer = peer.try_clone().expect("peer writer clone");
    let mut peer_reader = peer.try_clone().expect("peer reader clone");
    // The reader uses blocking reads with no per-call timeout and a small
    // overall budget. It terminates when the peer side is closed by the
    // server (Ok(0)), when the reply's NUL terminator arrives, or when
    // the budget elapses, whichever comes first. The `MAX_MSG_BYTES +
    // 1024` cap is a hang guard only: the server's reply is well under
    // this size and the reader normally terminates on NUL first.
    let reader_thread = std::thread::spawn(move || {
        let mut received = Vec::new();
        let mut byte = [0u8; 1];
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match peer_reader.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    received.push(byte[0]);
                    if byte[0] == 0x00 {
                        break;
                    }
                    if received.len() > MAX_MSG_BYTES + 1024 {
                        // Hang guard: should be unreachable in practice
                        // because the server's reply is ~200 bytes of
                        // JSON followed by NUL.
                        break;
                    }
                }
                Err(_) => {
                    // Transient EAGAIN/WouldBlock while the kernel buffer is
                    // being filled by the writer or drained by the server.
                    // Sleep briefly and retry until the deadline.
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        received
    });
    // Writer runs on a std::thread so it cannot block the tokio runtime
    // thread that drives the server task.
    let writer_thread = std::thread::spawn(move || {
        let chunk = vec![b'x'; 64 * 1024];
        let mut written = 0usize;
        while written < MAX_MSG_BYTES + 1 {
            let n = chunk.len().min(MAX_MSG_BYTES + 1 - written);
            if peer_writer.write_all(&chunk[..n]).is_err() {
                break;
            }
            written += n;
        }
        drop(peer_writer);
    });

    let server_done = tokio::time::timeout(Duration::from_secs(10), handle).await;
    writer_thread.join().expect("peer writer thread");
    let reply = reader_thread.join().expect("peer reader thread");
    assert!(
        server_done.is_ok(),
        "server must terminate after ProtocolError reply"
    );
    let reply_str = String::from_utf8_lossy(&reply);
    assert!(
        reply_str.contains("ProtocolError"),
        "expected ProtocolError reply on overflow, got: {:?}",
        reply_str
    );
}

#[tokio::test]
async fn test_handle_varlink_client_responds_to_valid_call() {
    let dir = tempdir().unwrap();
    let cas = Arc::new(CasStore::new(dir.path()).unwrap());
    let tags = Arc::new(TagRegistry::new(dir.path()).unwrap());
    let eviction = Arc::new(EvictionManager::new(dir.path()).unwrap());
    let ctx = ModelServiceContext { cas, tags, eviction };

    let (server, peer) = UnixStream::pair().expect("socketpair");
    server.set_nonblocking(true).expect("set nonblocking");
    let server = tokio::net::UnixStream::from_std(server).expect("from_std");

    let (_tx, rx) = watch::channel::<bool>(false);
    let handle = tokio::spawn(async move {
        let _ = handle_varlink_client(server, ctx, rx).await;
    });

    // Send a known method and verify the reply.
    let payload = br#"{"method":"org.varlink.service.GetInfo","parameters":null}"#;
    let mut buf = Vec::new();
    buf.extend_from_slice(payload);
    buf.push(0x00);
    let mut peer_for_read = peer.try_clone().expect("peer clone");
    let reader_thread = std::thread::spawn(move || {
        let mut received = Vec::new();
        let mut byte = [0u8; 1];
        let _ = peer_for_read.set_read_timeout(Some(Duration::from_millis(500)));
        while let Ok(n) = peer_for_read.read(&mut byte) {
            if n == 0 {
                break;
            }
            received.push(byte[0]);
            if byte[0] == 0x00 {
                break;
            }
        }
        received
    });
    let mut peer_for_write = peer;
    peer_for_write.write_all(&buf).expect("peer write");
    drop(peer_for_write);

    let server_done = tokio::time::timeout(Duration::from_secs(3), handle).await;
    let reply = reader_thread.join().expect("peer reader thread");
    assert!(
        server_done.is_ok(),
        "server must terminate cleanly after EOF"
    );
    let reply_str = String::from_utf8_lossy(&reply);
    assert!(
        reply_str.contains("\"product\":\"modeld\""),
        "expected GetInfo reply, got: {:?}",
        reply_str
    );
}

#[test]
fn test_max_msg_bytes_constant_is_one_mebibyte() {
    assert_eq!(MAX_MSG_BYTES, 1024 * 1024);
}
