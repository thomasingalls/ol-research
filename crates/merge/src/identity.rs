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
use crate::name::{Assertion, Name, OwnedName};
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

/// One assertion, interned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assert {
    #[allow(missing_docs)]
    pub name: NameIx,
    #[allow(missing_docs)]
    pub field: &'static str,
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
    /// Every assertion, the native one included, each name once.
    pub asserts: SmallVec<[Assert; 3]>,
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
}

/// The identity pass, accumulating over every set of every source.
#[derive(Debug, Default)]
pub struct Identity {
    arena: Arena,
    parent: Vec<NameIx>,
    roots: Vec<NameIx>,
    records: Vec<Record>,
    natives: HashMap<NameIx, RecordIx>,
    set_of: Vec<u32>,
    sets: Vec<SetInfo>,
    conflict_roots: HashSet<NameIx>,
    conflict: Vec<bool>,
    conflict_sets: HashSet<u32>,
    titles: HashMap<RecordIx, OwnedTitles>,
    stubs: Arena,
    stub_parent: Vec<NameIx>,
    pending: Vec<(SourceId, OwnedName)>,
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
        let mut asserts: SmallVec<[Assert; 3]> = SmallVec::new();
        let mut native = None;
        for a in assertions {
            let ix = self.arena.intern(a.name);
            if self.parent.len() <= ix as usize {
                self.parent.push(ix);
            }
            if a.native && native.is_none() {
                native = Some(ix);
            }
            if !asserts.iter().any(|x| x.name == ix) {
                asserts.push(Assert {
                    name: ix,
                    field: a.field,
                    native: a.native,
                });
            }
        }
        let Some(native) = native else {
            return Err(Error::NoNativeName {
                path: PathBuf::from("<memory>"),
                ordinal: before as u64 + 1,
            });
        };
        if let Some(&rix) = self.natives.get(&native) {
            *self.stats.repeated.entry(source).or_default() += 1;
            return Ok(rix);
        }
        for a in &asserts {
            self.union(native, a.name);
            *self.stats.asserted.entry((source, a.field)).or_default() += 1;
        }
        let rix = before as RecordIx;
        self.natives.insert(native, rix);
        self.records.push(Record {
            source,
            native,
            asserts,
        });
        self.set_of.push(set);
        self.titles.insert(rix, OwnedTitles::of(titles));
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
        let mut asserts: SmallVec<[Assert; 3]> = SmallVec::new();
        let mut native = None;
        let (arena, parent) = (&mut self.arena, &mut self.parent);
        for a in &assertions {
            let ix = arena.intern(a.name);
            if parent.len() <= ix as usize {
                parent.push(ix);
            }
            if a.native && native.is_none() {
                native = Some(ix);
            }
            if !asserts.iter().any(|x| x.name == ix) {
                asserts.push(Assert {
                    name: ix,
                    field: a.field,
                    native: a.native,
                });
            }
        }
        let Some(native) = native else {
            return Kept::NoNative;
        };
        if let Some(&rix) = self.natives.get(&native) {
            *self.stats.repeated.entry(S::SOURCE).or_default() += 1;
            let known = &self.records[rix as usize].asserts;
            let new: SmallVec<[Assert; 3]> = asserts
                .iter()
                .filter(|a| !known.iter().any(|x| x.name == a.name))
                .copied()
                .collect();
            for a in new {
                self.records[rix as usize].asserts.push(a);
                self.union(native, a.name);
                *self.stats.asserted.entry((S::SOURCE, a.field)).or_default() += 1;
                *self.stats.joined.entry(S::SOURCE).or_default() += 1;
            }
            // The title pass reads the set the speaking copy is in.
            if speaks {
                self.set_of[rix as usize] = set_ix;
                return Kept::Record;
            }
            return Kept::Joined;
        }
        for a in &asserts {
            self.union(native, a.name);
            *self.stats.asserted.entry((S::SOURCE, a.field)).or_default() += 1;
        }
        let rix = self.records.len() as RecordIx;
        self.natives.insert(native, rix);
        self.records.push(Record {
            source: S::SOURCE,
            native,
            asserts,
        });
        self.set_of.push(set_ix);
        *self.stats.records.entry(S::SOURCE).or_default() += 1;
        if speaks { Kept::Record } else { Kept::Joined }
    }

    /// Finds the conflict components (merge plan §3.2): more than one name
    /// in a namespace, or more than one record of a source. Call after
    /// the last [`Identity::add`].
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    pub fn mark(&mut self) {
        let n = self.arena.len();
        self.roots = (0..n as NameIx).map(|i| self.find(i)).collect();
        let mut tallies: HashMap<NameIx, Tally> = HashMap::new();
        for (i, name) in self.arena.iter().enumerate() {
            let t = tallies.entry(self.roots[i]).or_default();
            let k = &mut t.keys[key_ix(name.key())];
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
                let Some(rix) = native.and_then(|n| self.natives.get(&n).copied()) else {
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
        let mut held: SmallVec<[NameIx; 2]> = SmallVec::new();
        let mut unheld: SmallVec<[Name<'_>; 2]> = SmallVec::new();
        for &name in names {
            match self.arena.find(name) {
                Some(ix) => held.push(ix),
                None => unheld.push(name),
            }
        }
        let outcome = if held.is_empty() {
            if unheld.is_empty() {
                "none"
            } else {
                let mut first = None;
                for name in unheld {
                    let ix = self.stubs.intern(name);
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
        let mut names_by_root: HashMap<NameIx, Vec<NameIx>> = HashMap::new();
        for (i, &root) in self.roots.iter().enumerate() {
            names_by_root.entry(root).or_default().push(i as NameIx);
        }
        let mut recs_by_root: HashMap<NameIx, Vec<RecordIx>> = HashMap::new();
        for (rix, rec) in self.records.iter().enumerate() {
            recs_by_root
                .entry(self.roots[rec.native as usize])
                .or_default()
                .push(rix as RecordIx);
        }
        let mut roots: Vec<NameIx> = names_by_root.keys().copied().collect();
        roots.sort_unstable();
        let mut works = Vec::with_capacity(roots.len());
        let mut components = Components::default();
        for root in roots {
            let names = &names_by_root[&root];
            let recs = recs_by_root.get(&root).map_or(&[][..], Vec::as_slice);
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
        for work in &works {
            components.accepted += work.names.len() as u64;
            components.rejected += work.rejected.len() as u64;
            components.uncorroborated += u64::from(work.uncorroborated);
            for n in &work.names {
                *components
                    .standings
                    .entry((self.arena.get(n.name).key(), n.standing))
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
            works,
            stats: self.stats,
            components,
        })
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
        // Who asserts each name, as positions into `recs`.
        let mut asserters: BTreeMap<NameIx, SmallVec<[(usize, bool); 4]>> = BTreeMap::new();
        for (pos, &rix) in recs.iter().enumerate() {
            for a in &self.records[rix as usize].asserts {
                asserters.entry(a.name).or_default().push((pos, a.native));
            }
        }
        // Records sharing a name are one work when their titles agree.
        let mut uf: Vec<usize> = (0..recs.len()).collect();
        let default = OwnedTitles::default();
        let title_of = |pos: usize| self.titles.get(&recs[pos]).unwrap_or(&default);
        for list in asserters.values() {
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
        for (&name, list) in &asserters {
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
                            works[g].rejected.push(Rejection {
                                name,
                                record: recs[pos],
                                holder: holder_rec,
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
            let dois: Vec<&str> = work
                .names
                .iter()
                .filter(|n| n.standing != Standing::Shared)
                .filter_map(|n| match self.arena.get(n.name) {
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
                .filter(|n| matches!(self.arena.get(n.name), OwnedName::Pmid(_)))
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
                .filter(|(_, n)| {
                    n.standing != Standing::Shared && self.arena.get(n.name).key() == key
                })
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
                    Key::Pmc | Key::OpenAlex => 0,
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
            for n in &w.names {
                let _ = write!(out, " {}({})", self.arena.get(n.name), n.standing);
            }
            for r in &w.rejected {
                let _ = write!(
                    out,
                    " {}(rejected: {} holds it)",
                    self.arena.get(r.name),
                    self.arena.get(self.records[r.holder as usize].native)
                );
            }
            out.push('}');
        }
        out
    }
}

/// What became of one record read from a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kept {
    Record,
    /// A copy a newer one speaks over, whose names joined the record's
    /// (merge plan §8): its relations are not the record's — the
    /// speaking copy alone gives those.
    Joined,
    /// A copy a deletion withdrew.
    Silent,
    NoNative,
}

/// Names per namespace and records per source in one component.
#[derive(Default)]
struct Tally {
    keys: [u8; 4],
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

/// A rejected assertion: `record` claimed `name`, which `holder` holds
/// natively, and their titles disagree (a wrong link, merge plan §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejection {
    #[allow(missing_docs)]
    pub name: NameIx,
    #[allow(missing_docs)]
    pub record: RecordIx,
    #[allow(missing_docs)]
    pub holder: RecordIx,
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
    pub fn promoted<'a>(&self, key: Key, arena: &'a Arena) -> Option<&'a OwnedName> {
        self.names
            .iter()
            .filter(|n| n.standing == Standing::Promoted)
            .map(|n| arena.get(n.name))
            .find(|n| n.key() == key)
    }

    /// The names the registry keys the work by: every accepted name that
    /// is not shared, promoted ones first in namespace order, then the
    /// aliases sorted — a deterministic key for a deterministic order.
    #[must_use]
    pub fn registry_names(&self, arena: &Arena) -> Vec<OwnedName> {
        let mut out: Vec<OwnedName> = Key::ALL
            .iter()
            .filter_map(|&k| self.promoted(k, arena).cloned())
            .collect();
        let mut aliases: Vec<OwnedName> = self
            .names
            .iter()
            .filter(|n| n.standing == Standing::Alias)
            .map(|n| arena.get(n.name).clone())
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
    #[allow(missing_docs)]
    pub works: Vec<Work>,
    #[allow(missing_docs)]
    pub stats: Stats,
    #[allow(missing_docs)]
    pub components: Components,
}

impl Resolution {
    /// The held works in the registry's order (merge plan §4): each
    /// work's index into [`Resolution::works`] with its registry names,
    /// sorted by the names, then by index for the works that have none.
    #[must_use]
    pub fn held_order(&self) -> Vec<(usize, Vec<OwnedName>)> {
        let mut held: Vec<(usize, Vec<OwnedName>)> = self
            .works
            .iter()
            .enumerate()
            .map(|(i, w)| (i, w.registry_names(&self.arena)))
            .collect();
        held.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
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
                .push(self.stubs.get(ix).clone());
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

    use super::{Error, Identity, Resolution, Standing};
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
        let holder = &res.records[rejected.rejected[0].holder as usize];
        assert_eq!(holder.source, SourceId::Pubmed);
        assert_eq!(res.components.total, 1, "one component, two works");
        assert_eq!(res.works.len(), 2);
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
