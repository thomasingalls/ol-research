//! The incremental merge (serving plan §5): a batch of normalized records
//! merged against the live tables — the names lookup with the registry's
//! tail, the works rows, the graph with its fold — the merge plan's rules
//! re-run on exactly the works the batch touches, and what changed
//! appended: registry lines and public ids for mints, the four delta
//! logs, the journal line, and the batch absorbed into the lexical index.
//!
//! The rules are the full merge's own, on a component the batch cuts out
//! of the live state: every record of every work a batch record's names
//! touch, closed over shared names, with the batch's copies replacing
//! the rows' and the batch's deletions removing them. Each record's
//! evidence — its assertions, its view — is in its row (merge plan §5),
//! which is what makes the component resolvable without the normalized
//! layer. Where a case needs what only the normalized layer has, the
//! batch stops and says so, and a full merge is the answer: a split, a
//! work losing its last record while something cites it, a deleted or
//! moved duplicate, or a text that a record not in the batch and not
//! promoted before would have to supply.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use ingest::Store;
use merge::build::{self, OwnedAssert, OwnedCitation, OwnedNamed, OwnedUnresolved, OwnedWork};
use merge::build::{Referenced, Target};
use merge::delta::{self, Appender, EdgeLine, Related, RelationLine, UnresolvedLine, WorkLine};
use merge::identity::{Identity, Standing};
use merge::journal::{self, Kind, Line};
use merge::name::{Assertion, field_static};
use merge::public::{self, Key, Public};
use merge::record::Why;
use merge::registry::{self, Table};
use merge::title::Titles;
use merge::view::{Owned, Text};
use merge::{Document, OwnedName, Registry, SourceId, Sources, WorkId};

use crate::core::{Resolved, Serving};
use crate::row::Row;

/// One normalized record as the batch carries it, source-agnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    #[allow(missing_docs)]
    pub source: SourceId,
    #[allow(missing_docs)]
    pub native: OwnedName,
    /// Every assertion, the native one included.
    pub asserts: Vec<OwnedAssert>,
    #[allow(missing_docs)]
    pub view: Owned,
    #[allow(missing_docs)]
    pub text: Text,
    #[allow(missing_docs)]
    pub relations: Vec<merge::relation::Related>,
    #[allow(missing_docs)]
    pub references: Vec<Reference>,
}

/// One reference of a batch record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    #[allow(missing_docs)]
    pub names: Vec<OwnedName>,
    #[allow(missing_docs)]
    pub citation: OwnedCitation,
}

/// One batch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    #[allow(missing_docs)]
    pub records: Vec<Incoming>,
    /// Records the sources withdrew.
    pub deletions: Vec<(SourceId, OwnedName)>,
    /// The normalized sets the batch came from, relative to the store,
    /// for the journal.
    pub inputs: Vec<String>,
}

/// Why a batch stopped.
#[derive(Debug)]
pub enum Error {
    /// A case the incremental rules do not cover; a full merge does.
    FullMerge(String),
    /// The serving layer.
    Serve(crate::Error),
    /// The identity rules on a component.
    Identity(merge::identity::Error),
    #[allow(missing_docs)]
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: io::Error,
    },
    /// The lexical index.
    Search(search::Error),
    /// The store is not in a state a batch can be applied to.
    Mismatch(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FullMerge(what) => write!(f, "{what}; run a full merge"),
            Self::Serve(e) => write!(f, "{e}"),
            Self::Identity(e) => write!(f, "identity: {e}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Search(e) => write!(f, "lexical index: {e}"),
            Self::Mismatch(what) => f.write_str(what),
        }
    }
}

impl std::error::Error for Error {}

impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Self::Serve(e)
    }
}

impl From<merge::identity::Error> for Error {
    fn from(e: merge::identity::Error) -> Self {
        Self::Identity(e)
    }
}

impl From<search::Error> for Error {
    fn from(e: search::Error) -> Self {
        Self::Search(e)
    }
}

fn io(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// What a batch did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// The batch number.
    pub batch: u64,
    #[allow(missing_docs)]
    pub at: String,
    /// Records merged, the repeats left out.
    pub records: u64,
    /// Records skipped as repeats of an earlier one in the batch.
    pub repeated: u64,
    /// Deletions that found their record.
    pub deleted: u64,
    /// Deletions of records no work had.
    pub deletions_unknown: u64,
    #[allow(missing_docs)]
    pub components: u64,
    /// Held rows written.
    pub works_put: u64,
    /// Stub rows written: minted, or existing ones that gained a name, a
    /// citing source or a citing work.
    pub stubs_put: u64,
    #[allow(missing_docs)]
    pub minted: u64,
    #[allow(missing_docs)]
    pub stubs_minted: u64,
    /// Ids retired into an older one.
    pub merged: u64,
    /// Rows dropped: no records and nothing citing them.
    pub dropped: u64,
    #[allow(missing_docs)]
    pub names_attached: u64,
    #[allow(missing_docs)]
    pub names_dropped: u64,
    /// Arc-and-source pairs added and removed.
    pub arcs_added: u64,
    #[allow(missing_docs)]
    pub arcs_removed: u64,
    /// Arcs moved from a retired id to its survivor.
    pub arcs_moved: u64,
    #[allow(missing_docs)]
    pub relations_put: u64,
    #[allow(missing_docs)]
    pub unresolved_put: u64,
    #[allow(missing_docs)]
    pub public_minted: u64,
    /// References by what became of them.
    pub references: BTreeMap<&'static str, u64>,
    /// Works with duplicate records of one source where only some were
    /// in the batch: arcs added, none removed, until a full merge.
    pub union_only: u64,
    /// Stubs left with no citing arc: kept, until a full merge drops
    /// their names.
    pub uncited_stubs: u64,
    /// Lines a batch that never reached the journal left behind, cut.
    pub truncated: Vec<String>,
    /// What the lexical index absorbed.
    pub lexical: search::Absorbed,
}

/// The live names table with this batch's lines over it.
struct Live<'s> {
    serving: &'s Serving,
    names: HashMap<OwnedName, Option<WorkId>>,
    retired: HashMap<WorkId, Option<WorkId>>,
    minted: u32,
    lines: Vec<registry::Line>,
}

impl<'s> Live<'s> {
    fn new(serving: &'s Serving) -> Self {
        Self {
            serving,
            names: HashMap::new(),
            retired: HashMap::new(),
            minted: serving.minted(),
            lines: Vec::new(),
        }
    }

    fn live(&self, mut id: WorkId) -> Option<WorkId> {
        loop {
            if let Some(into) = self.retired.get(&id) {
                id = (*into)?;
                continue;
            }
            match self.serving.live(id) {
                Some(l) if l == id => return Some(id),
                Some(l) => id = l,
                None => return None,
            }
        }
    }

    fn resolved(&self, name: &OwnedName) -> Option<Resolved> {
        let follow = |id: WorkId| match self.live(id) {
            Some(l) => Resolved::One(l),
            None => Resolved::Split(id),
        };
        if let Some(over) = self.names.get(name) {
            return over.map(follow);
        }
        match self.serving.resolve_name(name)? {
            Resolved::One(id) => Some(follow(id)),
            other => Some(other),
        }
    }
}

impl Table for Live<'_> {
    fn lookup(&self, name: &OwnedName) -> Option<WorkId> {
        match self.resolved(name) {
            Some(Resolved::One(id)) => Some(id),
            _ => None,
        }
    }

    fn minted(&self) -> u32 {
        self.minted
    }

    fn apply(&mut self, line: &registry::Line) -> Result<(), String> {
        match line {
            registry::Line::Mint { id, names, .. } => {
                if id.get() != self.minted + 1 {
                    return Err(format!("mint of {id} after {} ids", self.minted));
                }
                self.minted = id.get();
                for name in names {
                    self.names.insert(name.clone(), Some(*id));
                }
            }
            registry::Line::Name { id, name, .. } => {
                self.names.insert(name.clone(), Some(*id));
            }
            registry::Line::Drop { name, .. } => {
                self.names.insert(name.clone(), None);
            }
            registry::Line::Merge { id, into, .. } => {
                if into >= id {
                    return Err(format!("merge of {id} into {into}, which is not older"));
                }
                self.retired.insert(*id, Some(*into));
            }
            registry::Line::Split { id, .. } => {
                self.retired.insert(*id, None);
            }
        }
        self.lines.push(line.clone());
        Ok(())
    }
}

/// Rows fetched once.
struct Rows<'s> {
    serving: &'s Serving,
    cache: HashMap<WorkId, Option<Row>>,
}

impl Rows<'_> {
    fn get(&mut self, id: WorkId) -> Result<Option<Row>, Error> {
        if let Some(row) = self.cache.get(&id) {
            return Ok(row.clone());
        }
        let row = self.serving.work(id)?;
        self.cache.insert(id, row.clone());
        Ok(row)
    }
}

/// The batch's records by source and native id, so that asking whether a
/// record is in the batch is a lookup rather than a scan of the batch.
/// The first copy of a record speaks, as in [`apply`].
#[derive(Default)]
struct BatchIndex(HashMap<SourceId, HashMap<OwnedName, usize>>);

impl BatchIndex {
    fn of(records: &[Incoming]) -> Self {
        let mut index = Self::default();
        for (i, r) in records.iter().enumerate() {
            index
                .0
                .entry(r.source)
                .or_default()
                .entry(r.native.clone())
                .or_insert(i);
        }
        index
    }

    /// The position of the batch's record of `source` with this native id.
    fn get(&self, source: SourceId, native: &OwnedName) -> Option<usize> {
        self.0.get(&source)?.get(native).copied()
    }

    fn contains(&self, source: SourceId, native: &OwnedName) -> bool {
        self.get(source, native).is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Batch(usize),
    Work(WorkId),
}

/// The items a batch touches, joined through their names.
#[derive(Default)]
struct Components {
    items: Vec<Item>,
    parent: Vec<usize>,
    first_of_name: HashMap<OwnedName, usize>,
    work_item: HashMap<WorkId, usize>,
}

impl Components {
    fn push(&mut self, item: Item) -> usize {
        let ix = self.items.len();
        self.items.push(item);
        self.parent.push(ix);
        if let Item::Work(id) = item {
            self.work_item.insert(id, ix);
        }
        ix
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn join(&mut self, item: usize, name: &OwnedName) {
        match self.first_of_name.get(name).copied() {
            Some(other) => {
                let (a, b) = (self.find(item), self.find(other));
                if a != b {
                    self.parent[a.max(b)] = a.min(b);
                }
            }
            None => {
                self.first_of_name.insert(name.clone(), item);
            }
        }
    }

    /// The components, each its items, in a deterministic order.
    fn groups(&mut self) -> Vec<Vec<Item>> {
        let mut by_root: BTreeMap<usize, Vec<Item>> = BTreeMap::new();
        for ix in 0..self.items.len() {
            let root = self.find(ix);
            by_root.entry(root).or_default().push(self.items[ix]);
        }
        by_root.into_values().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Batch(usize),
    Row(WorkId, usize),
}

/// Everything one component resolved to.
struct ResolvedComponent {
    /// Held rows to put, with their text.
    puts: Vec<(OwnedWork, Text)>,
    /// Ids retired into another by this component.
    retirements: Vec<(WorkId, WorkId)>,
    /// Ids dropped: no records, nothing citing them.
    dropped: Vec<WorkId>,
}

/// The engine's working state for one batch.
struct Engine<'s> {
    serving: &'s Serving,
    batch: &'s Batch,
    index: BatchIndex,
    at: String,
    live: Live<'s>,
    rows: Rows<'s>,
    /// The work each batch record ended in.
    work_of_batch: HashMap<usize, WorkId>,
    /// The work each batch record's native id was in before, and the
    /// row's record index.
    previous: HashMap<(SourceId, OwnedName), (WorkId, usize)>,
    /// Works of the batch's deletions.
    deleted_in: HashMap<(SourceId, OwnedName), WorkId>,
    /// Batch records of each work, by source.
    batch_sources: HashMap<WorkId, Sources>,
    puts: Vec<(OwnedWork, Option<Text>)>,
    retirements: Vec<(WorkId, WorkId)>,
    dropped: Vec<WorkId>,
    /// Ids no longer live after this batch.
    gone: HashSet<WorkId>,
    /// Arcs of a citing work and source moving from a stub a record took
    /// over to the stub its leftover names became.
    moves: Vec<(WorkId, SourceId, WorkId, WorkId)>,
    /// Stub rows to write, by id, held back until the arcs are known.
    stub_rows: BTreeMap<WorkId, StubRow>,
    names_dropped: u64,
    applied: Applied,
}

impl Engine<'_> {
    fn touch(&mut self, name: &OwnedName) -> Vec<WorkId> {
        match self.live.resolved(name) {
            Some(Resolved::One(id)) => vec![id],
            Some(Resolved::Shared(ids)) => ids,
            Some(Resolved::Split(_)) | None => Vec::new(),
        }
    }

    /// The components the batch cuts out of the live state.
    fn components(&mut self) -> Result<Vec<Vec<Item>>, Error> {
        let mut comps = Components::default();
        let mut queue: Vec<WorkId> = Vec::new();
        for i in 0..self.batch.records.len() {
            let item = comps.push(Item::Batch(i));
            let asserts = self.batch.records[i].asserts.clone();
            for a in &asserts {
                comps.join(item, &a.name);
                queue.extend(self.touch(&a.name));
            }
        }
        let deletions = self.batch.deletions.clone();
        for (source, native) in &deletions {
            let touched = self.touch(native);
            match touched.first() {
                Some(id) => {
                    self.deleted_in.insert((*source, native.clone()), *id);
                    self.applied.deleted += 1;
                }
                None => self.applied.deletions_unknown += 1,
            }
            queue.extend(touched);
        }
        let mut seen: HashSet<WorkId> = HashSet::new();
        while let Some(id) = queue.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(row) = self.rows.get(id)? else {
                continue;
            };
            let item = comps.push(Item::Work(id));
            for (j, record) in row.records.iter().enumerate() {
                self.previous
                    .insert((record.source, record.native.clone()), (id, j));
            }
            for n in &row.names {
                comps.join(item, &n.name);
                for other in self.touch(&n.name) {
                    if !seen.contains(&other) {
                        queue.push(other);
                    }
                }
            }
        }
        Ok(comps.groups())
    }

    /// The rules on one component, and the registry's answer.
    #[allow(clippy::too_many_lines)]
    fn resolve_component(&mut self, items: &[Item]) -> Result<ResolvedComponent, Error> {
        let batch_natives: HashSet<(SourceId, OwnedName)> = items
            .iter()
            .filter_map(|i| match i {
                Item::Batch(i) => {
                    let r = &self.batch.records[*i];
                    Some((r.source, r.native.clone()))
                }
                Item::Work(_) => None,
            })
            .collect();
        let mut identity = Identity::new();
        let mut origins: Vec<Origin> = Vec::new();
        let mut existing: BTreeMap<WorkId, Row> = BTreeMap::new();
        let push = |identity: &mut Identity,
                    source: SourceId,
                    asserts: &[OwnedAssert],
                    view: &Owned|
         -> Result<(), Error> {
            let assertions: Vec<Assertion<'_>> = asserts
                .iter()
                .map(|a| Assertion {
                    name: a.name.as_name(),
                    field: field_static(&a.field),
                    native: a.native,
                })
                .collect();
            identity.push(
                source,
                &assertions,
                Titles {
                    title: view.title.as_deref(),
                    vernacular: view.vernacular.as_deref(),
                },
            )?;
            Ok(())
        };
        for item in items {
            match *item {
                Item::Work(id) => {
                    let Some(row) = self.rows.get(id)? else {
                        continue;
                    };
                    for (j, record) in row.records.iter().enumerate() {
                        let key = (record.source, record.native.clone());
                        if self.deleted_in.contains_key(&key) || batch_natives.contains(&key) {
                            continue;
                        }
                        push(&mut identity, record.source, &record.asserts, &record.view)?;
                        origins.push(Origin::Row(id, j));
                    }
                    existing.insert(id, row);
                }
                Item::Batch(i) => {
                    let r = &self.batch.records[i];
                    push(&mut identity, r.source, &r.asserts, &r.view)?;
                    origins.push(Origin::Batch(i));
                }
            }
        }
        identity.mark();
        let res = identity.resolve(false)?;
        let order = res.held_order();
        let all: Vec<Vec<OwnedName>> = order.iter().map(|(_, n)| n.clone()).collect();
        let assignment = registry::assign(&mut self.live, &all, &self.at);
        if assignment.split > 0 {
            return Err(Error::FullMerge(format!(
                "a work's names split between works ({})",
                assignment
                    .lines
                    .iter()
                    .filter_map(|l| match l {
                        registry::Line::Split { id, into, .. } => Some(format!(
                            "{id} into {}",
                            into.iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        self.applied.minted += assignment.minted;
        self.applied.merged += assignment.merged;
        self.applied.names_attached += assignment.attached;
        let mut held_ids = vec![WorkId::FIRST; res.works.len()];
        for ((w, _), id) in order.iter().zip(&assignment.ids) {
            held_ids[*w] = *id;
        }
        let assigned: HashSet<WorkId> = held_ids.iter().copied().collect();
        let retirements: Vec<(WorkId, WorkId)> = assignment
            .lines
            .iter()
            .filter_map(|l| match l {
                registry::Line::Merge { id, into, .. } => Some((*id, *into)),
                _ => None,
            })
            .collect();
        let merged: HashSet<WorkId> = retirements.iter().map(|(id, _)| *id).collect();
        let mut dropped = Vec::new();
        for id in existing.keys() {
            if assigned.contains(id) || merged.contains(id) {
                continue;
            }
            if self.serving.citers(*id).next().is_some() {
                return Err(Error::FullMerge(format!(
                    "{id} lost its last record and is still cited; its stub's names are the citing references'"
                )));
            }
            dropped.push(*id);
        }
        // A stub a held work took over, by reusing its id or by taking it
        // in a merge: the names the work does not hold leave it — to a
        // stub of their own where references cite them without a kept
        // name, which the row's evidence says — and the arcs of those
        // references follow.
        let mut handled: HashSet<OwnedName> = HashSet::new();
        for (id, row) in &existing {
            if !row.stub {
                continue;
            }
            let survivor = if assigned.contains(id) {
                *id
            } else if let Some((_, into)) = retirements.iter().find(|(r, _)| r == id) {
                *into
            } else {
                continue;
            };
            let Some(w) = held_ids.iter().position(|x| *x == survivor) else {
                continue;
            };
            let kept: HashSet<OwnedName> = res.works[w]
                .names
                .iter()
                .map(|n| res.arena.get(n.name).clone())
                .collect();
            let kept_pairs: HashSet<(WorkId, SourceId)> = row
                .names
                .iter()
                .filter(|n| kept.contains(&n.name))
                .flat_map(|n| n.cited_by.iter().copied())
                .collect();
            let mut leftovers: Vec<(OwnedName, Vec<(WorkId, SourceId)>)> = Vec::new();
            for n in &row.names {
                if kept.contains(&n.name) {
                    continue;
                }
                handled.insert(n.name.clone());
                let movers: Vec<(WorkId, SourceId)> = n
                    .cited_by
                    .iter()
                    .copied()
                    .filter(|p| !kept_pairs.contains(p))
                    .collect();
                if movers.is_empty() {
                    if self.live.lookup(&n.name) == Some(survivor) {
                        let line = registry::Line::Drop {
                            name: n.name.clone(),
                            at: self.at.clone(),
                        };
                        self.live.apply(&line).map_err(Error::Mismatch)?;
                        self.names_dropped += 1;
                    }
                } else {
                    leftovers.push((n.name.clone(), movers));
                }
            }
            // Leftover names one citing record gave together are one stub.
            let mut parent: Vec<usize> = (0..leftovers.len()).collect();
            for i in 0..leftovers.len() {
                for j in i + 1..leftovers.len() {
                    if leftovers[i].1.iter().any(|p| leftovers[j].1.contains(p)) {
                        let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                        if a != b {
                            parent[a.max(b)] = a.min(b);
                        }
                    }
                }
            }
            let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for i in 0..leftovers.len() {
                let r = root(&mut parent, i);
                groups.entry(r).or_default().push(i);
            }
            for members in groups.into_values() {
                let mut names: BTreeMap<OwnedName, (Sources, Vec<(WorkId, SourceId)>)> =
                    BTreeMap::new();
                for i in &members {
                    let (name, movers) = &leftovers[*i];
                    let by: Sources = movers.iter().map(|(_, s)| *s).collect();
                    names.insert(name.clone(), (by, movers.clone()));
                }
                let stub_id = WorkId::new(self.live.minted() + 1)
                    .ok_or_else(|| Error::Mismatch("ids exhausted".into()))?;
                let line = registry::Line::Mint {
                    id: stub_id,
                    names: names.keys().cloned().collect(),
                    at: self.at.clone(),
                };
                self.live.apply(&line).map_err(Error::Mismatch)?;
                self.applied.minted += 1;
                self.applied.stubs_minted += 1;
                for (_, movers) in names.values() {
                    for (x, s) in movers {
                        self.moves.push((*x, *s, *id, stub_id));
                    }
                }
                self.stub_rows.insert(
                    stub_id,
                    StubRow {
                        names,
                        minted: true,
                        changed: true,
                    },
                );
            }
        }
        // Names the rows had that no work claims now.
        let claimed: HashSet<&OwnedName> = all.iter().flatten().collect();
        for (id, row) in &existing {
            for n in &row.names {
                if claimed.contains(&n.name) || handled.contains(&n.name) {
                    continue;
                }
                if self.live.lookup(&n.name) == Some(*id)
                    || (merged.contains(id) && self.live.lookup(&n.name).is_some())
                {
                    let line = registry::Line::Drop {
                        name: n.name.clone(),
                        at: self.at.clone(),
                    };
                    self.live.apply(&line).map_err(Error::Mismatch)?;
                    self.names_dropped += 1;
                }
            }
        }
        // Views, then rows.
        let views: Vec<Option<Owned>> = origins
            .iter()
            .map(|o| match o {
                Origin::Batch(i) => Some(self.batch.records[*i].view.clone()),
                Origin::Row(id, j) => existing.get(id).map(|r| r.records[*j].view.clone()),
            })
            .collect();
        let mut tallies = build::Tallies::new();
        let mut puts = Vec::with_capacity(res.works.len());
        for (w, work) in res.works.iter().enumerate() {
            let id = held_ids[w];
            let owned = build::held(&res, work, id, &views, &mut tallies);
            for &rix in &work.records {
                if let Origin::Batch(i) = origins[rix as usize] {
                    self.work_of_batch.insert(i, id);
                    self.batch_sources
                        .entry(id)
                        .or_insert(Sources::NONE)
                        .insert(self.batch.records[i].source);
                }
            }
            let text = self.text_for(&owned, &existing)?;
            puts.push((owned, text));
        }
        Ok(ResolvedComponent {
            puts,
            retirements,
            dropped,
        })
    }

    /// The text a held row carries: from the batch's records where they
    /// are the promoted ones, else from the row that carried it before.
    fn text_for(&self, work: &OwnedWork, existing: &BTreeMap<WorkId, Row>) -> Result<Text, Error> {
        let borrowed = work.borrow();
        let wants = [
            ("abstract", merge::document::abstract_source(&borrowed)),
            ("authors", work.fields.authors.as_ref().map(|f| f.from)),
            ("venue", work.fields.venue.as_ref().map(|f| f.from)),
        ];
        let mut text = Text::default();
        for (field, source) in wants {
            let Some(source) = source else {
                continue;
            };
            let record = work
                .records
                .iter()
                .find(|r| r.source == source && r.promoted)
                .or_else(|| work.records.iter().find(|r| r.source == source));
            let Some(record) = record else {
                continue;
            };
            let key = (record.source, record.native.clone());
            let supplied: Option<Text> = if let Some(i) = self.index.get(key.0, &key.1) {
                Some(self.batch.records[i].text.clone())
            } else if let Some((old_id, _)) = self.previous.get(&key) {
                let old =
                    existing
                        .get(old_id)
                        .or(self.rows.cache.get(old_id).and_then(Option::as_ref));
                old.and_then(|row| {
                    let from_this = match field {
                        "abstract" => {
                            merge::document::abstract_source(&row.work().borrow()) == Some(source)
                        }
                        "authors" => row.fields.authors.as_ref().map(|f| f.from) == Some(source),
                        _ => row.fields.venue.as_ref().map(|f| f.from) == Some(source),
                    };
                    let promoted_then = row
                        .records
                        .iter()
                        .any(|r| r.source == source && r.native == record.native && r.promoted);
                    (from_this && promoted_then)
                        .then(|| row.text.as_ref().map(crate::row::Text::body))
                        .flatten()
                })
            } else {
                None
            };
            let Some(supplied) = supplied else {
                return Err(Error::FullMerge(format!(
                    "the {field} of {} comes from {source}'s record {}, whose text only the normalized layer has",
                    work.id, record.native
                )));
            };
            match field {
                "abstract" => text.abstract_text = supplied.abstract_text,
                "authors" => text.authors = supplied.authors,
                _ => text.venue = supplied.venue,
            }
        }
        Ok(text)
    }

    /// Runs the rules over every component.
    fn resolve_all(&mut self) -> Result<(), Error> {
        let groups = self.components()?;
        self.applied.components = groups.len() as u64;
        for items in &groups {
            let resolved = self.resolve_component(items)?;
            for (work, text) in resolved.puts {
                self.puts.push((work, Some(text)));
            }
            for (id, into) in resolved.retirements {
                self.gone.insert(id);
                self.retirements.push((id, into));
            }
            for id in resolved.dropped {
                self.gone.insert(id);
                self.dropped.push(id);
            }
        }
        Ok(())
    }

    /// What a reference's name points at: a work of this batch's
    /// resolution first, then the live table.
    fn target(&self, name: &OwnedName, table: &HashMap<OwnedName, Target>) -> Target {
        target_of(&self.live, self.serving, table, name)
    }

    /// The batch's own resolution as a names table: every held row's
    /// names, shared ones apart, and the names of every stub whose row
    /// the batch is making.
    fn table(&self) -> HashMap<OwnedName, Target> {
        let mut table = HashMap::new();
        for (work, _) in &self.puts {
            for n in &work.names {
                let target = if n.standing == Standing::Shared {
                    Target::Shared
                } else {
                    Target::Work(work.id, work.stub)
                };
                table.insert(n.name.clone(), target);
            }
        }
        for (id, stub) in &self.stub_rows {
            for name in stub.names.keys() {
                table.insert(name.clone(), Target::Work(*id, true));
            }
        }
        table
    }

    /// A stub's row as the batch has it: the one it is already making,
    /// taken out to be settled again, or the row from before the batch.
    fn stub_row_of(&mut self, id: WorkId) -> Result<StubRow, Error> {
        if let Some(stub) = self.stub_rows.remove(&id) {
            return Ok(stub);
        }
        let Some(row) = self.rows.get(id)? else {
            return Err(Error::Mismatch(format!("stub {id} has no row")));
        };
        if !row.stub {
            return Err(Error::Mismatch(format!(
                "{id} is named as a stub and holds records"
            )));
        }
        Ok(StubRow {
            names: row
                .names
                .iter()
                .map(|n| (n.name.clone(), (n.by, n.cited_by.to_vec())))
                .collect(),
            minted: false,
            changed: false,
        })
    }
}

/// A stub's row while the batch makes it: each name with the sources
/// that cited it with no held name beside it, and the works whose arcs
/// landed on the stub through it — which only the arcs know, so the row
/// is written after them (serving plan §11, item 21).
struct StubRow {
    names: BTreeMap<OwnedName, (Sources, Vec<(WorkId, SourceId)>)>,
    /// Minted by this batch.
    minted: bool,
    /// Different from the row before the batch.
    changed: bool,
}

/// Stubs the batch's references and relations name: existing ones cited
/// again, joined with the unknown names cited beside them, and new
/// ones for names nothing holds (merge plan §4: a reference names one
/// work).
#[derive(Default)]
struct Stubs {
    nodes: Vec<Node>,
    parent: Vec<usize>,
    of_name: HashMap<OwnedName, usize>,
    of_id: HashMap<WorkId, usize>,
    /// The sources whose references or relations cited each name in this
    /// batch with no held name beside it. Which works cited it is the
    /// arcs' to say, in pass two.
    by: HashMap<OwnedName, Sources>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Existing(WorkId),
    New(OwnedName),
}

impl Stubs {
    fn node_of_id(&mut self, id: WorkId) -> usize {
        if let Some(ix) = self.of_id.get(&id) {
            return *ix;
        }
        let ix = self.nodes.len();
        self.nodes.push(Node::Existing(id));
        self.parent.push(ix);
        self.of_id.insert(id, ix);
        ix
    }

    fn node_of_name(&mut self, name: &OwnedName) -> usize {
        if let Some(ix) = self.of_name.get(name) {
            return *ix;
        }
        let ix = self.nodes.len();
        self.nodes.push(Node::New(name.clone()));
        self.parent.push(ix);
        self.of_name.insert(name.clone(), ix);
        ix
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra.max(rb)] = ra.min(rb);
        }
    }

    /// One reference or relation whose names hold no held work: its stub
    /// ids and unknown names become one stub, cited by `source`.
    fn cite(&mut self, source: SourceId, stubs: &[(OwnedName, WorkId)], unknown: &[OwnedName]) {
        let mut first: Option<usize> = None;
        for (name, id) in stubs {
            let ix = self.node_of_id(*id);
            self.by.entry(name.clone()).or_default().insert(source);
            match first {
                None => first = Some(ix),
                Some(f) => self.union(f, ix),
            }
        }
        for name in unknown {
            let ix = self.node_of_name(name);
            self.by.entry(name.clone()).or_default().insert(source);
            match first {
                None => first = Some(ix),
                Some(f) => self.union(f, ix),
            }
        }
    }

    /// The groups: each its existing ids, oldest first, and its new
    /// names, sorted; in a deterministic order.
    fn groups(&mut self) -> Vec<(Vec<WorkId>, Vec<OwnedName>)> {
        let mut by_root: BTreeMap<usize, (Vec<WorkId>, Vec<OwnedName>)> = BTreeMap::new();
        for ix in 0..self.nodes.len() {
            let root = self.find(ix);
            let entry = by_root.entry(root).or_default();
            match &self.nodes[ix] {
                Node::Existing(id) => entry.0.push(*id),
                Node::New(name) => entry.1.push(name.clone()),
            }
        }
        let mut groups: Vec<(Vec<WorkId>, Vec<OwnedName>)> = by_root.into_values().collect();
        for (ids, names) in &mut groups {
            ids.sort_unstable();
            names.sort();
        }
        groups.sort_by(|a, b| (a.0.first(), &a.1).cmp(&(b.0.first(), &b.1)));
        groups
    }
}

/// A stub's row from its names, each with who cited it and which works'
/// references did: one promoted per namespace in name order, the rest
/// aliases — the full merge's rule for a stub.
fn stub_row(
    id: WorkId,
    names: BTreeMap<OwnedName, (Sources, Vec<(WorkId, SourceId)>)>,
) -> OwnedWork {
    let mut seen: Vec<ingest::Key> = Vec::new();
    let rows = names
        .into_iter()
        .map(|(name, (by, mut cited_by))| {
            let key = name.key();
            let standing = if seen.contains(&key) {
                Standing::Alias
            } else {
                seen.push(key);
                Standing::Promoted
            };
            cited_by.sort_unstable();
            cited_by.dedup();
            OwnedNamed {
                name,
                standing,
                by,
                native: false,
                cited_by: cited_by.into_iter().collect(),
            }
        })
        .collect();
    OwnedWork {
        id,
        stub: true,
        records: smallvec::SmallVec::default(),
        names: rows,
        rejected: smallvec::SmallVec::default(),
        fields: build::OwnedFields::default(),
        flags: merge::record::Flags::default(),
    }
}

/// The arcs of one (work, source) as the batch leaves them.
#[derive(Default)]
struct Lists {
    edges: BTreeMap<(WorkId, WorkId), (Sources, Sources)>,
    relations: Vec<RelationLine>,
    unresolved: Vec<UnresolvedLine>,
}

impl Lists {
    fn add(&mut self, from: WorkId, to: WorkId, by: Sources) {
        let slot = self.edges.entry((from, to)).or_default();
        slot.0 = slot.0.union(by);
        slot.1 = slot.1.difference(by);
    }

    fn remove(&mut self, from: WorkId, to: WorkId, by: Sources) {
        let slot = self.edges.entry((from, to)).or_default();
        slot.1 = slot.1.union(by);
        slot.0 = slot.0.difference(by);
    }
}

impl Engine<'_> {
    /// The batch's references and relations: existing stubs cited again,
    /// new stubs minted, then every reference resolved into an arc or an
    /// unresolved row, the previous lists diffed against the new.
    #[allow(clippy::too_many_lines)]
    fn edges(&mut self, batch_number: u64) -> Result<Lists, Error> {
        let table = self.table();
        let mut stubs = Stubs::default();
        // Pass one: which references name no held work.
        let classify = |this: &Self, names: &[OwnedName]| -> Result<Classified, Error> {
            let mut held = false;
            let mut found: Vec<(OwnedName, WorkId)> = Vec::new();
            let mut unknown: Vec<OwnedName> = Vec::new();
            for name in names {
                match this.target(name, &table) {
                    Target::Work(id, _) if this.gone.contains(&id) => {
                        return Err(Error::FullMerge(format!(
                            "{id} lost its last record and a batch record cites it as {name}"
                        )));
                    }
                    Target::Work(_, false) | Target::Shared => held = true,
                    Target::Work(id, true) => found.push((name.clone(), id)),
                    Target::Unknown => unknown.push(name.clone()),
                }
            }
            Ok(Classified {
                held,
                stubs: found,
                unknown,
            })
        };
        for r in &self.batch.records {
            for reference in &r.references {
                let c = classify(self, &reference.names)?;
                if c.names_a_stub() {
                    stubs.cite(r.source, &c.stubs, &c.unknown);
                }
            }
            for relation in &r.relations {
                if let Some(target) = &relation.target {
                    let c = classify(self, std::slice::from_ref(target))?;
                    if c.names_a_stub() {
                        stubs.cite(r.source, &c.stubs, &c.unknown);
                    }
                }
            }
        }
        // Stubs settled: survivors, merges, mints. Their rows wait for pass
        // two, whose arcs say which works cite each name.
        for (ids, new_names) in stubs.groups() {
            let mut names: BTreeMap<OwnedName, (Sources, Vec<(WorkId, SourceId)>)> =
                BTreeMap::new();
            let mut changed = ids.len() != 1 || !new_names.is_empty();
            let mut minted = false;
            let survivor = if let Some(first) = ids.first() {
                {
                    for id in &ids {
                        let stub = self.stub_row_of(*id)?;
                        if stub.minted && ids.len() > 1 {
                            return Err(Error::FullMerge(format!(
                                "stub {id}, minted by this batch for names a record left, is cited together with stub {}",
                                ids.iter()
                                    .find(|other| *other != id)
                                    .map_or_else(String::new, ToString::to_string)
                            )));
                        }
                        minted |= stub.minted;
                        changed |= stub.changed;
                        for (name, (by, cited_by)) in stub.names {
                            let slot = names.entry(name).or_default();
                            slot.0 = slot.0.union(by);
                            slot.1.extend(cited_by);
                        }
                        if id != first {
                            let line = registry::Line::Merge {
                                id: *id,
                                into: *first,
                                at: self.at.clone(),
                            };
                            self.live.apply(&line).map_err(Error::Mismatch)?;
                            self.retirements.push((*id, *first));
                            self.gone.insert(*id);
                            self.applied.merged += 1;
                        }
                    }
                    for name in &new_names {
                        let line = registry::Line::Name {
                            id: *first,
                            name: name.clone(),
                            at: self.at.clone(),
                        };
                        self.live.apply(&line).map_err(Error::Mismatch)?;
                        self.applied.names_attached += 1;
                    }
                    *first
                }
            } else {
                let id = WorkId::new(self.live.minted() + 1)
                    .ok_or_else(|| Error::Mismatch("ids exhausted".into()))?;
                let line = registry::Line::Mint {
                    id,
                    names: new_names.clone(),
                    at: self.at.clone(),
                };
                self.live.apply(&line).map_err(Error::Mismatch)?;
                self.applied.minted += 1;
                self.applied.stubs_minted += 1;
                minted = true;
                id
            };
            for name in &new_names {
                names.insert(name.clone(), (Sources::NONE, Vec::new()));
            }
            // What this batch's citations add, looked up for this group's
            // names alone: the whole batch's table is never walked per
            // group.
            for (name, slot) in &mut names {
                if let Some(by) = stubs.by.get(name) {
                    let joined = slot.0.union(*by);
                    if joined != slot.0 {
                        slot.0 = joined;
                        changed = true;
                    }
                }
            }
            self.stub_rows.insert(
                survivor,
                StubRow {
                    names,
                    minted,
                    changed,
                },
            );
        }
        // Pass two: every reference through the final table.
        let table = self.table();
        // Per stub, per name, the works whose arcs landed on the stub
        // through that name: a reference that named a held work beside it
        // too, when the registry's order chose the stub, as the full merge
        // counts it.
        let mut landed: BTreeMap<WorkId, BTreeMap<OwnedName, Vec<(WorkId, SourceId)>>> =
            BTreeMap::new();
        let mut lists = Lists::default();
        let (live, serving) = (&self.live, self.serving);
        let resolver = |name: merge::Name<'_>| -> Target {
            target_of(live, serving, &table, &name.to_owned())
        };
        let batch_sources = self.batch_sources.clone();
        for (i, r) in self.batch.records.iter().enumerate() {
            let Some(&work) = self.work_of_batch.get(&i) else {
                return Err(Error::Mismatch(format!(
                    "{}={} ended in no work",
                    r.source, r.native
                )));
            };
            let mut new_arcs: BTreeSet<WorkId> = BTreeSet::new();
            let mut unresolved: Vec<OwnedUnresolved> = Vec::new();
            for reference in &r.references {
                let names: Vec<merge::Name<'_>> =
                    reference.names.iter().map(OwnedName::as_name).collect();
                let referenced = build::resolve_reference(
                    &names,
                    reference.citation.borrow().is_empty(),
                    resolver,
                );
                *self
                    .applied
                    .references
                    .entry(referenced.outcome(work))
                    .or_default() += 1;
                let why = match referenced {
                    Referenced::Empty | Referenced::Unknown => None,
                    Referenced::NoName => Some(Why::NoName),
                    Referenced::SharedOnly => Some(Why::Shared),
                    Referenced::Edge { to, disagree, stub } => {
                        new_arcs.insert(to);
                        if stub {
                            for name in &reference.names {
                                if target_of(live, serving, &table, name) == Target::Work(to, true)
                                {
                                    landed
                                        .entry(to)
                                        .or_default()
                                        .entry(name.clone())
                                        .or_default()
                                        .push((work, r.source));
                                }
                            }
                        }
                        disagree.then_some(Why::Disagree)
                    }
                };
                if let Some(why) = why {
                    unresolved.push(OwnedUnresolved {
                        from: work,
                        by: r.source,
                        why,
                        names: reference.names.iter().cloned().collect(),
                        citation: reference.citation.clone(),
                    });
                }
            }
            let mut relations: Vec<Related> = r
                .relations
                .iter()
                .filter_map(|rel| {
                    match rel
                        .target
                        .as_ref()
                        .map(|t| target_of(live, serving, &table, t))
                    {
                        Some(Target::Work(to, _)) => Some(Related { to, kind: rel.kind }),
                        _ => None,
                    }
                })
                .collect();
            relations.sort_unstable();
            relations.dedup();
            // The previous list of this record's source, on the work the
            // record was in before.
            let key = (r.source, r.native.clone());
            let previous = self.previous.get(&key).map(|(id, _)| *id);
            let by: Sources = [r.source].into_iter().collect();
            if let Some(old_work) = previous {
                let old_row = self.rows.get(old_work)?;
                let duplicates = old_row.as_ref().is_some_and(|row| {
                    row.records.iter().filter(|x| x.source == r.source).count() > 1
                });
                let all_in_batch = old_row.as_ref().is_some_and(|row| {
                    row.records
                        .iter()
                        .filter(|x| x.source == r.source)
                        .all(|x| self.index.contains(x.source, &x.native))
                });
                let old_arcs: BTreeSet<WorkId> = self
                    .serving
                    .references(old_work)
                    .filter(|a| a.by.contains(r.source))
                    .map(|a| a.work)
                    .collect();
                if duplicates && !all_in_batch {
                    if old_work != work {
                        return Err(Error::FullMerge(format!(
                            "{}={} moved from {old_work} to {work} while a duplicate record of its source stays",
                            r.source, r.native
                        )));
                    }
                    self.applied.union_only += 1;
                    for to in new_arcs.difference(&old_arcs) {
                        lists.add(work, *to, by);
                    }
                } else if old_work == work {
                    for to in old_arcs.difference(&new_arcs) {
                        lists.remove(work, *to, by);
                    }
                    for to in new_arcs.difference(&old_arcs) {
                        lists.add(work, *to, by);
                    }
                } else {
                    for to in &old_arcs {
                        lists.remove(old_work, *to, by);
                    }
                    for to in &new_arcs {
                        lists.add(work, *to, by);
                    }
                    lists.relations.push(RelationLine::Put {
                        batch: batch_number,
                        from: old_work,
                        by: r.source,
                        relations: Vec::new(),
                    });
                    lists.unresolved.push(UnresolvedLine::Put {
                        batch: batch_number,
                        from: old_work,
                        by: r.source,
                        rows: Vec::new(),
                    });
                }
            } else {
                for to in &new_arcs {
                    lists.add(work, *to, by);
                }
            }
            lists.relations.push(RelationLine::Put {
                batch: batch_number,
                from: work,
                by: r.source,
                relations,
            });
            lists.unresolved.push(UnresolvedLine::Put {
                batch: batch_number,
                from: work,
                by: r.source,
                rows: unresolved,
            });
        }
        // The stubs' rows, now that the arcs say which works cite each
        // name: a stub the batch did not settle is read for the first time
        // here when an arc lands on it beside a held name.
        for (id, by_name) in landed {
            let mut stub = self.stub_row_of(id)?;
            for (name, pairs) in by_name {
                let Some(slot) = stub.names.get_mut(&name) else {
                    return Err(Error::Mismatch(format!(
                        "an arc landed on stub {id} through {name}, which is not its name"
                    )));
                };
                let mut present: HashSet<(WorkId, SourceId)> = slot.1.iter().copied().collect();
                for p in pairs {
                    if present.insert(p) {
                        slot.1.push(p);
                        stub.changed = true;
                    }
                }
            }
            self.stub_rows.insert(id, stub);
        }
        for (id, stub) in std::mem::take(&mut self.stub_rows) {
            if stub.changed {
                self.puts.push((stub_row(id, stub.names), None));
                self.applied.stubs_put += 1;
            }
        }
        // Deleted records: their lists go.
        for ((source, native), work) in self.deleted_in.clone() {
            let row = self.rows.get(work)?;
            let duplicates = row
                .as_ref()
                .is_some_and(|row| row.records.iter().filter(|x| x.source == source).count() > 1);
            if duplicates {
                return Err(Error::FullMerge(format!(
                    "{source}={native} was deleted from {work}, which holds another record of its source"
                )));
            }
            let by: Sources = [source].into_iter().collect();
            let old_arcs: Vec<WorkId> = self
                .serving
                .references(work)
                .filter(|a| a.by.contains(source))
                .map(|a| a.work)
                .collect();
            for to in old_arcs {
                lists.remove(work, to, by);
            }
            lists.relations.push(RelationLine::Put {
                batch: batch_number,
                from: work,
                by: source,
                relations: Vec::new(),
            });
            lists.unresolved.push(UnresolvedLine::Put {
                batch: batch_number,
                from: work,
                by: source,
                rows: Vec::new(),
            });
        }
        // Leftover names of a stub a record took over: the arcs of the
        // references that used them alone follow them to the new stub.
        for (x, s, from, to) in self.moves.clone() {
            let by: Sources = [s].into_iter().collect();
            lists.remove(x, from, by);
            lists.add(x, to, by);
            self.applied.arcs_moved += 1;
        }
        // Retired ids: what is left of their lists moves to the survivor,
        // the arcs a leftover name took with it excepted.
        let moved_pairs: HashSet<(WorkId, SourceId, WorkId)> = self
            .moves
            .iter()
            .map(|(x, s, from, _)| (*x, *s, *from))
            .collect();
        let retired: HashSet<WorkId> = self.retirements.iter().map(|(id, _)| *id).collect();
        // The sources whose record left each work — deleted by the batch,
        // or in the batch after being in the work before — gathered once
        // rather than looked for again for every retired id.
        let mut left: HashMap<WorkId, Sources> = HashMap::new();
        for ((s, _), w) in &self.deleted_in {
            left.entry(*w).or_insert(Sources::NONE).insert(*s);
        }
        for ((s, n), (w, _)) in &self.previous {
            if self.index.contains(*s, n) {
                left.entry(*w).or_insert(Sources::NONE).insert(*s);
            }
        }
        for (id, into) in self.retirements.clone() {
            let taken = batch_sources.get(&into).copied().unwrap_or(Sources::NONE);
            let _ = taken;
            let own = left.get(&id).copied().unwrap_or(Sources::NONE);
            for arc in self.serving.references(id) {
                if retired.contains(&arc.work) && arc.work != id {
                    return Err(Error::FullMerge(format!(
                        "{id} and {} are both retired in one batch and {id} cites {}",
                        arc.work, arc.work
                    )));
                }
                let moved = arc.by.difference(own);
                if moved.is_empty() {
                    continue;
                }
                let to = if arc.work == id { into } else { arc.work };
                lists.remove(id, arc.work, moved);
                lists.add(into, to, moved);
                self.applied.arcs_moved += 1;
            }
            for arc in self.serving.citers(id) {
                if arc.work == id {
                    continue;
                }
                if retired.contains(&arc.work) {
                    return Err(Error::FullMerge(format!(
                        "{id} and {} are both retired in one batch and {} cites {id}",
                        arc.work, arc.work
                    )));
                }
                let theirs = batch_sources
                    .get(&arc.work)
                    .copied()
                    .unwrap_or(Sources::NONE);
                let gone_with_a_name: Sources = arc
                    .by
                    .iter()
                    .filter(|s| moved_pairs.contains(&(arc.work, *s, id)))
                    .collect();
                let moved = arc.by.difference(theirs).difference(gone_with_a_name);
                if moved.is_empty() {
                    continue;
                }
                lists.remove(arc.work, id, moved);
                lists.add(arc.work, into, moved);
                self.applied.arcs_moved += 1;
            }
            lists.relations.push(RelationLine::Move {
                batch: batch_number,
                from: id,
                into,
            });
            lists.unresolved.push(UnresolvedLine::Move {
                batch: batch_number,
                from: id,
                into,
            });
        }
        // Stubs left uncited: counted; a full merge drops their names.
        let added_to: HashSet<WorkId> = lists
            .edges
            .iter()
            .filter(|(_, (added, _))| !added.is_empty())
            .map(|((_, to), _)| *to)
            .collect();
        let mut checked: HashSet<WorkId> = HashSet::new();
        for ((_, to), (_, removed)) in &lists.edges {
            if removed.is_empty()
                || retired.contains(to)
                || !checked.insert(*to)
                || !self.serving.is_stub(*to)
            {
                continue;
            }
            let mut cited = false;
            for arc in self.serving.citers(*to) {
                let (added, gone) = lists
                    .edges
                    .get(&(arc.work, *to))
                    .copied()
                    .unwrap_or((Sources::NONE, Sources::NONE));
                if !arc.by.union(added).difference(gone).is_empty() {
                    cited = true;
                    break;
                }
            }
            if !cited && !added_to.contains(to) {
                self.applied.uncited_stubs += 1;
            }
        }
        Ok(lists)
    }
}

/// Union-find over indices: the root of `x`, paths halved on the way.
fn root(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

struct Classified {
    held: bool,
    stubs: Vec<(OwnedName, WorkId)>,
    unknown: Vec<OwnedName>,
}

impl Classified {
    /// No held work among the names, and something to make a stub of.
    fn names_a_stub(&self) -> bool {
        !self.held && (!self.stubs.is_empty() || !self.unknown.is_empty())
    }
}

/// What a name points at: a work of this batch's resolution first, then
/// the live table.
fn target_of(
    live: &Live<'_>,
    serving: &Serving,
    table: &HashMap<OwnedName, Target>,
    name: &OwnedName,
) -> Target {
    if let Some(t) = table.get(name) {
        return *t;
    }
    match live.resolved(name) {
        Some(Resolved::One(id)) => Target::Work(id, serving.is_stub(id)),
        Some(Resolved::Shared(_)) => Target::Shared,
        Some(Resolved::Split(_)) | None => Target::Unknown,
    }
}

/// Public ids for every id this batch minted, when a key was given:
/// the keyed hash, tried again with the counter on a collision with
/// any public id the live table knows.
fn mint_public(
    serving: &Serving,
    lines: &[registry::Line],
    key: Option<&Key>,
    at: &str,
) -> Vec<public::Line> {
    let Some(key) = key else {
        return Vec::new();
    };
    let mut minted: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for line in lines {
        let registry::Line::Mint { id, .. } = line else {
            continue;
        };
        let mut attempt = 0u32;
        let text = loop {
            let text = public::encode(public::derive(key, *id, attempt));
            if serving.resolve_key(&text).is_none() && !minted.contains(&text) {
                break text;
            }
            attempt += 1;
        };
        minted.insert(text.clone());
        out.push(public::Line {
            id: *id,
            public: text,
            at: at.to_owned(),
        });
    }
    out
}

/// The batch a lexical index must reach, applied from the works delta:
/// what a merge does at its start when an earlier batch reached the
/// journal but not the index.
fn heal_lexical(store: &Store, serving: &Serving, lines: &[Line]) -> Result<u64, Error> {
    let lexical = serving.lexical_batch();
    let last = lines.last().map_or(0, |l| l.batch);
    if lexical >= last {
        return Ok(0);
    }
    let merged = store.root().join("merged").join(&serving.epoch().merged);
    let works_path = delta::dir(&merged).join(delta::WORKS);
    let search_dir = store.root().join(&serving.epoch().artifacts["search"].dir);
    let mut healed = 0;
    let mut from = lines
        .iter()
        .find(|l| l.batch == lexical)
        .and_then(|l| l.delta.get("works").copied())
        .unwrap_or(0);
    for line in lines.iter().filter(|l| l.batch > lexical) {
        if line.kind == Kind::Full {
            return Err(Error::Mismatch(format!(
                "batch {} is a full merge since epoch {}; publish an epoch first",
                line.batch,
                serving.epoch().epoch
            )));
        }
        let to = line.delta.get("works").copied().unwrap_or(from);
        let (puts, removes) = documents_of(&works_path, from, to, line.batch)?;
        search::index::absorb(&search_dir, line.batch, &puts, &removes)?;
        from = to;
        healed += 1;
    }
    Ok(healed)
}

/// The documents a range of the works delta puts, and the ids it takes
/// out of the lexical index.
fn documents_of(
    works_path: &Path,
    from: u64,
    to: u64,
    batch: u64,
) -> Result<(Vec<Document>, Vec<WorkId>), Error> {
    let lines: Vec<WorkLine> =
        delta::read(works_path, from, to, Some(batch)).map_err(|e| io(works_path, e))?;
    let mut puts: BTreeMap<WorkId, Document> = BTreeMap::new();
    let mut removes: BTreeSet<WorkId> = BTreeSet::new();
    for line in lines {
        match line {
            WorkLine::Put { work, text, .. } => {
                if work.stub {
                    removes.insert(work.id);
                    puts.remove(&work.id);
                } else {
                    removes.remove(&work.id);
                    puts.insert(work.id, Document::of(&work.borrow(), text.as_ref()));
                }
            }
            WorkLine::Retire { id, .. } | WorkLine::Drop { id, .. } => {
                puts.remove(&id);
                removes.insert(id);
            }
        }
    }
    Ok((puts.into_values().collect(), removes.into_iter().collect()))
}

/// Applies one batch to the store the serving layer was opened from
/// (serving plan §5), and folds it into `serving`.
///
/// # Errors
///
/// A store not at the state a batch needs — no journal, a full merge
/// since the epoch, a merged directory of another version — a case the
/// incremental rules leave to a full merge ([`Error::FullMerge`]), or
/// I/O anywhere.
#[allow(clippy::too_many_lines)]
pub fn apply(
    store: &Store,
    serving: &Serving,
    batch: Batch,
    key: Option<&Key>,
) -> Result<Applied, Error> {
    let journal_path = store.journal();
    let lines = journal::read(&journal_path).map_err(|e| io(&journal_path, e))?;
    let last = lines
        .last()
        .cloned()
        .ok_or_else(|| Error::Mismatch("no journal; run merge first".into()))?;
    if serving.epoch().merged != merge::layout::version() {
        return Err(Error::Mismatch(format!(
            "epoch {} serves {}, not this binary's {}; merge and publish an epoch first",
            serving.epoch().epoch,
            serving.epoch().merged,
            merge::layout::version()
        )));
    }
    if let Some(full) = lines
        .iter()
        .find(|l| l.kind == Kind::Full && l.batch > serving.epoch().batch)
    {
        return Err(Error::Mismatch(format!(
            "batch {} is a full merge since epoch {}; publish an epoch first",
            full.batch,
            serving.epoch().epoch
        )));
    }
    heal_lexical(store, serving, &lines)?;
    serving.refresh()?;
    let watermark = serving.watermark().batch;
    if watermark != last.batch {
        return Err(Error::Mismatch(format!(
            "the serving layer folded to batch {watermark}, the journal is at {}",
            last.batch
        )));
    }
    let merged = store.root().join("merged").join(&serving.epoch().merged);
    let delta_dir = delta::dir(&merged);
    let mut applied = Applied::default();
    for file in delta::FILES {
        let path = delta_dir.join(file);
        let expected = last.delta.get(delta::key(file)).copied().unwrap_or(0);
        let actual = delta::len(&path).map_err(|e| io(&path, e))?;
        if actual > expected {
            delta::truncate(&path, expected).map_err(|e| io(&path, e))?;
            applied.truncated.push(format!(
                "{file}: {} bytes past batch {}",
                actual - expected,
                last.batch
            ));
        }
    }
    let at = ingest::time::now();
    let batch_number = last.batch + 1;
    // First copy speaks, as in the full merge.
    let mut seen: HashSet<(SourceId, OwnedName)> = HashSet::new();
    let mut records = Vec::with_capacity(batch.records.len());
    for r in batch.records {
        if seen.insert((r.source, r.native.clone())) {
            records.push(r);
        } else {
            applied.repeated += 1;
        }
    }
    let batch = Batch { records, ..batch };
    applied.records = batch.records.len() as u64;
    applied.at.clone_from(&at);
    let mut engine = Engine {
        serving,
        batch: &batch,
        index: BatchIndex::of(&batch.records),
        at: at.clone(),
        live: Live::new(serving),
        rows: Rows {
            serving,
            cache: HashMap::new(),
        },
        work_of_batch: HashMap::new(),
        previous: HashMap::new(),
        deleted_in: HashMap::new(),
        batch_sources: HashMap::new(),
        puts: Vec::new(),
        retirements: Vec::new(),
        dropped: Vec::new(),
        gone: HashSet::new(),
        moves: Vec::new(),
        stub_rows: BTreeMap::new(),
        names_dropped: 0,
        applied,
    };
    engine.resolve_all()?;
    let lists = engine.edges(batch_number)?;
    let Engine {
        live,
        puts,
        retirements,
        dropped,
        names_dropped,
        mut applied,
        ..
    } = engine;
    applied.names_dropped = names_dropped;
    applied.dropped = dropped.len() as u64;
    applied.works_put = puts.iter().filter(|(w, _)| !w.stub).count() as u64;
    // Everything of record first, then the deltas, then the clock.
    let registry_path = store.registry();
    Registry::append(&registry_path, &live.lines).map_err(|e| io(&registry_path, e))?;
    let public_lines = mint_public(serving, &live.lines, key, &at);
    let public_path = store.public();
    Public::append(&public_path, &public_lines).map_err(|e| io(&public_path, e))?;
    applied.public_minted = public_lines.len() as u64;
    let mut works = Appender::open(&delta_dir.join(delta::WORKS)).map_err(|e| io(&delta_dir, e))?;
    let mut edges = Appender::open(&delta_dir.join(delta::EDGES)).map_err(|e| io(&delta_dir, e))?;
    let mut relations =
        Appender::open(&delta_dir.join(delta::RELATIONS)).map_err(|e| io(&delta_dir, e))?;
    let mut unresolved =
        Appender::open(&delta_dir.join(delta::UNRESOLVED)).map_err(|e| io(&delta_dir, e))?;
    let mut documents: Vec<Document> = Vec::new();
    let mut removes: Vec<WorkId> = Vec::new();
    for (id, into) in &retirements {
        works
            .put(&WorkLine::Retire {
                batch: batch_number,
                id: *id,
                into: Some(*into),
            })
            .map_err(|e| io(works.path(), e))?;
        edges
            .put(&EdgeLine::Retire {
                batch: batch_number,
                id: *id,
                into: Some(*into),
            })
            .map_err(|e| io(edges.path(), e))?;
        removes.push(*id);
    }
    for id in &dropped {
        works
            .put(&WorkLine::Drop {
                batch: batch_number,
                id: *id,
            })
            .map_err(|e| io(works.path(), e))?;
        removes.push(*id);
    }
    let mut puts = puts;
    puts.sort_by_key(|(w, _)| w.id);
    for (work, text) in &puts {
        if !work.stub {
            documents.push(Document::of(&work.borrow(), text.as_ref()));
        }
        works
            .put(&WorkLine::Put {
                batch: batch_number,
                work: Box::new(work.clone()),
                text: text.clone(),
            })
            .map_err(|e| io(works.path(), e))?;
    }
    for ((from, to), (added, removed)) in &lists.edges {
        if !removed.is_empty() {
            edges
                .put(&EdgeLine::Remove {
                    batch: batch_number,
                    from: *from,
                    to: *to,
                    by: *removed,
                })
                .map_err(|e| io(edges.path(), e))?;
            applied.arcs_removed += removed.len() as u64;
        }
        if !added.is_empty() {
            edges
                .put(&EdgeLine::Add {
                    batch: batch_number,
                    from: *from,
                    to: *to,
                    by: *added,
                })
                .map_err(|e| io(edges.path(), e))?;
            applied.arcs_added += added.len() as u64;
        }
    }
    for line in &lists.relations {
        relations.put(line).map_err(|e| io(relations.path(), e))?;
    }
    applied.relations_put = lists.relations.len() as u64;
    for line in &lists.unresolved {
        unresolved.put(line).map_err(|e| io(unresolved.path(), e))?;
    }
    applied.unresolved_put = lists.unresolved.len() as u64;
    let delta_bytes: BTreeMap<String, u64> = [
        ("works", works.finish().map_err(|e| io(&delta_dir, e))?),
        ("edges", edges.finish().map_err(|e| io(&delta_dir, e))?),
        (
            "relations",
            relations.finish().map_err(|e| io(&delta_dir, e))?,
        ),
        (
            "unresolved",
            unresolved.finish().map_err(|e| io(&delta_dir, e))?,
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let size = |path: &Path| delta::len(path).map_err(|e| io(path, e));
    let line = Line {
        batch: batch_number,
        at: at.clone(),
        kind: Kind::Incremental,
        registry: last.registry + live.lines.len() as u64,
        public: last.public + public_lines.len() as u64,
        delta: delta_bytes,
        bytes: [
            ("registry".to_owned(), size(&registry_path)?),
            ("public".to_owned(), size(&public_path)?),
        ]
        .into_iter()
        .collect(),
        inputs: batch.inputs.clone(),
        records: applied.records,
        minted: applied.minted,
        merged: applied.merged,
        split: 0,
        dropped: applied.names_dropped,
    };
    journal::append(&journal_path, &line).map_err(|e| io(&journal_path, e))?;
    applied.batch = batch_number;
    let search_dir = store.root().join(&serving.epoch().artifacts["search"].dir);
    applied.lexical = search::index::absorb(&search_dir, batch_number, &documents, &removes)?;
    serving.refresh()?;
    Ok(applied)
}
