//! 1:1 unit QA tests for modelctl client::VarlinkClient.
//!
//! Drives the client against a single-shot fake Varlink server bound to
//! a temporary Unix socket so no daemon is required.

use modelctl::client::VarlinkClient;
use modelctl::client::MAX_REPLY_BYTES;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread::JoinHandle;
use tempfile::TempDir;

/// Binds a fake server that answers the next connection with `reply`.
///
/// Returns the tempdir (keeps the socket path alive), the socket path,
/// and a handle yielding the parsed request the client sent.
fn serve_once(reply: Vec<u8>) -> (TempDir, PathBuf, JoinHandle<Value>) {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("fake.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let handle = std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().expect("fake server accept");
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            conn.read_exact(&mut byte).expect("fake server read");
            if byte[0] == 0x00 {
                break;
            }
            buf.push(byte[0]);
        }
        let request: Value = serde_json::from_slice(&buf).expect("fake server parse");
        conn.write_all(&reply).expect("fake server write");
        conn.write_all(&[0x00]).expect("fake server framing");
        conn.flush().expect("fake server flush");
        request
    });
    (dir, sock, handle)
}

fn connect(sock: &PathBuf) -> VarlinkClient {
    VarlinkClient::connect(sock).expect("client connect")
}

#[test]
fn test_client_call_returns_parameters() {
    let reply = serde_json::to_vec(&json!({ "parameters": { "models": [] } })).unwrap();
    let (_dir, sock, handle) = serve_once(reply);
    let mut client = connect(&sock);
    let params = client.call("io.syntrop.Model1.List", None).unwrap();
    assert_eq!(params, json!({ "models": [] }));
    let request = handle.join().unwrap();
    assert_eq!(
        request.get("method").and_then(|v| v.as_str()),
        Some("io.syntrop.Model1.List")
    );
}

#[test]
fn test_client_call_forwards_parameters() {
    let reply = serde_json::to_vec(&json!({ "parameters": {} })).unwrap();
    let (_dir, sock, handle) = serve_once(reply);
    let mut client = connect(&sock);
    let _ = client
        .call(
            "io.syntrop.Model1.Inspect",
            Some(json!({ "id": "wisp:1b" })),
        )
        .unwrap();
    let request = handle.join().unwrap();
    assert_eq!(request.get("parameters"), Some(&json!({ "id": "wisp:1b" })));
}

#[test]
fn test_client_call_surfaces_varlink_error() {
    let reply = serde_json::to_vec(&json!({ "error": "io.syntrop.Model1.NoSuchModel" })).unwrap();
    let (_dir, sock, _handle) = serve_once(reply);
    let mut client = connect(&sock);
    let err = client.call("io.syntrop.Model1.Inspect", None).unwrap_err();
    assert!(err.to_string().contains("NoSuchModel"), "got: {err}");
}

#[test]
fn test_client_call_eof_is_error() {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("eof.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().expect("accept");
        // Drain the framed request, then drop without replying: the
        // client's write succeeds and its read must see EOF, not hang.
        let mut byte = [0u8; 1];
        loop {
            match conn.read(&mut byte) {
                Ok(0) => break,
                Ok(_) if byte[0] == 0x00 => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }
    });
    let mut client = connect(&sock);
    let err = client.call("io.syntrop.Model1.List", None).unwrap_err();
    assert!(err.to_string().contains("EOF"), "got: {err}");
}

#[test]
fn test_client_connect_missing_socket_fails() {
    assert!(VarlinkClient::connect("/nonexistent/modeld-test.sock").is_err());
}

#[test]
fn test_max_reply_bytes_is_one_mebibyte() {
    assert_eq!(MAX_REPLY_BYTES, 1024 * 1024);
}
