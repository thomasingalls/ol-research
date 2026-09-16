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

use crate::kind::{Form, Genre};
use crate::record::Work;
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
            Self::NoTable(_) => None,
        }
    }
}

/// One work as the serving layer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Which sources a document still wants text from.
#[derive(Debug, Clone, Copy, Default)]
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

/// The join, held in memory: the sample's 82,744 held works and their
/// text; the corpus wants the merge's range shape.
#[derive(Debug)]
pub struct Assembler {
    documents: Vec<Document>,
    wanted: Vec<Wanted>,
    /// A record's native name, in text form, to its document and whether
    /// it is its source's promoted record.
    by_native: HashMap<String, (usize, bool)>,
    stats: Stats,
}

impl Assembler {
    /// Reads every works set under a merged directory: one document per
    /// held work with its promoted fields, and which source's record
    /// supplies each text.
    ///
    /// # Errors
    ///
    /// I/O, or a set that is not a merge's.
    pub fn read(merged: &Path) -> Result<Self, Error> {
        let mut this = Self {
            documents: Vec::new(),
            wanted: Vec::new(),
            by_native: HashMap::new(),
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
                    this.take(&row.map_err(io)?);
                }
                Ok(())
            })?;
        }
        Ok(this)
    }

    fn take(&mut self, work: &Work<'_>) {
        if work.stub {
            self.stats.stubs += 1;
            return;
        }
        let f = &work.fields;
        let abstract_from = [SourceId::Pubmed, SourceId::OpenAlex, SourceId::Crossref]
            .into_iter()
            .find(|s| f.abstract_in.contains(*s));
        let document = Document {
            id: work.id,
            title: f.title.as_ref().map(|t| crate::view::strip_markup(t.value)),
            abstract_text: None,
            authors: Vec::new(),
            venue: None,
            year: f.published.as_ref().map(|p| p.value.year.get()),
            form: f.form.as_ref().map(|x| x.value),
            genre: f.genre.as_ref().map(|x| x.value),
            language: f.language.as_ref().map(|x| x.value),
            retracted: !f.status.retracted.is_empty(),
            corrected: !f.status.corrected.is_empty(),
            concern: !f.status.concern.is_empty(),
            access: f.access.as_ref().map(|x| x.value),
            held_by: work.records.iter().map(|r| r.source).collect(),
        };
        let ix = self.documents.len();
        for record in &work.records {
            self.by_native
                .insert(record.native.to_string(), (ix, record.promoted));
        }
        self.stats.documents += 1;
        if document.title.is_some() {
            self.stats.with_title += 1;
        }
        self.documents.push(document);
        self.wanted.push(Wanted {
            abstract_text: abstract_from,
            authors: f.authors.as_ref().map(|a| a.from),
            venue: f.venue.as_ref().map(|v| v.from),
        });
    }

    /// Streams one normalized works set of `S`, taking text from the
    /// records the documents want.
    ///
    /// # Errors
    ///
    /// I/O, a set without its table, or a record whose text cannot be
    /// read.
    pub fn add<S: Contributor>(&mut self, path: &Path) -> Result<u64, Error> {
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
                        native = Some(a.name.to_string());
                    }
                });
                let Some((ix, promoted)) = native.and_then(|n| self.by_native.get(&n).copied())
                else {
                    continue;
                };
                if !promoted {
                    continue;
                }
                let wanted = self.wanted[ix];
                let wants = |from: Option<SourceId>| from == Some(S::SOURCE);
                if !(wants(wanted.abstract_text) || wants(wanted.authors) || wants(wanted.venue)) {
                    continue;
                }
                let text = S::text(&work, table).map_err(|source| Error::Unmapped {
                    path: path.to_path_buf(),
                    ordinal: i as u64 + 1,
                    source,
                })?;
                self.fill(ix, S::SOURCE, wanted, text);
            }
            Ok(())
        })?;
        *self.stats.records.entry(S::SOURCE).or_default() += read;
        Ok(read)
    }

    fn fill(&mut self, ix: usize, source: SourceId, wanted: Wanted, text: Text) {
        let document = &mut self.documents[ix];
        let mut supplied = false;
        if wanted.abstract_text == Some(source)
            && let Some(abstract_text) = text.abstract_text
            && document.abstract_text.is_none()
        {
            document.abstract_text = Some(abstract_text);
            *self.stats.abstract_from.entry(source).or_default() += 1;
            supplied = true;
        }
        if wanted.authors == Some(source) && document.authors.is_empty() {
            self.stats.author_names += text.authors.len() as u64;
            if !text.authors.is_empty() {
                self.stats.with_authors += 1;
                supplied = true;
            }
            document.authors = text.authors;
        }
        if wanted.venue == Some(source)
            && let Some(venue) = text.venue
            && document.venue.is_none()
        {
            document.venue = Some(venue);
            self.stats.with_venue += 1;
            supplied = true;
        }
        if supplied {
            *self.stats.supplied.entry(source).or_default() += 1;
        }
    }

    /// The documents, in id order, and the counts.
    #[must_use]
    pub fn finish(mut self) -> (Vec<Document>, Stats) {
        self.documents.sort_by_key(|d| d.id);
        (self.documents, self.stats)
    }
}
