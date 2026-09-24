//! SCM_RIGHTS file descriptor passing over Unix domain sockets.
//!
//! Provides zero-copy descriptor transmission without userland data copies.

use crate::error::ModeldError;
use rustix::net::{
    recvmsg, sendmsg, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags,
};
use std::io::{IoSlice, IoSliceMut};
use std::os::unix::io::{AsFd, OwnedFd};

/// Sends an open file descriptor alongside an identification payload over a Unix socket.
pub fn send_fd_scm_rights<S: AsFd, F: AsFd>(
    socket: S,
    fd_to_send: F,
    payload: &[u8],
) -> Result<usize, ModeldError> {
    let mut space = [0u8; rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    let fds = [fd_to_send.as_fd()];
    
    if !ancillary.push(SendAncillaryMessage::ScmRights(&fds)) {
        return Err(ModeldError::Syscall("Failed to push FD to SCM_RIGHTS buffer".into()));
    }

    let iov = [IoSlice::new(payload)];
    let sent = sendmsg(socket.as_fd(), &iov, &mut ancillary, SendFlags::empty())
        .map_err(|e| ModeldError::Syscall(format!("sendmsg SCM_RIGHTS failed: {}", e)))?;

    Ok(sent)
}

/// Receives an open file descriptor and payload bytes from a Unix socket via SCM_RIGHTS.
pub fn recv_fd_scm_rights<S: AsFd>(
    socket: S,
    buf: &mut [u8],
) -> Result<(usize, Option<OwnedFd>), ModeldError> {
    let mut space = [0u8; rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let mut iov = [IoSliceMut::new(buf)];

    let msg = recvmsg(
        socket.as_fd(),
        &mut iov,
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )
    .map_err(|e| ModeldError::Syscall(format!("recvmsg SCM_RIGHTS failed: {}", e)))?;

    let mut received_fd = None;
    for cmsg in ancillary.drain() {
        if let RecvAncillaryMessage::ScmRights(fds) = cmsg {
            for owned in fds {
                if received_fd.is_none() {
                    received_fd = Some(owned);
                }
            }
        }
    }

    Ok((msg.bytes, received_fd))
}
