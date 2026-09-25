//! What the readers fold past their epoch (serving plan §4, §5): the
//! registry's and the public log's tails for the names lookup, the
//! works delta indexed by id for the works index, the edges delta as a
//! map of effective labels for the graph (graph plan §5), and the batch
//! the lexical index's last commit carries. One watermark for all four:
//! a batch is folded only when every artifact can answer at it, so an
//! answer never mixes two batches.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::iter::Peekable;
use std::path::{Path, PathBuf};

use graph::{Arc, Base};
use ingest::Store;
use merge::delta::{self, CitedAsLine, EdgeLine, WorkLine};
use merge::identity::Standing;
use merge::journal::{self, Kind, Line};
use merge::{OwnedName, SourceId, Sources, WorkId, public, registry};
use search::Opened;

use crate::cited::Row as CitedRow;

use crate::error::Error;
use crate::names::Resolved;

/// The registry's and the public log's lines past the epoch, folded.
#[derive(Debug, Default)]
pub struct NamesTail {
    /// A name's text form to its id; `None` for a name dropped since.
    names: HashMap<String, Option<WorkId>>,
    /// Names accepted into more than one work by a row in the works
    /// delta, which the registry does not key.
    shared: HashMap<String, Vec<WorkId>>,
    /// Names a row in the works delta refuses, to the ids of those rows,
    /// which the base's refused names are read beside.
    refused: HashMap<String, Vec<WorkId>>,
    retired: HashMap<WorkId, Option<WorkId>>,
    /// The parts of every id a split retired since the epoch.
    split: HashMap<WorkId, Vec<WorkId>>,
    public_by_id: HashMap<WorkId, String>,
    public_by_text: HashMap<String, WorkId>,
    /// The highest id minted in the tail.
    minted: u32,
    /// Bytes of each log folded so far.
    pub registry_bytes: u64,
    #[allow(missing_docs)]
    pub public_bytes: u64,
}

impl NamesTail {
    fn apply_registry(&mut self, line: &registry::Line) {
        match line {
            registry::Line::Mint { id, names, .. } => {
                self.minted = self.minted.max(id.get());
                for name in names {
                    self.names.insert(name.to_string(), Some(*id));
                }
            }
            registry::Line::Name { id, name, .. } => {
                self.names.insert(name.to_string(), Some(*id));
            }
            registry::Line::Drop { name, .. } => {
                self.names.insert(name.to_string(), None);
            }
            registry::Line::Merge { id, into, .. } => {
                self.retired.insert(*id, Some(*into));
            }
            registry::Line::Split { id, into, .. } => {
                self.retired.insert(*id, None);
                self.split.insert(*id, into.clone());
            }
        }
    }

    fn apply_public(&mut self, line: &public::Line) {
        self.public_by_text.insert(line.public.clone(), line.id);
        self.public_by_id.insert(line.id, line.public.clone());
    }

    /// A row's names: the shared ones join the side map, the rest leave
    /// it; and the names it refuses are its alone, whatever an older row
    /// of the id refused.
    fn apply_row(
        &mut self,
        id: WorkId,
        names: &[merge::build::OwnedNamed],
        rejected: &[merge::build::OwnedRejected],
    ) {
        self.forget_refused(id);
        for r in rejected {
            let ids = self.refused.entry(r.name.to_string()).or_default();
            if !ids.contains(&id) {
                ids.push(id);
                ids.sort_unstable();
            }
        }
        for n in names {
            let key = n.name.to_string();
            if n.standing == Standing::Shared {
                let ids = self.shared.entry(key).or_default();
                if !ids.contains(&id) {
                    ids.push(id);
                    ids.sort_unstable();
                }
            } else if let Some(ids) = self.shared.get_mut(&key) {
                ids.retain(|x| *x != id);
                if ids.is_empty() {
                    self.shared.remove(&key);
                }
            }
        }
    }

    fn forget(&mut self, id: WorkId) {
        self.shared.retain(|_, ids| {
            ids.retain(|x| *x != id);
            !ids.is_empty()
        });
        self.forget_refused(id);
    }

    fn forget_refused(&mut self, id: WorkId) {
        self.refused.retain(|_, ids| {
            ids.retain(|x| *x != id);
            !ids.is_empty()
        });
    }

    /// The ids whose latest row in the works delta refuses `key`.
    #[must_use]
    pub fn refusing(&self, key: &str) -> &[WorkId] {
        self.refused.get(key).map_or(&[], Vec::as_slice)
    }

    /// What the tail says about a key, if anything: `Some(None)` for a
    /// name dropped since the epoch.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Option<Resolved>> {
        if let Some(ids) = self.shared.get(key) {
            return Some(Some(Resolved::Shared(ids.clone())));
        }
        if let Some(id) = self.names.get(key) {
            return Some(id.map(Resolved::One));
        }
        self.public_by_text
            .get(key)
            .map(|id| Some(Resolved::One(*id)))
    }

    /// What the tail says an id was retired into: `Some(None)` for a
    /// split, `None` when the tail does not know.
    #[must_use]
    pub fn retired(&self, id: WorkId) -> Option<Option<WorkId>> {
        self.retired.get(&id).copied()
    }

    /// The parts a split retired `id` into, as the tail has them; empty
    /// when the tail does not know the split.
    #[must_use]
    pub fn split_into(&self, id: WorkId) -> &[WorkId] {
        self.split.get(&id).map_or(&[], Vec::as_slice)
    }

    /// A public id minted since the epoch.
    #[must_use]
    pub fn public_of(&self, id: WorkId) -> Option<&str> {
        self.public_by_id.get(&id).map(String::as_str)
    }

    /// The highest id minted since the epoch, zero for none.
    #[must_use]
    pub const fn minted(&self) -> u32 {
        self.minted
    }

    /// Whether the tail knows the name at all, dropped or not.
    #[must_use]
    pub fn knows(&self, name: &OwnedName) -> bool {
        self.names.contains_key(&name.to_string())
    }
}

/// What the works delta says about an id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The latest row, at this byte offset of the log.
    Put(u64),
    /// Retired: merged into the id, or split.
    Retired(Option<WorkId>),
    /// No records and nothing citing it: no row.
    Dropped,
}

/// The works delta, indexed: the latest line per id, by byte offset, the
/// file held open so that the rows are read on demand and survive the
/// directory being replaced by a compaction.
#[derive(Debug, Default)]
pub struct WorksTail {
    file: Option<File>,
    slots: HashMap<WorkId, Slot>,
    stubs: HashMap<WorkId, bool>,
    #[allow(missing_docs)]
    pub bytes: u64,
}

impl WorksTail {
    /// The tail's word on an id.
    #[must_use]
    pub fn slot(&self, id: WorkId) -> Option<Slot> {
        self.slots.get(&id).copied()
    }

    /// Whether the tail's latest row for an id is a stub's; `None` when
    /// the tail has no row for it.
    #[must_use]
    pub fn is_stub(&self, id: WorkId) -> Option<bool> {
        self.stubs.get(&id).copied()
    }

    /// The line at an offset the tail indexed.
    ///
    /// # Errors
    ///
    /// I/O, or a line that does not parse.
    pub fn line(&self, path: &Path, offset: u64) -> Result<WorkLine, Error> {
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| Error::Mismatch(format!("{}: not open", path.display())))?;
        delta::line_at(file, offset).map_err(|e| Error::io(path, e))
    }

    /// Every id the tail has a row for, in id order.
    #[must_use]
    pub fn put_ids(&self) -> Vec<WorkId> {
        let mut ids: Vec<WorkId> = self
            .slots
            .iter()
            .filter(|(_, s)| matches!(s, Slot::Put(_)))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Ids the tail put, retired or dropped.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Nothing folded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// The edges delta folded (graph plan §5): per work touched, the
/// effective label of every arc the delta touched — an empty label is an
/// arc gone — in both orientations, and the retirements.
#[derive(Debug, Default)]
pub struct EdgesFold {
    forward: HashMap<WorkId, BTreeMap<WorkId, Sources>>,
    transpose: HashMap<WorkId, BTreeMap<WorkId, Sources>>,
    retired: HashMap<WorkId, Option<WorkId>>,
    #[allow(missing_docs)]
    pub bytes: u64,
    /// Lines folded.
    pub lines: u64,
}

/// The cited-as delta, indexed by name: the rows each batch put, and the
/// (citing work, source) pairs whose rows in the base they replace
/// (2026-09-18).
#[derive(Debug, Default)]
pub struct CitedTail {
    by_name: HashMap<String, Vec<CitedRow>>,
    restated: HashSet<(WorkId, SourceId)>,
    moved: HashMap<WorkId, WorkId>,
    #[allow(missing_docs)]
    pub bytes: u64,
    /// Lines folded.
    pub lines: u64,
}

impl CitedTail {
    /// Whether a batch has restated what this work's records of that
    /// source gave, so the base's rows for the pair are stale.
    #[must_use]
    pub fn restated(&self, from: WorkId, by: SourceId) -> bool {
        self.restated.contains(&(from, by))
    }

    /// Where a retired citing work's rows went.
    #[must_use]
    pub fn survivor(&self, from: WorkId) -> WorkId {
        let mut id = from;
        while let Some(into) = self.moved.get(&id) {
            if *into == id {
                break;
            }
            id = *into;
        }
        id
    }

    /// Every name the tail has rows for, in no particular order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(String::as_str)
    }

    /// The tail's rows for a name.
    #[must_use]
    pub fn rows_of(&self, name: &str) -> Vec<CitedRow> {
        self.by_name.get(name).cloned().unwrap_or_default()
    }

    fn apply(&mut self, line: &CitedAsLine) {
        self.lines += 1;
        match line {
            CitedAsLine::Put { from, by, rows, .. } => {
                self.restated.insert((*from, *by));
                for list in self.by_name.values_mut() {
                    list.retain(|row| (row.from, row.by) != (*from, *by));
                }
                for row in rows {
                    self.by_name
                        .entry(row.name.to_string())
                        .or_default()
                        .push(CitedRow::of(&row.borrow()));
                }
            }
            CitedAsLine::Move { from, into, .. } => {
                self.moved.insert(*from, *into);
                for list in self.by_name.values_mut() {
                    for row in list.iter_mut() {
                        if row.from == *from {
                            row.from = *into;
                        }
                        if row.to == *from {
                            row.to = *into;
                        }
                    }
                }
            }
        }
    }
}

/// The base's label on an arc, if the base has it: a binary search over
/// the citing work's list, whose neighbours are sorted by id.
fn base_label(base: &Base, from: WorkId, to: WorkId) -> Option<Sources> {
    let pos = base.position(from)?;
    let lists = base.forward();
    let (mut lo, mut hi) = (
        lists.offset(u64::from(pos)),
        lists.offset(u64::from(pos) + 1),
    );
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (neighbour, label) = lists.arc_at(mid);
        match neighbour.cmp(&to.get()) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return base.asserters().get(usize::from(label)).copied(),
        }
    }
    None
}

impl EdgesFold {
    fn apply(&mut self, base: &Base, line: &EdgeLine) {
        match *line {
            EdgeLine::Add { from, to, by, .. } => self.relabel(base, from, to, |l| l.union(by)),
            EdgeLine::Remove { from, to, by, .. } => {
                self.relabel(base, from, to, |l| l.difference(by));
            }
            EdgeLine::Retire { id, into, .. } => {
                self.retired.insert(id, into);
            }
        }
        self.lines += 1;
    }

    fn relabel(&mut self, base: &Base, from: WorkId, to: WorkId, f: impl Fn(Sources) -> Sources) {
        let current = self
            .forward
            .get(&from)
            .and_then(|m| m.get(&to).copied())
            .or_else(|| base_label(base, from, to))
            .unwrap_or(Sources::NONE);
        let label = f(current);
        self.forward.entry(from).or_default().insert(to, label);
        self.transpose.entry(to).or_default().insert(from, label);
    }

    /// The delta's arcs out of a work, by target, with their effective
    /// labels; an empty label is an arc the delta removed.
    #[must_use]
    pub fn forward(&self, id: WorkId) -> Vec<(WorkId, Sources)> {
        self.forward
            .get(&id)
            .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
            .unwrap_or_default()
    }

    /// The delta's arcs into a work, by source work.
    #[must_use]
    pub fn transpose(&self, id: WorkId) -> Vec<(WorkId, Sources)> {
        self.transpose
            .get(&id)
            .map(|m| m.iter().map(|(k, v)| (*k, *v)).collect())
            .unwrap_or_default()
    }

    /// What the delta retired an id into.
    #[must_use]
    pub fn retired(&self, id: WorkId) -> Option<Option<WorkId>> {
        self.retired.get(&id).copied()
    }

    /// Works with a touched arc out of them.
    #[must_use]
    pub fn touched(&self) -> usize {
        self.forward.len()
    }
}

/// A list with the delta folded over the base: the base's arcs in their
/// order, each label replaced where the delta has the arc, the delta's
/// own arcs merged in by id, empty labels skipped.
#[derive(Debug)]
pub struct Arcs<'a> {
    base: Peekable<graph::Arcs<'a>>,
    delta: Peekable<std::vec::IntoIter<(WorkId, Sources)>>,
}

impl<'a> Arcs<'a> {
    #[allow(missing_docs)]
    #[must_use]
    pub fn new(base: graph::Arcs<'a>, delta: Vec<(WorkId, Sources)>) -> Self {
        Self {
            base: base.peekable(),
            delta: delta.into_iter().peekable(),
        }
    }
}

impl Iterator for Arcs<'_> {
    type Item = Arc;

    fn next(&mut self) -> Option<Arc> {
        loop {
            let next = match (self.base.peek(), self.delta.peek()) {
                (Some(b), Some(d)) if b.work < d.0 => self.base.next(),
                (Some(b), Some(d)) if b.work == d.0 => {
                    self.base.next();
                    self.delta.next().map(|(work, by)| Arc { work, by })
                }
                (_, Some(_)) => self.delta.next().map(|(work, by)| Arc { work, by }),
                (Some(_), None) => self.base.next(),
                (None, None) => return None,
            };
            match next {
                Some(arc) if arc.by.is_empty() => {}
                other => return other,
            }
        }
    }
}

/// Why the fold stopped short of the journal's last batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stopped {
    /// A full merge since the epoch: its epoch has to be published.
    Compaction(u64),
    /// The lexical index has not absorbed the batch yet.
    Lexical {
        #[allow(missing_docs)]
        batch: u64,
        #[allow(missing_docs)]
        lexical: u64,
    },
}

impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compaction(batch) => {
                write!(f, "batch {batch} is a full merge; waiting for its epoch")
            }
            Self::Lexical { batch, lexical } => write!(
                f,
                "the lexical index is at batch {lexical}, batch {batch} not absorbed yet"
            ),
        }
    }
}

/// What one advance did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advanced {
    /// Batches folded by this call.
    pub folded: u64,
    /// Where it stopped, if short of the journal.
    pub stopped: Option<Stopped>,
}

/// Everything folded past the epoch, and the watermark it stands at.
#[derive(Debug)]
pub struct Fold {
    /// The batch every artifact has folded to.
    pub watermark: u64,
    /// That batch's timestamp.
    pub at: String,
    #[allow(missing_docs)]
    pub names: NamesTail,
    #[allow(missing_docs)]
    pub works: WorksTail,
    #[allow(missing_docs)]
    pub edges: EdgesFold,
    #[allow(missing_docs)]
    pub cited: CitedTail,
    /// The batch the lexical index's last commit carried when last read.
    pub lexical: u64,
    /// Bytes of each delta log folded so far, by the journal's keys.
    pub offsets: BTreeMap<String, u64>,
    registry: PathBuf,
    public: PathBuf,
    delta: PathBuf,
}

impl Fold {
    /// The fold at an epoch's batch, nothing past it yet: the registry
    /// and public offsets the epoch's journal line records, or, for a
    /// line written before they were recorded, the offsets its line
    /// counts reach.
    ///
    /// # Errors
    ///
    /// I/O, or a journal line that is not a full merge's.
    pub fn new(store: &Store, merged: &Path, line: &Line) -> Result<Self, Error> {
        if line.kind != Kind::Full {
            return Err(Error::Mismatch(format!(
                "batch {} is not a full merge; an epoch is published at one",
                line.batch
            )));
        }
        let registry = store.registry();
        let public = store.public();
        let offset = |key: &str, path: &Path, lines: u64| -> Result<u64, Error> {
            match line.bytes.get(key) {
                Some(bytes) => Ok(*bytes),
                None => journal::offset_of(path, lines).map_err(|e| Error::io(path, e)),
            }
        };
        let names = NamesTail {
            registry_bytes: offset("registry", &registry, line.registry)?,
            public_bytes: offset("public", &public, line.public)?,
            ..NamesTail::default()
        };
        let delta_dir = delta::dir(merged);
        let mut offsets = BTreeMap::new();
        for file in delta::FILES {
            let key = delta::key(file).to_owned();
            offsets.insert(key.clone(), line.delta.get(&key).copied().unwrap_or(0));
        }
        let works_path = delta_dir.join(delta::WORKS);
        let file = match File::open(&works_path) {
            Ok(file) => Some(file),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(Error::io(&works_path, e)),
        };
        Ok(Self {
            watermark: line.batch,
            at: line.at.clone(),
            names,
            works: WorksTail {
                file,
                slots: HashMap::new(),
                stubs: HashMap::new(),
                bytes: offsets["works"],
            },
            edges: EdgesFold {
                bytes: offsets["edges"],
                ..EdgesFold::default()
            },
            cited: CitedTail {
                bytes: offsets["cited_as"],
                ..CitedTail::default()
            },
            lexical: line.batch,
            offsets,
            registry,
            public,
            delta: delta_dir,
        })
    }

    /// The works log's path.
    #[must_use]
    pub fn works_path(&self) -> PathBuf {
        self.delta.join(delta::WORKS)
    }

    /// Folds every batch past the watermark that every artifact can
    /// answer at, in order, and says where it stopped.
    ///
    /// # Errors
    ///
    /// I/O, a journal with a gap, a delta line stamped with the wrong
    /// batch, or an incremental line without the offsets a reader needs.
    pub fn advance(
        &mut self,
        base: &Base,
        search: &Opened,
        lines: &[Line],
    ) -> Result<Advanced, Error> {
        let mut folded = 0;
        let watermark = self.watermark;
        let pending = lines.iter().filter(|l| l.batch > watermark);
        let mut lexical = None;
        for line in pending {
            if line.kind == Kind::Full {
                return Ok(Advanced {
                    folded,
                    stopped: Some(Stopped::Compaction(line.batch)),
                });
            }
            if line.batch != self.watermark + 1 {
                return Err(Error::Mismatch(format!(
                    "the journal jumps from batch {} to {}",
                    self.watermark, line.batch
                )));
            }
            let at_lexical = if let Some(b) = lexical {
                b
            } else {
                search.reload()?;
                let b = search.batch()?;
                lexical = Some(b);
                self.lexical = b;
                b
            };
            if at_lexical < line.batch {
                return Ok(Advanced {
                    folded,
                    stopped: Some(Stopped::Lexical {
                        batch: line.batch,
                        lexical: at_lexical,
                    }),
                });
            }
            self.fold(base, line)?;
            folded += 1;
        }
        Ok(Advanced {
            folded,
            stopped: None,
        })
    }

    fn fold(&mut self, base: &Base, line: &Line) -> Result<(), Error> {
        let need = |key: &str| -> Result<u64, Error> {
            line.bytes.get(key).copied().ok_or_else(|| {
                Error::Mismatch(format!(
                    "batch {} records no {key} offset; a reader cannot tail it",
                    line.batch
                ))
            })
        };
        let registry_to = need("registry")?;
        let public_to = need("public")?;
        let registry: Vec<registry::Line> =
            delta::read_plain(&self.registry, self.names.registry_bytes, registry_to)
                .map_err(|e| Error::io(&self.registry, e))?;
        for l in &registry {
            self.names.apply_registry(l);
        }
        self.names.registry_bytes = registry_to;
        let public: Vec<public::Line> =
            delta::read_plain(&self.public, self.names.public_bytes, public_to)
                .map_err(|e| Error::io(&self.public, e))?;
        for l in &public {
            self.names.apply_public(l);
        }
        self.names.public_bytes = public_to;
        let range = |key: &str, so_far: u64| -> Result<u64, Error> {
            line.delta.get(key).copied().ok_or_else(|| {
                Error::Mismatch(format!(
                    "batch {} records no {key} delta offset",
                    line.batch
                ))
            })
            .and_then(|to| {
                if to < so_far {
                    Err(Error::Mismatch(format!(
                        "batch {} puts the {key} delta at {to} bytes, behind the {so_far} folded",
                        line.batch
                    )))
                } else {
                    Ok(to)
                }
            })
        };
        let works_to = range("works", self.works.bytes)?;
        let works_path = self.works_path();
        if self.works.file.is_none() && works_to > 0 {
            self.works.file = Some(File::open(&works_path).map_err(|e| Error::io(&works_path, e))?);
        }
        let works: Vec<(u64, WorkLine)> =
            delta::read_indexed(&works_path, self.works.bytes, works_to, Some(line.batch))
                .map_err(|e| Error::io(&works_path, e))?;
        for (offset, l) in &works {
            match l {
                WorkLine::Put { work, .. } => {
                    self.works.slots.insert(work.id, Slot::Put(*offset));
                    self.works.stubs.insert(work.id, work.stub);
                    self.names.apply_row(work.id, &work.names, &work.rejected);
                }
                WorkLine::Retire { id, into, .. } => {
                    self.works.slots.insert(*id, Slot::Retired(*into));
                    self.works.stubs.remove(id);
                    self.names.forget(*id);
                }
                WorkLine::Drop { id, .. } => {
                    self.works.slots.insert(*id, Slot::Dropped);
                    self.works.stubs.remove(id);
                    self.names.forget(*id);
                }
            }
        }
        self.works.bytes = works_to;
        let edges_to = range("edges", self.edges.bytes)?;
        let edges_path = self.delta.join(delta::EDGES);
        let edges: Vec<EdgeLine> =
            delta::read(&edges_path, self.edges.bytes, edges_to, Some(line.batch))
                .map_err(|e| Error::io(&edges_path, e))?;
        for l in &edges {
            self.edges.apply(base, l);
        }
        self.edges.bytes = edges_to;
        let cited_to = range("cited_as", self.cited.bytes)?;
        let cited_path = self.delta.join(delta::CITED_AS);
        let cited: Vec<CitedAsLine> =
            delta::read(&cited_path, self.cited.bytes, cited_to, Some(line.batch))
                .map_err(|e| Error::io(&cited_path, e))?;
        for l in &cited {
            self.cited.apply(l);
        }
        self.cited.bytes = cited_to;
        self.offsets.insert("cited_as".to_owned(), cited_to);
        for key in ["relations", "unresolved"] {
            let to = range(key, self.offsets[key])?;
            self.offsets.insert(key.to_owned(), to);
        }
        self.offsets.insert("works".to_owned(), works_to);
        self.offsets.insert("edges".to_owned(), edges_to);
        self.watermark = line.batch;
        self.at.clone_from(&line.at);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use graph::Arc;
    use merge::{SourceId, Sources, WorkId};

    use super::{Arcs, NamesTail};
    use crate::names::Resolved;

    fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    #[test]
    fn the_registry_tail_folds_like_the_registry() {
        let mut tail = NamesTail::default();
        let n = |t: &str| merge::OwnedName::parse(t).unwrap();
        tail.apply_registry(&merge::registry::Line::Mint {
            id: id(10),
            names: vec![n("pmid:1"), n("doi:10.1000/a")],
            at: "t".into(),
        });
        tail.apply_registry(&merge::registry::Line::Name {
            id: id(10),
            name: n("pmc:PMC1"),
            at: "t".into(),
        });
        tail.apply_registry(&merge::registry::Line::Drop {
            name: n("doi:10.1000/a"),
            at: "t".into(),
        });
        tail.apply_registry(&merge::registry::Line::Merge {
            id: id(10),
            into: id(3),
            at: "t".into(),
        });
        tail.apply_public(&merge::public::Line {
            id: id(10),
            public: "ol:0000000000000A0".into(),
            at: "t".into(),
        });
        assert_eq!(tail.get("pmid:1"), Some(Some(Resolved::One(id(10)))));
        assert_eq!(tail.get("pmc:PMC1"), Some(Some(Resolved::One(id(10)))));
        assert_eq!(tail.get("doi:10.1000/a"), Some(None), "dropped");
        assert_eq!(tail.get("pmid:2"), None, "unknown to the tail");
        assert_eq!(tail.retired(id(10)), Some(Some(id(3))));
        assert_eq!(tail.public_of(id(10)), Some("ol:0000000000000A0"));
        assert_eq!(
            tail.get("ol:0000000000000A0"),
            Some(Some(Resolved::One(id(10))))
        );
        assert_eq!(tail.minted(), 10);
        assert!(tail.knows(&n("doi:10.1000/a")));
    }

    #[test]
    fn folded_arcs_merge_by_id_and_skip_removed_ones() {
        let pubmed: Sources = [SourceId::Pubmed].into_iter().collect();
        let both: Sources = [SourceId::Pubmed, SourceId::Crossref].into_iter().collect();
        // The base lists arcs from 1 to 2 and 4; the delta relabels 4 and
        // adds 3 and 7.
        let mut nodes = graph::Nodes::new();
        for n in 1..=7 {
            nodes.add(id(n), n > 2, Sources::NONE).unwrap();
        }
        let edges = vec![
            merge::record::Edge {
                from: id(1),
                to: id(2),
                by: SourceId::ALL.into_iter().collect(),
            },
            merge::record::Edge {
                from: id(1),
                to: id(4),
                by: [SourceId::Crossref].into_iter().collect(),
            },
        ];
        let root = std::env::temp_dir().join(format!("ol-rht-serve-fold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join(graph::layout::name(false));
        let input = graph::layout::Input {
            merged: "v1.2".into(),
            shards: 1,
        };
        graph::build::build(&dir, &nodes, input, graph::build::Options::default(), edges).unwrap();
        let base = graph::Base::open(&dir).unwrap();
        let delta = vec![(id(3), pubmed), (id(4), both), (id(7), pubmed)];
        let folded: Vec<Arc> = Arcs::new(base.references(id(1)), delta).collect();
        assert_eq!(
            folded
                .iter()
                .map(|a| (a.work.get(), a.by.to_string()))
                .collect::<Vec<_>>(),
            vec![
                (2, "pubmed+crossref+openalex".to_owned()),
                (3, "pubmed".to_owned()),
                (4, "pubmed+crossref".to_owned()),
                (7, "pubmed".to_owned()),
            ]
        );
        let removed: Vec<Arc> =
            Arcs::new(base.references(id(1)), vec![(id(2), Sources::NONE)]).collect();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].work, id(4));
        assert_eq!(
            super::base_label(&base, id(1), id(4)),
            Some([SourceId::Crossref].into_iter().collect())
        );
        assert_eq!(super::base_label(&base, id(1), id(5)), None);
        assert_eq!(
            super::base_label(&base, id(9), id(1)),
            None,
            "past the base"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
