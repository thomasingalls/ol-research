//! The graph on disk (graph plan §8): under the store's
//! `graph/v<model>.<merge>.<graph>/`, eight flat little-endian arrays and
//! a manifest, replaced whole by every build and readable only by a
//! binary of the versions the manifest names. No framing: a reader is a
//! memory map and a slice.
//!
//! ```text
//! manifest.json            versions, counts, the asserter table, a digest per file
//! perm.u32                 id − 1 → position
//! inv.u32                  position → id
//! forward.offsets.u64      nodes + 1 offsets into the forward arrays
//! forward.neighbours.u32   the works each work cites, one list per position, sorted
//! forward.labels.u8        beside each: an index into the asserter table
//! transpose.offsets.u64    the same three for the works citing each work
//! transpose.neighbours.u32
//! transpose.labels.u8
//! ```

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use ingest::Store;
use merge::{SourceId, Sources};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Error;

#[allow(missing_docs)]
pub const MANIFEST: &str = "manifest.json";
#[allow(missing_docs)]
pub const PERM: &str = "perm.u32";
#[allow(missing_docs)]
pub const INV: &str = "inv.u32";
#[allow(missing_docs)]
pub const FORWARD_OFFSETS: &str = "forward.offsets.u64";
#[allow(missing_docs)]
pub const FORWARD_NEIGHBOURS: &str = "forward.neighbours.u32";
#[allow(missing_docs)]
pub const FORWARD_LABELS: &str = "forward.labels.u8";
#[allow(missing_docs)]
pub const TRANSPOSE_OFFSETS: &str = "transpose.offsets.u64";
#[allow(missing_docs)]
pub const TRANSPOSE_NEIGHBOURS: &str = "transpose.neighbours.u32";
#[allow(missing_docs)]
pub const TRANSPOSE_LABELS: &str = "transpose.labels.u8";

/// The array files, in the order the manifest lists them.
pub const FILES: [&str; 8] = [
    PERM,
    INV,
    FORWARD_OFFSETS,
    FORWARD_NEIGHBOURS,
    FORWARD_LABELS,
    TRANSPOSE_OFFSETS,
    TRANSPOSE_NEIGHBOURS,
    TRANSPOSE_LABELS,
];

/// The one ordering this version builds (plan §4): position is id − 1.
pub const ORDERING: &str = "workid";

/// What a neighbour value is: a `WorkId`, each list sorted by position.
pub const NEIGHBOURS: &str = "workid";

/// The directory name for this binary's versions: `v<model>.<merge>.<graph>`.
#[must_use]
pub fn version() -> String {
    format!("v{}.{}.{}", model::VERSION, merge::VERSION, crate::VERSION)
}

/// The directory name of a build: the version, `-held` for the held-works
/// subgraph.
#[must_use]
pub fn name(held: bool) -> String {
    if held {
        format!("{}-held", version())
    } else {
        version()
    }
}

/// Where this binary's graph lives under a store.
#[must_use]
pub fn dir(store: &Store, held: bool) -> PathBuf {
    let dir = store.graph(model::VERSION, merge::VERSION, crate::VERSION);
    if held {
        dir.with_file_name(name(true))
    } else {
        dir
    }
}

/// What a build was made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// The merged directory's name, `v<model>.<merge>`.
    pub merged: String,
    /// Its shards.
    pub shards: usize,
}

/// One array file as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileInfo {
    #[allow(missing_docs)]
    pub bytes: u64,
    #[allow(missing_docs)]
    pub sha256: String,
}

/// What a build records about itself, written last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[allow(missing_docs)]
    pub graph: u32,
    #[allow(missing_docs)]
    pub model: u32,
    #[allow(missing_docs)]
    pub merge: u32,
    #[allow(missing_docs)]
    pub built_at: String,
    /// [`ORDERING`].
    pub ordering: String,
    /// [`NEIGHBOURS`].
    pub neighbours: String,
    /// Built with `--held`: arcs between held works only.
    pub held_only: bool,
    /// Positions, which is the highest id.
    pub nodes: u32,
    #[allow(missing_docs)]
    pub works: u64,
    #[allow(missing_docs)]
    pub held: u64,
    #[allow(missing_docs)]
    pub stubs: u64,
    /// Distinct edges, each an arc in both orientations.
    pub arcs: u64,
    #[allow(missing_docs)]
    pub self_loops: u64,
    /// Edges `--held` left out.
    pub dropped: u64,
    /// The asserter table: a label is an index into it; each entry the
    /// names of the sources asserting an arc.
    pub asserters: Vec<Vec<String>>,
    #[allow(missing_docs)]
    pub input: Input,
    #[allow(missing_docs)]
    pub files: BTreeMap<String, FileInfo>,
}

impl Manifest {
    /// Reads `dir`'s manifest.
    ///
    /// # Errors
    ///
    /// I/O, or a manifest that does not parse — another version's, say.
    pub fn read(dir: &Path) -> Result<Self, Error> {
        let path = dir.join(MANIFEST);
        let file = File::open(&path).map_err(|e| Error::io(&path, e))?;
        serde_json::from_reader(BufReader::new(file))
            .map_err(|source| Error::Manifest { path, source })
    }

    /// Writes it under `dir`.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn write(&self, dir: &Path) -> Result<(), Error> {
        let path = dir.join(MANIFEST);
        let mut text = serde_json::to_string_pretty(self).map_err(|source| Error::Manifest {
            path: path.clone(),
            source,
        })?;
        text.push('\n');
        fs::write(&path, text).map_err(|e| Error::io(&path, e))
    }

    /// This binary's versions and layout, in a directory named for them,
    /// listing every file.
    ///
    /// # Errors
    ///
    /// Another version's, another ordering's, moved, or a file missing
    /// from the list.
    pub fn check(&self, dir: &Path) -> Result<(), Error> {
        let mismatch = |what: String| Error::Mismatch {
            what: format!("{}: {what}", dir.display()),
        };
        if (self.graph, self.model, self.merge) != (crate::VERSION, model::VERSION, merge::VERSION)
        {
            return Err(mismatch(format!(
                "built as graph {} over model {} and merge {}, not this binary's {}",
                self.graph,
                self.model,
                self.merge,
                version()
            )));
        }
        if self.ordering != ORDERING || self.neighbours != NEIGHBOURS {
            return Err(mismatch(format!(
                "ordering {} with {} neighbours, which this binary does not read",
                self.ordering, self.neighbours
            )));
        }
        let wanted = name(self.held_only);
        if dir
            .file_name()
            .and_then(|n| n.to_str())
            .is_none_or(|n| !n.starts_with(wanted.as_str()))
        {
            return Err(mismatch(format!(
                "not a {wanted} directory: moved, or another build's"
            )));
        }
        if let Some(file) = FILES.iter().find(|f| !self.files.contains_key(**f)) {
            return Err(mismatch(format!("the manifest lists no {file}")));
        }
        Ok(())
    }

    /// The asserter table as sources.
    ///
    /// # Errors
    ///
    /// A source name this binary does not know.
    pub fn table(&self) -> Result<Vec<Sources>, Error> {
        self.asserters
            .iter()
            .map(|names| {
                names
                    .iter()
                    .map(|name| {
                        SourceId::ALL
                            .into_iter()
                            .find(|s| s.name() == name)
                            .ok_or_else(|| Error::UnknownSource { name: name.clone() })
                    })
                    .collect()
            })
            .collect()
    }
}

/// The asserter table as names, for the manifest.
#[must_use]
pub fn names_of(table: &[Sources]) -> Vec<Vec<String>> {
    table
        .iter()
        .map(|set| set.iter().map(|s| s.name().to_owned()).collect())
        .collect()
}

/// Lowercase hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// An array file being written, digested on the way.
#[derive(Debug)]
pub struct Sink {
    path: PathBuf,
    file: BufWriter<File>,
    hasher: Sha256,
    bytes: u64,
}

impl Sink {
    /// Creates the file.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn create(path: &Path) -> Result<Self, Error> {
        let file = File::create(path).map_err(|e| Error::io(path, e))?;
        Ok(Self {
            path: path.to_path_buf(),
            file: BufWriter::with_capacity(1 << 20, file),
            hasher: Sha256::new(),
            bytes: 0,
        })
    }

    /// Appends bytes.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.file
            .write_all(bytes)
            .map_err(|e| Error::io(&self.path, e))?;
        self.hasher.update(bytes);
        self.bytes += bytes.len() as u64;
        Ok(())
    }

    /// Flushes, syncs and closes.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn finish(self) -> Result<FileInfo, Error> {
        let file = self
            .file
            .into_inner()
            .map_err(|e| Error::io(&self.path, e.into_error()))?;
        file.sync_all().map_err(|e| Error::io(&self.path, e))?;
        Ok(FileInfo {
            bytes: self.bytes,
            sha256: hex(&self.hasher.finalize()),
        })
    }
}

/// Reads a file through the digest.
///
/// # Errors
///
/// I/O.
pub fn digest(path: &Path) -> Result<FileInfo, Error> {
    let mut file = File::open(path).map_err(|e| Error::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|e| Error::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
    }
    Ok(FileInfo {
        bytes,
        sha256: hex(&hasher.finalize()),
    })
}

#[cfg(test)]
mod tests {
    use merge::{SourceId, Sources};

    use super::{FILES, Input, Manifest, dir, hex, name, names_of, version};

    #[test]
    fn names_and_paths() {
        assert!(version().starts_with('v'));
        assert_eq!(name(true), format!("{}-held", version()));
        let store = ingest::Store::new("s");
        assert!(dir(&store, false).ends_with(format!("graph/{}", version())));
        assert!(dir(&store, true).ends_with(format!("graph/{}-held", version())));
        assert_eq!(hex(&[0, 255, 16]), "00ff10");
        assert_eq!(FILES.len(), 8);
    }

    #[test]
    fn the_asserter_table_round_trips_by_name() {
        let both: Sources = [SourceId::Pubmed, SourceId::OpenAlex].into_iter().collect();
        let table = vec![Sources::NONE, both];
        let names = names_of(&table);
        assert_eq!(
            names,
            vec![
                Vec::<String>::new(),
                vec!["pubmed".into(), "openalex".into()]
            ]
        );
        let manifest = Manifest {
            graph: crate::VERSION,
            model: model::VERSION,
            merge: merge::VERSION,
            built_at: String::new(),
            ordering: super::ORDERING.into(),
            neighbours: super::NEIGHBOURS.into(),
            held_only: false,
            nodes: 0,
            works: 0,
            held: 0,
            stubs: 0,
            arcs: 0,
            self_loops: 0,
            dropped: 0,
            asserters: names,
            input: Input {
                merged: "v1.1".into(),
                shards: 0,
            },
            files: std::collections::BTreeMap::new(),
        };
        assert_eq!(manifest.table().unwrap(), table);
        let mut bad = manifest.clone();
        bad.asserters = vec![vec!["dimensions".into()]];
        assert!(bad.table().is_err());
        assert!(
            manifest
                .check(std::path::Path::new("x/v0.0.0"))
                .unwrap_err()
                .to_string()
                .contains("not a")
        );
    }
}
