//! Zero-copy reflink cloning using Linux FICLONE ioctl.
//!
//! Provides instantaneous Btrfs/XFS reflink copying with fallback to copy.

use crate::error::ModeldError;
use std::fs::{self, File};
use std::os::unix::io::AsRawFd;
use std::path::Path;

/// Linux ioctl command identifier for FICLONE (copy-on-write file clone).
const FICLONE_IOCTL: u64 = 0x40049409;

/// Performs an atomic reflink copy from source to destination.
///
/// Falls back to standard byte-by-byte file copy if the underlying
/// filesystem does not support copy-on-write extents.
pub fn reflink_or_copy<P: AsRef<Path>, Q: AsRef<Path>>(
    src: P,
    dest: Q,
) -> Result<bool, ModeldError> {
    let src_file = File::open(&src)?;
    
    // Create destination file with truncate
    let dest_file = File::create(&dest)?;

    // Attempt zero-copy reflink clone via Linux kernel ioctl
    let res = unsafe {
        libc::ioctl(
            dest_file.as_raw_fd(),
            FICLONE_IOCTL as _,
            src_file.as_raw_fd(),
        )
    };

    if res == 0 {
        // Reflink succeeded instantly
        Ok(true)
    } else {
        // Filesystem does not support FICLONE (e.g. ext4 without reflinks)
        // Drop file handles before standard fallback copy
        drop(dest_file);
        drop(src_file);
        fs::copy(src, dest)?;
        Ok(false)
    }
}
