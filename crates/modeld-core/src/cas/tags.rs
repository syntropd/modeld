//! Human-readable tag registry mapping model names to content digests.
//!
//! Stores tags under `/var/lib/models/tags/<model>/<tag>` referencing SHA-256.

use crate::error::ModeldError;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Represents a model tag mapping to a cryptographic digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTag {
    /// Model name (e.g. "llama3.2").
    pub name: String,
    /// Tag variant (e.g. "1b" or "latest").
    pub tag: String,
    /// Target SHA-256 digest.
    pub digest: String,
}

/// Validates that a model name or tag variant is a safe single path segment.
///
/// Rejects empty strings, embedded path separators (`/`, `\`), NUL bytes,
/// parent-directory references (`..`), and any byte that is not allowed in
/// a portable POSIX filename. This blocks path traversal attacks where a
/// hostile tag like `../../tmp/pwned` would otherwise escape the tags
/// directory.
pub fn validate_tag_segment(segment: &str) -> Result<&str, ModeldError> {
    if segment.is_empty() {
        return Err(ModeldError::InvalidFormat(
            "tag name and variant must not be empty".into(),
        ));
    }
    if segment == "." || segment == ".." {
        return Err(ModeldError::InvalidFormat(format!(
            "tag segment '{}' is reserved",
            segment
        )));
    }
    if segment.contains('/') || segment.contains('\\') || segment.contains('\0') {
        return Err(ModeldError::InvalidFormat(format!(
            "tag segment '{}' contains forbidden characters",
            segment
        )));
    }
    if !segment
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
    {
        return Err(ModeldError::InvalidFormat(format!(
            "tag segment '{}' contains unsupported characters; \
             allowed: [A-Za-z0-9._-+]",
            segment
        )));
    }
    Ok(segment)
}

/// Tag registry manager.
#[derive(Debug, Clone)]
pub struct TagRegistry {
    tags_dir: PathBuf,
}

impl TagRegistry {
    /// Initializes a new tag registry under the designated root path.
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self, ModeldError> {
        let tags_dir = root.as_ref().join("tags");
        fs::create_dir_all(&tags_dir)?;
        Ok(Self { tags_dir })
    }

    /// Associates a model name and tag with a target SHA-256 digest.
    pub fn set_tag(&self, name: &str, tag: &str, digest: &str) -> Result<(), ModeldError> {
        let safe_name = validate_tag_segment(name)?;
        let safe_tag = validate_tag_segment(tag)?;
        // Validate the *trimmed* digest at write time so downstream
        // consumers (`EvictionManager::pin`, `fd_server`, `cas.blob_size`)
        // see exactly the value that was validated. Validating the raw
        // input but storing the trimmed one would let a caller bypass
        // length checks via leading/trailing whitespace.
        let trimmed_digest = digest.trim();
        super::digest_format::validate_digest_format(trimmed_digest)?;

        let model_dir = self.tags_dir.join(safe_name);
        fs::create_dir_all(&model_dir)?;
        let tag_file = model_dir.join(safe_tag);
        let mut file = File::create(tag_file)?;
        file.write_all(trimmed_digest.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    /// Resolves a model name and tag to its associated SHA-256 digest.
    pub fn get_tag(&self, name: &str, tag: &str) -> Result<Option<String>, ModeldError> {
        // Validate inputs the same way as `set_tag` to prevent a malicious
        // identifier from escaping the registry via `Path::join`.
        validate_tag_segment(name)?;
        validate_tag_segment(tag)?;
        let tag_file = self.tags_dir.join(name).join(tag);
        if !tag_file.is_file() {
            return Ok(None);
        }
        let mut file = File::open(tag_file)?;
        let mut digest = String::new();
        file.read_to_string(&mut digest)?;
        Ok(Some(digest.trim().to_string()))
    }

    /// Removes an existing tag mapping.
    pub fn remove_tag(&self, name: &str, tag: &str) -> Result<bool, ModeldError> {
        validate_tag_segment(name)?;
        validate_tag_segment(tag)?;
        let tag_file = self.tags_dir.join(name).join(tag);
        if tag_file.is_file() {
            fs::remove_file(tag_file)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lists all registered model tags in the store.
    pub fn list_tags(&self) -> Result<Vec<ModelTag>, ModeldError> {
        let mut result = Vec::new();
        if !self.tags_dir.is_dir() {
            return Ok(result);
        }

        for model_entry in fs::read_dir(&self.tags_dir)? {
            let model_entry = model_entry?;
            if model_entry.file_type()?.is_dir() {
                let model_name = model_entry.file_name().to_string_lossy().to_string();
                for tag_entry in fs::read_dir(model_entry.path())? {
                    let tag_entry = tag_entry?;
                    if tag_entry.file_type()?.is_file() {
                        let tag_name = tag_entry.file_name().to_string_lossy().to_string();
                        if let Some(digest) = self.get_tag(&model_name, &tag_name)? {
                            result.push(ModelTag {
                                name: model_name.clone(),
                                tag: tag_name,
                                digest,
                            });
                        }
                    }
                }
            }
        }

        result.sort_by(|a, b| a.name.cmp(&b.name).then(a.tag.cmp(&b.tag)));
        Ok(result)
    }

    /// Resolves an identifier which may be either `name:tag` or raw SHA-256 digest.
    ///
    /// Returns:
    /// - `Ok(Some(digest))` for a known raw digest or `name:tag`.
    /// - `Ok(None)` if the identifier parses cleanly but no tag exists.
    /// - `Err(ModeldError::InvalidFormat)` if the identifier fails segment
    ///   or digest-format validation, so callers can distinguish "rejected
    ///   as unsafe" from "not found" and report the right Varlink error.
    pub fn resolve(&self, identifier: &str) -> Result<Option<String>, ModeldError> {
        let trimmed = identifier.trim();
        // The raw-hex branch must pass through the same strict validator
        // used at write time so callers can't smuggle uppercase hex
        // (which would otherwise fail to match a lowercase CAS filename).
        if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            super::digest_format::validate_digest_format(trimmed)?;
            return Ok(Some(trimmed.to_string()));
        }

        let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
        let (name, tag) = match parts.as_slice() {
            [n, t] => (*n, *t),
            [n] => (*n, "latest"),
            _ => return Ok(None),
        };

        // Validation errors propagate so the daemon can surface them as
        // `io.syntrop.Model1.InvalidIdentifier` rather than the misleading
        // `NoSuchModel`.
        validate_tag_segment(name)?;
        validate_tag_segment(tag)?;

        self.get_tag(name, tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_validate_tag_segment_allows_safe_names() {
        assert!(validate_tag_segment("llama3.2").is_ok());
        assert!(validate_tag_segment("qwen2.5-7b").is_ok());
        assert!(validate_tag_segment("model_v1+exp").is_ok());
        assert_eq!(validate_tag_segment("latest").unwrap(), "latest");
    }

    #[test]
    fn test_validate_tag_segment_rejects_traversal() {
        assert!(validate_tag_segment("..").is_err());
        assert!(validate_tag_segment("../etc").is_err());
        assert!(validate_tag_segment("foo/bar").is_err());
        assert!(validate_tag_segment("foo\\bar").is_err());
        assert!(validate_tag_segment(".").is_err());
        assert!(validate_tag_segment("").is_err());
    }

    #[test]
    fn test_validate_tag_segment_rejects_interior_nul() {
        assert!(validate_tag_segment("foo\0bar").is_err());
    }

    #[test]
    fn test_validate_tag_segment_rejects_shell_metacharacters() {
        // Disallow characters that could be used to break out of the tag
        // segment or to confuse shell tooling.
        assert!(validate_tag_segment("foo$bar").is_err());
        assert!(validate_tag_segment("foo bar").is_err());
        assert!(validate_tag_segment("foo;bar").is_err());
    }

    #[test]
    fn test_set_tag_blocks_traversal() {
        let dir = tempdir().unwrap();
        let registry = TagRegistry::new(dir.path()).unwrap();
        let digest = "0".repeat(64);

        // Attempted traversal must be rejected.
        assert!(registry.set_tag("../../tmp/pwned", "x", &digest).is_err());
        assert!(registry.set_tag("ok", "../escape", &digest).is_err());

        // The 'ok' subdirectory must not have been created by the rejected
        // calls — only the valid tag write below creates it.
        let ok_dir = registry.tags_dir.join("ok");
        assert!(!ok_dir.exists(), "rejected tag must not create directories");

        // Sanity: a valid tag still works.
        registry.set_tag("ok", "v1", &digest).unwrap();
        assert!(registry.get_tag("ok", "v1").unwrap().is_some());
    }
}
