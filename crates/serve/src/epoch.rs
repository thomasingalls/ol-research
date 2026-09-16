//! The epoch (serving plan §4): one manifest naming a batch and the bases
//! built from it, each by its directory and the digest of its own
//! manifest; `epochs/current` names the latest complete one, and writing
//! it is the switch.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use ingest::Store;
use serde::{Deserialize, Serialize};

use crate::digest::{self, FileInfo};
use crate::error::Error;

/// One artifact as an epoch names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Relative to the store root.
    pub dir: String,
    /// The artifact's own manifest, digested.
    pub manifest: FileInfo,
}

/// One epoch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[allow(missing_docs)]
    pub epoch: u64,
    /// The journal batch every base was built from.
    pub batch: u64,
    #[allow(missing_docs)]
    pub at: String,
    /// The merged directory's name.
    pub merged: String,
    /// By artifact name: `names`, `works`, `graph`, `search`.
    pub artifacts: BTreeMap<String, Artifact>,
}

/// The epoch `current` names, if any.
///
/// # Errors
///
/// I/O, or a `current` that is not a number.
pub fn current(store: &Store) -> Result<Option<u64>, Error> {
    let path = store.current_epoch();
    match fs::read_to_string(&path) {
        Ok(text) => {
            text.trim().parse().map(Some).map_err(|_| {
                Error::Mismatch(format!("{}: {text:?} is not an epoch", path.display()))
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(&path, e)),
    }
}

/// Reads epoch `n`.
///
/// # Errors
///
/// I/O, or a manifest that does not parse.
pub fn read(store: &Store, n: u64) -> Result<Manifest, Error> {
    let path = store.epoch(n);
    let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    serde_json::from_str(&text).map_err(|source| Error::Manifest { path, source })
}

/// An artifact entry for `dir`, whose manifest is `manifest_name`.
///
/// # Errors
///
/// I/O, or a directory outside the store.
pub fn artifact(store: &Store, dir: &Path, manifest_name: &str) -> Result<Artifact, Error> {
    let relative = dir.strip_prefix(store.root()).map_err(|_| {
        Error::Mismatch(format!(
            "{} is not under the store {}",
            dir.display(),
            store.root().display()
        ))
    })?;
    Ok(Artifact {
        dir: relative.to_string_lossy().into_owned(),
        manifest: digest::digest(&dir.join(manifest_name))?,
    })
}

/// Writes the manifest, then `current`: the switch.
///
/// # Errors
///
/// I/O, or an epoch that is not the next.
pub fn publish(store: &Store, manifest: &Manifest) -> Result<(), Error> {
    let expected = current(store)?.map_or(1, |n| n + 1);
    if manifest.epoch != expected {
        return Err(Error::Mismatch(format!(
            "epoch {} published where {expected} was next",
            manifest.epoch
        )));
    }
    let dir = store.epochs();
    fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let path = store.epoch(manifest.epoch);
    let mut text = serde_json::to_string_pretty(manifest).map_err(|source| Error::Manifest {
        path: path.clone(),
        source,
    })?;
    text.push('\n');
    fs::write(&path, text).map_err(|e| Error::io(&path, e))?;
    let current = store.current_epoch();
    let part = current.with_extension("part");
    fs::write(&part, format!("{}\n", manifest.epoch)).map_err(|e| Error::io(&part, e))?;
    fs::rename(&part, &current).map_err(|e| Error::io(&current, e))
}
