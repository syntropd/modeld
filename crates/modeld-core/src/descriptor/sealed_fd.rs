//! Linux sealed anonymous memory file descriptor helpers.
//!
//! Creates sealed memfd descriptors that guarantee memory immutability.

use crate::error::ModeldError;
use rustix::fs::{fcntl_add_seals, memfd_create, MemfdFlags, SealFlags};
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::io::{FromRawFd, IntoRawFd, OwnedFd};

/// Creates a sealed anonymous memory file descriptor containing the provided data.
pub fn create_sealed_memfd(name: &str, data: &[u8]) -> Result<OwnedFd, ModeldError> {
    let flags = MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC;
    let raw_fd = memfd_create(name, flags)
        .map_err(|e| ModeldError::Syscall(format!("memfd_create failed: {}", e)))?;

    // Write contents to memfd
    let mut file = unsafe { File::from_raw_fd(raw_fd.into_raw_fd()) };
    file.write_all(data)?;
    file.flush()?;
    file.seek(SeekFrom::Start(0))?;

    let owned_fd = unsafe { OwnedFd::from_raw_fd(file.into_raw_fd()) };

    // Apply immutable kernel seals
    let seals = SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL;
    fcntl_add_seals(&owned_fd, seals)
        .map_err(|e| ModeldError::Syscall(format!("fcntl_add_seals failed: {}", e)))?;

    Ok(owned_fd)
}
