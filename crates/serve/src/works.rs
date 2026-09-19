//! The works index (serving plan §3): a read index over the merged
//! shards, which stay as the merge writes them. The works are re-laid in
//! blocks of a few hundred by id, each block compressed on its own, with
//! an offsets array pointing at the blocks, so one lookup decompresses
//! one block and decodes one row: tens of microseconds, and only the
//! blocks touched are ever read. A row is [`Row`]: everything the merge
//! knows about the work and the text its records carry. Stubs are rows
//! too, with their names only; a retired id is an empty slot.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ingest::Kind;
use merge::public::Public;
use merge::record::Work;
use merge::{Document, WorkId};
use serde::{Deserialize, Serialize};

use crate::digest::{FileInfo, Sink};
use crate::error::Error;
use crate::mapped::{Mapped, at, u32_at, u64_at};
use crate::names::part_of;
use crate::row::Row;

/// Works per block.
pub const BLOCK: u32 = 256;
#[allow(missing_docs)]
pub const BLOCKS: &str = "blocks.zst";
#[allow(missing_docs)]
pub const OFFSETS: &str = "offsets.u64";
#[allow(missing_docs)]
pub const MANIFEST: &str = "manifest.json";

/// zstd level for a block.
const LEVEL: i32 = 3;

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
    /// Slots: the highest id.
    pub nodes: u32,
    #[allow(missing_docs)]
    pub works: u64,
    #[allow(missing_docs)]
    pub held: u64,
    #[allow(missing_docs)]
    pub stubs: u64,
    /// Rows with text.
    pub with_text: u64,
    /// Rows with a public id.
    pub with_public: u64,
    #[allow(missing_docs)]
    pub block: u32,
    #[allow(missing_docs)]
    pub blocks: u64,
    /// Rows' bytes before compression.
    pub raw_bytes: u64,
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

    fn write(&self, dir: &Path) -> Result<(), Error> {
        let path = dir.join(MANIFEST);
        let mut text = serde_json::to_string_pretty(self).map_err(|source| Error::Manifest {
            path: path.clone(),
            source,
        })?;
        text.push('\n');
        fs::write(&path, text).map_err(|e| Error::io(&path, e))
    }

    fn check(&self, dir: &Path) -> Result<(), Error> {
        if (self.serve, self.model, self.merge) != (crate::VERSION, model::VERSION, merge::VERSION)
            || self.block != BLOCK
        {
            return Err(Error::Mismatch(format!(
                "{}: built as serve {} over model {} and merge {} in blocks of {}, not this binary's {} in blocks of {BLOCK}",
                dir.display(),
                self.serve,
                self.model,
                self.merge,
                self.block,
                crate::version()
            )));
        }
        for file in [BLOCKS, OFFSETS] {
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

/// What a build left on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    #[allow(missing_docs)]
    pub dir: PathBuf,
    #[allow(missing_docs)]
    pub manifest: Manifest,
}

/// Rows accumulating into one block.
struct Block {
    slots: Vec<Vec<u8>>,
}

impl Block {
    fn new() -> Self {
        Self {
            slots: Vec::with_capacity(at(u64::from(BLOCK))),
        }
    }

    fn full(&self) -> bool {
        self.slots.len() == at(u64::from(BLOCK))
    }

    /// The block as bytes: `BLOCK + 1` little-endian `u32` slot offsets,
    /// then the slots, then compressed.
    fn encode(&mut self) -> Result<Vec<u8>, Error> {
        let empty = postcard::to_allocvec(&None::<Row>)?;
        while !self.full() {
            self.slots.push(empty.clone());
        }
        let body: usize = self.slots.iter().map(Vec::len).sum();
        let mut raw = Vec::with_capacity(self.slots.len() * 4 + 4 + body);
        let mut offset = 0u32;
        for slot in &self.slots {
            raw.extend_from_slice(&offset.to_le_bytes());
            offset +=
                u32::try_from(slot.len()).map_err(|_| Error::Mismatch("a row over 4 GB".into()))?;
        }
        raw.extend_from_slice(&offset.to_le_bytes());
        for slot in &self.slots {
            raw.extend_from_slice(slot);
        }
        let compressed = zstd::stream::encode_all(&raw[..], LEVEL)
            .map_err(|e| Error::io(Path::new(BLOCKS), e))?;
        self.slots.clear();
        Ok(compressed)
    }
}

/// Rows and counts accumulating through a build.
struct Writer {
    blocks: Sink,
    offsets: Vec<u64>,
    block: Block,
    /// The id the next slot stands for.
    next: u32,
    empty: Vec<u8>,
    works: u64,
    held: u64,
    stubs: u64,
    with_text: u64,
    with_public: u64,
    raw_bytes: u64,
}

impl Writer {
    fn slot(&mut self, bytes: Vec<u8>) -> Result<(), Error> {
        self.block.slots.push(bytes);
        if self.block.full() {
            self.blocks.put(&self.block.encode()?)?;
            self.offsets.push(self.blocks.position());
        }
        self.next += 1;
        Ok(())
    }

    /// One merged work: empty slots up to its id, then its row.
    fn work(
        &mut self,
        work: &Work<'_>,
        table: &model::symbol::Table,
        documents: &[Document],
        public: &Public,
    ) -> Result<(), Error> {
        if work.id.get() < self.next {
            return Err(Error::Mismatch(format!("{} out of order", work.id)));
        }
        while self.next < work.id.get() {
            self.slot(self.empty.clone())?;
        }
        let document = documents
            .binary_search_by_key(&work.id, |d| d.id)
            .ok()
            .map(|i| &documents[i]);
        let public_id = public.of(work.id);
        let row = Row::of(work, table, public_id, document);
        self.works += 1;
        if work.stub {
            self.stubs += 1;
        } else {
            self.held += 1;
        }
        if row.text.is_some() {
            self.with_text += 1;
        }
        if public_id.is_some() {
            self.with_public += 1;
        }
        let bytes = postcard::to_allocvec(&Some(row))?;
        self.raw_bytes += bytes.len() as u64;
        self.slot(bytes)
    }
}

/// Builds the index into `dir` from the works sets under `merged`, the
/// documents (sorted by id) for their text and the public log for the
/// ids, replacing what was there.
///
/// # Errors
///
/// I/O, a set that is not a merge's, or a row that does not encode.
pub fn build(
    dir: &Path,
    merged: &Path,
    documents: &[Document],
    public: &Public,
    batch: u64,
) -> Result<Built, Error> {
    let part = part_of(dir);
    if part.exists() {
        fs::remove_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    }
    fs::create_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    let mut w = Writer {
        blocks: Sink::create(&part.join(BLOCKS))?,
        offsets: vec![0],
        block: Block::new(),
        next: 1,
        empty: postcard::to_allocvec(&None::<Row>)?,
        works: 0,
        held: 0,
        stubs: 0,
        with_text: 0,
        with_public: 0,
        raw_bytes: 0,
    };
    for path in merge::sets::find(merged, Kind::Works).map_err(|e| Error::io(merged, e))? {
        let reader = merge::layout::open(&path).map_err(|e| Error::io(&path, e))?;
        let table = reader
            .table()
            .ok_or_else(|| Error::Mismatch(format!("{}: no symbol table", path.display())))?;
        reader.with_table(|| -> Result<(), Error> {
            for row in reader.records::<Work<'_>>() {
                let work = row.map_err(|e| Error::io(&path, e))?;
                w.work(&work, table, documents, public)
                    .map_err(|e| Error::Mismatch(format!("{}: {e}", path.display())))?;
            }
            Ok(())
        })?;
    }
    if !w.block.slots.is_empty() {
        w.blocks.put(&w.block.encode()?)?;
        w.offsets.push(w.blocks.position());
    }
    let mut files = BTreeMap::new();
    files.insert(BLOCKS.to_owned(), w.blocks.finish()?);
    let mut sink = Sink::create(&part.join(OFFSETS))?;
    for o in &w.offsets {
        sink.put(&o.to_le_bytes())?;
    }
    files.insert(OFFSETS.to_owned(), sink.finish()?);
    let manifest = Manifest {
        serve: crate::VERSION,
        model: model::VERSION,
        merge: merge::VERSION,
        batch,
        built_at: ingest::time::now(),
        nodes: w.next - 1,
        works: w.works,
        held: w.held,
        stubs: w.stubs,
        with_text: w.with_text,
        with_public: w.with_public,
        block: BLOCK,
        blocks: w.offsets.len() as u64 - 1,
        raw_bytes: w.raw_bytes,
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

/// One slot of a decompressed block: the row it holds, or nothing where
/// the id is retired or was never minted.
fn decode(raw: &[u8], slot: usize) -> Result<Option<Row>, Error> {
    let header = at(u64::from(BLOCK) + 1) * 4;
    let s0 = at(u64::from(u32_at(raw, slot)));
    let s1 = at(u64::from(u32_at(raw, slot + 1)));
    let body = &raw[header..];
    Ok(postcard::from_bytes::<Option<Row>>(&body[s0..s1])?)
}

/// An open index.
#[derive(Debug)]
pub struct Works {
    manifest: Manifest,
    blocks: Mapped,
    offsets: Mapped,
    /// The last block decompressed, so that ids running through one
    /// block decompress it once. Shared and not copied: a block of held
    /// works is two thirds of a megabyte raw on the epoch of
    /// 2026-09-19, and handing a lookup its own copy of that to read one
    /// row out of cost more than the rest of the lookup put together.
    last: Mutex<Option<(usize, Arc<Vec<u8>>)>>,
    /// Blocks actually decompressed since the index was opened, and the
    /// bytes they came to: what a lookup costs (serving plan §11, items
    /// 5 and 13), counted because it is the number the bulk read is
    /// measured by and two relaxed adds are nothing beside a zstd frame.
    decompressed: AtomicU64,
    #[allow(missing_docs)]
    decompressed_bytes: AtomicU64,
}

impl Works {
    /// Opens the index under `dir`, checking the manifest and the files'
    /// sizes.
    ///
    /// # Errors
    ///
    /// I/O, another version's base, or a file of the wrong size.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let manifest = Manifest::read(dir)?;
        manifest.check(dir)?;
        let blocks = Mapped::open(&dir.join(BLOCKS), manifest.files[BLOCKS].bytes)?;
        let offsets = Mapped::open(&dir.join(OFFSETS), manifest.files[OFFSETS].bytes)?;
        if manifest.files[OFFSETS].bytes != (manifest.blocks + 1) * 8 {
            return Err(Error::Mismatch(format!(
                "{}: {} blocks but {} offset bytes",
                dir.display(),
                manifest.blocks,
                manifest.files[OFFSETS].bytes
            )));
        }
        Ok(Self {
            manifest,
            blocks,
            offsets,
            last: Mutex::new(None),
            decompressed: AtomicU64::new(0),
            decompressed_bytes: AtomicU64::new(0),
        })
    }

    /// One block, decompressed: the one decompressed last, or one
    /// decompressed now.
    fn block(&self, block: usize) -> Result<Arc<Vec<u8>>, Error> {
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((b, raw)) = last.as_ref()
            && *b == block
        {
            return Ok(raw.clone());
        }
        let start = at(u64_at(self.offsets.bytes(), block));
        let end = at(u64_at(self.offsets.bytes(), block + 1));
        let raw = Arc::new(
            zstd::stream::decode_all(&self.blocks.bytes()[start..end])
                .map_err(|e| Error::io(Path::new(BLOCKS), e))?,
        );
        self.decompressed.fetch_add(1, Ordering::Relaxed);
        self.decompressed_bytes
            .fetch_add(raw.len() as u64, Ordering::Relaxed);
        *last = Some((block, raw.clone()));
        Ok(raw)
    }

    /// Blocks this index has decompressed for lookups since it was
    /// opened — [`get`](Self::get) and [`get_many`](Self::get_many),
    /// each counted only where the one-block cache missed. The whole
    /// scan of [`rows`](Self::rows) decompresses every block and is not
    /// counted: it reads past the cache by design.
    #[must_use]
    pub fn decompressed(&self) -> u64 {
        self.decompressed.load(Ordering::Relaxed)
    }

    /// What those blocks came to, decompressed: the bytes a lookup
    /// actually pays for, against the row it keeps.
    #[must_use]
    pub fn decompressed_bytes(&self) -> u64 {
        self.decompressed_bytes.load(Ordering::Relaxed)
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Slots: the highest id the index holds.
    #[must_use]
    pub const fn nodes(&self) -> u32 {
        self.manifest.nodes
    }

    /// One work's row; `None` for a retired id or one past the index.
    ///
    /// # Errors
    ///
    /// A block that does not decompress, or a row that does not decode.
    pub fn get(&self, id: WorkId) -> Result<Option<Row>, Error> {
        let ix = id.get() - 1;
        if ix >= self.manifest.nodes {
            return Ok(None);
        }
        let raw = self.block(at(u64::from(ix / BLOCK)))?;
        decode(&raw, at(u64::from(ix % BLOCK)))
    }

    /// The rows of many works in one walk of the index: each block the
    /// ids fall in decompressed once, however many of them it holds,
    /// against one decompression per id through [`get`](Self::get)
    /// (serving plan §11, item 13). Ids past the index and slots no
    /// live work holds are absent from the answer, so the result is
    /// shorter than `ids` whenever some id has no row.
    ///
    /// `ids` should be **sorted and distinct** — that is what makes a
    /// block's ids adjacent and so decompressed together. Any other
    /// order answers the same rows and only costs more: the ids of one
    /// block are gathered while they run consecutively and the next
    /// spelling of that block decompresses it again.
    ///
    /// # Errors
    ///
    /// A block that does not decompress, or a row that does not decode.
    pub fn get_many(&self, ids: &[WorkId]) -> Result<Vec<(WorkId, Row)>, Error> {
        let mut out = Vec::with_capacity(ids.len());
        let mut i = 0usize;
        while i < ids.len() {
            let ix = ids[i].get() - 1;
            if ix >= self.manifest.nodes {
                i += 1;
                continue;
            }
            let block = ix / BLOCK;
            let raw = self.block(at(u64::from(block)))?;
            // Every id of this block, while they run consecutively.
            while let Some(id) = ids.get(i) {
                let ix = id.get() - 1;
                if ix >= self.manifest.nodes || ix / BLOCK != block {
                    break;
                }
                if let Some(row) = decode(&raw, at(u64::from(ix % BLOCK)))? {
                    out.push((*id, row));
                }
                i += 1;
            }
        }
        Ok(out)
    }

    /// Every row in id order, block by block.
    pub fn rows(&self) -> impl Iterator<Item = Result<Row, Error>> + '_ {
        let blocks = at(self.manifest.blocks);
        (0..blocks).flat_map(move |b| {
            let start = at(u64_at(self.offsets.bytes(), b));
            let end = at(u64_at(self.offsets.bytes(), b + 1));
            let decoded = zstd::stream::decode_all(&self.blocks.bytes()[start..end])
                .map_err(|e| Error::io(Path::new(BLOCKS), e));
            let rows: Vec<Result<Row, Error>> = match decoded {
                Err(e) => vec![Err(e)],
                Ok(raw) => {
                    let header = at(u64::from(BLOCK) + 1) * 4;
                    let body = &raw[header..];
                    (0..at(u64::from(BLOCK)))
                        .filter_map(|slot| {
                            let s0 = at(u64::from(u32_at(&raw, slot)));
                            let s1 = at(u64::from(u32_at(&raw, slot + 1)));
                            match postcard::from_bytes::<Option<Row>>(&body[s0..s1]) {
                                Ok(Some(row)) => Some(Ok(row)),
                                Ok(None) => None,
                                Err(e) => Some(Err(Error::Postcard(e))),
                            }
                        })
                        .collect()
                }
            };
            rows.into_iter()
        })
    }
}
