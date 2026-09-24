//! Handler for `modelctl import` subcommand.

use anyhow::{anyhow, Result};
use modeld_core::cas::{CasStore, TagRegistry};
use modeld_core::format::validate_file_safety;
use std::fs::File;
use std::path::Path;

/// Imports a local file directly into the CAS store and optionally assigns a tag.
pub fn run_import<P: AsRef<Path>>(
    storage_root: P,
    source_path: P,
    tag: Option<&str>,
) -> Result<()> {
    let src = source_path.as_ref();
    if !src.is_file() {
        return Err(anyhow!("Source file not found: {:?}", src));
    }

    validate_file_safety(src)?;

    let cas = CasStore::new(storage_root.as_ref())?;
    let tags = TagRegistry::new(storage_root.as_ref())?;

    let file = File::open(src)?;
    let (digest, total_bytes) = cas.store_blob(file, None)?;

    println!("Imported {:?} into CAS store.", src);
    println!("  SHA-256: {}", digest);
    println!("  Size:    {} bytes", total_bytes);

    if let Some(t) = tag {
        let parts: Vec<&str> = t.splitn(2, ':').collect();
        let (name, variant) = match parts.as_slice() {
            [n, v] => (*n, *v),
            [n] => (*n, "latest"),
            // An empty tag string is a user error, not a silent default.
            _ => return Err(anyhow!("Tag must be in 'name:variant' form, got '{}'", t)),
        };
        // `TagRegistry::set_tag` now validates segments; propagate its
        // error so the user sees the rejection instead of a panic or
        // a silently dropped tag.
        tags.set_tag(name, variant, &digest)?;
        println!("  Tagged:  {}:{}", name, variant);
    }

    Ok(())
}
