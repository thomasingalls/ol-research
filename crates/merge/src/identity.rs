//! The identity pass (merge plan §3, §8): every record's assertions, the
//! components that shared names connect, the verdicts of §3.3, and the
//! works those leave; and the names references point at, which mint
//! stubs (§4).
//!
//! Three phases over the normalized sets, each generic over a
//! [`Contributor`] and called once per set by the binary:
//! [`Identity::add`] reads every works set and keeps only names;
//! [`Identity::mark`] finds the conflict components; [`Identity::titles`]
//! re-reads the sets that hold a conflict record and keeps those records'
//! titles, the corroboration §3.3 needs; [`Identity::targets`] reads the
//! references sets for the names they point at, and [`Identity::relate`]
//! does the same for the relation targets `add` collected (§6).
//! [`Identity::resolve`] then applies the rules. Memory is names, not
//! records: a record is a source, a native name and a few name indices.

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use ingest::{Key, Kind};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::arena::{Arena, NameIx};
use crate::contributor::Contributor;
use crate::copies::{self, Copies, Fate};
use crate::name::{Assertion, Field, Name, OwnedName};
use crate::sets;
use crate::title::{self, Agreement, OwnedTitles};
use crate::view::Unmapped;
use crate::work::{SourceId, Sources};

/// An index into [`Resolution::records`].
pub type RecordIx = u32;

/// A component larger than this is a hairball the rules were never tested
/// on, and stops the run (merge plan §3.3).
const HAIRBALL_RECORDS: usize = 32;
const HAIRBALL_NAMES: usize = 64;
/// Examples kept per shape for the report.
const EXAMPLES: usize = 8;
/// In the natives table, a name no record has as its native one.
const NO_RECORD: RecordIx = RecordIx::MAX;

/// One assertion, interned: eight bytes (the merge's memory, step 2c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assert {
    #[allow(missing_docs)]
    pub name: NameIx,
    #[allow(missing_docs)]
    pub field: Field,
    #[allow(missing_docs)]
    pub native: bool,
}

/// One source record as the identity pass keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    #[allow(missing_docs)]
    pub source: SourceId,
    /// The record's native name.
    pub native: NameIx,
    /// Every assertion, the native one included, each name once; five
    /// without a second allocation, which is nearly every record.
    pub asserts: SmallVec<[Assert; 5]>,
}

#[derive(Debug)]
struct SetInfo {
    path: PathBuf,
}

/// Why the pass stopped.
#[derive(Debug)]
pub enum Error {
    /// A set could not be read.
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: io::Error,
    },
    /// A record asserted no native name, which a [`Contributor`] promises.
    NoNativeName {
        #[allow(missing_docs)]
        path: PathBuf,
        /// From one.
        ordinal: u64,
    },
    /// A set given to a later phase that [`Identity::add`] never read.
    UnknownSet(PathBuf),
    /// A set without its symbol table beside it.
    NoTable(PathBuf),
    /// A record's value outside the merge's tables (merge plan §5, §6).
    Unmapped {
        #[allow(missing_docs)]
        path: PathBuf,
        /// From one.
        ordinal: u64,
        #[allow(missing_docs)]
        source: Unmapped,
    },
    /// A component too large for the rules (merge plan §3.3).
    Hairball {
        #[allow(missing_docs)]
        records: usize,
        #[allow(missing_docs)]
        names: usize,
        #[allow(missing_docs)]
        example: String,
    },
    /// A shape the rules do not cover; the fix is a rule and a fixture.
    Unmodelled(String),
    /// A record's name the run's dictionary lacks, or a name of the
    /// dictionary no record asserted: the dictionary was made from other
    /// sets than the pass read (the merge's memory, step 2b).
    NotInDictionary {
        #[allow(missing_docs)]
        path: PathBuf,
        /// From one; 0 for a name no record asserted.
        ordinal: u64,
        #[allow(missing_docs)]
        name: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NoNativeName { path, ordinal } => {
                write!(f, "{}: record {ordinal} has no native name", path.display())
            }
            Self::UnknownSet(path) => {
                write!(f, "{}: not read by the identity pass", path.display())
            }
            Self::NoTable(path) => write!(f, "{}: no symbol table beside it", path.display()),
            Self::Unmapped {
                path,
                ordinal,
                source,
            } => write!(f, "{}: record {ordinal}: {source}", path.display()),
            Self::Hairball {
                records,
                names,
                example,
            } => write!(
                f,
                "a component of {records} records and {names} names is beyond the rules: {example}"
            ),
            Self::Unmodelled(what) => write!(f, "a shape the rules do not cover: {what}"),
            Self::NotInDictionary {
                path,
                ordinal: 0,
                name,
            } => write!(
                f,
                "{}: {name} is in the run's dictionary and no record asserts it",
                path.display()
            ),
            Self::NotInDictionary {
                path,
                ordinal,
                name,
            } => write!(
                f,
                "{}: record {ordinal} asserts {name}, which the run's dictionary lacks",
                path.display()
            ),
        }
    }
}

impl From<copies::Error> for Error {
    fn from(e: copies::Error) -> Self {
        match e {
            copies::Error::Io { path, source } => Self::Io { path, source },
            copies::Error::NoNativeName { path, ordinal } => Self::NoNativeName { path, ordinal },
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Unmapped { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// What the phases counted.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    /// Sets read per source, works and references.
    pub sets: BTreeMap<SourceId, u64>,
    /// Records kept per source.
    pub records: BTreeMap<SourceId, u64>,
    /// Copies whose native name was already a record's: another copy of
    /// the same record, whose names join it (§8), or two copies given in
    /// memory, where no copy rule ranks them.
    pub repeated: BTreeMap<SourceId, u64>,
    /// Names a copy added to a record another copy made, per source.
    pub joined: BTreeMap<SourceId, u64>,
    /// Copies skipped because a later deletions set withdrew them.
    pub deleted: BTreeMap<SourceId, u64>,
    /// Copies a newer copy of the record speaks over (merge plan §8):
    /// their names and relations still join the record's, and their
    /// fields fill what the newest has none of.
    pub superseded: BTreeMap<SourceId, u64>,
    /// Reference rows of withdrawn copies, not read. A superseded copy's
    /// references are read: a record's citations are the union over its
    /// standing copies (merge plan §8, 2026-09-18).
    pub silent_references: BTreeMap<SourceId, u64>,
    /// Assertions per source and field.
    pub asserted: BTreeMap<(SourceId, &'static str), u64>,
    /// References read per source.
    pub references: BTreeMap<SourceId, u64>,
    /// Per source: references whose names include a held work's, whose
    /// names are all unheld (stubs), and with no name at all.
    pub targets: BTreeMap<(SourceId, &'static str), u64>,
    /// Per source: relation targets likewise (merge plan §6).
    pub related: BTreeMap<(SourceId, &'static str), u64>,
    /// Under `--survey`: values outside the merge's tables, by source
    /// and `field: value`, that stopped nothing.
    pub unmapped: BTreeMap<(SourceId, String), u64>,
    /// Publisher ids one source claimed for more than one of its own
    /// records, and so no article's identifier (merge plan §3.1): how
    /// many distinct values, how many assertions they cost, and a few of
    /// them written out.
    pub collided: u64,
    #[allow(missing_docs)]
    pub collided_assertions: u64,
    #[allow(missing_docs)]
    pub collided_examples: Vec<String>,
    /// Names every record of a star beyond the rules carries under a
    /// title of its own, taken off them ([`Identity::cut_stars`]), and the
    /// assertions that took.
    pub stars: u64,
    #[allow(missing_docs)]
    pub star_assertions: u64,
    /// The largest, written out, for the report.
    pub star_examples: Vec<String>,
}

/// The identity pass, accumulating over every set of every source.
#[derive(Debug, Default)]
pub struct Identity {
    arena: Arena,
    parent: Vec<NameIx>,
    roots: Vec<NameIx>,
    records: Vec<Record>,
    /// Each name's record, if it is a record's native name, by name
    /// index: [`NO_RECORD`] where it is not.
    natives: Vec<RecordIx>,
    set_of: Vec<u32>,
    sets: Vec<SetInfo>,
    conflict_roots: HashSet<NameIx>,
    conflict: Vec<bool>,
    conflict_sets: HashSet<u32>,
    titles: HashMap<RecordIx, OwnedTitles>,
    stubs: Arena,
    stub_parent: Vec<NameIx>,
    pending: Vec<(SourceId, OwnedName)>,
    /// Publisher-id claims, held back from the union until every record
    /// is in: see [`Identity::settle`].
    deferred: Vec<(RecordIx, NameIx, Field)>,
    /// The claims `settle` and `cut_stars` rejected, each on the
    /// record that made it.
    collisions: Vec<(RecordIx, Refusal)>,
    /// The names those rejections leave in no component.
    refused: HashSet<NameIx>,
    /// Refused values written out, largest first, for the report.
    refusals: Vec<(usize, String)>,
    settled: bool,
    /// [`Identity::cut_stars`] has run.
    stars_cut: bool,
    /// Under a dictionary, which of its names a record has asserted, a
    /// bit each, and how many: every one must be, or the dictionary was
    /// made from other sets than the pass read.
    interned: Vec<u64>,
    interned_count: usize,
    survey: bool,
    /// Counts so far.
    pub stats: Stats,
}

impl Identity {
    #[allow(missing_docs)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A pass whose names are `dictionary`'s, numbered in name order: a
    /// full merge's, which gathers every name it will read before it reads
    /// any (the merge's memory, step 2b, 2026-09-24). Every name a record
    /// asserts must be in it, and after the last [`Identity::add`],
    /// [`Identity::check_dictionary`] says whether every name in it was
    /// asserted.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    pub fn with_dictionary(dictionary: crate::dictionary::Dictionary) -> Self {
        let n = dictionary.len();
        Self {
            arena: Arena::sorted(dictionary),
            parent: (0..n as NameIx).collect(),
            interned: vec![0; n.div_ceil(64)],
            ..Self::default()
        }
    }

    /// Under a dictionary, whether the records read asserted every name
    /// in it, as they would have had the names been interned as read.
    ///
    /// # Errors
    ///
    /// A name of the dictionary no record asserted.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    pub fn check_dictionary(&self) -> Result<(), Error> {
        let Some(dictionary) = self.arena.dictionary() else {
            return Ok(());
        };
        if self.interned_count == dictionary.len() {
            return Ok(());
        }
        let unseen = (0..dictionary.len())
            .find(|&i| self.interned[i / 64] & (1 << (i % 64)) == 0)
            .unwrap_or(0);
        Err(Error::NotInDictionary {
            path: PathBuf::from("<dictionary>"),
            ordinal: 0,
            name: self.arena.get(unseen as NameIx).to_string(),
        })
    }

    /// The record whose native name is at `ix`, if one is.
    fn native_record(&self, ix: NameIx) -> Option<RecordIx> {
        self.natives
            .get(ix as usize)
            .copied()
            .filter(|&rix| rix != NO_RECORD)
    }

    fn set_native(&mut self, ix: NameIx, rix: RecordIx) {
        if self.natives.len() <= ix as usize {
            self.natives.resize(ix as usize + 1, NO_RECORD);
        }
        self.natives[ix as usize] = rix;
    }

    /// Notes that a record asserted the name at `ix`, under a dictionary.
    fn seen(&mut self, ix: NameIx) {
        if self.interned.is_empty() {
            return;
        }
        let (word, bit) = (ix as usize / 64, 1u64 << (ix % 64));
        if self.interned[word] & bit == 0 {
            self.interned[word] |= bit;
            self.interned_count += 1;
        }
    }

    /// Under `--survey`, a value outside the merge's tables is counted
    /// in [`Stats::unmapped`] instead of stopping the run.
    pub fn set_survey(&mut self, on: bool) {
        self.survey = on;
    }

    /// How many records so far.
    #[must_use]
    pub fn records(&self) -> usize {
        self.records.len()
    }

    /// How many distinct names so far.
    #[must_use]
    pub fn names(&self) -> usize {
        self.arena.len()
    }

    fn find(&mut self, mut x: NameIx) -> NameIx {
        while self.parent[x as usize] != x {
            let grand = self.parent[self.parent[x as usize] as usize];
            self.parent[x as usize] = grand;
            x = grand;
        }
        x
    }

    /// Joins two names' components; the smaller index stays the root, so
    /// roots do not depend on the order of joins.
    fn union(&mut self, a: NameIx, b: NameIx) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.parent[hi as usize] = lo;
        }
    }

    /// Reads one works set of `S`: every record's names, interned and
    /// joined, and the names its relations point at, kept for
    /// [`Identity::relate`]; at most `limit` records. A record whose
    /// native name `dead` vouches for — withdrawn by a later deletions
    /// set — is skipped and counted.
    ///
    /// # Errors
    ///
    /// The set cannot be read or has no symbol table, a record has no
    /// native name, or a relation's type is outside the table.
    pub fn add<S: Contributor>(
        &mut self,
        path: &Path,
        set: &str,
        limit: Option<usize>,
        copies: &Copies,
    ) -> Result<u64, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = sets::open::<S>(path, Kind::Works).map_err(io)?;
        let table = reader
            .table()
            .ok_or_else(|| Error::NoTable(path.to_path_buf()))?;
        let set_ix =
            u32::try_from(self.sets.len()).map_err(|_| io(io::Error::other("too many sets")))?;
        self.sets.push(SetInfo {
            path: path.to_path_buf(),
        });
        *self.stats.sets.entry(S::SOURCE).or_default() += 1;
        let mut read = 0u64;
        reader.with_table(|| -> Result<(), Error> {
            for (i, work) in reader
                .records::<S::Work<'_>>()
                .take(limit.unwrap_or(usize::MAX))
                .enumerate()
            {
                let work = work.map_err(io)?;
                match self.record::<S>(&work, set_ix, set, i as u64 + 1, copies) {
                    Kept::Record => {}
                    Kept::Joined | Kept::Silent => continue,
                    Kept::NoNative => {
                        return Err(Error::NoNativeName {
                            path: path.to_path_buf(),
                            ordinal: i as u64 + 1,
                        });
                    }
                    Kept::NotInDictionary(name) => {
                        return Err(Error::NotInDictionary {
                            path: path.to_path_buf(),
                            ordinal: i as u64 + 1,
                            name: name.to_string(),
                        });
                    }
                }
                let related = S::relations(&work, table, &mut |r| {
                    if let Some(target) = r.target {
                        self.pending.push((S::SOURCE, target));
                    }
                });
                match related {
                    Ok(()) => {}
                    Err(source) if self.survey => {
                        *self
                            .stats
                            .unmapped
                            .entry((S::SOURCE, source.to_string()))
                            .or_default() += 1;
                    }
                    Err(source) => {
                        return Err(Error::Unmapped {
                            path: path.to_path_buf(),
                            ordinal: i as u64 + 1,
                            source,
                        });
                    }
                }
                read += 1;
            }
            Ok(())
        })?;
        Ok(read)
    }

    /// One record given directly, with its titles kept: for tests and for
    /// callers that hold records in memory. The set phases keep titles
    /// for conflict records only.
    ///
    /// # Errors
    ///
    /// No native name among the assertions.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 records in a run
    pub fn push(
        &mut self,
        source: SourceId,
        assertions: &[Assertion<'_>],
        titles: title::Titles<'_>,
    ) -> Result<RecordIx, Error> {
        let before = self.records.len();
        let set = self.set_index(Path::new("<memory>")).unwrap_or_else(|| {
            self.sets.push(SetInfo {
                path: PathBuf::from("<memory>"),
            });
            (self.sets.len() - 1) as u32
        });
        let mut asserts: SmallVec<[Assert; 5]> = SmallVec::new();
        // As in `record`: a publisher id waits for `settle`.
        let mut claims: SmallVec<[Assert; 2]> = SmallVec::new();
        let mut native = None;
        for a in assertions {
            let Some(ix) = self.arena.intern(a.name) else {
                return Err(Error::NotInDictionary {
                    path: PathBuf::from("<memory>"),
                    ordinal: before as u64 + 1,
                    name: a.name.to_string(),
                });
            };
            self.seen(ix);
            if self.parent.len() <= ix as usize {
                self.parent.push(ix);
            }
            if a.native && native.is_none() {
                native = Some(ix);
            }
            let assert = Assert {
                name: ix,
                field: Field::of(a.field),
                native: a.native,
            };
            if a.name.key() == Key::Publisher {
                if !claims.iter().any(|x| x.name == ix) {
                    claims.push(assert);
                }
            } else if !asserts.iter().any(|x| x.name == ix) {
                asserts.push(assert);
            }
        }
        let Some(native) = native else {
            return Err(Error::NoNativeName {
                path: PathBuf::from("<memory>"),
                ordinal: before as u64 + 1,
            });
        };
        if let Some(rix) = self.native_record(native) {
            *self.stats.repeated.entry(source).or_default() += 1;
            self.defer(rix, &claims);
            return Ok(rix);
        }
        for a in &asserts {
            self.union(native, a.name);
            *self
                .stats
                .asserted
                .entry((source, a.field.name()))
                .or_default() += 1;
        }
        let rix = before as RecordIx;
        self.set_native(native, rix);
        self.records.push(Record {
            source,
            native,
            asserts,
        });
        self.set_of.push(set);
        self.titles.insert(rix, OwnedTitles::of(titles));
        self.defer(rix, &claims);
        *self.stats.records.entry(source).or_default() += 1;
        Ok(rix)
    }

    /// Interns one copy's names and joins them, unless a deletion
    /// withdrew the copy (merge plan §8) or it asserts no native name. A
    /// record delivered more than once asserts the union of its standing
    /// copies' names: absence is not an assertion (§8, 2026-09-18), so a
    /// name the newest copy stopped giving — a version's own DOI, an id
    /// an editor dropped — stays the record's, and the references that
    /// cite it keep landing on this work.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 records in a run
    fn record<S: Contributor>(
        &mut self,
        work: &S::Work<'_>,
        set_ix: u32,
        set: &str,
        ordinal: u64,
        copies: &Copies,
    ) -> Kept {
        let mut assertions: SmallVec<[Assertion<'_>; 3]> = SmallVec::new();
        S::names(work, &mut |a| assertions.push(a));
        let Some(native_name) = assertions.iter().find(|a| a.native).map(|a| a.name) else {
            return Kept::NoNative;
        };
        let delivery = copies::delivery_of::<S>(work, set, ordinal);
        let speaks = match copies.fate(S::SOURCE, native_name, &delivery) {
            Fate::Speaks => true,
            Fate::Superseded => {
                *self.stats.superseded.entry(S::SOURCE).or_default() += 1;
                false
            }
            Fate::Withdrawn => {
                *self.stats.deleted.entry(S::SOURCE).or_default() += 1;
                return Kept::Silent;
            }
        };
        let mut asserts: SmallVec<[Assert; 5]> = SmallVec::new();
        // Publisher ids are held back from the union until every record
        // is in. Whether one identifies an article at all is a fact about
        // the whole run and not about this record (§3.3, 2026-09-19), and
        // a union cannot be undone once made.
        let mut claims: SmallVec<[Assert; 2]> = SmallVec::new();
        let mut native = None;
        for a in &assertions {
            let Some(ix) = self.arena.intern(a.name) else {
                return Kept::NotInDictionary(a.name.to_owned());
            };
            self.seen(ix);
            if self.parent.len() <= ix as usize {
                self.parent.push(ix);
            }
            if a.native && native.is_none() {
                native = Some(ix);
            }
            let assert = Assert {
                name: ix,
                field: Field::of(a.field),
                native: a.native,
            };
            if a.name.key() == Key::Publisher {
                if !claims.iter().any(|x| x.name == ix) {
                    claims.push(assert);
                }
            } else if !asserts.iter().any(|x| x.name == ix) {
                asserts.push(assert);
            }
        }
        let Some(native) = native else {
            return Kept::NoNative;
        };
        if let Some(rix) = self.native_record(native) {
            *self.stats.repeated.entry(S::SOURCE).or_default() += 1;
            let known = &self.records[rix as usize].asserts;
            let new: SmallVec<[Assert; 5]> = asserts
                .iter()
                .filter(|a| !known.iter().any(|x| x.name == a.name))
                .copied()
                .collect();
            for a in new {
                self.records[rix as usize].asserts.push(a);
                self.union(native, a.name);
                *self
                    .stats
                    .asserted
                    .entry((S::SOURCE, a.field.name()))
                    .or_default() += 1;
                *self.stats.joined.entry(S::SOURCE).or_default() += 1;
            }
            self.defer(rix, &claims);
            // The title pass reads the set the speaking copy is in.
            if speaks {
                self.set_of[rix as usize] = set_ix;
                return Kept::Record;
            }
            return Kept::Joined;
        }
        for a in &asserts {
            self.union(native, a.name);
            *self
                .stats
                .asserted
                .entry((S::SOURCE, a.field.name()))
                .or_default() += 1;
        }
        let rix = self.records.len() as RecordIx;
        self.set_native(native, rix);
        self.records.push(Record {
            source: S::SOURCE,
            native,
            asserts,
        });
        self.set_of.push(set_ix);
        self.defer(rix, &claims);
        *self.stats.records.entry(S::SOURCE).or_default() += 1;
        if speaks { Kept::Record } else { Kept::Joined }
    }

    /// Keeps a record's publisher-id claims for [`Identity::settle`]. The
    /// same claim from two copies of one record is one claim, which
    /// `settle` sees when it groups them, so nothing is counted here.
    fn defer(&mut self, rix: RecordIx, claims: &[Assert]) {
        for a in claims {
            self.deferred.push((rix, a.name, a.field));
        }
    }

    /// The refusals of [`Identity::settle`], put on the work each
    /// claiming record landed in: an assertion with a verdict, like a
    /// wrong link, and counted with them.
    fn refusals_onto(&self, works: &mut [Work]) {
        if self.collisions.is_empty() {
            return;
        }
        let mut by_record: HashMap<RecordIx, SmallVec<[Refusal; 1]>> = HashMap::new();
        for &(rix, refusal) in &self.collisions {
            by_record.entry(rix).or_default().push(refusal);
        }
        for work in works {
            let refused: SmallVec<[Rejection; 1]> = work
                .records
                .iter()
                .filter_map(|rix| by_record.get(rix).map(|names| (*rix, names)))
                .flat_map(|(rix, names)| {
                    names
                        .iter()
                        .map(move |&(name, why, holder, field)| Rejection {
                            name,
                            record: rix,
                            field,
                            holder,
                            why,
                        })
                })
                .collect();
            work.rejected.extend(refused);
        }
    }

    /// Settles the publisher ids `add` held back (merge plan §3.1, §3.3;
    /// the user, 2026-09-19), before any component exists.
    ///
    /// A source holds one record per article — one PubMed record per
    /// PMID, one Crossref record per DOI — so a source claiming one
    /// publisher id for two of its own records is that source saying the
    /// value names more than one of its articles. Those values are real:
    /// a journal's title in the PII slot (`Afr J Reprod Health` on 15
    /// records), a volume and issue (`JAMP-14-1` on 10), a year and
    /// volume (`2025-vol6` on 10). Joined, they fuse every article of an
    /// issue into one work and the components grow past the rules — a
    /// hairball of 762 records on the sample. So the claim is rejected
    /// with its reason ([`Reason::NotAnArticleId`]) and never joins, and
    /// the census has to run here because a union cannot be undone.
    ///
    /// The test is *one source's* records and not all of them: one work
    /// seen from two sources carrying the same publisher id is the id
    /// doing its job, 18,194 of the 20,647 the census found on more than
    /// one record, and rejecting those would empty the namespace of the
    /// use it is for. The price is a source's duplicate records of one
    /// article (§3.3): their shared publisher id is refused too, which
    /// costs a name and never a merge.
    fn settle(&mut self) {
        if self.settled {
            return;
        }
        self.settled = true;
        let deferred = std::mem::take(&mut self.deferred);
        // In the order each name was first claimed, which is the order the
        // sets were read in, so that the order records gain names — and so
        // the merged rows — follows neither a hash map's iteration nor how
        // names are numbered.
        let mut first: HashMap<NameIx, usize> = HashMap::new();
        let mut claims: Vec<(NameIx, Claimants)> = Vec::new();
        for (rix, ix, field) in deferred {
            let at = *first.entry(ix).or_insert_with(|| {
                claims.push((ix, SmallVec::new()));
                claims.len() - 1
            });
            let by = &mut claims[at].1;
            if !by.iter().any(|(r, _)| *r == rix) {
                by.push((rix, field));
            }
        }
        drop(first);
        for (ix, mut by) in claims {
            by.sort_unstable_by_key(|(rix, _)| *rix);
            let mut per_source = [0u32; SourceId::ALL.len()];
            for &(rix, field) in &by {
                let source = self.records[rix as usize].source;
                per_source[source as usize] += 1;
                *self
                    .stats
                    .asserted
                    .entry((source, field.name()))
                    .or_default() += 1;
            }
            if per_source.iter().any(|&n| n > 1) {
                self.refused.insert(ix);
                self.stats.collided += 1;
                self.stats.collided_assertions += by.len() as u64;
                let per: Vec<String> = SourceId::ALL
                    .iter()
                    .zip(per_source)
                    .filter(|(_, n)| *n > 0)
                    .map(|(s, n)| format!("{s} {n}"))
                    .collect();
                self.refusals.push((
                    by.len(),
                    format!(
                        "{} on {} records ({})",
                        self.arena.get(ix),
                        by.len(),
                        per.join(", ")
                    ),
                ));
                // The worst are the ones worth printing, and holding
                // every refusal of a corpus is not worth the bytes.
                if self.refusals.len() > EXAMPLES * 4 {
                    self.refusals.sort_by_key(|(n, _)| Reverse(*n));
                    self.refusals.truncate(EXAMPLES);
                }
                for &(rix, field) in &by {
                    self.collisions
                        .push((rix, (ix, Reason::NotAnArticleId, None, field)));
                }
                continue;
            }
            for (rix, field) in by {
                let native = self.records[rix as usize].native;
                self.records[rix as usize].asserts.push(Assert {
                    name: ix,
                    field,
                    native: false,
                });
                self.union(native, ix);
            }
        }
        self.refusals.sort_by_key(|(n, _)| Reverse(*n));
        self.refusals.truncate(EXAMPLES);
        self.stats.collided_examples = self.refusals.iter().map(|(_, e)| e.clone()).collect();
    }

    /// Finds the conflict components (merge plan §3.2): more than one name
    /// in a namespace, or more than one record of a source. Call after
    /// the last [`Identity::add`].
    pub fn mark(&mut self) {
        self.settle();
        self.components();
    }

    /// The components and which of them are conflicts, from the joins as
    /// they stand: after [`Identity::settle`], and again after
    /// [`Identity::cut_stars`] takes a star's shared names off its
    /// records.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    fn components(&mut self) {
        let n = self.arena.len();
        self.roots = (0..n as NameIx).map(|i| self.find(i)).collect();
        let mut tallies: HashMap<NameIx, Tally> = HashMap::new();
        for i in 0..n {
            // A refused publisher id is in no component: `settle` took it
            // off every record, and a name no record asserts would
            // otherwise read as a stub.
            if self.refused.contains(&(i as NameIx)) {
                continue;
            }
            let t = tallies.entry(self.roots[i]).or_default();
            let k = &mut t.keys[key_ix(self.arena.key(i as NameIx))];
            *k = k.saturating_add(1);
        }
        for rec in &self.records {
            let t = tallies.entry(self.roots[rec.native as usize]).or_default();
            let s = &mut t.sources[rec.source as usize];
            *s = s.saturating_add(1);
        }
        self.conflict_roots = tallies
            .into_iter()
            .filter(|(_, t)| t.keys.iter().chain(&t.sources).any(|&c| c > 1))
            .map(|(root, _)| root)
            .collect();
        self.conflict = self
            .records
            .iter()
            .map(|r| self.conflict_roots.contains(&self.roots[r.native as usize]))
            .collect();
        self.conflict_sets = self
            .conflict
            .iter()
            .zip(&self.set_of)
            .filter(|(c, _)| **c)
            .map(|(_, set)| *set)
            .collect();
    }

    /// Takes the names every record of a star carries off its records
    /// (merge plan §3.3, the user's decisions of 2026-09-24). Once, after
    /// the titles are read and before anything is cited: by
    /// [`Identity::targets`], [`Identity::cite`] or [`Identity::resolve`],
    /// whichever comes first.
    ///
    /// A star is a component larger than the rules were tested on
    /// (`HAIRBALL_RECORDS`) in which every name is either one record's
    /// alone or every record's, and no two records are copies of one
    /// article: no two titles equal, after §3.3's fingerprint, or one the
    /// other's vernacular title. Its shared names name none of its
    /// records. Most often they are a container's — a proceedings, a block
    /// of abstracts, a supplement, a set of letters on one article,
    /// published under one DOI whose parts PubMed indexes one by one with
    /// that DOI beside each: the Physiological Society's proceedings of
    /// 1955 on 42 records, the Marine Biological Laboratory's abstracts of
    /// 1950 on 96. But the shape says only that the value is no article's,
    /// not what it is: `10.1016/s0197` on 38 records from two journals is a
    /// DOI cut short in the deposit. So a shared name no record holds
    /// natively is refused on every record as no article's id
    /// ([`Reason::NotAnArticleId`], the verdict a publisher id one source
    /// gives several records gets), and is a stub's where cited; one a
    /// record of the component holds natively — a Crossref record of the
    /// container itself — stays that record's, and the others' claims are
    /// wrong links ([`Reason::WrongLink`]), as the rules make them below
    /// the size cap. Whether a refused name was a container's is merge
    /// plan §11 item 25's to decide, from evidence.
    ///
    /// Titles are compared for equality only, not by §3.3's looser
    /// agreement: its prefix and near-spelling clauses recognise one
    /// article under two sources' spellings, and among the parts of one
    /// container they call "The two-hour marathon: how?" a copy of "The
    /// two-hour marathon: how important is age?". A star whose records are
    /// copies of one article — forty under one DOI with one title — or
    /// whose records' titles cannot be compared stays a hairball, and
    /// anything else that large still stops the run.
    fn cut_stars(&mut self) {
        if self.stars_cut {
            return;
        }
        self.stars_cut = true;
        // The records of each conflict component, by root: conflicts are
        // few, and only a conflict can be larger than the rules take.
        let mut components: BTreeMap<NameIx, Vec<RecordIx>> = BTreeMap::new();
        for (rix, r) in self.records.iter().enumerate() {
            let root = self.roots[r.native as usize];
            if self.conflict_roots.contains(&root) {
                components
                    .entry(root)
                    .or_default()
                    .push(RecordIx::try_from(rix).expect("records fit u32"));
            }
        }
        let mut examples: Vec<(usize, String)> = Vec::new();
        let mut cut = false;
        for recs in components.into_values() {
            let Some((names, shared)) = self.star(&recs) else {
                continue;
            };
            let all = recs.len();
            for &name in &shared {
                let holder = recs
                    .iter()
                    .copied()
                    .find(|&rix| self.records[rix as usize].native == name);
                for &rix in &recs {
                    if Some(rix) == holder {
                        continue;
                    }
                    let asserts = &mut self.records[rix as usize].asserts;
                    let field = asserts
                        .iter()
                        .find(|a| a.name == name)
                        .map(|a| a.field)
                        .expect("a star's shared name is asserted by every record but its holder");
                    asserts.retain(|a| a.name != name);
                    let why = if holder.is_some() {
                        Reason::WrongLink
                    } else {
                        Reason::NotAnArticleId
                    };
                    self.collisions.push((rix, (name, why, holder, field)));
                    self.stats.star_assertions += 1;
                }
                if holder.is_none() {
                    // No record holds it now: a name in no component, as a
                    // refused publisher id is, and a stub's where cited.
                    self.refused.insert(name);
                }
                self.stats.stars += 1;
                examples.push((all, format!("{} on {all} records", self.arena.get(name))));
            }
            // The component joined again without them: its names each
            // their own again, then each record's remaining names joined
            // to its native name, as `add` joins them.
            for &name in &names {
                self.parent[name as usize] = name;
            }
            for &rix in &recs {
                let native = self.records[rix as usize].native;
                let names: SmallVec<[NameIx; 4]> = self.records[rix as usize]
                    .asserts
                    .iter()
                    .map(|a| a.name)
                    .collect();
                for name in names {
                    self.union(native, name);
                }
            }
            cut = true;
        }
        examples.sort_by_key(|(n, e)| (Reverse(*n), e.clone()));
        examples.truncate(EXAMPLES);
        self.stats.star_examples = examples.into_iter().map(|(_, e)| e).collect();
        if cut {
            self.components();
        }
    }

    /// A conflict component's names and, if it is a star past the rules,
    /// the names every record of it carries: see [`Identity::cut_stars`].
    fn star(&self, recs: &[RecordIx]) -> Option<(Vec<NameIx>, Vec<NameIx>)> {
        let mut by: BTreeMap<NameIx, SmallVec<[RecordIx; 1]>> = BTreeMap::new();
        for &rix in recs {
            let r = &self.records[rix as usize];
            let mut own: SmallVec<[NameIx; 4]> = SmallVec::new();
            own.push(r.native);
            for a in &r.asserts {
                if !own.contains(&a.name) {
                    own.push(a.name);
                }
            }
            for name in own {
                by.entry(name).or_default().push(rix);
            }
        }
        let all = recs.len();
        if all <= HAIRBALL_RECORDS && by.len() <= HAIRBALL_NAMES {
            return None;
        }
        if !by.values().all(|v| v.len() == 1 || v.len() == all) {
            return None;
        }
        let default = OwnedTitles::default();
        let title_of = |rix: RecordIx| self.titles.get(&rix).unwrap_or(&default);
        let apart = (0..all).all(|i| {
            (i + 1..all).all(|j| {
                !matches!(
                    title::agree(title_of(recs[i]), title_of(recs[j])),
                    Agreement::Equal | Agreement::Vernacular | Agreement::Untestable
                )
            })
        });
        if !apart {
            return None;
        }
        let shared = by
            .iter()
            .filter(|(_, v)| v.len() == all)
            .map(|(name, _)| *name)
            .collect();
        Some((by.into_keys().collect(), shared))
    }

    /// How many components are conflicts, after [`Identity::mark`].
    #[must_use]
    pub fn conflicts(&self) -> usize {
        self.conflict_roots.len()
    }

    /// Whether a set read by [`Identity::add`] holds a record of a
    /// conflict component, so [`Identity::titles`] needs to read it.
    #[must_use]
    pub fn needs_titles(&self, path: &Path) -> bool {
        self.set_index(path)
            .is_some_and(|set| self.conflict_sets.contains(&set))
    }

    #[allow(clippy::cast_possible_truncation)] // set indices were u32 when added
    fn set_index(&self, path: &Path) -> Option<u32> {
        self.sets
            .iter()
            .position(|s| s.path == path)
            .map(|i| i as u32)
    }

    /// Re-reads one works set for the titles of its conflict records.
    ///
    /// # Errors
    ///
    /// The set was not read by [`Identity::add`], or cannot be read now.
    pub fn titles<S: Contributor>(
        &mut self,
        path: &Path,
        set: &str,
        limit: Option<usize>,
        copies: &Copies,
    ) -> Result<u64, Error> {
        if self.set_index(path).is_none() {
            return Err(Error::UnknownSet(path.to_path_buf()));
        }
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = sets::open::<S>(path, Kind::Works).map_err(io)?;
        let mut kept = 0u64;
        reader.with_table(|| -> Result<(), Error> {
            for (i, work) in reader
                .records::<S::Work<'_>>()
                .take(limit.unwrap_or(usize::MAX))
                .enumerate()
            {
                let work = work.map_err(io)?;
                let Some(native_name) = copies::native_of::<S>(&work) else {
                    continue;
                };
                let delivery = copies::delivery_of::<S>(&work, set, i as u64 + 1);
                if copies.fate(S::SOURCE, native_name, &delivery) != Fate::Speaks {
                    continue;
                }
                let native = self.arena.find(native_name);
                let Some(rix) = native.and_then(|n| self.native_record(n)) else {
                    continue;
                };
                if self.conflict.get(rix as usize).copied().unwrap_or(false)
                    && !self.titles.contains_key(&rix)
                {
                    self.titles.insert(rix, OwnedTitles::of(S::titles(&work)));
                    kept += 1;
                }
            }
            Ok(())
        })?;
        Ok(kept)
    }

    /// Reads one references set of `S` for the names its references point
    /// at: a name no record asserted is a stub's (merge plan §4). Call
    /// after [`Identity::mark`].
    ///
    /// # Errors
    ///
    /// The set cannot be read.
    pub fn targets<S: Contributor>(
        &mut self,
        path: &Path,
        set: &str,
        limit: Option<usize>,
        copies: &Copies,
    ) -> Result<u64, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = sets::open::<S>(path, Kind::References).map_err(io)?;
        *self.stats.sets.entry(S::SOURCE).or_default() += 1;
        let mut read = 0u64;
        reader.with_table(|| -> Result<(), Error> {
            for record in reader
                .records::<(S::Id<'_>, S::Reference<'_>)>()
                .take(limit.unwrap_or(usize::MAX))
            {
                let (id, reference) = record.map_err(io)?;
                if !copies.alive_in(S::SOURCE, S::native(&id), set) {
                    *self.stats.silent_references.entry(S::SOURCE).or_default() += 1;
                    continue;
                }
                let mut names: SmallVec<[Name<'_>; 3]> = SmallVec::new();
                S::targets(&reference, &mut |name| names.push(name));
                self.cite(S::SOURCE, &names);
                read += 1;
            }
            Ok(())
        })?;
        *self.stats.references.entry(S::SOURCE).or_default() += read;
        Ok(read)
    }

    /// The relation targets [`Identity::add`] collected, cited like a
    /// reference's names (merge plan §6): a name no work has is a stub's.
    /// Call after [`Identity::mark`].
    pub fn relate(&mut self) {
        for (source, name) in std::mem::take(&mut self.pending) {
            self.cite_in(source, &[name.as_name()], true);
        }
    }

    /// One reference's names, as [`Identity::targets`] sees them: a
    /// reference names one work, so a name beside a held one is that
    /// work's (asserted by the citing record; counted, not used), and
    /// names beside no held one are one stub's. Returns what it was.
    pub fn cite(&mut self, source: SourceId, names: &[Name<'_>]) -> &'static str {
        self.cite_in(source, names, false)
    }

    /// [`Identity::cite`], counted among references or among relation
    /// targets.
    fn cite_in(&mut self, source: SourceId, names: &[Name<'_>], related: bool) -> &'static str {
        self.cut_stars();
        let mut held: SmallVec<[NameIx; 2]> = SmallVec::new();
        let mut unheld: SmallVec<[Name<'_>; 2]> = SmallVec::new();
        for &name in names {
            // A name every record's assertion of it was refused — a
            // container's, or a publisher id one source gave several
            // records — is held by no work, and a stub's where cited.
            match self.arena.find(name) {
                Some(ix) if !self.refused.contains(&ix) => held.push(ix),
                _ => unheld.push(name),
            }
        }
        let outcome = if held.is_empty() {
            if unheld.is_empty() {
                "none"
            } else {
                let mut first = None;
                for name in unheld {
                    let ix = self.stubs.intern(name).expect("the stubs' arena is hashed");
                    if self.stub_parent.len() <= ix as usize {
                        self.stub_parent.push(ix);
                    }
                    match first {
                        None => first = Some(ix),
                        Some(f) => stub_union(&mut self.stub_parent, f, ix),
                    }
                }
                "stub"
            }
        } else if held
            .iter()
            .any(|&ix| self.roots[ix as usize] != self.roots[held[0] as usize])
        {
            "disagree"
        } else {
            if !unheld.is_empty() {
                *self
                    .counter(related)
                    .entry((source, "alias beside a held name"))
                    .or_default() += 1;
            }
            "held"
        };
        *self.counter(related).entry((source, outcome)).or_default() += 1;
        outcome
    }

    fn counter(&mut self, related: bool) -> &mut BTreeMap<(SourceId, &'static str), u64> {
        if related {
            &mut self.stats.related
        } else {
            &mut self.stats.targets
        }
    }

    /// Applies the rules of merge plan §3.3 to every component. With
    /// `survey`, a component the rules cannot resolve is counted with an
    /// example instead of stopping the run, and its records are left as
    /// one work with nothing promoted; the caller exits non-zero when
    /// anything was counted.
    ///
    /// # Errors
    ///
    /// Without `survey`: a hairball, or a shape the rules do not cover.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names or records in a run
    pub fn resolve(mut self, survey: bool) -> Result<Resolution, Error> {
        if self.roots.len() != self.arena.len() {
            self.mark();
        }
        self.cut_stars();
        let (names_of, recs_of, roots) = self.grouped()?;
        let mut works = Vec::with_capacity(roots.len());
        let mut components = Components::default();
        for root in roots {
            let names = names_of.get(root as usize);
            let recs = recs_of.get(root as usize);
            components.total += 1;
            if !self.conflict_roots.contains(&root) {
                works.push(self.trivial(names, recs));
                continue;
            }
            components.conflict += 1;
            let (resolved, labels) = match self.conflict(names, recs) {
                Ok(resolved) => resolved,
                Err(e) if survey => {
                    let what = match &e {
                        Error::Hairball { .. } => "hairball",
                        _ => "unmodelled",
                    };
                    *components.unresolved.entry(what).or_default() += 1;
                    let examples = components.unresolved_examples.entry(what).or_default();
                    if examples.len() < EXAMPLES {
                        examples.push(e.to_string());
                    }
                    let mut work = self.trivial(names, recs);
                    for n in &mut work.names {
                        n.standing = Standing::Shared;
                    }
                    works.push(work);
                    continue;
                }
                Err(e) => return Err(e),
            };
            let label = labels.to_string();
            *components.shapes.entry(label.clone()).or_default() += 1;
            let examples = components.examples.entry(label).or_default();
            if examples.len() < EXAMPLES {
                examples.push(self.describe(&resolved));
            }
            works.extend(resolved);
        }
        drop((names_of, recs_of));
        self.refusals_onto(&mut works);
        for work in &works {
            components.accepted += work.names.len() as u64;
            components.rejected += work.rejected.len() as u64;
            components.uncorroborated += u64::from(work.uncorroborated);
            for n in &work.names {
                *components
                    .standings
                    .entry((self.arena.key(n.name), n.standing))
                    .or_default() += 1;
            }
            let pattern = work.sources(&self.records).to_string();
            let dup = if work.duplicates.is_empty() {
                pattern
            } else {
                format!("{pattern}, duplicates at {}", work.duplicates)
            };
            *components.patterns.entry(dup).or_default() += 1;
        }
        Ok(Resolution {
            arena: self.arena,
            stubs: self.stubs,
            stub_parent: self.stub_parent,
            records: self.records,
            natives: self.natives,
            works,
            stats: self.stats,
            components,
        })
    }

    /// Each component's names and records, grouped by its root in one
    /// array apiece rather than a list per component — on the corpus the
    /// lists were about 11 GiB of the pass's peak (the merge's memory,
    /// step 2a, 2026-09-24) — and the roots in the order of each
    /// component's first record, which is the order the sets were read in
    /// and not how the names are numbered. Every name that is not refused
    /// is some record's, so every component has one.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names or records in a run
    fn grouped(&self) -> Result<(Groups, Groups, Vec<NameIx>), Error> {
        let n = self.arena.len();
        let names_of = Groups::of(n, || {
            self.roots
                .iter()
                .enumerate()
                .filter(|(i, _)| !self.refused.contains(&(*i as NameIx)))
                .map(|(i, &root)| (root, i as NameIx))
        });
        let recs_of = Groups::of(n, || {
            self.records
                .iter()
                .enumerate()
                .map(|(rix, rec)| (self.roots[rec.native as usize], rix as RecordIx))
        });
        let mut seen = vec![0u64; n.div_ceil(64)];
        let mut roots: Vec<NameIx> = Vec::new();
        for rec in &self.records {
            let root = self.roots[rec.native as usize] as usize;
            if seen[root / 64] & (1 << (root % 64)) == 0 {
                seen[root / 64] |= 1 << (root % 64);
                roots.push(root as NameIx);
            }
        }
        if let Some(stray) =
            (0..n).find(|&r| !names_of.get(r).is_empty() && recs_of.get(r).is_empty())
        {
            return Err(Error::Unmodelled(format!(
                "{} is in a component with no record",
                self.arena.get(names_of.get(stray)[0])
            )));
        }
        Ok((names_of, recs_of, roots))
    }

    /// A component with at most one name per namespace and one record per
    /// source: every name promoted.
    fn trivial(&self, names: &[NameIx], recs: &[RecordIx]) -> Work {
        let mut work = Work {
            records: recs.iter().copied().collect(),
            ..Work::default()
        };
        for &name in names {
            let mut by = Sources::NONE;
            let mut native = false;
            for &rix in recs {
                let rec = &self.records[rix as usize];
                if let Some(a) = rec.asserts.iter().find(|a| a.name == name) {
                    by.insert(rec.source);
                    native |= a.native;
                }
            }
            work.names.push(WorkName {
                name,
                standing: Standing::Promoted,
                by,
                native,
            });
        }
        work
    }

    /// The rules of §3.3 on one conflict component, kept in one place so
    /// they read in the plan's order.
    #[allow(clippy::too_many_lines)]
    fn conflict(&self, names: &[NameIx], recs: &[RecordIx]) -> Result<(Vec<Work>, Labels), Error> {
        if recs.len() > HAIRBALL_RECORDS || names.len() > HAIRBALL_NAMES {
            return Err(Error::Hairball {
                records: recs.len(),
                names: names.len(),
                example: self.describe(&[self.trivial(names, recs)]),
            });
        }
        let mut labels = Labels::default();
        // Who asserts each name, as positions into `recs`, the names in
        // name order: what a verdict or an error names first follows the
        // names, not how they were numbered.
        let mut found: BTreeMap<NameIx, Asserting> = BTreeMap::new();
        for (pos, &rix) in recs.iter().enumerate() {
            for a in &self.records[rix as usize].asserts {
                found.entry(a.name).or_default().push((pos, a.native));
            }
        }
        let mut asserters: Vec<(NameIx, Asserting)> = found.into_iter().collect();
        asserters.sort_by(|a, b| self.arena.get(a.0).cmp(&self.arena.get(b.0)));
        // Records sharing a name are one work when their titles agree.
        let mut uf: Vec<usize> = (0..recs.len()).collect();
        let default = OwnedTitles::default();
        let title_of = |pos: usize| self.titles.get(&recs[pos]).unwrap_or(&default);
        for (_, list) in &asserters {
            for i in 0..list.len() {
                for j in i + 1..list.len() {
                    let (a, b) = (list[i].0, list[j].0);
                    let agreement = title::agree(title_of(a), title_of(b));
                    if agreement.is_agreement() || agreement == Agreement::Untestable {
                        let (ra, rb) = (record_root(&mut uf, a), record_root(&mut uf, b));
                        if ra != rb {
                            uf[ra.max(rb)] = ra.min(rb);
                        }
                        if agreement == Agreement::Untestable {
                            labels.uncorroborated = true;
                        }
                    }
                }
            }
        }
        let mut group_of = vec![0usize; recs.len()];
        let mut groups: Vec<usize> = Vec::new();
        for (pos, slot) in group_of.iter_mut().enumerate() {
            let r = record_root(&mut uf, pos);
            *slot = if let Some(g) = groups.iter().position(|&x| x == r) {
                g
            } else {
                groups.push(r);
                groups.len() - 1
            };
        }
        let mut works: Vec<Work> = groups.iter().map(|_| Work::default()).collect();
        for (pos, &rix) in recs.iter().enumerate() {
            works[group_of[pos]].records.push(rix);
        }
        for w in &mut works {
            w.uncorroborated = labels.uncorroborated;
        }
        // Verdicts, name by name.
        for &(name, ref list) in &asserters {
            let mut asserting: SmallVec<[usize; 4]> = SmallVec::new();
            let mut natives: SmallVec<[usize; 2]> = SmallVec::new();
            for &(pos, native) in list {
                let g = group_of[pos];
                if !asserting.contains(&g) {
                    asserting.push(g);
                }
                if native && !natives.contains(&g) {
                    natives.push(g);
                }
            }
            let by_in = |g: usize| -> Sources {
                list.iter()
                    .filter(|(pos, _)| group_of[*pos] == g)
                    .map(|(pos, _)| self.records[recs[*pos] as usize].source)
                    .collect()
            };
            match (asserting.len(), natives.len()) {
                (1, _) => {
                    let g = asserting[0];
                    works[g].names.push(WorkName {
                        name,
                        standing: Standing::Alias,
                        by: by_in(g),
                        native: !natives.is_empty(),
                    });
                }
                (_, 1) => {
                    let holder = natives[0];
                    let holder_rec = list
                        .iter()
                        .find(|(pos, native)| *native && group_of[*pos] == holder)
                        .map_or(recs[0], |(pos, _)| recs[*pos]);
                    works[holder].names.push(WorkName {
                        name,
                        standing: Standing::Alias,
                        by: by_in(holder),
                        native: true,
                    });
                    for &(pos, native) in list {
                        let g = group_of[pos];
                        if g != holder && !native {
                            let field = self.records[recs[pos] as usize]
                                .asserts
                                .iter()
                                .find(|a| a.name == name)
                                .map(|a| a.field)
                                .expect("an asserter asserts the name");
                            works[g].rejected.push(Rejection {
                                name,
                                record: recs[pos],
                                field,
                                holder: Some(holder_rec),
                                why: Reason::WrongLink,
                            });
                            labels.wrong_link = true;
                        }
                    }
                }
                (_, 0) => {
                    for &g in &asserting {
                        works[g].names.push(WorkName {
                            name,
                            standing: Standing::Shared,
                            by: by_in(g),
                            native: false,
                        });
                    }
                    labels.shared = true;
                }
                (_, n) => {
                    return Err(Error::Unmodelled(format!(
                        "{} is held natively by records in {n} different works: {}",
                        self.arena.get(name),
                        self.describe(&works)
                    )));
                }
            }
        }
        for work in &mut works {
            self.promote(work);
            let mut per_source = [0u8; 3];
            for &rix in &work.records {
                per_source[self.records[rix as usize].source as usize] += 1;
            }
            for s in SourceId::ALL {
                if per_source[s as usize] > 1 {
                    work.duplicates.insert(s);
                    labels.duplicates.insert(s);
                }
            }
            let names: Vec<Cow<'_, OwnedName>> = work
                .names
                .iter()
                .filter(|n| n.standing != Standing::Shared && self.arena.key(n.name) == Key::Doi)
                .map(|n| self.arena.get(n.name))
                .collect();
            let dois: Vec<&str> = names
                .iter()
                .filter_map(|n| match &**n {
                    OwnedName::Doi(d) => Some(&**d),
                    _ => None,
                })
                .collect();
            if dois.len() > 1 {
                if dois.iter().any(|a| {
                    dois.iter()
                        .any(|b| a != b && a.replace("//", "/") == b.replace("//", "/"))
                }) {
                    labels.spelling = true;
                } else {
                    labels.two_dois = true;
                }
            }
            let pmids = work
                .names
                .iter()
                .filter(|n| n.standing != Standing::Shared)
                .filter(|n| self.arena.key(n.name) == Key::Pmid)
                .count();
            if pmids > 1 {
                labels.two_pmids = true;
            }
        }
        Ok((works, labels))
    }

    /// The precedence of §3.4: one promoted name per namespace, the rest
    /// aliases; shared names stay shared.
    fn promote(&self, work: &mut Work) {
        for key in Key::ALL {
            let candidates: SmallVec<[usize; 4]> = work
                .names
                .iter()
                .enumerate()
                .filter(|(_, n)| n.standing != Standing::Shared && self.arena.key(n.name) == key)
                .map(|(i, _)| i)
                .collect();
            let Some(best) = candidates.iter().copied().min_by_key(|&i| {
                let n = &work.names[i];
                let rank = match key {
                    Key::Pmid | Key::Doi => {
                        if n.native {
                            0
                        } else if n.by.contains(SourceId::Pubmed) {
                            1
                        } else {
                            2
                        }
                    }
                    // A publisher id has one asserter kind: the
                    // publisher, through whichever record carries it.
                    Key::Pmc | Key::OpenAlex | Key::Publisher => 0,
                };
                (rank, self.arena.get(n.name).clone())
            }) else {
                continue;
            };
            for i in candidates {
                work.names[i].standing = if i == best {
                    Standing::Promoted
                } else {
                    Standing::Alias
                };
            }
        }
    }

    /// One line a person can read: each work's records and names.
    fn describe(&self, works: &[Work]) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        for (i, w) in works.iter().enumerate() {
            if i > 0 {
                out.push_str(" || ");
            }
            out.push('{');
            for (j, &rix) in w.records.iter().enumerate() {
                if j > 0 {
                    out.push(' ');
                }
                let rec = &self.records[rix as usize];
                let _ = write!(out, "{}={}", rec.source, self.arena.get(rec.native));
            }
            out.push_str(" |");
            // Names in name order, as the rows write them, not in the
            // order they were numbered.
            let mut names: Vec<(Cow<'_, OwnedName>, String)> = w
                .names
                .iter()
                .map(|n| {
                    (
                        self.arena.get(n.name),
                        format!(" {}({})", self.arena.get(n.name), n.standing),
                    )
                })
                .collect();
            names.extend(w.rejected.iter().map(|r| {
                let text = match r.holder {
                    Some(holder) => format!(
                        " {}(rejected: {} holds it)",
                        self.arena.get(r.name),
                        self.arena.get(self.records[holder as usize].native)
                    ),
                    None => format!(" {}(rejected: {})", self.arena.get(r.name), r.why),
                };
                (self.arena.get(r.name), text)
            }));
            names.sort();
            for (_, text) in names {
                out.push_str(&text);
            }
            out.push('}');
        }
        out
    }
}

/// What became of one record read from a set.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kept {
    Record,
    /// A copy a newer one speaks over, whose names joined the record's
    /// (merge plan §8): its relations are not the record's — the
    /// speaking copy alone gives those.
    Joined,
    /// A copy a deletion withdrew.
    Silent,
    NoNative,
    /// A name the run's dictionary was not made with.
    NotInDictionary(OwnedName),
}

/// A rejected name, why, and the record that holds it, if one does.
type Refusal = (NameIx, Reason, Option<RecordIx>, Field);

/// The records claiming one publisher id, each once, with the field.
type Claimants = SmallVec<[(RecordIx, Field); 2]>;

/// The records asserting one name in a component, as positions into its
/// records, and whether as their native name.
type Asserting = SmallVec<[(usize, bool); 4]>;

/// Items grouped by a key below a bound, each group's items in the order
/// they were given: a counting sort into one array of starts and one of
/// items, where a map of lists would hold a list per key.
struct Groups {
    starts: Vec<u32>,
    items: Vec<u32>,
}

impl Groups {
    /// `pairs` is called twice, and must give the same (key, item) pairs
    /// both times.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names or records in a run
    fn of<I: Iterator<Item = (u32, u32)>>(keys: usize, pairs: impl Fn() -> I) -> Self {
        let mut starts = vec![0u32; keys + 1];
        for (key, _) in pairs() {
            starts[key as usize + 1] += 1;
        }
        for k in 1..=keys {
            starts[k] += starts[k - 1];
        }
        let mut items = vec![0u32; starts[keys] as usize];
        // Each start serves as its group's cursor, which leaves it at the
        // next group's start; shifting them back restores them.
        for (key, item) in pairs() {
            let at = &mut starts[key as usize];
            items[*at as usize] = item;
            *at += 1;
        }
        starts.copy_within(0..keys, 1);
        starts[0] = 0;
        Self { starts, items }
    }

    fn get(&self, key: usize) -> &[u32] {
        &self.items[self.starts[key] as usize..self.starts[key + 1] as usize]
    }
}

/// Names per namespace and records per source in one component.
#[derive(Default)]
struct Tally {
    keys: [u8; Key::ALL.len()],
    sources: [u8; 3],
}

/// Union-find over the records of one conflict component.
fn record_root(uf: &mut [usize], mut x: usize) -> usize {
    while uf[x] != x {
        uf[x] = uf[uf[x]];
        x = uf[x];
    }
    x
}

fn stub_find(parent: &mut [NameIx], mut x: NameIx) -> NameIx {
    while parent[x as usize] != x {
        let grand = parent[parent[x as usize] as usize];
        parent[x as usize] = grand;
        x = grand;
    }
    x
}

fn stub_union(parent: &mut [NameIx], a: NameIx, b: NameIx) {
    let (ra, rb) = (stub_find(parent, a), stub_find(parent, b));
    if ra != rb {
        parent[ra.max(rb) as usize] = ra.min(rb);
    }
}

fn key_ix(key: Key) -> usize {
    Key::ALL
        .iter()
        .position(|k| *k == key)
        .expect("every key is in ALL")
}

/// What the rules said about a name's place in a work (merge plan §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Standing {
    /// The one name of its namespace used for display and link-out.
    Promoted,
    /// Accepted, not promoted.
    Alias,
    /// Accepted into more than one work; never promoted.
    Shared,
}

impl fmt::Display for Standing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Promoted => "promoted",
            Self::Alias => "alias",
            Self::Shared => "shared",
        })
    }
}

/// An accepted name of a work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkName {
    #[allow(missing_docs)]
    pub name: NameIx,
    #[allow(missing_docs)]
    pub standing: Standing,
    /// Who asserted it.
    pub by: Sources,
    /// Whether a record of the work holds it natively.
    pub native: bool,
}

/// A rejected assertion: `record` claimed `name` from `field`, and the
/// rule `why` refused it (merge plan §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejection {
    #[allow(missing_docs)]
    pub name: NameIx,
    #[allow(missing_docs)]
    pub record: RecordIx,
    #[allow(missing_docs)]
    pub field: Field,
    /// The record that holds the name; none when no record does, because
    /// the value identifies no article at all.
    pub holder: Option<RecordIx>,
    #[allow(missing_docs)]
    pub why: Reason,
}

/// Why an assertion was rejected (merge plan §3.3). A rejected assertion
/// stays on the work with its reason; nothing is deleted (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// A foreign id naming a record whose title disagrees: the record
    /// that holds the name keeps it, the one that claimed it loses it.
    WrongLink,
    /// A name no article's: a publisher id one source claimed for more
    /// than one of its own records (§3.1, 2026-09-19) — an issue's id, a
    /// volume's, a journal's title in the slot — or a name every record of
    /// a star beyond the rules carries under a title of its own (§3.3,
    /// 2026-09-24) — a container's DOI on its parts, or a DOI cut short.
    NotAnArticleId,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WrongLink => "wrong link",
            Self::NotAnArticleId => "not an article's id",
        })
    }
}

/// A work as identity leaves it: records, names with verdicts, flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Work {
    #[allow(missing_docs)]
    pub records: SmallVec<[RecordIx; 3]>,
    #[allow(missing_docs)]
    pub names: SmallVec<[WorkName; 4]>,
    #[allow(missing_docs)]
    pub rejected: SmallVec<[Rejection; 1]>,
    /// Sources with more than one record in this work.
    pub duplicates: Sources,
    /// Some record had no title to test, and was joined untested.
    pub uncorroborated: bool,
}

impl Work {
    /// The sources holding a record of the work.
    #[must_use]
    pub fn sources(&self, records: &[Record]) -> Sources {
        self.records
            .iter()
            .map(|&rix| records[rix as usize].source)
            .collect()
    }

    /// The promoted name of one namespace.
    #[must_use]
    pub fn promoted<'a>(&self, key: Key, arena: &'a Arena) -> Option<Cow<'a, OwnedName>> {
        self.names
            .iter()
            .find(|n| n.standing == Standing::Promoted && arena.key(n.name) == key)
            .map(|n| arena.get(n.name))
    }

    /// The names the registry keys the work by: every accepted name that
    /// is not shared, promoted ones first in namespace order, then the
    /// aliases sorted — a deterministic key for a deterministic order.
    #[must_use]
    pub fn registry_names(&self, arena: &Arena) -> Vec<OwnedName> {
        let mut out: Vec<OwnedName> = Key::ALL
            .iter()
            .filter_map(|&k| self.promoted(k, arena).map(Cow::into_owned))
            .collect();
        let mut aliases: Vec<OwnedName> = self
            .names
            .iter()
            .filter(|n| n.standing == Standing::Alias)
            .map(|n| arena.get(n.name).into_owned())
            .collect();
        aliases.sort();
        out.extend(aliases);
        out
    }
}

/// The shapes a conflict component showed (merge plan §3.3), for the
/// report: one flag per shape, several of which can hold at once.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct Labels {
    duplicates: Sources,
    two_dois: bool,
    spelling: bool,
    two_pmids: bool,
    wrong_link: bool,
    shared: bool,
    uncorroborated: bool,
}

impl fmt::Display for Labels {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = Vec::new();
        if !self.duplicates.is_empty() {
            parts.push(format!("duplicate records ({})", self.duplicates));
        }
        for (on, label) in [
            (self.two_dois, "two dois"),
            (self.spelling, "doi spellings"),
            (self.two_pmids, "two pmids"),
            (self.wrong_link, "wrong link"),
            (self.shared, "shared name"),
            (self.uncorroborated, "uncorroborated"),
        ] {
            if on {
                parts.push(label.to_owned());
            }
        }
        if parts.is_empty() {
            parts.push("resolved without a rule".to_owned());
        }
        f.write_str(&parts.join(" + "))
    }
}

/// What resolving counted.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Components {
    #[allow(missing_docs)]
    pub total: u64,
    #[allow(missing_docs)]
    pub conflict: u64,
    /// Conflict components by the shapes they showed, with up to three
    /// examples each.
    pub shapes: BTreeMap<String, u64>,
    #[allow(missing_docs)]
    pub examples: BTreeMap<String, Vec<String>>,
    /// Accepted names over all works.
    pub accepted: u64,
    #[allow(missing_docs)]
    pub rejected: u64,
    /// Works joined on an untestable title.
    pub uncorroborated: u64,
    /// Names by namespace and standing.
    pub standings: BTreeMap<(Key, Standing), u64>,
    /// Works by the sources holding them, duplicates noted.
    pub patterns: BTreeMap<String, u64>,
    /// Under `--survey`: components the rules could not resolve, by why.
    pub unresolved: BTreeMap<&'static str, u64>,
    #[allow(missing_docs)]
    pub unresolved_examples: BTreeMap<&'static str, Vec<String>>,
}

/// What the identity pass leaves: every work with its verdicts, the names
/// references point at that no work has, and the counts.
#[derive(Debug)]
pub struct Resolution {
    #[allow(missing_docs)]
    pub arena: Arena,
    /// Names cited and held by no work: the stubs' names.
    pub stubs: Arena,
    stub_parent: Vec<NameIx>,
    #[allow(missing_docs)]
    pub records: Vec<Record>,
    /// Each name's record, where it is one's native name: see
    /// [`Resolution::record_of`].
    natives: Vec<RecordIx>,
    #[allow(missing_docs)]
    pub works: Vec<Work>,
    #[allow(missing_docs)]
    pub stats: Stats,
    #[allow(missing_docs)]
    pub components: Components,
}

/// A full merge's names by number (the merge's memory, step 2d): the
/// arena's first, by their index — a name's rank, under a dictionary — then
/// the stubs', past the arena's. A name in both — refused wherever it was
/// asserted and cited, so a stub's — is the arena's index, which is how a
/// stub work's list and the registry's log both find it.
impl crate::registry::NameIndex for Resolution {
    fn len(&self) -> usize {
        self.arena.len() + self.stubs.len()
    }

    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    fn index(&self, name: &OwnedName) -> Option<u32> {
        let name = name.as_name();
        self.arena
            .find(name)
            .or_else(|| self.stubs.find(name).map(|s| self.arena.len() as u32 + s))
    }

    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    fn name(&self, ix: u32) -> OwnedName {
        let held = self.arena.len() as u32;
        if ix < held {
            self.arena.get(ix).into_owned()
        } else {
            self.stubs.get(ix - held).into_owned()
        }
    }
}

impl Resolution {
    /// The held works in the registry's order, as [`Resolution::held_order`]
    /// gives it, each with its registry names as [`NameIndex`] numbers in
    /// one [`Flat`] list, and each one's index into
    /// [`Resolution::works`]. Under a dictionary a name's number is its
    /// rank, so a list of numbers sorts as the list of names would, and
    /// no work's names are read out to sort them: on the corpus the copy
    /// was about 5 GB (the merge's memory, step 2d, 2026-09-24).
    ///
    /// [`NameIndex`]: crate::registry::NameIndex
    /// [`Flat`]: crate::registry::Flat
    ///
    /// # Panics
    ///
    /// A registry name that is not the run's, which cannot be.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 works in a run
    pub fn registry_works(&self) -> (Vec<u32>, crate::registry::Flat) {
        use crate::registry::{Flat, NameIndex, Works};
        let mut out = Flat::new();
        if self.arena.dictionary().is_none() {
            let order = self.held_order();
            for (_, names) in &order {
                out.push(names.iter().map(|n| self.index(n).expect("the run's name")));
            }
            return (order.iter().map(|(w, _)| *w as u32).collect(), out);
        }
        let mut by_work = Flat::new();
        for work in &self.works {
            let promoted = Key::ALL.iter().filter_map(|&key| {
                work.names
                    .iter()
                    .find(|n| n.standing == Standing::Promoted && self.arena.key(n.name) == key)
                    .map(|n| n.name)
            });
            let mut aliases: SmallVec<[NameIx; 4]> = work
                .names
                .iter()
                .filter(|n| n.standing == Standing::Alias)
                .map(|n| n.name)
                .collect();
            aliases.sort_unstable();
            by_work.push(promoted.chain(aliases));
        }
        let natives = |w: usize| -> SmallVec<[NameIx; 3]> {
            let mut n: SmallVec<[NameIx; 3]> = self.works[w]
                .records
                .iter()
                .map(|&rix| self.records[rix as usize].native)
                .collect();
            n.sort_unstable();
            n
        };
        let mut order: Vec<u32> = (0..self.works.len() as u32).collect();
        order.sort_by(|&a, &b| {
            let (a, b) = (a as usize, b as usize);
            by_work
                .names(a)
                .cmp(by_work.names(b))
                .then_with(|| natives(a).cmp(&natives(b)))
        });
        for &w in &order {
            out.push(by_work.names(w as usize).iter().copied());
        }
        (order, out)
    }

    /// The record whose native name is at `ix`, if one is.
    #[must_use]
    pub fn record_of(&self, ix: NameIx) -> Option<RecordIx> {
        self.natives
            .get(ix as usize)
            .copied()
            .filter(|&rix| rix != NO_RECORD)
    }

    /// The held works in the registry's order (merge plan §4): each
    /// work's index into [`Resolution::works`] with its registry names,
    /// sorted by the names, then, for the works that have none — a
    /// hairball under `--survey`, whose names are all shared — by their
    /// records' native names. Nothing in it follows how names are
    /// numbered or the order works were made in.
    #[must_use]
    pub fn held_order(&self) -> Vec<(usize, Vec<OwnedName>)> {
        let mut held: Vec<(usize, Vec<OwnedName>)> = self
            .works
            .iter()
            .enumerate()
            .map(|(i, w)| (i, w.registry_names(&self.arena)))
            .collect();
        let natives = |w: usize| -> Vec<Cow<'_, OwnedName>> {
            let mut n: Vec<Cow<'_, OwnedName>> = self.works[w]
                .records
                .iter()
                .map(|&rix| self.arena.get(self.records[rix as usize].native))
                .collect();
            n.sort();
            n
        };
        held.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| natives(a.0).cmp(&natives(b.0))));
        held
    }

    /// The stubs (merge plan §4), each the names one or more references
    /// gave one uncited work, sorted within and between, so the registry
    /// sees them in one order every run.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    pub fn stub_works(&self) -> Vec<Vec<OwnedName>> {
        let mut parent = self.stub_parent.clone();
        let mut by_root: HashMap<NameIx, Vec<OwnedName>> = HashMap::new();
        for ix in 0..self.stubs.len() as NameIx {
            let root = stub_find(&mut parent, ix);
            by_root
                .entry(root)
                .or_default()
                .push(self.stubs.get(ix).into_owned());
        }
        let mut works: Vec<Vec<OwnedName>> = by_root.into_values().collect();
        for names in &mut works {
            names.sort();
        }
        works.sort();
        works
    }
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)] // real ids, as the sample spells them
mod tests {
    use model::{OpenAlexId, PmcId, Pmid};

    use std::borrow::Cow;

    use super::{Error, Identity, Reason, Resolution, Standing};
    use crate::dictionary::Dictionary;
    use crate::name::{Assertion, Name, OwnedName};
    use crate::title::Titles;
    use crate::work::{SourceId, Sources};

    fn pmid(n: u32) -> Pmid {
        Pmid::new(n).unwrap()
    }

    fn w(n: u64) -> OpenAlexId {
        OpenAlexId::from_bare(&format!("W{n}")).unwrap()
    }

    fn pubmed(id: &mut Identity, p: u32, doi: Option<&str>, pmc: Option<u32>, title: &str) {
        let mut a = vec![Assertion {
            name: Name::Pmid(pmid(p)),
            field: "pmid",
            native: true,
        }];
        if let Some(d) = doi {
            a.push(Assertion {
                name: Name::Doi(d),
                field: "ids.doi",
                native: false,
            });
        }
        if let Some(c) = pmc {
            a.push(Assertion {
                name: Name::Pmc(PmcId::new(c).unwrap()),
                field: "ids.pmc",
                native: false,
            });
        }
        let titles = Titles {
            title: Some(title).filter(|t| !t.is_empty()),
            vernacular: None,
        };
        id.push(SourceId::Pubmed, &a, titles).unwrap();
    }

    fn crossref(id: &mut Identity, doi: &str, title: &str) {
        let a = [Assertion {
            name: Name::Doi(doi),
            field: "doi",
            native: true,
        }];
        id.push(
            SourceId::Crossref,
            &a,
            Titles {
                title: Some(title),
                vernacular: None,
            },
        )
        .unwrap();
    }

    fn openalex(id: &mut Identity, n: u64, doi: Option<&str>, p: Option<u32>, title: &str) {
        let mut a = vec![Assertion {
            name: Name::OpenAlex(w(n)),
            field: "id",
            native: true,
        }];
        if let Some(d) = doi {
            a.push(Assertion {
                name: Name::Doi(d),
                field: "doi",
                native: false,
            });
        }
        if let Some(p) = p {
            a.push(Assertion {
                name: Name::Pmid(pmid(p)),
                field: "pmid",
                native: false,
            });
        }
        let titles = Titles {
            title: Some(title).filter(|t| !t.is_empty()),
            vernacular: None,
        };
        id.push(SourceId::OpenAlex, &a, titles).unwrap();
    }

    /// Each work as its names with standings, sorted, for assertions.
    fn works(res: &Resolution) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = res
            .works
            .iter()
            .map(|work| {
                let mut names: Vec<String> = work
                    .names
                    .iter()
                    .map(|n| format!("{}={}", res.arena.get(n.name), n.standing))
                    .collect();
                names.extend(
                    work.rejected
                        .iter()
                        .map(|r| format!("{}=rejected", res.arena.get(r.name))),
                );
                names.sort();
                names
            })
            .collect();
        out.sort();
        out
    }

    fn shapes(res: &Resolution) -> Vec<(String, u64)> {
        res.components
            .shapes
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    #[test]
    fn a_work_seen_from_three_sources_needs_no_rule() {
        let mut id = Identity::new();
        pubmed(
            &mut id,
            41575207,
            Some("10.1128/aem.02335-25"),
            None,
            "Title.",
        );
        crossref(&mut id, "10.1128/aem.02335-25", "Title");
        openalex(
            &mut id,
            1,
            Some("10.1128/aem.02335-25"),
            Some(41575207),
            "Title",
        );
        pubmed(&mut id, 2, None, None, "Alone");
        id.mark();
        assert_eq!(id.conflicts(), 0);
        let res = id.resolve(false).unwrap();
        assert_eq!(res.components.total, 2);
        assert_eq!(
            works(&res),
            [
                vec![
                    "doi:10.1128/aem.02335-25=promoted",
                    "openalex:W1=promoted",
                    "pmid:41575207=promoted"
                ],
                vec!["pmid:2=promoted"]
            ]
        );
        let three = res.works.iter().find(|w| w.records.len() == 3).unwrap();
        assert_eq!(
            three.sources(&res.records),
            SourceId::ALL.into_iter().collect::<Sources>()
        );
        assert_eq!(
            three
                .registry_names(&res.arena)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["doi:10.1128/aem.02335-25", "pmid:41575207", "openalex:W1"]
        );
        assert_eq!(res.components.patterns["pubmed+crossref+openalex"], 1);
        assert_eq!(res.components.patterns["pubmed"], 1);
    }

    #[test]
    fn enrichment_is_not_a_conflict() {
        let mut id = Identity::new();
        pubmed(&mut id, 16212218, None, None, "Evaluation of availability");
        openalex(
            &mut id,
            2116538089,
            Some("10.26719/2004.10.3.406"),
            Some(16212218),
            "Evaluation of availability",
        );
        id.mark();
        assert_eq!(id.conflicts(), 0);
        let res = id.resolve(false).unwrap();
        assert_eq!(res.works.len(), 1);
        assert_eq!(
            res.works[0]
                .promoted(ingest::Key::Doi, &res.arena)
                .unwrap()
                .to_string(),
            "doi:10.26719/2004.10.3.406"
        );
    }

    #[test]
    fn duplicate_records_of_one_source_are_one_work_flagged() {
        let mut id = Identity::new();
        pubmed(
            &mut id,
            41575906,
            Some("10.1021/acs.jcim.5c01856"),
            None,
            "Tokenization for Molecular Foundation Models.",
        );
        crossref(
            &mut id,
            "10.1021/acs.jcim.5c01856",
            "Tokenization for Molecular Foundation Models",
        );
        openalex(
            &mut id,
            7125474195,
            Some("10.1021/acs.jcim.5c01856"),
            Some(41575906),
            "Tokenization for Molecular Foundation Models",
        );
        openalex(
            &mut id,
            4403785850,
            Some("10.1021/acs.jcim.5c01856"),
            Some(41575906),
            "Tokenization for Molecular Foundation Models",
        );
        id.mark();
        assert_eq!(id.conflicts(), 1);
        let res = id.resolve(false).unwrap();
        assert_eq!(res.works.len(), 1);
        let work = &res.works[0];
        assert_eq!(work.duplicates, [SourceId::OpenAlex].into_iter().collect());
        assert_eq!(
            work.promoted(ingest::Key::OpenAlex, &res.arena)
                .unwrap()
                .to_string(),
            "openalex:W4403785850",
            "the lowest id"
        );
        assert_eq!(
            shapes(&res),
            [("duplicate records (openalex)".to_owned(), 1)]
        );
        assert_eq!(
            res.components.patterns["pubmed+crossref+openalex, duplicates at openalex"],
            1
        );
    }

    #[test]
    fn two_dois_bridged_by_a_pmid_are_one_work_with_the_registry_doi_promoted() {
        let mut id = Identity::new();
        pubmed(
            &mut id,
            10958752,
            Some("10.1097/00005392-200009020-00046"),
            None,
            "The long-term followup of newborns with severe unilateral hydronephrosis.",
        );
        crossref(
            &mut id,
            "10.1097/00005392-200009020-00046",
            "The long-term followup of newborns with severe unilateral hydronephrosis",
        );
        openalex(
            &mut id,
            2097792642,
            Some("10.1016/s0022-5347(05)67262-x"),
            Some(10958752),
            "THE LONG-TERM FOLLOWUP OF NEWBORNS WITH SEVERE UNILATERAL HYDRONEPHROSIS",
        );
        id.mark();
        let res = id.resolve(false).unwrap();
        assert_eq!(
            works(&res),
            [vec![
                "doi:10.1016/s0022-5347(05)67262-x=alias",
                "doi:10.1097/00005392-200009020-00046=promoted",
                "openalex:W2097792642=promoted",
                "pmid:10958752=promoted",
            ]]
        );
        assert_eq!(shapes(&res), [("two dois".to_owned(), 1)]);
    }

    #[test]
    fn two_spellings_of_one_doi_are_both_names() {
        let mut id = Identity::new();
        pubmed(
            &mut id,
            11129362,
            Some("10.1037//0278-6133.19.6.586"),
            None,
            "Relationship of subjective and objective social status",
        );
        openalex(
            &mut id,
            2133344766,
            Some("10.1037/0278-6133.19.6.586"),
            Some(11129362),
            "Relationship of subjective and objective social status",
        );
        id.mark();
        let res = id.resolve(false).unwrap();
        assert_eq!(
            works(&res),
            [vec![
                "doi:10.1037//0278-6133.19.6.586=promoted",
                "doi:10.1037/0278-6133.19.6.586=alias",
                "openalex:W2133344766=promoted",
                "pmid:11129362=promoted",
            ]],
            "PubMed's spelling outranks OpenAlex's when Crossref holds neither"
        );
        assert_eq!(shapes(&res), [("doi spellings".to_owned(), 1)]);
    }

    #[test]
    fn two_pmids_for_one_doi_keep_the_held_records_pmid_promoted() {
        let mut id = Identity::new();
        pubmed(
            &mut id,
            41575784,
            Some("10.1146/annurev-biophys-021424-011450"),
            None,
            "Single-Cell Proteomic Technologies: Tools in the Quest for Principles.",
        );
        crossref(
            &mut id,
            "10.1146/annurev-biophys-021424-011450",
            "Single-Cell Proteomic Technologies: Tools in the Quest for Principles",
        );
        openalex(
            &mut id,
            4414625078,
            Some("10.1146/annurev-biophys-021424-011450"),
            Some(41019209),
            "Single-Cell Proteomic Technologies: Tools in the quest for principles.",
        );
        id.mark();
        let res = id.resolve(false).unwrap();
        assert_eq!(
            works(&res),
            [vec![
                "doi:10.1146/annurev-biophys-021424-011450=promoted",
                "openalex:W4414625078=promoted",
                "pmid:41019209=alias",
                "pmid:41575784=promoted",
            ]]
        );
        assert_eq!(shapes(&res), [("two pmids".to_owned(), 1)]);
    }

    #[test]
    fn a_wrong_link_is_rejected_and_the_component_splits() {
        let mut id = Identity::new();
        pubmed(
            &mut id,
            39406291,
            Some("10.1016/j.nbd.2024.106701"),
            None,
            "Neuropathology in an &#x3b1;-synuclein preformed fibril mouse model occurs independent of",
        );
        crossref(
            &mut id,
            "10.1016/j.nbd.2024.106701",
            "Neuropathology in an α-synuclein preformed fibril mouse model occurs independent of",
        );
        openalex(
            &mut id,
            39406291,
            Some("10.22028/d291-24694"),
            Some(39406291),
            "Preparation of functional coatings for display glass applications by sol-gel",
        );
        id.mark();
        assert_eq!(id.conflicts(), 1);
        let res = id.resolve(false).unwrap();
        assert_eq!(
            works(&res),
            [
                vec![
                    "doi:10.1016/j.nbd.2024.106701=promoted",
                    "pmid:39406291=promoted"
                ],
                vec![
                    "doi:10.22028/d291-24694=promoted",
                    "openalex:W39406291=promoted",
                    "pmid:39406291=rejected"
                ],
            ]
        );
        assert_eq!(res.components.rejected, 1);
        assert_eq!(shapes(&res), [("wrong link".to_owned(), 1)]);
        let rejected = res.works.iter().find(|w| !w.rejected.is_empty()).unwrap();
        let holder = &res.records[rejected.rejected[0].holder.unwrap() as usize];
        assert_eq!(holder.source, SourceId::Pubmed);
        assert_eq!(res.components.total, 1, "one component, two works");
        assert_eq!(res.works.len(), 2);
    }

    #[test]
    fn a_publisher_id_one_source_gives_two_records_is_no_article_id() {
        // `JAMP-14-1` is a journal, volume and issue, and PubMed gave it
        // on ten records of one issue; two is enough to prove the rule.
        let mut id = Identity::new();
        let registrant = model::Registrant::parse("10.30476").unwrap();
        for (p, doi, title) in [
            (
                1u32,
                "10.30476/jamp.2025.01",
                "Simulation in nursing education",
            ),
            (2, "10.30476/jamp.2025.02", "A flipped classroom trial"),
        ] {
            let a = [
                Assertion {
                    name: Name::Pmid(pmid(p)),
                    field: "pmid",
                    native: true,
                },
                Assertion {
                    name: Name::Doi(doi),
                    field: "ids.doi",
                    native: false,
                },
                Assertion {
                    name: Name::Publisher(registrant, "JAMP-14-1"),
                    field: "ids.pii",
                    native: false,
                },
            ];
            id.push(
                SourceId::Pubmed,
                &a,
                Titles {
                    title: Some(title),
                    vernacular: None,
                },
            )
            .unwrap();
        }
        id.mark();
        assert_eq!(id.conflicts(), 0, "the issue's id never joined them");
        let res = id.resolve(false).unwrap();
        assert_eq!(
            works(&res),
            [
                vec![
                    "doi:10.30476/jamp.2025.01=promoted",
                    "pmid:1=promoted",
                    "reg30476:JAMP-14-1=rejected",
                ],
                vec![
                    "doi:10.30476/jamp.2025.02=promoted",
                    "pmid:2=promoted",
                    "reg30476:JAMP-14-1=rejected",
                ],
            ],
            "two works, and the claim on each with its verdict"
        );
        assert_eq!(res.stats.collided, 1);
        assert_eq!(res.stats.collided_assertions, 2);
        assert_eq!(res.components.rejected, 2);
        for work in &res.works {
            let r = &work.rejected[0];
            assert_eq!(r.why, Reason::NotAnArticleId);
            assert_eq!(r.holder, None, "no record holds what names no article");
        }
        assert_eq!(
            res.stats.collided_examples,
            ["reg30476:JAMP-14-1 on 2 records (pubmed 2)"]
        );
    }

    #[test]
    fn one_article_from_two_sources_keeps_its_publisher_id() {
        // The id doing its job, and in the two spellings the two sources
        // write: one name, kept, and no refusal.
        let mut id = Identity::new();
        let registrant = model::Registrant::parse("10.1016").unwrap();
        let doi = "10.1016/j.bpj.2025.01.001";
        let title = "Lipid bilayer mechanics";
        let a = [
            Assertion {
                name: Name::Pmid(pmid(7)),
                field: "pmid",
                native: true,
            },
            Assertion {
                name: Name::Doi(doi),
                field: "ids.doi",
                native: false,
            },
            Assertion {
                name: Name::Publisher(registrant, "S0006-3495(25)00123-4"),
                field: "ids.pii",
                native: false,
            },
        ];
        id.push(
            SourceId::Pubmed,
            &a,
            Titles {
                title: Some(title),
                vernacular: None,
            },
        )
        .unwrap();
        let b = [
            Assertion {
                name: Name::Doi(doi),
                field: "doi",
                native: true,
            },
            Assertion {
                name: Name::Publisher(registrant, "S0006349525001234"),
                field: "alternative_ids",
                native: false,
            },
        ];
        id.push(
            SourceId::Crossref,
            &b,
            Titles {
                title: Some(title),
                vernacular: None,
            },
        )
        .unwrap();
        id.mark();
        let res = id.resolve(false).unwrap();
        assert_eq!(
            works(&res),
            [vec![
                "doi:10.1016/j.bpj.2025.01.001=promoted",
                "pmid:7=promoted",
                "reg1016:S0006349525001234=promoted",
            ]]
        );
        assert_eq!(res.stats.collided, 0);
        assert_eq!(res.components.rejected, 0);
    }

    #[test]
    fn a_name_two_works_claim_and_nobody_holds_is_shared() {
        let mut id = Identity::new();
        openalex(
            &mut id,
            4282919733,
            Some("10.3760/cma.j.cn511374-20210920-00765"),
            Some(35598257),
            "[Expert consensus on the application of prenatal exome sequencing]",
        );
        openalex(
            &mut id,
            4283779715,
            Some("10.3760/cma.j.cn511374-20210920-00765"),
            Some(35773760),
            "[Analysis of two pedigrees affected with inherited dysfibrinogenemia]",
        );
        id.mark();
        let res = id.resolve(false).unwrap();
        assert_eq!(
            works(&res),
            [
                vec![
                    "doi:10.3760/cma.j.cn511374-20210920-00765=shared",
                    "openalex:W4282919733=promoted",
                    "pmid:35598257=promoted"
                ],
                vec![
                    "doi:10.3760/cma.j.cn511374-20210920-00765=shared",
                    "openalex:W4283779715=promoted",
                    "pmid:35773760=promoted"
                ],
            ]
        );
        assert_eq!(shapes(&res), [("shared name".to_owned(), 1)]);
        for work in &res.works {
            assert!(
                work.promoted(ingest::Key::Doi, &res.arena).is_none(),
                "a shared name is never promoted"
            );
            assert_eq!(
                work.registry_names(&res.arena).len(),
                2,
                "and never keys the registry"
            );
        }
        assert_eq!(
            res.components.standings[&(ingest::Key::Doi, Standing::Shared)],
            2
        );
    }

    #[test]
    fn a_record_without_a_title_joins_untested_and_is_flagged() {
        let mut id = Identity::new();
        openalex(&mut id, 1, Some("10.1/x"), None, "");
        openalex(&mut id, 2, Some("10.1/x"), None, "A title");
        id.mark();
        let res = id.resolve(false).unwrap();
        assert_eq!(res.works.len(), 1);
        assert!(res.works[0].uncorroborated);
        assert_eq!(res.components.uncorroborated, 1);
        assert_eq!(
            shapes(&res),
            [(
                "duplicate records (openalex) + uncorroborated".to_owned(),
                1
            )]
        );
    }

    /// Forty of the 42 titles the Physiological Society's proceedings of
    /// 1955 carry in PubMed, all under `10.1113/jphysiol.1955.sp005387`.
    const PROCEEDINGS: [&str; 40] = [
        "Measurement of oxidation-reduction potentials and pH of tissues in vivo",
        "The righting movements of the freely falling cat (filmed at 1500 f.p.s.)",
        "A vectorgraph of extracellular currents in grey matter",
        "A time marker for use with sweep expanders",
        "The action of drugs on central cholinergic synapses",
        "Foveal photopigments in normal and colour-blind",
        "Giant nerve fibers in Protopterus",
        "The early metabolic effects of single injections of 'thyroid hormone'",
        "Control of brain potentials by pseudo-cholinesterase",
        "Thresholds of taste for phenylthiocarbamide in various ethnic groups",
        "Vagal effects on the sinus venosus of the frog's heart",
        "Effects of vagal and sympathetic nerve impulses on the membrane potential of the frog's heart",
        "Effects of lesions in the posterior parietal lobe in trained monkeys",
        "The effect of chlorpromazine on the electrical activity of the brain of the conscious cat",
        "The after effects of impulses in giant nerve fibres",
        "The unidirectional d.c. element of e.e.g. abnormalities",
        "The effect of zinc deficiency on the male genital system",
        "Vago-vagal reflex effects on gastric and pancreatic secretion in cats",
        "Reflex secretion by the parotid gland of the sheep",
        "The absorption of radioactive B12 in normal and gastrectomized rats",
        "Observations on the cerebellar responses to afferent and cerebellar cortical stimulation",
        "Action currents within the central nervous system",
        "The absorption spectrum of rhodopsin in solution and in intact rods",
        "Some central effects of lysergic acid diethylamide on vasomotor responses",
        "Effects of vitamin E deficiency and of methylene blue on oxygen toxicity in the rat",
        "Temperature responses in rat liver and abdomen following cold exposure",
        "Irin, a smooth-muscle contracting substance present in rabbit iris",
        "Occlusion of the ductus venosus in the mature foetal lamb",
        "Changes in composition of the foetal fluids of the sheep during gestation",
        "Acetate uptake by the foetal sheep",
        "Placental glycogen in the rabbit",
        "The cat's nictitating membrane as an isolated preparation",
        "The effect of different environmental and cold-bath temperatures on the cold pressor response in man",
        "Thermal balance in man",
        "Changes in the foetal membranes during pregnancy in the rat",
        "A method for continuously indicating blood pressure",
        "Hyperpnoea and renal water excretion",
        "Bromsulphthalein abstraction by perfused canine livers",
        "Preliminary observations on the protein requirements of kittens receiving a mixed diet",
        "General pharmacology of oenanthotoxin",
    ];

    /// A star beyond the rules (§3.3, 2026-09-24): forty abstracts of one
    /// proceedings, each its own PubMed record, all under the proceedings'
    /// DOI and every title different. No record holds the DOI, so it comes
    /// off every one as no article's id, the records are forty works, and a
    /// reference citing the DOI makes a stub for it.
    #[test]
    fn a_stars_shared_doi_comes_off_its_records() {
        const DOI: &str = "10.1113/jphysiol.1955.sp005387";
        let mut id = Identity::new();
        for (n, title) in (1u32..).zip(PROCEEDINGS) {
            pubmed(&mut id, 13_264_100 + n, Some(DOI), None, title);
        }
        pubmed(&mut id, 1, Some("10.1/cites"), None, "A citing paper");
        id.mark();
        assert_eq!(id.cite(SourceId::Pubmed, &[Name::Doi(DOI)]), "stub");
        let res = id.resolve(false).expect("a star is no hairball");
        assert_eq!(res.works.len(), 41);
        assert_eq!(res.stats.stars, 1);
        assert_eq!(res.stats.star_assertions, 40);
        assert_eq!(
            res.stats.star_examples,
            [format!("doi:{DOI} on 40 records")]
        );
        for work in res
            .works
            .iter()
            .filter(|w| w.records.len() == 1 && !w.rejected.is_empty())
        {
            assert_eq!(work.names.len(), 1, "its PMID alone");
            assert_eq!(work.rejected.len(), 1);
            assert_eq!(work.rejected[0].why, Reason::NotAnArticleId);
            assert_eq!(work.rejected[0].holder, None);
            assert_eq!(
                res.arena.get(work.rejected[0].name).to_string(),
                format!("doi:{DOI}")
            );
        }
        assert_eq!(
            res.works.iter().filter(|w| !w.rejected.is_empty()).count(),
            40
        );
        let stubs = res.stub_works();
        assert_eq!(stubs.len(), 1);
        assert_eq!(stubs[0][0].to_string(), format!("doi:{DOI}"));
    }

    /// Where a record holds the shared name natively — Crossref's record of
    /// the proceedings itself — that record keeps it, and the others'
    /// claims are wrong links, as the rules make them below the cap.
    #[test]
    fn a_stars_name_stays_with_the_record_that_holds_it() {
        const DOI: &str = "10.1113/jphysiol.1955.sp005387";
        let mut id = Identity::new();
        for (n, title) in (1u32..).zip(PROCEEDINGS) {
            pubmed(&mut id, 13_264_100 + n, Some(DOI), None, title);
        }
        crossref(&mut id, DOI, "Proceedings of the Physiological Society");
        id.mark();
        assert_eq!(id.cite(SourceId::Pubmed, &[Name::Doi(DOI)]), "held");
        let res = id.resolve(false).expect("a star is no hairball");
        assert_eq!(res.works.len(), 41);
        let container = res
            .works
            .iter()
            .find(|w| {
                w.names
                    .iter()
                    .any(|n| res.arena.get(n.name).to_string() == format!("doi:{DOI}"))
            })
            .expect("the container's record holds its DOI");
        assert_eq!(container.records.len(), 1);
        let parts: Vec<_> = res
            .works
            .iter()
            .filter(|w| !w.rejected.is_empty())
            .collect();
        assert_eq!(parts.len(), 40);
        assert!(parts.iter().all(|w| w.rejected[0].why == Reason::WrongLink
            && w.rejected[0].holder == Some(container.records[0])));
        assert!(res.stub_works().is_empty());
    }

    /// Below the cap the rules decide, not the star rule: twelve
    /// parts of the same proceedings are twelve works that share its DOI,
    /// none promoting it, and a reference citing it is ambiguous. The
    /// line B draws is the cap's; merge plan §11 item 25 asks whether it
    /// should be.
    #[test]
    fn a_container_under_the_cap_is_a_shared_name() {
        const DOI: &str = "10.1113/jphysiol.1955.sp005387";
        let mut id = Identity::new();
        for (n, title) in (1u32..).zip(&PROCEEDINGS[..12]) {
            pubmed(&mut id, 13_264_100 + n, Some(DOI), None, title);
        }
        id.mark();
        assert_eq!(id.cite(SourceId::Pubmed, &[Name::Doi(DOI)]), "held");
        let res = id
            .resolve(false)
            .expect("twelve records are within the rules");
        assert_eq!(res.stats.stars, 0);
        assert_eq!(res.works.len(), 12);
        let standings: Vec<Standing> = res
            .works
            .iter()
            .flat_map(|w| &w.names)
            .filter(|n| res.arena.get(n.name).to_string() == format!("doi:{DOI}"))
            .map(|n| n.standing)
            .collect();
        assert_eq!(standings, [Standing::Shared; 12]);
        assert!(res.works.iter().all(|w| w.rejected.is_empty()));
        assert!(res.stub_works().is_empty());
    }

    /// Letters answering one viewpoint, "The two-hour marathon: Who and when?"
    /// (J Appl Physiol 2011;110:278–293), as PubMed titles them.
    const MARATHON: [&str; 38] = [
        "Don't forget the gut--it is an important athletic organ!",
        "Marathon run is an important stress to muscle and brain functions.",
        "Wealth and athletic records.",
        "Implications of the critical speed and slow component of Vo2 for the 2-hour marathon.",
        "Maturity effect of training.",
        "The 2-h marathon--running from east.",
        "The two-hour marathon--who and when? And in what environmental conditions?",
        "The 2-hour marathon--more than physiology.",
        "The 2-hour marathon: Olympian vs. Paralympian.",
        "The 2-hour marathon: will women close the gap on men?",
        "Feasiblity of the two-hour marathon is a burning issue.",
        "The two-hour marathon: how important is age?",
        "What for nature, and who to nurture?",
        "The two-hour marathon: through a highly individualized training process?",
        "Physiological determinants of best performance in marathon running.",
        "Influence of early life factors on elite performance.",
        "Two-hour marathon: Bayes takes a day off?",
        "The two-hour marathon: running economy and lower body flexibility.",
        "Is the men's marathon world record 25 years behind the women's record?",
        "Role of altitude training and the two-hour marathon.",
        "A small body mass and higher muscle recruitment strategy is the key.",
        "Two-hour marathon and running economy.",
        "Oxidative stress: friend and foe of the elite marathoner.",
        "Critical velocity and maximal lactate steady state: better determinants of 2-hour marathon.",
        "The two-hour marathon, who and when? Haïle Gebrselassie in 2000.",
        "The two-hour marathon: genetics will become increasingly important.",
        "The two-hour marathon: physiological and biochemical factors determinants running economy.",
        "What is coefficient of speed endurance to achieve two-hour marathon?",
        "Genetic information will influence but not predict the first two-hour marathon.",
        "Two-hour marathon requires Mother Nature's help.",
        "Low pulmonary vascular resistance breaks the two-hour marathon limit.",
        "The highest possible skeletal muscle metabolic stability as a key factor for breaking the 2-hour marathon barrier.",
        "The two-hour marathon: the importance of breathing economy.",
        "The two-hour marathon: how?",
        "The athlete with maintained cerebral oxygenation breaks the record.",
        "Do not underplay the roles of Vo2max and central fatigue in the 2-h marathon.",
        "Mechanisms of self-optimized pacing.",
        "Optimizing strategies for the marathon.",
    ];

    /// Two records whose titles only begin alike are not copies of one
    /// article: §3.3's prefix clause calls "The two-hour marathon: how?" a
    /// dropped subtitle of "The two-hour marathon: how important is age?",
    /// and that kept the corpus's letters on one viewpoint a hairball until
    /// the star's test asked for equal titles (2026-09-24).
    #[test]
    fn a_star_whose_titles_only_begin_alike_is_cut() {
        const DOI: &str = "10.1152/japplphysiol.01259.2010";
        let mut id = Identity::new();
        for (n, title) in (1u32..).zip(MARATHON) {
            pubmed(&mut id, 21_542_146 + n, Some(DOI), None, title);
        }
        id.mark();
        let res = id.resolve(false).expect("a star is no hairball");
        assert_eq!(res.works.len(), 38);
        assert_eq!(res.stats.stars, 1);
        assert_eq!(res.stats.star_assertions, 38);
    }

    /// The same shape with equal titles — forty copies of one article under
    /// one DOI — is a hairball still.
    #[test]
    fn a_star_of_copies_of_one_article_is_a_hairball() {
        let mut id = Identity::new();
        for n in 1..=40 {
            pubmed(
                &mut id,
                n,
                Some("10.1/same"),
                None,
                "The same title everywhere",
            );
        }
        id.mark();
        let err = id.resolve(false).unwrap_err();
        assert!(matches!(err, Error::Hairball { records: 40, .. }), "{err}");
    }

    #[test]
    fn a_hairball_stops_the_run_unless_surveyed() {
        let mut id = Identity::new();
        for n in 1..=40 {
            openalex(
                &mut id,
                n,
                Some("10.1/hair"),
                None,
                "The same title everywhere",
            );
        }
        id.mark();
        let err = id.resolve(false).unwrap_err();
        assert!(matches!(err, Error::Hairball { records: 40, .. }), "{err}");
        let mut id = Identity::new();
        for n in 1..=40 {
            openalex(
                &mut id,
                n,
                Some("10.1/hair"),
                None,
                "The same title everywhere",
            );
        }
        openalex(&mut id, 99, Some("10.1/fine"), None, "Fine");
        id.mark();
        let res = id.resolve(true).unwrap();
        assert_eq!(res.components.unresolved["hairball"], 1);
        assert_eq!(res.works.len(), 2);
        assert!(res.works.iter().any(
            |w| w.records.len() == 40 && w.names.iter().all(|n| n.standing == Standing::Shared)
        ));
    }

    #[test]
    fn a_record_without_a_native_name_is_refused_and_a_repeat_is_counted() {
        let mut id = Identity::new();
        let claim = [Assertion {
            name: Name::Doi("10.1/x"),
            field: "doi",
            native: false,
        }];
        assert!(matches!(
            id.push(SourceId::Crossref, &claim, Titles::default()),
            Err(Error::NoNativeName { .. })
        ));
        crossref(&mut id, "10.1/x", "T");
        crossref(&mut id, "10.1/x", "T again");
        assert_eq!(id.records(), 1);
        assert_eq!(id.stats.repeated[&SourceId::Crossref], 1);
    }

    #[test]
    fn works_do_not_depend_on_the_order_records_arrive() {
        type Step = Box<dyn Fn(&mut Identity)>;
        let build = |reverse: bool| {
            let mut id = Identity::new();
            let mut steps: Vec<Step> = vec![
                Box::new(|id| pubmed(id, 1, Some("10.1/a"), Some(7), "Alpha")),
                Box::new(|id| crossref(id, "10.1/a", "Alpha")),
                Box::new(|id| openalex(id, 10, Some("10.1/a"), Some(1), "Alpha")),
                Box::new(|id| openalex(id, 11, Some("10.1/a"), Some(1), "Alpha")),
                Box::new(|id| pubmed(id, 2, Some("10.1/b"), None, "Beta")),
                Box::new(|id| openalex(id, 12, Some("10.1/b"), Some(2), "Gamma, not beta")),
            ];
            if reverse {
                steps.reverse();
            }
            for step in steps {
                step(&mut id);
            }
            id.mark();
            id.resolve(false).unwrap()
        };
        let (a, b) = (build(false), build(true));
        assert_eq!(works(&a), works(&b));
        let mut ka: Vec<Vec<OwnedName>> =
            a.works.iter().map(|w| w.registry_names(&a.arena)).collect();
        let mut kb: Vec<Vec<OwnedName>> =
            b.works.iter().map(|w| w.registry_names(&b.arena)).collect();
        ka.sort();
        kb.sort();
        assert_eq!(ka, kb);
    }

    /// The merge's memory, step 2a (2026-09-24): numbering the names in
    /// another order — as a dictionary in name order will — changes
    /// nothing the pass gives. Not the works, not the order they are in,
    /// not the order a record keeps its names in (which the rows write),
    /// not the registry's order, and not a word of the report's examples.
    #[test]
    #[allow(clippy::too_many_lines)] // one scenario, run under every numbering
    fn nothing_follows_how_names_are_numbered() {
        let registrant = model::Registrant::parse("10.30476").unwrap();
        // Names numbered before any record is read: `first` in that order,
        // or, with `sorted`, a full merge's dictionary of them.
        let build = |first: &[OwnedName], sorted: bool| {
            let mut id = if sorted {
                Identity::with_dictionary(
                    Dictionary::of(first.iter().map(OwnedName::as_name)).unwrap(),
                )
            } else {
                let mut id = Identity::new();
                for name in first {
                    let ix = id.arena.intern(name.as_name()).unwrap();
                    if id.parent.len() <= ix as usize {
                        id.parent.push(ix);
                    }
                }
                id
            };
            pubmed(&mut id, 1, Some("10.1/a"), Some(7), "Alpha");
            crossref(&mut id, "10.1/a", "Alpha");
            openalex(&mut id, 10, Some("10.1/a"), Some(1), "Alpha");
            openalex(&mut id, 11, Some("10.1/a"), Some(1), "Alpha");
            pubmed(&mut id, 2, Some("10.1/b"), None, "Beta");
            openalex(&mut id, 12, Some("10.1/b"), Some(2), "Gamma, not beta");
            pubmed(&mut id, 3, Some("10.1/shared"), None, "One letter");
            pubmed(&mut id, 4, Some("10.1/shared"), None, "Another letter");
            // Two publisher ids on one record, claimed out of name order,
            // and one that two records claim, which is refused.
            for (p, piis) in [
                (5u32, &["X-2", "X-1"][..]),
                (6, &["JAMP-14-1"][..]),
                (8, &["JAMP-14-1"][..]),
            ] {
                let mut a = vec![Assertion {
                    name: Name::Pmid(pmid(p)),
                    field: "pmid",
                    native: true,
                }];
                a.extend(piis.iter().map(|v| Assertion {
                    name: Name::Publisher(registrant, v),
                    field: "ids.pii",
                    native: false,
                }));
                let title = format!("Paper {p}");
                id.push(
                    SourceId::Pubmed,
                    &a,
                    Titles {
                        title: Some(&title),
                        vernacular: None,
                    },
                )
                .unwrap();
            }
            for (n, title) in (1u32..).zip(PROCEEDINGS) {
                pubmed(&mut id, 13_264_100 + n, Some("10.1113/jp"), None, title);
            }
            for n in 100..140 {
                openalex(
                    &mut id,
                    n,
                    Some("10.1/hair"),
                    None,
                    "The same title everywhere",
                );
            }
            id.check_dictionary().unwrap();
            id.mark();
            id.cite(
                SourceId::Pubmed,
                &[Name::Doi("10.1/unheld"), Name::Doi("10.1/b2")],
            );
            id.resolve(true).unwrap()
        };
        let plain = build(&[], false);
        let mut names: Vec<OwnedName> = plain.arena.iter().map(Cow::into_owned).collect();
        names.sort();
        let reversed: Vec<OwnedName> = names.iter().rev().cloned().collect();
        // In name order and in reverse — `X-1` and `X-2` above are read in
        // neither — and through a dictionary, as a full merge numbers them.
        for (names, sorted) in [(&names, false), (&reversed, false), (&names, true)] {
            let renumbered = build(names, sorted);
            assert_eq!(renumbered.arena.dictionary().is_some(), sorted);
            assert_ne!(
                plain.arena.get(0),
                renumbered.arena.get(0),
                "numbered differently"
            );
            assert_eq!(works(&plain), works(&renumbered));
            assert_eq!(in_order(&plain), in_order(&renumbered), "the works' order");
            assert_eq!(
                kept(&plain),
                kept(&renumbered),
                "a record's names, in order"
            );
            assert_eq!(plain.held_order(), renumbered.held_order());
            // What the registry is given, by number, is the same order and
            // the same names (the merge's memory, step 2d).
            assert_eq!(
                given(&renumbered),
                plain.held_order(),
                "the registry's works"
            );
            assert_eq!(plain.stub_works(), renumbered.stub_works());
            assert_eq!(
                plain.components, renumbered.components,
                "the report's examples"
            );
            assert_eq!(plain.stats, renumbered.stats);
        }
        assert_eq!(plain.components.unresolved.get("hairball"), Some(&1));
        assert_eq!(plain.stats.stars, 1);
        assert_eq!(plain.stats.collided, 1);
    }

    /// A dictionary made from other records than the pass reads stops it:
    /// a name it lacks when a record asserts it, and a name no record
    /// asserted once the records are in.
    #[test]
    fn a_dictionary_must_hold_exactly_the_names_read() {
        let one = Dictionary::of([Name::Pmid(pmid(1)), Name::Doi("10.1/a")]).unwrap();
        let mut id = Identity::with_dictionary(one);
        let a = [Assertion {
            name: Name::Pmid(pmid(2)),
            field: "pmid",
            native: true,
        }];
        let err = id
            .push(
                SourceId::Pubmed,
                &a,
                Titles {
                    title: Some("Two"),
                    vernacular: None,
                },
            )
            .unwrap_err();
        assert!(
            matches!(err, Error::NotInDictionary { ordinal: 1, .. }),
            "{err}"
        );

        let two = Dictionary::of([Name::Pmid(pmid(1)), Name::Doi("10.1/a")]).unwrap();
        let mut id = Identity::with_dictionary(two);
        pubmed(&mut id, 1, None, None, "One");
        let err = id.check_dictionary().unwrap_err();
        assert_eq!(
            err.to_string(),
            "<dictionary>: doi:10.1/a is in the run's dictionary and no record asserts it"
        );
        pubmed(&mut id, 1, Some("10.1/a"), None, "One");
        id.check_dictionary().unwrap();
    }

    /// The held works as [`Resolution::registry_works`] gives them to the
    /// registry, their names read back.
    fn given(res: &Resolution) -> Vec<(usize, Vec<OwnedName>)> {
        let (order, works) = res.registry_works();
        order
            .iter()
            .enumerate()
            .map(|(i, &w)| {
                let names = crate::registry::Works::names(&works, i)
                    .iter()
                    .map(|&ix| crate::registry::NameIndex::name(res, ix))
                    .collect();
                (w as usize, names)
            })
            .collect()
    }

    /// Each work's names, the works in the order the pass made them.
    fn in_order(res: &Resolution) -> Vec<Vec<String>> {
        res.works
            .iter()
            .map(|w| {
                let mut names: Vec<String> = w
                    .names
                    .iter()
                    .map(|n| format!("{}={}", res.arena.get(n.name), n.standing))
                    .collect();
                names.sort();
                names
            })
            .collect()
    }

    /// Each record's names in the order it keeps them, which the rows write.
    fn kept(res: &Resolution) -> Vec<Vec<String>> {
        res.records
            .iter()
            .map(|r| {
                r.asserts
                    .iter()
                    .map(|a| format!("{} {} {}", res.arena.get(a.name), a.field, a.native))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_reference_names_one_work_held_or_stub() {
        let mut id = Identity::new();
        pubmed(&mut id, 1, Some("10.1/a"), None, "Alpha");
        id.mark();
        assert_eq!(
            id.cite(
                SourceId::Pubmed,
                &[Name::Pmid(pmid(1)), Name::Doi("10.1/other")]
            ),
            "held"
        );
        assert_eq!(
            id.cite(
                SourceId::Pubmed,
                &[Name::Pmid(pmid(5)), Name::Doi("10.1/five")]
            ),
            "stub"
        );
        assert_eq!(
            id.cite(SourceId::Crossref, &[Name::Doi("10.1/five")]),
            "stub"
        );
        assert_eq!(
            id.cite(SourceId::Crossref, &[Name::Doi("10.1/six")]),
            "stub"
        );
        assert_eq!(id.cite(SourceId::Pubmed, &[]), "none");
        assert_eq!(
            id.stats.targets[&(SourceId::Pubmed, "alias beside a held name")],
            1
        );
        let res = id.resolve(false).unwrap();
        let stubs: Vec<Vec<String>> = res
            .stub_works()
            .iter()
            .map(|names| names.iter().map(ToString::to_string).collect())
            .collect();
        assert_eq!(
            stubs,
            [vec!["doi:10.1/five", "pmid:5"], vec!["doi:10.1/six"]],
            "the other name beside a held one is no stub"
        );
    }
}
