//! The cited-as lookup (serving plan §3, 2026-09-18): for a name nothing
//! held, the references that gave it beside the work they landed on —
//! merge plan §11 item 14's evidence, in the shape a lookup by name
//! wants. A batch that gives such a name to a work or a stub reads this
//! to resolve those references again, which is the divergence of §11
//! item 26.
//!
//! The same shape as the names lookup: a transducer from the name's text
//! to a run of fixed-size rows in a side file, both mapped. Its delta is
//! the `cited_as` log the merged layer appends, folded by
//! [`crate::fold`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use fst::{Map, MapBuilder};
use ingest::{Key, Kind};
use memmap2::Mmap;
use merge::record::CitedAs;
use merge::{SourceId, WorkId};
use serde::{Deserialize, Serialize};

use crate::digest::{FileInfo, Sink};
use crate::error::Error;
use crate::mapped::Mapped;

#[allow(missing_docs)]
pub const FST: &str = "cited.fst";
#[allow(missing_docs)]
pub const ROWS: &str = "cited.rows";
#[allow(missing_docs)]
pub const MANIFEST: &str = "manifest.json";

/// Bytes per row in [`ROWS`].
pub const ROW: usize = 12;

/// One reference that gave a name beside the work it landed on.
// Five flags, each a fact §6 and §11 item 14 name about one row:
// what it was for, how it landed, whether it can move, and whether a
// held work stands beside it.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Row {
    /// The citing work.
    pub from: WorkId,
    /// The work the reference landed on.
    pub to: WorkId,
    /// The source whose record cited it.
    pub by: SourceId,
    /// The namespace that landed it there.
    pub landed: Key,
    /// Whether this was the only reference of that work and source
    /// landing on `to`.
    pub sole: bool,
    /// A relation named it, not a reference: no arc and no citer.
    pub relation: bool,
    /// The name named no work when the reference was read: only such a
    /// row can move an arc (serving plan §11, item 26).
    pub beside: bool,
    /// The citation text repeats a DOI of the work it landed on, so the
    /// order alone does not move the arc (merge plan §6).
    pub by_text: bool,
    /// The citation text repeats this name.
    pub in_text: bool,
    /// The reference also named a held work other than the one it landed
    /// on, so item 14 makes no stub name of this one when the landing's
    /// work is divided later (2026-09-19).
    pub beside_held: bool,
}

impl Row {
    /// The row of a merged set's line.
    #[must_use]
    pub fn of(row: &CitedAs<'_>) -> Self {
        Self {
            from: row.from,
            to: row.to,
            by: row.by,
            landed: row.landed,
            sole: row.sole,
            relation: row.relation,
            beside: row.beside,
            by_text: row.by_text,
            in_text: row.in_text,
            beside_held: row.beside_held,
        }
    }

    fn write(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.from.get().to_le_bytes());
        out.extend_from_slice(&self.to.get().to_le_bytes());
        out.push(self.by as u8);
        out.push(key_bits(self.landed));
        out.push(u8::from(self.sole));
        out.push(
            u8::from(self.relation)
                | (u8::from(self.beside) << 1)
                | (u8::from(self.by_text) << 2)
                | (u8::from(self.in_text) << 3)
                | (u8::from(self.beside_held) << 4),
        );
    }

    fn read(bytes: &[u8]) -> Option<Self> {
        let at = |i: usize| -> u32 {
            u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
        };
        if bytes.len() < ROW {
            return None;
        }
        Some(Self {
            from: WorkId::new(at(0))?,
            to: WorkId::new(at(4))?,
            by: *SourceId::ALL.get(bytes[8] as usize)?,
            landed: *Key::ALL.get(bytes[9] as usize)?,
            sole: bytes[10] != 0,
            relation: bytes[11] & 1 != 0,
            beside: bytes[11] & 2 != 0,
            by_text: bytes[11] & 4 != 0,
            in_text: bytes[11] & 8 != 0,
            beside_held: bytes[11] & 16 != 0,
        })
    }
}

/// A namespace's place in [`Key::ALL`], which the row stores.
fn key_bits(key: Key) -> u8 {
    Key::ALL
        .iter()
        .position(|k| *k == key)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

/// What a build records about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[allow(missing_docs)]
    pub serve: u32,
    #[allow(missing_docs)]
    pub model: u32,
    #[allow(missing_docs)]
    pub merge: u32,
    /// The journal batch the base is built from.
    pub batch: u64,
    #[allow(missing_docs)]
    pub built_at: String,
    /// Distinct names.
    pub keys: u64,
    /// Rows over every name.
    pub rows: u64,
    #[allow(missing_docs)]
    pub files: BTreeMap<String, FileInfo>,
}

impl Manifest {
    /// Reads `dir`'s manifest.
    ///
    /// # Errors
    ///
    /// I/O, or a manifest that does not parse.
    pub fn read(dir: &Path) -> Result<Self, Error> {
        let path = dir.join(MANIFEST);
        let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        serde_json::from_str(&text).map_err(|source| Error::Manifest { path, source })
    }

    /// Writes it into `dir`.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn write(&self, dir: &Path) -> Result<(), Error> {
        let path = dir.join(MANIFEST);
        let text = serde_json::to_string_pretty(self).map_err(|source| Error::Manifest {
            path: path.clone(),
            source,
        })?;
        fs::write(&path, text + "\n").map_err(|e| Error::io(&path, e))
    }

    /// Checks every file's digest and size against the manifest.
    ///
    /// # Errors
    ///
    /// I/O, or a file that does not match.
    pub fn check(&self, dir: &Path) -> Result<(), Error> {
        if (self.serve, self.model, self.merge) != (crate::VERSION, model::VERSION, merge::VERSION)
        {
            return Err(Error::Mismatch(format!(
                "{}: built as serve {} over model {} and merge {}, not this binary's {}",
                dir.display(),
                self.serve,
                self.model,
                self.merge,
                crate::version()
            )));
        }
        for file in [FST, ROWS] {
            if !self.files.contains_key(file) {
                return Err(Error::Mismatch(format!(
                    "{}: the manifest lists no {file}",
                    dir.display()
                )));
            }
        }
        Ok(())
    }
}

#[allow(missing_docs)]
#[derive(Debug)]
pub struct Built {
    #[allow(missing_docs)]
    pub dir: PathBuf,
    #[allow(missing_docs)]
    pub manifest: Manifest,
}

/// Bytes of rows a sorted run holds before it is written.
const RUN: usize = 256 << 20;

/// Builds the lookup into `dir` from the `cited_as` sets under `merged`,
/// replacing what was there. The rows are sorted on disk and the
/// transducer and the rows file written as the sorted rows come back
/// (the serving layer's memory, 2026-09-24): on the corpus one sorted
/// `Vec` of them was 55–65 GB.
///
/// # Errors
///
/// I/O, a set that is not a merge's, or the transducer refusing a key.
pub fn build(dir: &Path, merged: &Path, batch: u64) -> Result<Built, Error> {
    static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "ol-rht-cited-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut runs: merge::runs::Runs<(String, Row)> =
        merge::runs::Runs::create(&scratch, RUN).map_err(|e| Error::io(&scratch, e))?;
    for path in merge::sets::find(merged, Kind::CitedAs).map_err(|e| Error::io(merged, e))? {
        let reader = merge::layout::open(&path).map_err(|e| Error::io(&path, e))?;
        reader.with_table(|| -> Result<(), Error> {
            for row in reader.records::<CitedAs<'_>>() {
                let row = row.map_err(|e| Error::io(&path, e))?;
                let name = row.name.to_string();
                let bytes = name.len();
                runs.push((name, Row::of(&row)), bytes)
                    .map_err(|e| Error::io(&scratch, e))?;
            }
            Ok(())
        })?;
    }
    let sorted = runs.sorted().map_err(|e| Error::io(&scratch, e))?;
    let part = crate::names::part_of(dir);
    if part.exists() {
        fs::remove_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    }
    fs::create_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    let mut builder = MapBuilder::new(Sink::create(&part.join(FST))?)?;
    let mut rows = Sink::create(&part.join(ROWS))?;
    let mut bytes: Vec<u8> = Vec::with_capacity(ROW);
    let (mut keys, mut written) = (0u64, 0u64);
    // The key being written, the row it starts at, and how many it has.
    let mut run: Option<(String, u64, u64)> = None;
    let mut close =
        |builder: &mut MapBuilder<Sink>, run: (String, u64, u64)| -> Result<(), Error> {
            let (key, start, len) = run;
            let offset = u32::try_from(start)
                .map_err(|_| Error::Mismatch("too many cited-as rows".into()))?;
            let len = u32::try_from(len).unwrap_or(u32::MAX);
            builder.insert(key, (u64::from(offset) << 32) | u64::from(len))?;
            keys += 1;
            Ok(())
        };
    for item in sorted {
        let (name, row) = item.map_err(|e| Error::io(&scratch, e))?;
        run = match run.take() {
            Some((key, start, len)) if key == name => Some((key, start, len + 1)),
            Some(done) => {
                close(&mut builder, done)?;
                Some((name, written, 1))
            }
            None => Some((name, written, 1)),
        };
        bytes.clear();
        row.write(&mut bytes);
        rows.put(&bytes)?;
        written += 1;
    }
    if let Some(done) = run {
        close(&mut builder, done)?;
    }
    let mut files = BTreeMap::new();
    files.insert(FST.to_owned(), builder.into_inner()?.finish()?);
    files.insert(ROWS.to_owned(), rows.finish()?);
    let manifest = Manifest {
        serve: crate::VERSION,
        model: model::VERSION,
        merge: merge::VERSION,
        batch,
        built_at: ingest::time::now(),
        keys,
        rows: written,
        files,
    };
    manifest.write(&part)?;
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::rename(&part, dir).map_err(|e| Error::io(dir, e))?;
    Ok(Built {
        dir: dir.to_path_buf(),
        manifest,
    })
}

/// An open lookup.
pub struct Cited {
    manifest: Manifest,
    map: Option<Map<Mmap>>,
    rows: Mapped,
}

impl std::fmt::Debug for Cited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cited")
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}

impl Cited {
    /// Opens the lookup under `dir`, checking the manifest and every
    /// file's size.
    ///
    /// # Errors
    ///
    /// I/O, another version's base, or a file of the wrong size.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let manifest = Manifest::read(dir)?;
        manifest.check(dir)?;
        let open = |name: &str| Mapped::open(&dir.join(name), manifest.files[name].bytes);
        let map = match open(FST)?.into_inner() {
            Some(bytes) => Some(Map::new(bytes)?),
            None => None,
        };
        let rows = open(ROWS)?;
        if manifest.files[ROWS].bytes != manifest.rows * ROW as u64 {
            return Err(Error::Mismatch(format!(
                "{}: {} rows but {} bytes",
                dir.display(),
                manifest.rows,
                manifest.files[ROWS].bytes
            )));
        }
        Ok(Self {
            manifest,
            map,
            rows,
        })
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Every name the lookup has rows for, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        use fst::Streamer;
        let Some(map) = self.map.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut stream = map.stream();
        while let Some((key, _)) = stream.next() {
            out.push(String::from_utf8_lossy(key).into_owned());
        }
        out
    }

    /// The references that gave `name` beside the work they landed on.
    #[must_use]
    pub fn rows_of(&self, name: &str) -> Vec<Row> {
        let Some(map) = self.map.as_ref() else {
            return Vec::new();
        };
        let Some(value) = map.get(name) else {
            return Vec::new();
        };
        let offset = usize::try_from(value >> 32).unwrap_or(usize::MAX);
        let len = usize::try_from(value & 0xFFFF_FFFF).unwrap_or(0);
        let bytes = self.rows.bytes();
        (0..len)
            .filter_map(|i| {
                let at = (offset + i) * ROW;
                bytes.get(at..at + ROW).and_then(Row::read)
            })
            .collect()
    }
}
