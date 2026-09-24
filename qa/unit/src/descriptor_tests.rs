//! 1:1 unit QA tests for descriptor management and SCM_RIGHTS passing.

use modeld_core::descriptor::{create_sealed_memfd, recv_fd_scm_rights, send_fd_scm_rights};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[test]
fn test_create_sealed_memfd_content_and_immutability() {
    let payload = b"sealed tensor weights test content";
    let fd = create_sealed_memfd("test_model_tensor", payload).expect("memfd creation failed");

    let mut file = File::from(fd);
    let mut read_back = Vec::new();
    file.read_to_end(&mut read_back).unwrap();
    assert_eq!(read_back, payload);

    // Attempting to write to sealed memfd MUST fail with EPERM
    let write_res = file.write_all(b"corrupting data");
    assert!(write_res.is_err());
}

#[test]
fn test_scm_rights_roundtrip_over_unix_pair() {
    let (s1, s2) = UnixStream::pair().expect("UnixStream::pair failed");

    let payload = b"tensor weights for SCM_RIGHTS";
    let memfd = create_sealed_memfd("transfer_test", payload).unwrap();

    let tag_payload = b"llama3.2:1b";

    // Send FD from s1 to s2
    let sent = send_fd_scm_rights(&s1, &memfd, tag_payload).expect("send_fd failed");
    assert_eq!(sent, tag_payload.len());

    // Receive FD at s2
    let mut recv_buf = [0u8; 64];
    let (n, received_fd) = recv_fd_scm_rights(&s2, &mut recv_buf).expect("recv_fd failed");

    assert_eq!(&recv_buf[..n], tag_payload);
    assert!(received_fd.is_some());

    let mut received_file = File::from(received_fd.unwrap());
    let mut content = Vec::new();
    received_file.read_to_end(&mut content).unwrap();
    assert_eq!(content, payload);
}
