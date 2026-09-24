//! File descriptor management and SCM_RIGHTS IPC subsystem.
//!
//! Implements sealed anonymous memfd creation and zero-copy Unix socket transmission.

pub mod fd_pass;
pub mod sealed_fd;

pub use fd_pass::{recv_fd_scm_rights, send_fd_scm_rights};
pub use sealed_fd::{create_sealed_memfd, create_sealed_memfd_from_file};
