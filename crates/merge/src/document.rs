//! The assembled work (serving plan §3, §7): the merged record's promoted
//! fields joined with the text its promoted source records carry —
//! abstract, authors, venue name — which the merged record indexes but
//! does not hold (merge plan §5). What the lexical index indexes now and
//! what the works index serves at SP3.
//!
//! Assembly is a join in two passes: the works set says, per held work,
//! which source's record supplies each text; then each source's
//! normalized works sets are streamed once and the wanted records give
//! their text through [`Contributor::text`]. Stubs have no text and no
//! document.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::kind::{Form, Genre};
use crate::name::OwnedName;
use crate::record::Work;
use crate::spill::Spill;
use crate::view::{Text, Unmapped};
use crate::{Contributor, SourceId, Sources, WorkId};
use ingest::Kind;
use model::Lang;

/// An assembly that stopped.
#[derive(Debug)]
pub enum Error {
    /// I/O on a set.
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: io::Error,
    },
    /// A normalized set without its symbol table.
    NoTable(#[allow(missing_docs)] PathBuf),
    /// What was given a range of documents could not take it: the
    /// lexical index, say.
    Consumer(#[allow(missing_docs)] String),
    /// A record's text could not be read: a symbol beyond the table, or
    /// an abstract the source encoded in a way we cannot decode.
    Unmapped {
        #[allow(missing_docs)]
        path: PathBuf,
        /// The record's position in its set, from one.
        ordinal: u64,
        #[allow(missing_docs)]
        source: Unmapped,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NoTable(path) => write!(f, "{}: no symbol table beside it", path.display()),
            Self::Consumer(what) => f.write_str(what),
            Self::Unmapped {
                path,
                ordinal,
                source,
            } => write!(f, "{}: record {ordinal}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Unmapped { source, .. } => Some(source),
            Self::NoTable(_) | Self::Consumer(_) => None,
        }
    }
}

/// One work as the serving layer sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    #[allow(missing_docs)]
    pub id: WorkId,
    /// The promoted title, markup stripped.
    pub title: Option<String>,
    /// The abstract of the first source in the merge plan's precedence
    /// that has one — PubMed, OpenAlex, Crossref — as plain text.
    pub abstract_text: Option<String>,
    /// From the source the author count was promoted from.
    pub authors: Vec<String>,
    /// The venue's name from the source the venue was promoted from.
    pub venue: Option<String>,
    #[allow(missing_docs)]
    pub year: Option<u16>,
    #[allow(missing_docs)]
    pub form: Option<Form>,
    #[allow(missing_docs)]
    pub genre: Option<Genre>,
    #[allow(missing_docs)]
    pub language: Option<Lang>,
    #[allow(missing_docs)]
    pub retracted: bool,
    #[allow(missing_docs)]
    pub corrected: bool,
    #[allow(missing_docs)]
    pub concern: bool,
    #[allow(missing_docs)]
    pub access: Option<bool>,
    /// The sources holding a record of the work.
    pub held_by: Sources,
}

impl Document {
    /// A held work's document from its merged row and, when given, the
    /// text its promoted records carry; a stub's document is empty but
    /// for its id.
    #[must_use]
    pub fn of(work: &Work<'_>, text: Option<&Text>) -> Self {
        let f = &work.fields;
        Self {
            id: work.id,
            title: f.title.as_ref().map(|t| crate::view::strip_markup(t.value)),
            abstract_text: text.and_then(|t| t.abstract_text.clone()),
            authors: text.map(|t| t.authors.clone()).unwrap_or_default(),
            venue: text.and_then(|t| t.venue.clone()),
            year: f.published.as_ref().map(|p| p.value.year.get()),
            form: f.form.as_ref().map(|x| x.value),
            genre: f.genre.as_ref().map(|x| x.value),
            language: f.language.as_ref().map(|x| x.value),
            retracted: !f.status.retracted.is_empty(),
            corrected: !f.status.corrected.is_empty(),
            concern: !f.status.concern.is_empty(),
            access: f.access.as_ref().map(|x| x.value),
            held_by: work.records.iter().map(|r| r.source).collect(),
        }
    }
}

/// The source a work's abstract comes from: the first in the merge
/// plan's precedence — PubMed, OpenAlex, Crossref — that has one.
#[must_use]
pub fn abstract_source(work: &Work<'_>) -> Option<SourceId> {
    [SourceId::Pubmed, SourceId::OpenAlex, SourceId::Crossref]
        .into_iter()
        .find(|s| work.fields.abstract_in.contains(*s))
}

/// Which sources a document still wants text from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Wanted {
    abstract_text: Option<SourceId>,
    authors: Option<SourceId>,
    venue: Option<SourceId>,
}

/// What an assembly counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Held works, each a document.
    pub documents: u64,
    /// Stubs, skipped.
    pub stubs: u64,
    #[allow(missing_docs)]
    pub with_title: u64,
    /// Documents with an abstract, by the source it came from.
    pub abstract_from: BTreeMap<SourceId, u64>,
    #[allow(missing_docs)]
    pub with_authors: u64,
    #[allow(missing_docs)]
    pub author_names: u64,
    #[allow(missing_docs)]
    pub with_venue: u64,
    /// Normalized records read, by source.
    pub records: BTreeMap<SourceId, u64>,
    /// Records that supplied text, by source.
    pub supplied: BTreeMap<SourceId, u64>,
}

/// Ids a range of the spill spans: eight of the merge's shards.
pub const RANGE: u32 = 8 * crate::layout::SHARD;

/// One piece of the join as it is spilled, under its work's id: the
/// work's document without its text, or one copy's text for it, in the
/// order the join made them — every document first, since the merged sets
/// are read before any normalized one, then the text in the order the
/// normalized sets are read, which is the order [`fill`] relies on.
#[derive(Debug, Serialize, Deserialize)]
enum Piece {
    Document(Document),
    Text {
        id: WorkId,
        source: SourceId,
        wanted: Wanted,
        text: Text,
        speaks: bool,
    },
}

/// Where a record's text goes: its work, whether it is its source's
/// promoted record, what the work wants, and the delivery of the copy that
/// speaks for it (merge plan §8) — its stamp, its version and its set, the
/// set a place in [`Assembler`]'s list of sets.
#[derive(Debug, Clone, Copy)]
struct Target {
    id: WorkId,
    promoted: bool,
    wanted: Wanted,
    revised: Option<model::Date>,
    version: u32,
    set: u32,
}

/// The join (serving plan §3, §7). Documents and their text go to disk by
/// work id as they are made ([`Spill`]) and come back a range of ids at a
/// time ([`Assembly`]), so what is held is one range's documents and, for
/// the join, one small entry per source record: on the corpus the whole
/// text in memory at once was 80–120 GB (the serving layer's memory,
/// 2026-09-24; `docs/inbox/serving-memory.md`). A small store's documents
/// are one range, and [`Assembler::finish`] gathers every range.
#[derive(Debug)]
pub struct Assembler {
    spill: Spill<Piece>,
    /// Every held work's record by its native name.
    targets: HashMap<OwnedName, Target>,
    sets: Vec<Box<str>>,
    set_ix: HashMap<Box<str>, u32>,
    stats: Stats,
}

impl Assembler {
    /// Reads every works set under a merged directory, spilling under a
    /// directory of its own in the system's temporary directory; see
    /// [`Assembler::read_in`].
    ///
    /// # Errors
    ///
    /// I/O, or a set that is not a merge's.
    pub fn read(merged: &Path) -> Result<Self, Error> {
        static RUNS: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ol-rht-documents-{}-{}",
            std::process::id(),
            RUNS.fetch_add(1, Ordering::Relaxed)
        ));
        Self::read_in(merged, &dir, RANGE)
    }

    /// Reads every works set under a merged directory: one document per
    /// held work with its promoted fields, and which source's record
    /// supplies each text. The pieces go under `dir`, ranges `width` ids
    /// wide.
    ///
    /// # Errors
    ///
    /// I/O, or a set that is not a merge's.
    pub fn read_in(merged: &Path, dir: &Path, width: u32) -> Result<Self, Error> {
        let mut this = Self {
            spill: Spill::growing(dir, width).map_err(|source| Error::Io {
                path: dir.to_path_buf(),
                source,
            })?,
            targets: HashMap::new(),
            sets: Vec::new(),
            set_ix: HashMap::new(),
            stats: Stats::default(),
        };
        for path in crate::sets::find(merged, Kind::Works).map_err(|e| Error::Io {
            path: merged.to_path_buf(),
            source: e,
        })? {
            let io = |source| Error::Io {
                path: path.clone(),
                source,
            };
            let reader = crate::layout::open(&path).map_err(io)?;
            reader.with_table(|| -> Result<(), Error> {
                for row in reader.records::<Work<'_>>() {
                    this.take(&row.map_err(io)?).map_err(io)?;
                }
                Ok(())
            })?;
        }
        Ok(this)
    }

    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 sets
    fn set(&mut self, set: &str) -> u32 {
        if let Some(&ix) = self.set_ix.get(set) {
            return ix;
        }
        let ix = self.sets.len() as u32;
        self.sets.push(set.into());
        self.set_ix.insert(set.into(), ix);
        ix
    }

    fn take(&mut self, work: &Work<'_>) -> io::Result<()> {
        if work.stub {
            self.stats.stubs += 1;
            return Ok(());
        }
        let f = &work.fields;
        let document = Document::of(work, None);
        let wanted = Wanted {
            abstract_text: abstract_source(work),
            authors: f.authors.as_ref().map(|a| a.from),
            venue: f.venue.as_ref().map(|v| v.from),
        };
        for record in &work.records {
            let set = self.set(record.view.set);
            self.targets.insert(
                record.native.to_owned(),
                Target {
                    id: work.id,
                    promoted: record.promoted,
                    wanted,
                    revised: record.view.revised,
                    version: record.view.version,
                    set,
                },
            );
        }
        self.stats.documents += 1;
        if document.title.is_some() {
            self.stats.with_title += 1;
        }
        self.spill.put(work.id, &Piece::Document(document))
    }

    /// Streams one normalized works set of `S`, named `set` as the copy
    /// rule names sets, taking text from the records the documents want —
    /// from the copy that speaks for each record, and no other.
    ///
    /// # Errors
    ///
    /// I/O, a set without its table, or a record whose text cannot be
    /// read.
    pub fn add<S: Contributor>(&mut self, path: &Path, set: &str) -> Result<u64, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = crate::sets::open::<S>(path, Kind::Works).map_err(io)?;
        let table = reader
            .table()
            .ok_or_else(|| Error::NoTable(path.to_path_buf()))?;
        let mut read = 0u64;
        reader.with_table(|| -> Result<(), Error> {
            for (i, work) in reader.records::<S::Work<'_>>().enumerate() {
                let work = work.map_err(io)?;
                read += 1;
                let mut native = None;
                S::names(&work, &mut |a| {
                    if a.native && native.is_none() {
                        native = Some(a.name.to_owned());
                    }
                });
                let Some(target) = native.and_then(|n| self.targets.get(&n).copied()) else {
                    continue;
                };
                let (revised, version) = S::revision(&work);
                let speaking = (
                    target.revised,
                    target.version,
                    &*self.sets[target.set as usize],
                );
                let delivery = (revised, version, set);
                // The speaking copy's text, and where it has none, an
                // older copy's: absence is not an assertion (merge plan
                // §8, 2026-09-18), the rule the record's fields follow.
                let speaks = speaking == delivery;
                if !target.promoted || !(speaks || delivery < speaking) {
                    continue;
                }
                let wanted = target.wanted;
                let wants = |from: Option<SourceId>| from == Some(S::SOURCE);
                if !(wants(wanted.abstract_text) || wants(wanted.authors) || wants(wanted.venue)) {
                    continue;
                }
                let text = S::text(&work, table).map_err(|source| Error::Unmapped {
                    path: path.to_path_buf(),
                    ordinal: i as u64 + 1,
                    source,
                })?;
                let piece = Piece::Text {
                    id: target.id,
                    source: S::SOURCE,
                    wanted,
                    text,
                    speaks,
                };
                self.spill.put(target.id, &piece).map_err(io)?;
            }
            Ok(())
        })?;
        *self.stats.records.entry(S::SOURCE).or_default() += read;
        Ok(read)
    }

    /// The documents a range of ids at a time.
    #[must_use]
    pub fn ranged(self) -> Assembly {
        Assembly {
            ranges: self.spill.ranges(),
            spill: self.spill,
            next: 0,
            stats: self.stats,
        }
    }

    /// Every document, in id order, and the counts: every range at once,
    /// for a store small enough to hold them.
    ///
    /// # Errors
    ///
    /// A spilled piece that does not read back.
    pub fn finish(self) -> Result<(Vec<Document>, Stats), Error> {
        let mut assembly = self.ranged();
        let mut documents = Vec::new();
        while let Some(range) = assembly.next_range()? {
            documents.extend(range);
        }
        Ok((documents, assembly.stats))
    }
}

/// The assembled documents a range of ids at a time, in id order: each
/// range's documents joined with their text as the join made it, and the
/// counts the text gives as each range is taken.
#[derive(Debug)]
pub struct Assembly {
    spill: Spill<Piece>,
    ranges: u32,
    next: u32,
    stats: Stats,
}

impl Assembly {
    /// The next range's documents, sorted by id; `None` after the last.
    ///
    /// # Errors
    ///
    /// A spilled piece that does not read back.
    pub fn next_range(&mut self) -> Result<Option<Vec<Document>>, Error> {
        if self.next >= self.ranges {
            return Ok(None);
        }
        let range = self.next;
        self.next += 1;
        let pieces = self.spill.take(range).map_err(|source| Error::Io {
            path: self.spill.dir().to_path_buf(),
            source,
        })?;
        let mut documents: Vec<Document> = Vec::new();
        let mut texts = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Document(d) => documents.push(d),
                text @ Piece::Text { .. } => texts.push(text),
            }
        }
        documents.sort_by_key(|d| d.id);
        for piece in texts {
            let Piece::Text {
                id,
                source,
                wanted,
                text,
                speaks,
            } = piece
            else {
                continue;
            };
            let Ok(ix) = documents.binary_search_by_key(&id, |d| d.id) else {
                continue;
            };
            fill(
                &mut documents[ix],
                &mut self.stats,
                source,
                wanted,
                text,
                speaks,
            );
        }
        Ok(Some(documents))
    }

    /// The counts: whole once every range is taken.
    #[must_use]
    pub const fn stats(&self) -> &Stats {
        &self.stats
    }
}

/// Documents asked for one work at a time, by increasing id: what the
/// works index is built from ([`Sorted`] for a store's documents held
/// whole, [`Ranged`] for an [`Assembly`] taken a range at a time).
pub trait Documents {
    /// Work `id`'s document, if it is a held work's.
    ///
    /// # Errors
    ///
    /// What taking the next range of an assembly can meet.
    fn document(&mut self, id: WorkId) -> Result<Option<&Document>, Error>;
}

/// Documents held whole, sorted by id.
#[derive(Debug, Clone, Copy)]
pub struct Sorted<'a>(pub &'a [Document]);

impl Documents for Sorted<'_> {
    fn document(&mut self, id: WorkId) -> Result<Option<&Document>, Error> {
        Ok(self
            .0
            .binary_search_by_key(&id, |d| d.id)
            .ok()
            .map(|i| &self.0[i]))
    }
}

/// An [`Assembly`]'s documents, a range at a time as the ids asked for
/// reach it, each range given to `observe` as it is taken: so one pass
/// over the ranges can feed the works index and the lexical index both.
/// Ids must be asked for in increasing order; [`Ranged::finish`] takes and
/// observes the ranges no id reached.
pub struct Ranged<'a, F: FnMut(&[Document]) -> Result<(), Error>> {
    assembly: &'a mut Assembly,
    observe: F,
    current: Vec<Document>,
    /// The range `current` is; `None` before the first.
    at: Option<u32>,
    width: u32,
}

impl<F: FnMut(&[Document]) -> Result<(), Error>> fmt::Debug for Ranged<'_, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ranged")
            .field("at", &self.at)
            .field("documents", &self.current.len())
            .finish_non_exhaustive()
    }
}

impl<'a, F: FnMut(&[Document]) -> Result<(), Error>> Ranged<'a, F> {
    #[allow(missing_docs)]
    pub fn new(assembly: &'a mut Assembly, observe: F) -> Self {
        let width = assembly.spill.width();
        Self {
            assembly,
            observe,
            current: Vec::new(),
            at: None,
            width,
        }
    }

    /// Takes the next range and observes it; `false` after the last.
    fn advance(&mut self) -> Result<bool, Error> {
        let Some(range) = self.assembly.next_range()? else {
            self.current = Vec::new();
            return Ok(false);
        };
        (self.observe)(&range)?;
        self.current = range;
        self.at = Some(self.at.map_or(0, |at| at + 1));
        Ok(true)
    }

    /// Takes and observes every range no id reached.
    ///
    /// # Errors
    ///
    /// As [`Documents::document`].
    pub fn finish(mut self) -> Result<(), Error> {
        while self.advance()? {}
        Ok(())
    }
}

impl<F: FnMut(&[Document]) -> Result<(), Error>> Documents for Ranged<'_, F> {
    fn document(&mut self, id: WorkId) -> Result<Option<&Document>, Error> {
        let want = (id.get() - 1) / self.width;
        while self.at.is_none_or(|at| at < want) {
            if !self.advance()? {
                return Ok(None);
            }
        }
        if self.at != Some(want) {
            return Ok(None);
        }
        Ok(self
            .current
            .binary_search_by_key(&id, |d| d.id)
            .ok()
            .map(|i| &self.current[i]))
    }
}

/// One copy's text into a document: the speaking copy's replaces what an
/// older copy filled, an older copy's only fills what is still empty. Two
/// older copies with different text: the earlier set's, since the sets are
/// read in path order and a range's pieces come back in the order they
/// were made.
fn fill(
    document: &mut Document,
    stats: &mut Stats,
    source: SourceId,
    wanted: Wanted,
    text: Text,
    speaks: bool,
) {
    let mut supplied = false;
    if wanted.abstract_text == Some(source)
        && let Some(abstract_text) = text.abstract_text
        && (speaks || document.abstract_text.is_none())
    {
        if document.abstract_text.is_none() {
            *stats.abstract_from.entry(source).or_default() += 1;
            supplied = true;
        }
        document.abstract_text = Some(abstract_text);
    }
    if wanted.authors == Some(source)
        && (speaks || document.authors.is_empty())
        && !(speaks && text.authors.is_empty())
    {
        if document.authors.is_empty() {
            stats.author_names += text.authors.len() as u64;
            if !text.authors.is_empty() {
                stats.with_authors += 1;
                supplied = true;
            }
        }
        document.authors = text.authors;
    }
    if wanted.venue == Some(source)
        && let Some(venue) = text.venue
        && (speaks || document.venue.is_none())
    {
        if document.venue.is_none() {
            stats.with_venue += 1;
            supplied = true;
        }
        document.venue = Some(venue);
    }
    if supplied {
        *stats.supplied.entry(source).or_default() += 1;
    }
}
