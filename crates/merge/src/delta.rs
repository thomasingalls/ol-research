//! The delta logs (serving plan §3, §5, §8): what an incremental merge
//! appends beside the merged sets it changes — one JSON-lines file per
//! merged kind under the merged directory's `delta/`, every line stamped
//! with the batch that wrote it. A cache like the sets: a full merge
//! replaces the directory and the logs start over. Readers position
//! themselves by the byte offsets the journal records per batch, never
//! by anything they computed.
//!
//! ```text
//! delta/works.jsonl       put(work, text) — the merged record whole, latest wins; retire(id, into); drop(id)
//! delta/edges.jsonl       add(from, to, by); remove(from, to, by); retire(id, into)
//! delta/relations.jsonl   put(from, by, relations) — a source's relations of a work, whole; move(from, into)
//! delta/unresolved.jsonl  put(from, by, rows) — a source's unresolved references of a work, whole; move(from, into)
//! ```

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::build::{OwnedUnresolved, OwnedWork};
use crate::relation;
use crate::view::Text;
use crate::work::{SourceId, Sources, WorkId};

/// The directory under the merged directory.
pub const DIR: &str = "delta";
#[allow(missing_docs)]
pub const WORKS: &str = "works.jsonl";
#[allow(missing_docs)]
pub const EDGES: &str = "edges.jsonl";
#[allow(missing_docs)]
pub const RELATIONS: &str = "relations.jsonl";
#[allow(missing_docs)]
pub const UNRESOLVED: &str = "unresolved.jsonl";

#[allow(missing_docs)]
pub const CITED_AS: &str = "cited_as.jsonl";

/// The five logs, in the order the journal lists them. `cited_as` joined
/// them 2026-09-18 (serving plan §11, item 26).
pub const FILES: [&str; 5] = [WORKS, EDGES, RELATIONS, UNRESOLVED, CITED_AS];

/// The key a log's byte count is filed under in the journal: the file
/// name without its extension.
#[must_use]
pub fn key(file: &str) -> &str {
    file.strip_suffix(".jsonl").unwrap_or(file)
}

/// Where the logs live under a merged directory.
#[must_use]
pub fn dir(merged: &Path) -> PathBuf {
    merged.join(DIR)
}

/// One line of the works log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum WorkLine {
    /// The merged record whole, with the text its promoted records carry;
    /// the latest line for an id wins.
    Put {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        work: Box<OwnedWork>,
        /// None for a stub.
        text: Option<Text>,
    },
    /// An id retired by the registry: merged into `into`, or split.
    Retire {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        into: Option<WorkId>,
    },
    /// A work with no records and nothing citing it any more: no row.
    Drop {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        id: WorkId,
    },
}

/// One line of the edges log (graph plan §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum EdgeLine {
    /// Sources asserting an arc that did not assert it before.
    Add {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        to: WorkId,
        #[allow(missing_docs)]
        by: Sources,
    },
    /// Sources withdrawing an arc; an arc nobody asserts is gone.
    Remove {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        to: WorkId,
        #[allow(missing_docs)]
        by: Sources,
    },
    /// The registry's `merge` or `split` echoed, so the graph answers for
    /// a retired id as its survivor without the registry.
    Retire {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        into: Option<WorkId>,
    },
}

/// One relation as the relations log carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Related {
    #[allow(missing_docs)]
    pub to: WorkId,
    #[allow(missing_docs)]
    pub kind: relation::Kind,
}

/// One line of the relations log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum RelationLine {
    /// The relations `by`'s record of `from` asserts, whole.
    Put {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        by: SourceId,
        #[allow(missing_docs)]
        relations: Vec<Related>,
    },
    /// Every relation of `from` now belongs to `into`: a merge.
    Move {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        into: WorkId,
    },
}

/// One line of the cited-as log: the names a work's references of one
/// source gave beside the works they landed on, whole (2026-09-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum CitedAsLine {
    /// What `by`'s record of `from` gave, whole.
    Put {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        by: SourceId,
        #[allow(missing_docs)]
        rows: Vec<crate::build::OwnedCitedAs>,
    },
    /// Every row of `from` now belongs to `into`: a merge.
    Move {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        into: WorkId,
    },
}

/// One line of the unresolved log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum UnresolvedLine {
    /// The unresolved references of `by`'s record of `from`, whole.
    Put {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        by: SourceId,
        #[allow(missing_docs)]
        rows: Vec<OwnedUnresolved>,
    },
    /// Every unresolved reference of `from` now belongs to `into`.
    Move {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        from: WorkId,
        #[allow(missing_docs)]
        into: WorkId,
    },
}

/// The batch a line carries.
pub trait Stamped {
    #[allow(missing_docs)]
    fn batch(&self) -> u64;
}

impl Stamped for WorkLine {
    fn batch(&self) -> u64 {
        match self {
            Self::Put { batch, .. } | Self::Retire { batch, .. } | Self::Drop { batch, .. } => {
                *batch
            }
        }
    }
}

impl Stamped for EdgeLine {
    fn batch(&self) -> u64 {
        match self {
            Self::Add { batch, .. } | Self::Remove { batch, .. } | Self::Retire { batch, .. } => {
                *batch
            }
        }
    }
}

impl Stamped for CitedAsLine {
    fn batch(&self) -> u64 {
        match self {
            Self::Put { batch, .. } | Self::Move { batch, .. } => *batch,
        }
    }
}

impl Stamped for RelationLine {
    fn batch(&self) -> u64 {
        match self {
            Self::Put { batch, .. } | Self::Move { batch, .. } => *batch,
        }
    }
}

impl Stamped for UnresolvedLine {
    fn batch(&self) -> u64 {
        match self {
            Self::Put { batch, .. } | Self::Move { batch, .. } => *batch,
        }
    }
}

/// One log open for appending, its byte position tracked so the journal
/// can record where the batch ended.
#[derive(Debug)]
pub struct Appender {
    path: PathBuf,
    file: io::BufWriter<File>,
    position: u64,
}

impl Appender {
    /// Opens the log, creating it and its directory if needed.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn open(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let position = file.metadata()?.len();
        Ok(Self {
            path: path.to_path_buf(),
            file: io::BufWriter::with_capacity(1 << 20, file),
            position,
        })
    }

    /// Bytes the log holds, the buffered ones included.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.position
    }

    /// Appends one line.
    ///
    /// # Errors
    ///
    /// I/O, or a line that does not serialize.
    pub fn put<T: Serialize>(&mut self, line: &T) -> io::Result<()> {
        let mut text = serde_json::to_vec(line)?;
        text.push(b'\n');
        self.file.write_all(&text)?;
        self.position += text.len() as u64;
        Ok(())
    }

    /// Flushes and syncs.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn finish(self) -> io::Result<u64> {
        let file = self
            .file
            .into_inner()
            .map_err(io::IntoInnerError::into_error)?;
        file.sync_all()?;
        Ok(self.position)
    }

    #[allow(missing_docs)]
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Reads the lines between two byte offsets, which must both fall on
/// line boundaries, checking every line's batch against `batch` when one
/// is given. A missing file with `from == to == 0` is empty.
///
/// # Errors
///
/// I/O, a range past the file or not on line boundaries, a line that
/// does not parse, or a line stamped with another batch.
pub fn read<T: DeserializeOwned + Stamped>(
    path: &Path,
    from: u64,
    to: u64,
    batch: Option<u64>,
) -> io::Result<Vec<T>> {
    if from == to {
        return Ok(Vec::new());
    }
    let bad = |what: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {what}", path.display()),
        )
    };
    if from > to {
        return Err(bad(format!("a range from {from} back to {to}")));
    }
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if to > len {
        return Err(bad(format!("{to} bytes asked of {len}")));
    }
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = vec![0u8; usize::try_from(to - from).map_err(|_| bad("too long".into()))?];
    file.read_exact(&mut bytes)?;
    if bytes.last() != Some(&b'\n') {
        return Err(bad(format!("the range {from}..{to} does not end a line")));
    }
    let mut out = Vec::new();
    for (i, line) in bytes.split(|b| *b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let value: T = serde_json::from_slice(line)
            .map_err(|e| bad(format!("line {} after byte {from}: {e}", i + 1)))?;
        if let Some(expected) = batch
            && value.batch() != expected
        {
            return Err(bad(format!(
                "line {} after byte {from} is batch {}, not {expected}",
                i + 1,
                value.batch()
            )));
        }
        out.push(value);
    }
    Ok(out)
}

/// Reads the lines of any JSON-lines file between two byte offsets on
/// line boundaries: the registry's or the public log's tail past an
/// epoch, which carry no batch of their own.
///
/// # Errors
///
/// As [`read`], batches aside.
pub fn read_plain<T: DeserializeOwned>(path: &Path, from: u64, to: u64) -> io::Result<Vec<T>> {
    if from == to {
        return Ok(Vec::new());
    }
    let bad = |what: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {what}", path.display()),
        )
    };
    if from > to {
        return Err(bad(format!("a range from {from} back to {to}")));
    }
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if to > len {
        return Err(bad(format!("{to} bytes asked of {len}")));
    }
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = vec![0u8; usize::try_from(to - from).map_err(|_| bad("too long".into()))?];
    file.read_exact(&mut bytes)?;
    if bytes.last() != Some(&b'\n') {
        return Err(bad(format!("the range {from}..{to} does not end a line")));
    }
    let mut out = Vec::new();
    for (i, line) in bytes.split(|b| *b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let value: T = serde_json::from_slice(line)
            .map_err(|e| bad(format!("line {} after byte {from}: {e}", i + 1)))?;
        out.push(value);
    }
    Ok(out)
}

/// One line at a byte offset, read positionally, for a reader that
/// indexed it and holds the file open.
///
/// # Errors
///
/// I/O, an offset past the file, or a line that does not parse.
pub fn line_at<T: DeserializeOwned>(file: &File, offset: u64) -> io::Result<T> {
    use std::os::unix::fs::FileExt;
    let mut line = Vec::new();
    let mut chunk = vec![0u8; 1 << 14];
    let mut at = offset;
    loop {
        let n = file.read_at(&mut chunk, at)?;
        if n == 0 {
            break;
        }
        if let Some(end) = chunk[..n].iter().position(|b| *b == b'\n') {
            line.extend_from_slice(&chunk[..end]);
            break;
        }
        line.extend_from_slice(&chunk[..n]);
        at += n as u64;
    }
    if line.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("no line at byte {offset}"),
        ));
    }
    serde_json::from_slice(&line).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("line at byte {offset}: {e}"),
        )
    })
}

/// One line at a byte offset, for a reader that indexed it.
///
/// # Errors
///
/// I/O, an offset past the file, or a line that does not parse.
pub fn read_at<T: DeserializeOwned>(file: &mut File, offset: u64) -> io::Result<T> {
    file.seek(SeekFrom::Start(offset))?;
    let mut reader = BufReader::with_capacity(1 << 16, file);
    let mut line = Vec::new();
    reader.read_until(b'\n', &mut line)?;
    serde_json::from_slice(&line).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("line at byte {offset}: {e}"),
        )
    })
}

/// The byte offset of every line in a range, with the line: what a
/// reader keeps when it indexes the log by id rather than holding the
/// rows.
///
/// # Errors
///
/// As [`read`].
pub fn read_indexed<T: DeserializeOwned + Stamped>(
    path: &Path,
    from: u64,
    to: u64,
    batch: Option<u64>,
) -> io::Result<Vec<(u64, T)>> {
    if from == to {
        return Ok(Vec::new());
    }
    let bad = |what: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {what}", path.display()),
        )
    };
    if from > to {
        return Err(bad(format!("a range from {from} back to {to}")));
    }
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if to > len {
        return Err(bad(format!("{to} bytes asked of {len}")));
    }
    file.seek(SeekFrom::Start(from))?;
    let mut reader = BufReader::with_capacity(1 << 20, file);
    let mut out = Vec::new();
    let mut offset = from;
    let mut line = Vec::new();
    while offset < to {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 || line.last() != Some(&b'\n') {
            return Err(bad(format!("the range {from}..{to} does not end a line")));
        }
        let value: T = serde_json::from_slice(&line)
            .map_err(|e| bad(format!("line at byte {offset}: {e}")))?;
        if let Some(expected) = batch
            && value.batch() != expected
        {
            return Err(bad(format!(
                "line at byte {offset} is batch {}, not {expected}",
                value.batch()
            )));
        }
        out.push((offset, value));
        offset += n as u64;
    }
    if offset != to {
        return Err(bad(format!("the range {from}..{to} does not end a line")));
    }
    Ok(out)
}

/// The size of a log, zero when it does not exist.
///
/// # Errors
///
/// I/O other than absence.
pub fn len(path: &Path) -> io::Result<u64> {
    match fs::metadata(path) {
        Ok(m) => Ok(m.len()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(e),
    }
}

/// Cuts a log back to `at` bytes: what a batch that never reached the
/// journal left behind is not part of any batch.
///
/// # Errors
///
/// I/O, or a log shorter than `at`.
pub fn truncate(path: &Path, at: u64) -> io::Result<()> {
    let len = len(path)?;
    if len == at {
        return Ok(());
    }
    if len < at {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}: {len} bytes, shorter than the {at} the journal recorded",
                path.display()
            ),
        ));
    }
    let file = OpenOptions::new().write(true).open(path)?;
    file.set_len(at)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::{Appender, EdgeLine, Stamped, len, read, read_at, read_indexed, truncate};
    use crate::work::{SourceId, Sources, WorkId};

    fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    #[test]
    fn lines_append_and_read_back_by_offset() {
        let dir = std::env::temp_dir().join(format!("ol-rht-delta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("edges.jsonl");
        assert_eq!(len(&path).unwrap(), 0);
        let by: Sources = [SourceId::Pubmed].into_iter().collect();
        let mut a = Appender::open(&path).unwrap();
        a.put(&EdgeLine::Add {
            batch: 3,
            from: id(1),
            to: id(2),
            by,
        })
        .unwrap();
        let first = a.finish().unwrap();
        let mut a = Appender::open(&path).unwrap();
        assert_eq!(a.position(), first);
        a.put(&EdgeLine::Retire {
            batch: 4,
            id: id(2),
            into: Some(id(1)),
        })
        .unwrap();
        a.put(&EdgeLine::Remove {
            batch: 4,
            from: id(1),
            to: id(2),
            by,
        })
        .unwrap();
        let second = a.finish().unwrap();
        assert_eq!(len(&path).unwrap(), second);
        let lines: Vec<EdgeLine> = read(&path, 0, first, Some(3)).unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].batch(), 3);
        let lines: Vec<EdgeLine> = read(&path, first, second, Some(4)).unwrap();
        assert_eq!(lines.len(), 2);
        assert!(matches!(lines[0], EdgeLine::Retire { .. }));
        assert!(
            read::<EdgeLine>(&path, 0, second, Some(3)).is_err(),
            "mixed batches"
        );
        assert!(
            read::<EdgeLine>(&path, 0, first - 1, None).is_err(),
            "mid-line"
        );
        assert!(
            read::<EdgeLine>(&path, 0, second + 1, None).is_err(),
            "past the end"
        );
        let indexed: Vec<(u64, EdgeLine)> = read_indexed(&path, first, second, None).unwrap();
        assert_eq!(indexed[0].0, first);
        let mut file = std::fs::File::open(&path).unwrap();
        let at: EdgeLine = read_at(&mut file, indexed[1].0).unwrap();
        assert_eq!(at, lines[1]);
        let positional: EdgeLine = super::line_at(&file, indexed[1].0).unwrap();
        assert_eq!(positional, lines[1]);
        let plain: Vec<EdgeLine> = super::read_plain(&path, 0, second).unwrap();
        assert_eq!(plain.len(), 3);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.starts_with(r#"{"op":"add","batch":3,"from":1,"to":2,"by":1}"#),
            "{text}"
        );
        truncate(&path, first).unwrap();
        assert_eq!(len(&path).unwrap(), first);
        assert!(truncate(&path, second).is_err());
        assert_eq!(super::key(super::WORKS), "works");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
