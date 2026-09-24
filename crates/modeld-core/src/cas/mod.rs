//! Content-Addressable Storage (CAS) subsystem.
//!
//! Provides content hashing, atomic blob commits, reflink copies,
//! human-readable tags, and LRU eviction.

pub mod digest;
pub mod digest_format;
pub mod eviction;
pub mod reflink;
pub mod store;
pub mod tags;

pub use digest::{compute_file_digest, compute_stream_digest, verify_stream_digest};
pub use digest_format::validate_digest_format;
pub use eviction::EvictionManager;
pub use reflink::reflink_or_copy;
pub use store::CasStore;
pub use tags::{ModelTag, TagRegistry};
