//! 1:1 unit QA tests for modelctl cmd::run_* subcommand handlers.
//!
//! Socket-backed commands run against a single-shot fake Varlink
//! server; `run_import` runs against temporary storage directories.

use modelctl::client::VarlinkClient;
use modelctl::cmd::{run_import, run_inspect, run_list, run_pin, run_prune, run_unpin};
use modeld_core::cas::TagRegistry;
use serde_json::json;
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use tempfile::TempDir;

/// Binds a fake server answering the next connection with `reply` JSON.
fn serve_once(reply: Vec<u8>) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("fake.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    std::thread::spawn(move || {
        let (mut conn, _) = listener.accept().expect("fake server accept");
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if conn.read_exact(&mut byte).is_err() {
                return;
            }
            if byte[0] == 0x00 {
                break;
            }
            buf.push(byte[0]);
        }
        let _ = conn.write_all(&reply);
        let _ = conn.write_all(&[0x00]);
        let _ = conn.flush();
    });
    (dir, sock)
}

fn connect(sock: &PathBuf) -> VarlinkClient {
    VarlinkClient::connect(sock).expect("client connect")
}

#[test]
fn test_run_import_stores_blob_and_tag() {
    let storage = TempDir::new().unwrap();
    let storage_path = storage.path().join("cas");
    let src = storage.path().join("model.gguf");
    std::fs::write(&src, b"fake gguf payload bytes").unwrap();

    run_import(&storage_path, &src, Some("wisp:1b")).expect("import failed");

    let tags = TagRegistry::new(&storage_path).unwrap();
    let digest = tags.resolve("wisp:1b").unwrap().expect("tag must resolve");
    assert_eq!(digest.len(), 64);
}

#[test]
fn test_run_import_without_tag_stores_blob() {
    let storage = TempDir::new().unwrap();
    let storage_path = storage.path().join("cas");
    let src = storage.path().join("plain.gguf");
    std::fs::write(&src, b"untagged payload").unwrap();

    run_import(&storage_path, &src, None).expect("untagged import failed");
}

#[test]
fn test_run_import_missing_source_fails() {
    let storage = TempDir::new().unwrap();
    let storage_path = storage.path().join("cas");
    let missing = storage.path().join("ghost.gguf");
    assert!(run_import(&storage_path, &missing, None).is_err());
}

#[test]
fn test_run_import_rejects_unsafe_extension() {
    let storage = TempDir::new().unwrap();
    let storage_path = storage.path().join("cas");
    let src = storage.path().join("evil.pt");
    std::fs::write(&src, b"pickle bytes").unwrap();
    assert!(run_import(&storage_path, &src, None).is_err());
}

#[test]
fn test_run_import_rejects_empty_tag() {
    let storage = TempDir::new().unwrap();
    let storage_path = storage.path().join("cas");
    let src = storage.path().join("model.gguf");
    std::fs::write(&src, b"payload").unwrap();
    assert!(run_import(&storage_path, &src, Some("")).is_err());
}

#[test]
fn test_run_inspect_human_and_json() {
    let info = json!({
        "parameters": {
            "info": {
                "digest": "ab".repeat(32),
                "size_bytes": 42,
                "pinned": true,
                "format": "gguf"
            }
        }
    });
    for json_output in [false, true] {
        let reply = serde_json::to_vec(&info).unwrap();
        let (_dir, sock) = serve_once(reply);
        let mut client = connect(&sock);
        run_inspect(&mut client, "wisp:1b", json_output).expect("inspect failed");
    }
}

#[test]
fn test_run_list_empty_populated_and_json() {
    let empty = serde_json::to_vec(&json!({ "parameters": { "models": [] } })).unwrap();
    let (_dir, sock) = serve_once(empty);
    run_list(&mut connect(&sock), false).expect("empty list failed");

    let digest = "cd".repeat(32);
    let populated = serde_json::to_vec(&json!({
        "parameters": { "models": [
            { "id": "wisp:1b", "size_bytes": 2048, "pinned": false, "digest": digest },
            { "id": "wisp:7b", "size_bytes": 5 * 1024 * 1024, "pinned": true, "digest": digest },
        ] }
    }))
    .unwrap();
    let (_dir, sock) = serve_once(populated);
    run_list(&mut connect(&sock), false).expect("populated list failed");

    let json_reply = serde_json::to_vec(&json!({ "parameters": { "models": [] } })).unwrap();
    let (_dir, sock) = serve_once(json_reply);
    run_list(&mut connect(&sock), true).expect("json list failed");
}

#[test]
fn test_run_pin_and_unpin() {
    for run in [
        run_pin as fn(&mut VarlinkClient, &str) -> anyhow::Result<()>,
        run_unpin,
    ] {
        let reply = serde_json::to_vec(&json!({ "parameters": {} })).unwrap();
        let (_dir, sock) = serve_once(reply);
        run(&mut connect(&sock), "wisp:1b").expect("pin lifecycle failed");
    }
}

#[test]
fn test_run_prune_rejects_zero_max_bytes() {
    // The zero guard fires before any socket I/O, so the server never
    // needs to answer; the listener only needs to exist for connect().
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("prune.sock");
    let _listener = UnixListener::bind(&sock).unwrap();
    let mut client = connect(&sock);
    let err = run_prune(&mut client, 0).unwrap_err();
    assert!(err.to_string().contains("--max-bytes"), "got: {err}");
}

#[test]
fn test_run_prune_reports_reclaimed_bytes() {
    let reply = serde_json::to_vec(&json!({ "parameters": { "reclaimed_bytes": 1024 } })).unwrap();
    let (_dir, sock) = serve_once(reply);
    run_prune(&mut connect(&sock), 1024).expect("prune failed");
}
