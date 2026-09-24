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
        let model_dir = self.tags_dir.join(name);
        fs::create_dir_all(&model_dir)?;
        let tag_file = model_dir.join(tag);
        let mut file = File::create(tag_file)?;
        file.write_all(digest.trim().as_bytes())?;
        file.flush()?;
        Ok(())
    }

    /// Resolves a model name and tag to its associated SHA-256 digest.
    pub fn get_tag(&self, name: &str, tag: &str) -> Result<Option<String>, ModeldError> {
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
    pub fn resolve(&self, identifier: &str) -> Result<Option<String>, ModeldError> {
        let trimmed = identifier.trim();
        if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(Some(trimmed.to_string()));
        }

        let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
        let (name, tag) = match parts.as_slice() {
            [n, t] => (*n, *t),
            [n] => (*n, "latest"),
            _ => return Ok(None),
        };

        self.get_tag(name, tag)
    }
}
