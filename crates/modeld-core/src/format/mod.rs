//! Model format parsing and validation subsystem.
//!
//! Provides pure Rust stream parsers for GGUF and SafeTensors formats,
//! and enforces zero-trust security policies against unsafe formats.

pub mod gguf;
pub mod safetensors;
pub mod validator;

pub use gguf::{parse_gguf_header, GgufMetadata};
pub use safetensors::{parse_safetensors_header, SafeTensorsMetadata};
pub use validator::{detect_safe_format, validate_file_safety, SafeFormat};
