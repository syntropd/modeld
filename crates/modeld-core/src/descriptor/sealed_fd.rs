//! Linux sealed anonymous memory file descriptor helpers.
//!
//! Creates sealed memfd descriptors that guarantee memory immutability.

use crate::error::ModeldError;
use rustix::fs::{fcntl_add_seals, memfd_create, MemfdFlags, SealFlags};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::io::{FromRawFd, IntoRawFd, OwnedFd};

/// Streaming copy chunk for memfd sealing (1 MiB).
const COPY_CHUNK: usize = 1024 * 1024;

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

/// Streams a regular file's contents into a sealed memfd.
///
/// The resulting `OwnedFd` carries an immutable, read-only view of the
/// file's content: the kernel enforces that the consumer cannot mutate,
/// shrink, grow, or further-seal the descriptor. This delivers the
/// immutability guarantee that ARCHITECTURE.md §2.3 promises and that
/// the prior `File::open`-based path did not.
///
/// The copy happens through a fixed-size stack buffer so memory use stays
/// bounded regardless of source size. The kernel page cache handles
/// readahead so the wall-clock cost is comparable to a plain sequential
/// read of the file.
pub fn create_sealed_memfd_from_file(name: &str, source: &mut File) -> Result<OwnedFd, ModeldError> {
    let flags = MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC;
    let raw_fd = memfd_create(name, flags)
        .map_err(|e| ModeldError::Syscall(format!("memfd_create failed: {}", e)))?;

    // `memfd_create` returns an `OwnedFd`; convert to a `File` for ergonomic
    // I/O, and recover the raw fd from the File once writes are done.
    let mut memfd_file = unsafe { File::from_raw_fd(raw_fd.into_raw_fd()) };

    let mut buf = [0u8; COPY_CHUNK];
    loop {
        let n = source.read(&mut buf)?;
        if n == 0 {
            break;
        }
        memfd_file.write_all(&buf[..n])?;
    }
    memfd_file.flush()?;
    // Rewind so the consumer sees the content from offset 0.
    memfd_file.seek(SeekFrom::Start(0))?;

    let raw_fd_back = memfd_file.into_raw_fd();
    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd_back) };
    let seals = SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL;
    fcntl_add_seals(&owned_fd, seals)
        .map_err(|e| ModeldError::Syscall(format!("fcntl_add_seals failed: {}", e)))?;

    Ok(owned_fd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_create_sealed_memfd_from_file_content_matches() {
        let payload = b"streamed sealed memfd payload";
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(tmp.as_file_mut(), payload).unwrap();
        tmp.as_file_mut().seek(SeekFrom::Start(0)).unwrap();

        let owned = create_sealed_memfd_from_file("test", tmp.as_file_mut()).unwrap();

        let mut f = File::from(owned);
        let mut read_back = Vec::new();
        f.read_to_end(&mut read_back).unwrap();
        assert_eq!(read_back, payload);

        // Sealed memfd must reject writes.
        assert!(std::io::Write::write_all(&mut f, b"corrupt").is_err());
    }

    #[test]
    fn test_create_sealed_memfd_from_file_handles_empty_source() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.as_file_mut().seek(SeekFrom::Start(0)).unwrap();

        let owned = create_sealed_memfd_from_file("empty", tmp.as_file_mut()).unwrap();
        let mut f = File::from(owned);
        let mut read_back = Vec::new();
        f.read_to_end(&mut read_back).unwrap();
        assert!(read_back.is_empty());
    }
}
