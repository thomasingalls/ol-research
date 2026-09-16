//! The journal (serving plan §4): one line per merge run, numbered from
//! one — the clock every artifact and every answer is positioned by. A
//! full run replaces the merged sets and starts their deltas empty; an
//! incremental one (SP5) appends to them. Every delta line and every
//! epoch names a batch. Of record, beside the registry.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// What kind of run a line records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// The merged sets replaced whole.
    Full,
    /// Appended to their deltas.
    Incremental,
}

/// One run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    /// From one, consecutive.
    pub batch: u64,
    #[allow(missing_docs)]
    pub at: String,
    #[allow(missing_docs)]
    pub kind: Kind,
    /// Registry lines after this run.
    pub registry: u64,
    /// Public-id lines after this run.
    pub public: u64,
    /// Bytes each delta file reached after this run; empty for a full run.
    pub delta: BTreeMap<String, u64>,
    /// Normalized records merged.
    pub records: u64,
    #[allow(missing_docs)]
    pub minted: u64,
    #[allow(missing_docs)]
    pub merged: u64,
    #[allow(missing_docs)]
    pub split: u64,
    #[allow(missing_docs)]
    pub dropped: u64,
}

/// Every line, batches consecutive from one; none when the file does not
/// exist.
///
/// # Errors
///
/// I/O, a line that does not parse, or batches out of sequence.
pub fn read(path: &Path) -> io::Result<Vec<Line>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut lines = Vec::new();
    for (i, text) in BufReader::new(file).lines().enumerate() {
        let text = text?;
        if text.trim().is_empty() {
            continue;
        }
        let bad = |what: String| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: line {}: {what}", path.display(), i + 1),
            )
        };
        let line: Line = serde_json::from_str(&text).map_err(|e| bad(e.to_string()))?;
        let expected = lines.len() as u64 + 1;
        if line.batch != expected {
            return Err(bad(format!("batch {}, expected {expected}", line.batch)));
        }
        lines.push(line);
    }
    Ok(lines)
}

/// The last line, if any.
///
/// # Errors
///
/// As [`read`].
pub fn last(path: &Path) -> io::Result<Option<Line>> {
    Ok(read(path)?.pop())
}

/// Appends one line, which must be the batch after the last.
///
/// # Errors
///
/// I/O, or a batch out of sequence.
pub fn append(path: &Path, line: &Line) -> io::Result<()> {
    let expected = last(path)?.map_or(1, |l| l.batch + 1);
    if line.batch != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("batch {} appended where {expected} was next", line.batch),
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut text = serde_json::to_string(line)?;
    text.push('\n');
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Kind, Line, append, last, read};

    fn line(batch: u64) -> Line {
        Line {
            batch,
            at: "t".into(),
            kind: Kind::Full,
            registry: 10,
            public: 10,
            delta: BTreeMap::new(),
            records: 3,
            minted: 1,
            merged: 0,
            split: 0,
            dropped: 0,
        }
    }

    #[test]
    fn batches_are_consecutive() {
        let dir = std::env::temp_dir().join(format!("ol-rht-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("journal.jsonl");
        assert_eq!(last(&path).unwrap(), None);
        append(&path, &line(1)).unwrap();
        assert!(append(&path, &line(3)).is_err());
        append(&path, &line(2)).unwrap();
        assert_eq!(read(&path).unwrap().len(), 2);
        assert_eq!(last(&path).unwrap().unwrap().batch, 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
