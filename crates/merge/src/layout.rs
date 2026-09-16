//! The merged sets on disk (merge plan §8): under the store's
//! `merged/v<model>.<merge>/`, sharded by `WorkId` — 65,536 works a
//! shard, a work in its id's shard, an edge, a relation or an unresolved
//! reference in its citing work's — each shard a set in the normalized
//! layout's framing with this layer's kinds: `works`, `edges`, `names`,
//! `relations`, `unresolved`, `symbols`. A cache: the directory is
//! replaced whole by every run, and a set is readable only by a binary
//! of the model and merge versions in its directory's name.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use ingest::{Kind, Reader, Store, Writer};

use crate::build::Merged;
use crate::work::WorkId;

/// Works per shard.
pub const SHARD: u32 = 65_536;

/// What the header's source field says for a merged set.
pub const SOURCE: &str = "merge";

/// The kinds of a merged set, in file order.
pub const KINDS: [Kind; 6] = [
    Kind::Works,
    Kind::Edges,
    Kind::Names,
    Kind::Relations,
    Kind::Unresolved,
    Kind::Symbols,
];

/// The shard an id lives in.
#[must_use]
pub const fn shard(id: WorkId) -> u32 {
    (id.get() - 1) / SHARD
}

/// The first id of a shard.
#[must_use]
pub const fn first(shard: u32) -> u32 {
    shard * SHARD + 1
}

/// A shard's set prefix under `dir`: `shard_00000`.
#[must_use]
pub fn prefix(dir: &Path, shard: u32) -> PathBuf {
    dir.join(format!("shard_{shard:05}"))
}

/// The directory name for this binary's versions: `v<model>.<merge>`.
#[must_use]
pub fn version() -> String {
    format!("v{}.{}", model::VERSION, crate::VERSION)
}

/// Where this binary's merged sets live under a store.
#[must_use]
pub fn dir(store: &Store) -> PathBuf {
    store.merged(model::VERSION, crate::VERSION)
}

/// What a write left on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    #[allow(missing_docs)]
    pub shards: u32,
    /// Records per kind over every shard.
    pub records: BTreeMap<Kind, u64>,
    /// Compressed bytes on disk.
    pub bytes: u64,
}

/// Writes every row of `merged` under `dir`, replacing what was there.
///
/// # Errors
///
/// `dir` is not named for this binary's versions, or I/O; a shard that
/// failed is removed, earlier shards stay.
pub fn write(dir: &Path, merged: &Merged) -> io::Result<Summary> {
    if dir.file_name().and_then(|n| n.to_str()) != Some(version().as_str()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not this binary's {}", dir.display(), version()),
        ));
    }
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::create_dir_all(dir)?;
    let shards = merged.works.last().map_or(0, |w| shard(w.id) + 1);
    let mut summary = Summary {
        shards,
        ..Summary::default()
    };
    let (mut w, mut e, mut r, mut u) = (0usize, 0usize, 0usize, 0usize);
    for s in 0..shards {
        let end = first(s) + SHARD;
        let in_shard = |id: WorkId| id.get() < end;
        let mut writer = Writer::create(&prefix(dir, s), SOURCE, model::VERSION, &KINDS)?;
        while w < merged.works.len() && in_shard(merged.works[w].id) {
            let work = &merged.works[w];
            writer.put(Kind::Works, &work.borrow())?;
            for row in work.name_rows() {
                writer.put(Kind::Names, &row)?;
            }
            w += 1;
        }
        while e < merged.edges.len() && in_shard(merged.edges[e].from) {
            writer.put(Kind::Edges, &merged.edges[e])?;
            e += 1;
        }
        while r < merged.relations.len() && in_shard(merged.relations[r].from) {
            writer.put(Kind::Relations, &merged.relations[r])?;
            r += 1;
        }
        while u < merged.unresolved.len() && in_shard(merged.unresolved[u].from) {
            writer.put(Kind::Unresolved, &merged.unresolved[u].borrow())?;
            u += 1;
        }
        let finished = writer.finish()?;
        for (kind, n) in finished.records {
            *summary.records.entry(kind).or_default() += n;
        }
        summary.bytes += finished.bytes;
    }
    Ok(summary)
}

/// The one merge at a time: a file under `merged/` that a run creates on
/// its way in and removes on its way out, full and incremental runs
/// alike, so two never append to the registry or the deltas at once. A
/// run that died leaves it; the message says to remove it.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    /// The lock's path under a store.
    #[must_use]
    pub fn path_of(store: &Store) -> PathBuf {
        store.root().join("merged").join("merge.lock")
    }

    /// Takes the lock.
    ///
    /// # Errors
    ///
    /// Another run holds it, or I/O.
    pub fn acquire(store: &Store) -> io::Result<Self> {
        let path = Self::path_of(store);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let holder = fs::read_to_string(&path).unwrap_or_default();
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "{}: another merge is running ({}); if none is, remove the file",
                        path.display(),
                        holder.trim()
                    ),
                ));
            }
            Err(e) => return Err(e),
        };
        writeln!(
            file,
            "pid {} since {}",
            std::process::id(),
            ingest::time::now()
        )?;
        Ok(Self { path })
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Opens one merged set file and checks that a merge wrote it, under this
/// binary's model version, in a directory named for this binary's merge
/// version.
///
/// # Errors
///
/// I/O, a file that is not the layout, or a header or a path that says
/// otherwise.
pub fn open(path: &Path) -> io::Result<Reader> {
    let reader = Reader::open(path)?;
    let header = reader.header();
    let bad = |what: String| io::Error::new(io::ErrorKind::InvalidData, what);
    if header.source != SOURCE {
        return Err(bad(format!("written by {:?}, not a merge", header.source)));
    }
    if header.model != model::VERSION {
        return Err(bad(format!(
            "model version {} is not this binary's {}",
            header.model,
            model::VERSION
        )));
    }
    let wanted = version();
    if !path
        .ancestors()
        .skip(1)
        .any(|p| p.file_name().and_then(|n| n.to_str()) == Some(wanted.as_str()))
    {
        return Err(bad(format!(
            "not under a {wanted} directory: another merge version, or moved"
        )));
    }
    Ok(reader)
}

#[cfg(test)]
mod tests {
    use ingest::Store;

    use super::{SHARD, first, prefix, shard, version};
    use crate::work::WorkId;

    #[test]
    fn shards_are_ranges_of_ids() {
        let id = |n| WorkId::new(n).unwrap();
        assert_eq!(shard(id(1)), 0);
        assert_eq!(shard(id(SHARD)), 0);
        assert_eq!(shard(id(SHARD + 1)), 1);
        assert_eq!(first(1), SHARD + 1);
        assert_eq!(
            prefix(std::path::Path::new("m"), 3).to_str().unwrap(),
            "m/shard_00003"
        );
        assert!(version().starts_with('v'));
    }

    #[test]
    fn one_merge_at_a_time() {
        let root = std::env::temp_dir().join(format!("ol-rht-merge-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = Store::new(&root);
        let held = super::Lock::acquire(&store).unwrap();
        let again = super::Lock::acquire(&store).unwrap_err();
        assert!(again.to_string().contains("another merge"), "{again}");
        drop(held);
        assert!(super::Lock::acquire(&store).is_ok());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
