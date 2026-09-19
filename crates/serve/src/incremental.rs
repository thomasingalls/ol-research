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
use merge::build::{
    self, OwnedAssert, OwnedCitation, OwnedCitedAs, OwnedNamed, OwnedUnresolved, OwnedWork,
};
use merge::build::{Referenced, Target};
use merge::copies::Delivery;
use merge::delta::{
    self, Appender, CitedAsLine, EdgeLine, Related, RelationLine, UnresolvedLine, WorkLine,
};
use merge::identity::{Identity, Standing};
use merge::journal::{self, Kind, Line};
use merge::name::{Assertion, field_static};
use merge::public::{self, Key, Public};
use merge::record::{Citer, Why};
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

impl Incoming {
    /// Another copy of this record that this one speaks over: its names
    /// join this copy's, and its values fill what this copy has none of
    /// (merge plan §8, 2026-09-18). Its citations are appended by the
    /// caller, which knows the order; its relations stay behind, which
    /// the speaking copy alone gives.
    pub fn absorb(&mut self, older: &Self) {
        for a in &older.asserts {
            if !self.asserts.iter().any(|x| x.name == a.name) {
                self.asserts.push(a.clone());
            }
        }
        self.view.fill_from(&older.view);
        if self.text.abstract_text.is_none() {
            self.text
                .abstract_text
                .clone_from(&older.text.abstract_text);
        }
        if self.text.authors.is_empty() {
            self.text.authors.clone_from(&older.text.authors);
        }
        if self.text.venue.is_none() {
            self.text.venue.clone_from(&older.text.venue);
        }
    }

    /// The same, for the copy a row holds, which carries no text of its
    /// own and no relations.
    fn absorb_row(&mut self, older: &crate::row::Record) {
        for a in &older.asserts {
            if !self.asserts.iter().any(|x| x.name == a.name) {
                self.asserts.push(a.clone());
            }
        }
        self.view.fill_from(&older.view);
    }
}

/// One reference of a batch record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    #[allow(missing_docs)]
    pub names: Vec<OwnedName>,
    #[allow(missing_docs)]
    pub citation: OwnedCitation,
}

/// A record a source withdrew, and the deletions set that says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deletion {
    #[allow(missing_docs)]
    pub source: SourceId,
    #[allow(missing_docs)]
    pub native: OwnedName,
    /// The deletions set, named as the copy rule names sets (merge plan
    /// §8): it withdraws the copies in the sets before it.
    pub set: Box<str>,
}

/// One batch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    /// Each record's view names its set, which the copy rule ranks by.
    pub records: Vec<Incoming>,
    /// Copies a newer one supersedes — the row's, since a batch's own are
    /// folded into the copy that speaks — kept for their citations alone,
    /// which are the union over a record's standing copies (merge plan §8,
    /// 2026-09-18). They give no names, fields, relations or text.
    pub cited_only: Vec<Incoming>,
    /// Records the sources withdrew, in the batch's own deletions sets.
    pub deletions: Vec<Deletion>,
    /// The deletions sets the store already holds for the batch's sources:
    /// they withdraw the batch's copies in the sets before theirs, as the
    /// full merge's rule does, and nothing else — the rows already answer
    /// for them.
    pub store_deletions: Vec<Deletion>,
    /// The normalized sets the batch came from, relative to the store,
    /// for the journal.
    pub inputs: Vec<String>,
}

/// A record the batch needs and does not carry: the one whose text a
/// work's promoted field wants, or the one that stays on a work while
/// the batch withdraws or moves its twin of the same source. Every
/// merged row names, per record, the set its speaking copy came from, so
/// the normalized layer can answer — which the binary does, being the one
/// that knows which crate reads a source's sets. Without a reader the
/// batch stops and asks for a full merge, as it did before 2026-09-18.
pub trait Records {
    /// The record `native` of `source`, read from `set`: its names, its
    /// view, its text, its relations and its references, as a batch
    /// carries them. `None` when the set does not hold it.
    ///
    /// # Errors
    ///
    /// The set cannot be read.
    fn record(
        &self,
        source: SourceId,
        native: &OwnedName,
        set: &str,
    ) -> Result<Option<Incoming>, String>;
}

/// No reader: what [`apply`] passes, so a batch stops where the
/// normalized layer alone has what it needs.
#[derive(Debug, Clone, Copy)]
pub struct NoRecords;

impl Records for NoRecords {
    fn record(
        &self,
        _source: SourceId,
        _native: &OwnedName,
        _set: &str,
    ) -> Result<Option<Incoming>, String> {
        Ok(None)
    }
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
    /// Records merged: the copies that speak.
    pub records: u64,
    /// Copies a newer one superseded (merge plan §8): another in the
    /// batch, or the one the record's row holds.
    pub superseded: u64,
    /// Copies of the batch a deletion later in path order withdrew.
    pub withdrawn: u64,
    /// Deletions that withdrew nothing: the copy speaking for the record
    /// is not before the deletion.
    pub deletions_not_applying: u64,
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
    /// Arcs of an earlier copy of a re-delivered record that its new
    /// copy no longer names, kept: a record's citations are the union
    /// over its copies (merge plan §8, 2026-09-18).
    pub arcs_kept: u64,
    /// Copies in the batch that a row's newer copy supersedes, read for
    /// their citations alone.
    pub cited_only: u64,
    /// Arcs moved because a name this batch gave to a work outranks the
    /// name that had landed an older reference (serving plan §11, item
    /// 26).
    pub references_moved: u64,
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
    /// The names this batch gave to a work or a stub: what a reference
    /// that gave one beside a held work must be resolved against again
    /// (serving plan §11, item 26).
    fn attached(&self) -> Vec<OwnedName> {
        let mut names: Vec<OwnedName> = self
            .lines
            .iter()
            .flat_map(|line| match line {
                registry::Line::Mint { names, .. } => names.clone(),
                registry::Line::Name { name, .. } => vec![name.clone()],
                _ => Vec::new(),
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        names
    }

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

/// Every name a reference gave beside the held work it landed on: the
/// rows of the `cited_as` set, as the full merge's second pass writes
/// them (merge plan §11, item 14) — who cited each name, and for a name
/// nothing holds, the work its reference landed on.
/// Whether a reference names a held work besides the stub it landed on:
/// then it makes no stub name of its own, its other names being
/// assertions about that work (merge plan §11, item 14).
fn held_beside(
    live: &Live<'_>,
    serving: &Serving,
    table: &HashMap<OwnedName, Target>,
    names: &[OwnedName],
) -> bool {
    names
        .iter()
        .any(|n| matches!(target_of(live, serving, table, n), Target::Work(_, false)))
}

/// Whether a citation text repeats a name. Only a DOI is ever written
/// out beside the identifier it names (merge plan §6), so every other
/// namespace is false.
fn repeats(text: Option<&str>, name: &OwnedName) -> bool {
    match name.as_name() {
        merge::Name::Doi(doi) => text.is_some_and(|t| build::repeats(t, doi)),
        _ => false,
    }
}

fn beside_rows(
    out: &mut Vec<OwnedCitedAs>,
    names: &[OwnedName],
    from: WorkId,
    by: SourceId,
    to: WorkId,
    text: Option<&str>,
    target: impl Fn(&OwnedName) -> Target,
) {
    let Some(landed) = names
        .iter()
        .filter(|n| target(n) == Target::Work(to, false))
        .map(|n| n.as_name().key())
        .min_by_key(|k| build::rank(*k))
    else {
        return;
    };
    // Whether the citation text backs the landing, and each name.
    let backs = |name: &OwnedName| repeats(text, name);
    let by_text = names
        .iter()
        .any(|n| backs(n) && matches!(target(n), Target::Work(w, _) if w == to));
    for name in names {
        out.push(OwnedCitedAs {
            from,
            by,
            name: name.clone(),
            to,
            landed,
            // Settled once every reference of the pair is read.
            sole: true,
            relation: false,
            beside: target(name) == Target::Unknown,
            by_text,
            in_text: backs(name),
        });
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
    /// Records neither the batch nor a row has, from the normalized layer.
    layer: &'s dyn Records,
    /// The copies in [`Batch::cited_only`] by source and native id: the
    /// row's copy speaks over them, and their names and values join the
    /// row's record (merge plan §8, 2026-09-18).
    older: BatchIndex,
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
    /// The (work, source) pairs whose one record of that source the batch
    /// replaces or withdraws: the rows' evidence of their citations is the
    /// batch's to restate, and no rule leans on it.
    /// Pairs whose record this batch withdraws ([`Engine::withdrawn_pairs`]).
    replaced: HashSet<(WorkId, SourceId)>,
    names_dropped: u64,
    applied: Applied,
}

impl Engine<'_> {
    /// A row's record as the rule leaves it (merge plan §8, 2026-09-18):
    /// with the names of the batch's older copy of the same record, and
    /// its values wherever the row's copy has none. Absence is not an
    /// assertion, so a copy delivered out of order still gives what the
    /// newer one never had.
    fn joined(&self, record: &crate::row::Record) -> (Vec<OwnedAssert>, Owned) {
        let mut asserts: Vec<OwnedAssert> = record.asserts.to_vec();
        let mut view = record.view.clone();
        if let Some(i) = self.older.get(record.source, &record.native) {
            let older = &self.batch.cited_only[i];
            for a in &older.asserts {
                if !asserts.iter().any(|x| x.name == a.name) {
                    asserts.push(a.clone());
                }
            }
            view.fill_from(&older.view);
        }
        (asserts, view)
    }

    /// The records of `work` and `source` other than `except`, as a batch
    /// would carry them: the batch's own where it has them, else read
    /// from the normalized layer, the row naming the set each speaking
    /// copy came from (serving plan §5, 2026-09-18). Empty when the work
    /// holds no other record of that source — the usual case, where a
    /// withdrawn or moved record takes the pair's whole list with it.
    fn twins(
        &mut self,
        work: WorkId,
        source: SourceId,
        except: &OwnedName,
    ) -> Result<Vec<Incoming>, Error> {
        let Some(row) = self.rows.get(work)? else {
            return Ok(Vec::new());
        };
        let others: Vec<crate::row::Record> = row
            .records
            .iter()
            .filter(|x| x.source == source && x.native != *except)
            .cloned()
            .collect();
        let mut out = Vec::with_capacity(others.len());
        for x in others {
            if let Some(i) = self.index.get(x.source, &x.native) {
                out.push(self.batch.records[i].clone());
                continue;
            }
            let read = self
                .layer
                .record(x.source, &x.native, &x.view.set)
                .map_err(Error::FullMerge)?;
            let Some(record) = read else {
                return Err(Error::FullMerge(format!(
                    "{}={} stays on {work} while the batch withdraws or moves another record of its source, and only the normalized layer has it",
                    x.source, x.native
                )));
            };
            out.push(record);
        }
        Ok(out)
    }

    /// Who cited `name` while it belonged to `work`, from the cited-as
    /// evidence (serving plan §11, item 28): the citing work and the
    /// source of the record that cited it, each pair once, those whose
    /// arc is gone or whose record this batch restates left out.
    fn citers_of(&self, name: &OwnedName, work: WorkId) -> Vec<Citer> {
        let mut out: Vec<Citer> = Vec::new();
        for row in self.serving.cited_as(name) {
            if row.relation || self.survivor_of(row.to) != work {
                continue;
            }
            let pair = (self.survivor_of(row.from), row.by);
            let citer = Citer {
                from: pair.0,
                by: row.by,
                in_text: row.in_text,
                // The reference landed on this work, which is held; what
                // else it named is item 26's question, not this one.
                held_beside: false,
            };
            if self.replaced.contains(&pair) || self.gone.contains(&pair.0) {
                continue;
            }
            if !self
                .serving
                .references(pair.0)
                .any(|a| self.survivor_of(a.work) == work && a.by.contains(pair.1))
            {
                continue;
            }
            out.push(citer);
        }
        // One record's several references to the work are one citer, the
        // text mark theirs together, as a stub's own row has it.
        build::join_citers(&mut out);
        out
    }

    /// Whether a relation of a record still names `name` while it is
    /// `work`'s: a relation makes no arc and no citer, but a name it
    /// names is a work of its own once it leaves this one, as the full
    /// merge's identity pass has it (merge plan §6).
    fn related_to(&self, name: &OwnedName, work: WorkId) -> bool {
        self.serving
            .cited_as(name)
            .iter()
            .any(|row| row.relation && self.survivor_of(row.to) == work)
    }

    fn touch(&mut self, name: &OwnedName) -> Vec<WorkId> {
        match self.live.resolved(name) {
            Some(Resolved::One(id)) => vec![id],
            Some(Resolved::Shared(ids)) => ids,
            Some(Resolved::Split(_)) | None => Vec::new(),
        }
    }

    /// Where an id retired in this batch went, following the chain.
    fn survivor_of(&self, id: WorkId) -> WorkId {
        let mut at = id;
        while let Some((_, into)) = self.retirements.iter().find(|(from, _)| *from == at) {
            if *into == at {
                break;
            }
            at = *into;
        }
        at
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
        // An older copy's work, so that its names join the row's record.
        let older: Vec<OwnedName> = self
            .batch
            .cited_only
            .iter()
            .flat_map(|r| r.asserts.iter().map(|a| a.name.clone()))
            .collect();
        for name in &older {
            queue.extend(self.touch(name));
        }
        let deletions = self.batch.deletions.clone();
        for Deletion { source, native, .. } in &deletions {
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
                        let (asserts, view) = self.joined(record);
                        push(&mut identity, record.source, &asserts, &view)?;
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
        let mut handled: HashSet<OwnedName> = HashSet::new();
        for (id, row) in &existing {
            if assigned.contains(id) || merged.contains(id) {
                continue;
            }
            if self.serving.citers(*id).next().is_none() {
                dropped.push(*id);
                continue;
            }
            // The work lost its last record and is still cited: it keeps
            // its id and becomes the stub its citers' references name,
            // each name with the works that cited it, which the cited-as
            // evidence says (serving plan §11, item 28, 2026-09-18). Its
            // citers' arcs already point here and do not move.
            let mut names: BTreeMap<OwnedName, Vec<Citer>> = BTreeMap::new();
            for n in &row.names {
                let citers = self.citers_of(&n.name, *id);
                if !citers.is_empty() || self.related_to(&n.name, *id) {
                    names.insert(n.name.clone(), citers);
                }
            }
            if names.is_empty() {
                return Err(Error::FullMerge(format!(
                    "{id} lost its last record and is still cited, and no name of its own says who cited it; its stub's names are the citing references'"
                )));
            }
            for n in &row.names {
                // Kept or dropped, the stub's row answers for the name.
                handled.insert(n.name.clone());
                if names.contains_key(&n.name) {
                    continue;
                }
                if self.live.lookup(&n.name) == Some(*id) {
                    let line = registry::Line::Drop {
                        name: n.name.clone(),
                        at: self.at.clone(),
                    };
                    self.live.apply(&line).map_err(Error::Mismatch)?;
                    self.names_dropped += 1;
                }
            }
            self.stub_rows.insert(
                *id,
                StubRow {
                    names,
                    minted: false,
                    changed: true,
                },
            );
        }
        // A stub a held work took over, by reusing its id or by taking it
        // in a merge: the names the work does not hold leave it — to a
        // stub of their own where references cite them without a kept
        // name, which the row's evidence says — and the arcs of those
        // references follow.
        for (id, row) in &existing {
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
            // A stub's row says who cited each of its names; a held
            // work's names are answered by the cited-as evidence, which
            // says the same for every name a reference gave (serving plan
            // §11, item 28, 2026-09-18).
            let cited_by = |this: &Self, n: &crate::row::Named| -> Vec<Citer> {
                if row.stub {
                    n.cited_by.to_vec()
                } else {
                    this.citers_of(&n.name, *id)
                }
            };
            // Each citing record's own reference, as the names of this
            // row divide: what the record kept, and what it left. A
            // split is that reference resolving again, so it is settled
            // the way §6 settles one — the text first (serving plan §11,
            // item 28, 2026-09-19).
            let mut gave: BTreeMap<(WorkId, SourceId), Split> = BTreeMap::new();
            for n in &row.names {
                let side = kept.contains(&n.name);
                for c in cited_by(self, n) {
                    let split = gave.entry(c.pair()).or_default();
                    let side = if side {
                        &mut split.kept
                    } else {
                        &mut split.left
                    };
                    side.push((n.name.clone(), c.in_text));
                }
            }
            let mut leftovers: Vec<(OwnedName, Vec<Citer>)> = Vec::new();
            for n in &row.names {
                if kept.contains(&n.name) || handled.contains(&n.name) {
                    continue;
                }
                handled.insert(n.name.clone());
                // A leftover name is a name of its own only where some
                // reference gave it with no name the record kept: a
                // reference naming a held work makes no stub, its other
                // names being assertions about that work (merge plan §11,
                // item 14) — the rule `Classified::names_a_stub` states
                // for the batch's own references. Where the arc goes is
                // a second question, and §6's; it is asked below, once
                // the names that survive are known.
                let mut movers: Vec<Citer> = Vec::new();
                for c in cited_by(self, n) {
                    let pair = c.pair();
                    if self.replaced.contains(&pair) {
                        continue;
                    }
                    let split = gave.get(&pair).ok_or_else(|| {
                        Error::Mismatch(format!("{pair:?} cited {} of no row", n.name))
                    })?;
                    if split.kept.is_empty() && !c.held_beside {
                        movers.push(c);
                    }
                }
                if movers.is_empty() && !row.stub && self.related_to(&n.name, *id) {
                    // No arc to move, but a relation still names it, so
                    // it is a work of its own: a stub with no citer, as
                    // the full merge leaves one (§11, item 28).
                    leftovers.push((n.name.clone(), Vec::new()));
                } else if movers.is_empty() {
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
                    if leftovers[i]
                        .1
                        .iter()
                        .any(|c| leftovers[j].1.iter().any(|o| o.pair() == c.pair()))
                    {
                        let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                        if a != b {
                            parent[a.max(b)] = a.min(b);
                        }
                    }
                }
            }
            let mut by_root: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for i in 0..leftovers.len() {
                let r = root(&mut parent, i);
                by_root.entry(r).or_default().push(i);
            }
            let groups: Vec<Vec<usize>> = by_root.into_values().collect();
            let mut group_of: HashMap<&OwnedName, usize> = HashMap::new();
            for (g, members) in groups.iter().enumerate() {
                for i in members {
                    group_of.insert(&leftovers[*i].0, g);
                }
            }
            // A reference that also named a name the record kept makes no
            // stub name, but it still resolves again, and §6 may give it
            // one of the names the record left — then its arc follows
            // that name to the stub the name became. This is the whole of
            // the divergence item 26 fixes for a held landing (serving
            // plan §11, item 28, 2026-09-19).
            let made: HashSet<(WorkId, SourceId)> = leftovers
                .iter()
                .flat_map(|(_, movers)| movers.iter().map(|c| c.pair()))
                .collect();
            let mut joined: Vec<Vec<(WorkId, SourceId)>> = vec![Vec::new(); groups.len()];
            for (pair, split) in &gave {
                if made.contains(pair) || self.replaced.contains(pair) {
                    continue;
                }
                if let Some(g) = split_arc(split, *id, &group_of)? {
                    joined[g].push(*pair);
                }
            }
            for (g, members) in groups.iter().enumerate() {
                let mut names: BTreeMap<OwnedName, Vec<Citer>> = BTreeMap::new();
                for i in members {
                    let (name, movers) = &leftovers[*i];
                    names.insert(name.clone(), movers.clone());
                }
                // A reference §6 sends here cites every name of this stub
                // it gave, as the full merge's own reference pass has it.
                for pair in &joined[g] {
                    for (name, in_text) in &gave[pair].left {
                        if let Some(citers) = names.get_mut(name) {
                            citers.push(Citer {
                                from: pair.0,
                                by: pair.1,
                                in_text: *in_text,
                                // It made no name of this stub, and will
                                // not of the next split either.
                                held_beside: true,
                            });
                        }
                    }
                }
                for citers in names.values_mut() {
                    build::join_citers(citers);
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
                let mut moved: HashSet<(WorkId, SourceId)> = HashSet::new();
                for movers in names.values() {
                    for c in movers {
                        if moved.insert(c.pair()) {
                            self.moves.push((c.from, c.by, *id, stub_id));
                        }
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
                Origin::Row(id, j) => existing.get(id).map(|r| self.joined(&r.records[*j]).1),
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
            let supplied = match supplied {
                Some(text) => text,
                // The row kept another source's text and the batch does
                // not carry this record: its speaking copy's set does.
                None => self
                    .layer
                    .record(record.source, &record.native, &record.view.set)
                    .map_err(Error::FullMerge)?
                    .map(|r| r.text)
                    .ok_or_else(|| {
                        Error::FullMerge(format!(
                            "the {field} of {} comes from {source}'s record {}, whose text only the normalized layer has",
                            work.id, record.native
                        ))
                    })?,
            };
            match field {
                "abstract" => text.abstract_text = supplied.abstract_text,
                "authors" => text.authors = supplied.authors,
                _ => text.venue = supplied.venue,
            }
        }
        Ok(text)
    }

    /// The (work, source) pairs whose one record of that source this batch
    /// withdraws. A re-delivered record is not among them since
    /// 2026-09-18: its earlier copy's citations stand, so its citations of
    /// a stub stand too. A work holding two records of the source keeps
    /// the pair, its arcs being their union.
    fn withdrawn_pairs(&mut self) -> Result<HashSet<(WorkId, SourceId)>, Error> {
        let candidates: Vec<(WorkId, SourceId)> =
            self.deleted_in.iter().map(|((s, _), w)| (*w, *s)).collect();
        let mut pairs = HashSet::new();
        for (w, s) in candidates {
            let Some(row) = self.rows.get(w)? else {
                continue;
            };
            if row.records.iter().filter(|x| x.source == s).count() == 1 {
                pairs.insert((w, s));
            }
        }
        Ok(pairs)
    }

    /// Runs the rules over every component.
    fn resolve_all(&mut self) -> Result<(), Error> {
        let groups = self.components()?;
        self.replaced = self.withdrawn_pairs()?;
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
                .map(|n| (n.name.clone(), n.cited_by.to_vec()))
                .collect(),
            minted: false,
            changed: false,
        })
    }
}

/// One citing record's reference as a split divides it: the names it
/// gave that the record kept, and those it left, each with whether the
/// reference's own citation text repeats that name.
#[derive(Default)]
struct Split {
    kept: Vec<(OwnedName, bool)>,
    left: Vec<(OwnedName, bool)>,
}

/// Where §6 sends a reference that named both what a record kept and
/// what it left: the group of leftover names its arc moves to, or `None`
/// where a name the record kept wins and the arc stays.
///
/// Only a leftover name that became some group's is a candidate — a name
/// no reference gave without a kept name beside it is dropped (merge plan
/// §11, item 14), and a dropped name names nothing to resolve to. Among
/// the candidates the rule is [`merge::build::resolve_reference`]'s: a
/// DOI the citation text repeats beats the order, and the order decides
/// where nothing corroborates.
///
/// The row's own names are the whole contest, which is what lets the
/// batch settle this at all. §6 picks a *name* and then follows it to a
/// work; a split changes no name's namespace and no name's text mark,
/// only which work a name names, so the name §6 picked before the split
/// is the name it picks after — and that name was this row's, since the
/// reference landed here. The two shapes where §6 would have to reach
/// past the row are refused rather than guessed (serving plan §5).
fn split_arc(
    split: &Split,
    id: WorkId,
    group_of: &HashMap<&OwnedName, usize>,
) -> Result<Option<usize>, Error> {
    let left: Vec<(&OwnedName, bool, usize)> = split
        .left
        .iter()
        .filter_map(|(n, t)| group_of.get(n).map(|g| (n, *t, *g)))
        .collect();
    if left.is_empty() {
        if split.kept.is_empty() {
            // The reference named a held work and §6 landed it here on
            // its text instead; now the names it gave here are dropped,
            // so §6 falls back to an order over names the row never
            // held, and the arc leaves for a work the row cannot name.
            return Err(Error::FullMerge(format!(
                "a reference to {id} named a held work too, and every name it gave here is dropped: where its arc goes is an order over names the row does not hold"
            )));
        }
        return Ok(None);
    }
    // Only a DOI is ever repeated in a citation text (§6), so a marked
    // name is one `corroborated` would have found.
    let marked: Vec<&(&OwnedName, bool, usize)> = left.iter().filter(|(_, t, _)| *t).collect();
    let one = |of: &[&(&OwnedName, bool, usize)], why: &str| -> Result<Option<usize>, Error> {
        let g = of[0].2;
        if of.iter().any(|c| c.2 != g) {
            return Err(Error::FullMerge(format!(
                "a reference to {id} {why}, which the split puts in different works"
            )));
        }
        Ok(Some(g))
    };
    if !marked.is_empty() {
        if split.kept.iter().any(|(_, t)| *t) {
            // §6 gives the text up when two DOIs it repeats name two
            // works, and falls back to an order over every name of the
            // reference — names this row does not hold included.
            return Err(Error::FullMerge(format!(
                "the text of a reference to {id} repeats a DOI of each side of the split, which leaves §6 to the order over names the row does not hold"
            )));
        }
        return one(&marked, "repeats two of the DOIs it left in its own text");
    }
    if split.kept.iter().any(|(_, t)| *t) {
        return Ok(None);
    }
    // Nothing corroborates: the lowest namespace decides.
    let rank = |n: &OwnedName| build::rank(n.key());
    let best = left
        .iter()
        .map(|(n, _, _)| rank(n))
        .min()
        .unwrap_or(u8::MAX);
    match split.kept.iter().map(|(n, _)| rank(n)).min() {
        Some(k) if k < best => Ok(None),
        // One namespace on each side: §6 takes the first such name the
        // reference gave, and no row keeps a reference's order.
        Some(k) if k == best => Err(Error::FullMerge(format!(
            "a reference to {id} gave one namespace on each side of the split, which the order alone does not settle"
        ))),
        _ => {
            let lowest: Vec<&(&OwnedName, bool, usize)> =
                left.iter().filter(|(n, _, _)| rank(n) == best).collect();
            one(&lowest, "gave one namespace twice among the names it left")
        }
    }
}

/// A stub's row while the batch makes it: each name with its citers, the
/// works whose arcs landed on the stub through it and the source of each
/// citing record — which only the arcs know, so the row is written after
/// them (serving plan §11, item 21). A name's sources are its citers'.
struct StubRow {
    names: BTreeMap<OwnedName, Vec<Citer>>,
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
    /// ids and unknown names become one stub. Who cites it is the arcs'
    /// to say, in pass two.
    fn cite(&mut self, stubs: &[(OwnedName, WorkId)], unknown: &[OwnedName]) {
        let mut first: Option<usize> = None;
        for (_, id) in stubs {
            let ix = self.node_of_id(*id);
            match first {
                None => first = Some(ix),
                Some(f) => self.union(f, ix),
            }
        }
        for name in unknown {
            let ix = self.node_of_name(name);
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

/// A stub's row from its names, each with its citers: one promoted per
/// namespace in name order, the rest aliases, and a name's sources its
/// citers' — the full merge's rule for a stub.
fn stub_row(id: WorkId, names: BTreeMap<OwnedName, Vec<Citer>>) -> OwnedWork {
    let mut seen: Vec<ingest::Key> = Vec::new();
    let rows = names
        .into_iter()
        .map(|(name, mut cited_by)| {
            let key = name.key();
            let standing = if seen.contains(&key) {
                Standing::Alias
            } else {
                seen.push(key);
                Standing::Promoted
            };
            build::join_citers(&mut cited_by);
            let by: Sources = cited_by.iter().map(|c| c.by).collect();
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
    cited_as: Vec<CitedAsLine>,
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
    /// The stubs' rows, now that the arcs say which works cite each name:
    /// the citations the batch takes back removed, a retired citing work's
    /// restated under its survivor, then the landings added. A stub the
    /// batch did not settle is read here for the first time; a stub a
    /// record took over is a held work now, whose names keep no citers.
    fn write_stub_rows(
        &mut self,
        landed: BTreeMap<WorkId, BTreeMap<OwnedName, Vec<Citer>>>,
        stale: &[(WorkId, (WorkId, SourceId))],
        renamed: &[(WorkId, WorkId, WorkId, Sources)],
    ) -> Result<(), Error> {
        let held: HashSet<WorkId> = self
            .puts
            .iter()
            .filter(|(w, _)| !w.stub)
            .map(|(w, _)| w.id)
            .collect();
        // The stub a name of `t` belongs to now, if a stub: `t` itself or
        // the stub it was merged into by this batch.
        let stub_now = |this: &Self, t: WorkId| -> Option<WorkId> {
            let id = this.live.live(t)?;
            (!held.contains(&id) && (this.stub_rows.contains_key(&id) || this.serving.is_stub(id)))
                .then_some(id)
        };
        for (t, pair) in stale {
            let Some(id) = stub_now(self, *t) else {
                continue;
            };
            let mut stub = self.stub_row_of(id)?;
            for citers in stub.names.values_mut() {
                let before = citers.len();
                citers.retain(|c| c.pair() != *pair);
                stub.changed |= citers.len() != before;
            }
            self.stub_rows.insert(id, stub);
        }
        for (t, from, into, sources) in renamed {
            let Some(id) = stub_now(self, *t) else {
                continue;
            };
            let mut stub = self.stub_row_of(id)?;
            for citers in stub.names.values_mut() {
                for c in citers.iter_mut() {
                    if c.from == *from && sources.contains(c.by) {
                        c.from = *into;
                        stub.changed = true;
                    }
                }
            }
            self.stub_rows.insert(id, stub);
        }
        for (id, by_name) in landed {
            let mut stub = self.stub_row_of(id)?;
            for (name, pairs) in by_name {
                let Some(citers) = stub.names.get_mut(&name) else {
                    return Err(Error::Mismatch(format!(
                        "an arc landed on stub {id} through {name}, which is not its name"
                    )));
                };
                // A record's several references to this name are one
                // citer, and their marks join — the text mark theirs
                // together, the held-beside mark only where every one of
                // them named a held work. Keeping the first and dropping
                // the rest would not be the full merge's answer.
                let before = citers.clone();
                citers.extend(pairs);
                build::join_citers(citers);
                stub.changed |= *citers != before;
            }
            self.stub_rows.insert(id, stub);
        }
        for (id, stub) in std::mem::take(&mut self.stub_rows) {
            if !stub.changed {
                continue;
            }
            let row = stub_row(id, stub.names);
            // A citation taken back and restated leaves the row as it was.
            if !stub.minted
                && let Some(before) = self.rows.get(id)?
                && before.stub
                && before.names.as_slice() == row.names.as_slice()
            {
                continue;
            }
            self.puts.push((row, None));
            self.applied.stubs_put += 1;
        }
        Ok(())
    }

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
        for r in &self.batch.cited_only {
            for reference in &r.references {
                let c = classify(self, &reference.names)?;
                if c.names_a_stub() {
                    stubs.cite(&c.stubs, &c.unknown);
                }
            }
        }
        for r in &self.batch.records {
            for reference in &r.references {
                let c = classify(self, &reference.names)?;
                if c.names_a_stub() {
                    stubs.cite(&c.stubs, &c.unknown);
                }
            }
            for relation in &r.relations {
                if let Some(target) = &relation.target {
                    let c = classify(self, std::slice::from_ref(target))?;
                    if c.names_a_stub() {
                        stubs.cite(&c.stubs, &c.unknown);
                    }
                }
            }
        }
        // Stubs settled: survivors, merges, mints. Their rows wait for pass
        // two, whose arcs say which works cite each name.
        for (ids, new_names) in stubs.groups() {
            let mut names: BTreeMap<OwnedName, Vec<Citer>> = BTreeMap::new();
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
                        for (name, cited_by) in stub.names {
                            names.entry(name).or_default().extend(cited_by);
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
                names.insert(name.clone(), Vec::new());
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
        // Records a work keeps of a source whose other record this batch
        // withdraws or moves: the pair's arcs, relations, unresolved rows
        // and cited-as evidence are theirs, not empty (serving plan §5,
        // 2026-09-18). Read before pass two, which holds the tables.
        let mut twins: HashMap<(WorkId, SourceId), Vec<Incoming>> = HashMap::new();
        for ((source, native), work) in self.deleted_in.clone() {
            let kept = self.twins(work, source, &native)?;
            if !kept.is_empty() {
                twins.insert((work, source), kept);
            }
        }
        let moved: Vec<(WorkId, SourceId, OwnedName)> = self
            .batch
            .records
            .iter()
            .enumerate()
            .filter_map(|(i, r)| {
                let work = *self.work_of_batch.get(&i)?;
                let (old_work, _) = *self.previous.get(&(r.source, r.native.clone()))?;
                (old_work != work).then(|| (old_work, r.source, r.native.clone()))
            })
            .collect();
        for (work, source, native) in moved {
            let kept = self.twins(work, source, &native)?;
            if !kept.is_empty() {
                twins.insert((work, source), kept);
            }
        }
        // Pass two: every reference through the final table.
        let table = self.table();
        // Names the batch's references give beside the works they land
        // on, whole per citing work and source, with how many references
        // of that pair landed on each work — what says whether an arc
        // would move whole (merge plan §11, item 14, 2026-09-18).
        let mut beside: Vec<OwnedCitedAs> = Vec::new();
        let mut landings: BTreeMap<(WorkId, SourceId, WorkId), u32> = BTreeMap::new();
        let mut put_cited_as: BTreeSet<(WorkId, SourceId)> = BTreeSet::new();
        // Per stub, per name, the works whose arcs landed on the stub
        // through that name: a reference that named a held work beside it
        // too, when the registry's order chose the stub, as the full merge
        // counts it.
        let mut landed: BTreeMap<WorkId, BTreeMap<OwnedName, Vec<Citer>>> = BTreeMap::new();
        // Citations the batch takes back (serving plan §11, item 23): a
        // stub a replaced or withdrawn record's arc pointed at, and the
        // (work, source) whose citation it was, removed from every name of
        // the stub before the landings restate what still cites it.
        let mut stale: Vec<(WorkId, (WorkId, SourceId))> = Vec::new();
        let mut lists = Lists::default();
        let (live, serving) = (&self.live, self.serving);
        let resolver = |name: merge::Name<'_>| -> Target {
            target_of(live, serving, &table, &name.to_owned())
        };
        // What the records a work keeps of a source still give, through
        // the final table: the arcs they land on, their relations, their
        // unresolved rows, and the names they gave beside a landing.
        let kept_of = |records: &[Incoming],
                       work: WorkId,
                       beside: &mut Vec<OwnedCitedAs>,
                       landings: &mut BTreeMap<(WorkId, SourceId, WorkId), u32>|
         -> (BTreeSet<WorkId>, Vec<Related>, Vec<OwnedUnresolved>) {
            let mut arcs: BTreeSet<WorkId> = BTreeSet::new();
            let mut relations: Vec<Related> = Vec::new();
            let mut rows: Vec<OwnedUnresolved> = Vec::new();
            for r in records {
                for reference in &r.references {
                    let names: Vec<merge::Name<'_>> =
                        reference.names.iter().map(OwnedName::as_name).collect();
                    let referenced =
                        build::resolve_reference(&names, &reference.citation.borrow(), resolver);
                    let why = match referenced {
                        Referenced::Empty | Referenced::Unknown => None,
                        Referenced::NoName => Some(Why::NoName),
                        Referenced::SharedOnly => Some(Why::Shared),
                        Referenced::Edge { to, settled, stub } => {
                            arcs.insert(to);
                            if !stub {
                                *landings.entry((work, r.source, to)).or_default() += 1;
                                beside_rows(
                                    beside,
                                    &reference.names,
                                    work,
                                    r.source,
                                    to,
                                    reference.citation.text.as_deref(),
                                    |name| target_of(live, serving, &table, name),
                                );
                            }
                            (settled != build::Settled::Agreed).then_some(Why::Disagree)
                        }
                    };
                    if let Some(why) = why {
                        rows.push(OwnedUnresolved {
                            from: work,
                            by: r.source,
                            why,
                            names: reference.names.iter().cloned().collect(),
                            citation: reference.citation.clone(),
                        });
                    }
                }
                for rel in &r.relations {
                    if let Some(target) = rel.target.as_ref()
                        && let Target::Work(to, stub) = target_of(live, serving, &table, target)
                    {
                        relations.push(Related { to, kind: rel.kind });
                        if !stub {
                            beside.push(OwnedCitedAs {
                                from: work,
                                by: r.source,
                                name: target.clone(),
                                to,
                                landed: target.as_name().key(),
                                sole: true,
                                relation: true,
                                beside: false,
                                by_text: false,
                                in_text: false,
                            });
                        }
                    }
                }
            }
            relations.sort_unstable();
            relations.dedup();
            (arcs, relations, rows)
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
                let referenced =
                    build::resolve_reference(&names, &reference.citation.borrow(), resolver);
                *self
                    .applied
                    .references
                    .entry(referenced.outcome(work))
                    .or_default() += 1;
                let why = match referenced {
                    Referenced::Empty | Referenced::Unknown => None,
                    Referenced::NoName => Some(Why::NoName),
                    Referenced::SharedOnly => Some(Why::Shared),
                    Referenced::Edge { to, settled, stub } => {
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
                                        .push(Citer {
                                            from: work,
                                            by: r.source,
                                            in_text: repeats(
                                                reference.citation.text.as_deref(),
                                                name,
                                            ),
                                            held_beside: held_beside(
                                                live,
                                                serving,
                                                &table,
                                                &reference.names,
                                            ),
                                        });
                                }
                            }
                        } else {
                            *landings.entry((work, r.source, to)).or_default() += 1;
                            beside_rows(
                                &mut beside,
                                &reference.names,
                                work,
                                r.source,
                                to,
                                reference.citation.text.as_deref(),
                                |name| target_of(live, serving, &table, name),
                            );
                        }
                        (settled != build::Settled::Agreed).then_some(Why::Disagree)
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
            let mut relations: Vec<Related> = Vec::new();
            for rel in &r.relations {
                let Some(target) = rel.target.as_ref() else {
                    continue;
                };
                let Target::Work(to, stub) = target_of(live, serving, &table, target) else {
                    continue;
                };
                relations.push(Related { to, kind: rel.kind });
                if !stub {
                    // A relation names a held work: kept beside it like a
                    // reference's names, since it says the name is still
                    // named if it leaves (serving plan §11, item 28).
                    beside.push(OwnedCitedAs {
                        from: work,
                        by: r.source,
                        name: target.clone(),
                        to,
                        landed: target.as_name().key(),
                        sole: true,
                        relation: true,
                        beside: false,
                        by_text: false,
                        in_text: false,
                    });
                    put_cited_as.insert((work, r.source));
                }
            }
            relations.sort_unstable();
            relations.dedup();
            // The previous list of this record's source, on the work the
            // record was in before.
            let key = (r.source, r.native.clone());
            let previous = self.previous.get(&key).map(|(id, _)| *id);
            let by: Sources = [r.source].into_iter().collect();
            if let Some(old_work) = previous {
                let old_arcs: BTreeSet<WorkId> = self
                    .serving
                    .references(old_work)
                    .filter(|a| a.by.contains(r.source))
                    .map(|a| a.work)
                    .collect();
                if old_work == work {
                    // A record's citations are the union over its copies
                    // (merge plan §8, 2026-09-18): a later copy adds arcs
                    // and takes none away, because PubMed swaps a
                    // publisher's reference list for PMC's when an article
                    // lands there and the ids the publisher gave are
                    // evidence we already hold.
                    self.applied.arcs_kept += old_arcs.difference(&new_arcs).count() as u64;
                    for to in new_arcs.difference(&old_arcs) {
                        lists.add(work, *to, by);
                    }
                } else {
                    // The record moved: its citations, the earlier copies'
                    // included, move with it. What a record of its source
                    // the work keeps still gives stays behind, and that
                    // record restates the pair it leaves (2026-09-18).
                    let kept = twins.get(&(old_work, r.source)).map_or_else(
                        || (BTreeSet::new(), Vec::new(), Vec::new()),
                        |records| kept_of(records, old_work, &mut beside, &mut landings),
                    );
                    let (kept_arcs, kept_relations, kept_rows) = kept;
                    // As in the deletions loop: an arc whose target this
                    // batch retired is the kept record's under its new
                    // id, and stays behind — the retirement moves it.
                    for to in old_arcs
                        .iter()
                        .filter(|to| !kept_arcs.contains(&self.survivor_of(**to)))
                    {
                        lists.remove(old_work, *to, by);
                        lists.add(work, *to, by);
                        if self.serving.is_stub(*to) {
                            stale.push((*to, (old_work, r.source)));
                        }
                    }
                    for to in new_arcs.difference(&old_arcs) {
                        lists.add(work, *to, by);
                    }
                    lists.relations.push(RelationLine::Put {
                        batch: batch_number,
                        from: old_work,
                        by: r.source,
                        relations: kept_relations,
                    });
                    lists.unresolved.push(UnresolvedLine::Put {
                        batch: batch_number,
                        from: old_work,
                        by: r.source,
                        rows: kept_rows,
                    });
                    if twins.contains_key(&(old_work, r.source)) {
                        put_cited_as.insert((old_work, r.source));
                    }
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
            put_cited_as.insert((work, r.source));
        }
        // Copies the row's copy supersedes: their citations stand, so
        // their arcs are added where the record's work is, and nothing of
        // theirs is removed (merge plan §8, 2026-09-18).
        for r in &self.batch.cited_only {
            let Some(Resolved::One(work)) = self.live.resolved(&r.native) else {
                continue;
            };
            let work = self
                .retirements
                .iter()
                .find(|(id, _)| *id == work)
                .map_or(work, |(_, into)| *into);
            let by: Sources = [r.source].into_iter().collect();
            let old_arcs: BTreeSet<WorkId> = self
                .serving
                .references(work)
                .filter(|a| a.by.contains(r.source))
                .map(|a| a.work)
                .collect();
            for reference in &r.references {
                let names: Vec<merge::Name<'_>> =
                    reference.names.iter().map(OwnedName::as_name).collect();
                let referenced =
                    build::resolve_reference(&names, &reference.citation.borrow(), resolver);
                if let Referenced::Edge { to, stub, .. } = referenced {
                    if !old_arcs.contains(&to) {
                        lists.add(work, to, by);
                    }
                    if stub {
                        for name in &reference.names {
                            if target_of(live, serving, &table, name) == Target::Work(to, true) {
                                landed
                                    .entry(to)
                                    .or_default()
                                    .entry(name.clone())
                                    .or_default()
                                    .push(Citer {
                                        from: work,
                                        by: r.source,
                                        in_text: repeats(reference.citation.text.as_deref(), name),
                                        held_beside: held_beside(
                                            live,
                                            serving,
                                            &table,
                                            &reference.names,
                                        ),
                                    });
                            }
                        }
                    } else {
                        *landings.entry((work, r.source, to)).or_default() += 1;
                        beside_rows(
                            &mut beside,
                            &reference.names,
                            work,
                            r.source,
                            to,
                            reference.citation.text.as_deref(),
                            |name| target_of(live, serving, &table, name),
                        );
                        put_cited_as.insert((work, r.source));
                    }
                }
            }
            self.applied.cited_only += 1;
        }
        // A name this batch gave to a work or a stub: references that
        // gave it beside the work they landed on resolve again, and the
        // arc moves when the name's namespace outranks the one that
        // landed it (serving plan §11, item 26, 2026-09-18).
        for name in self.live.attached() {
            let Some(Resolved::One(work)) = self.live.resolved(&name) else {
                continue;
            };
            let key = name.as_name().key();
            for row in self.serving.cited_as(&name) {
                // Only a name that named no work can take an arc with it
                // (§11, item 26); the rest is evidence of who cited it.
                // A name the reference's own citation text repeats beats
                // every order, and a landing the text backs is beaten by
                // none (merge plan §6): rank them both with the text
                // first, exactly as a full merge settles the reference.
                let mine = if row.in_text { 0 } else { build::rank(key) + 1 };
                let theirs = if row.by_text {
                    0
                } else {
                    build::rank(row.landed) + 1
                };
                if !row.beside || mine >= theirs {
                    continue;
                }
                let from = self.survivor_of(row.from);
                let to = self.survivor_of(row.to);
                if to == work {
                    continue;
                }
                let by: Sources = [row.by].into_iter().collect();
                let held = self
                    .serving
                    .references(from)
                    .any(|a| a.work == to && a.by.contains(row.by));
                if held && row.sole {
                    lists.remove(from, to, by);
                }
                lists.add(from, work, by);
                self.applied.references_moved += 1;
                if matches!(self.target(&name, &table), Target::Work(w, true) if w == work) {
                    landed
                        .entry(work)
                        .or_default()
                        .entry(name.clone())
                        .or_default()
                        .push(Citer {
                            from,
                            by: row.by,
                            in_text: row.in_text,
                            // A cited-as row is a held landing's, so the
                            // reference names a held work by definition.
                            held_beside: true,
                        });
                }
            }
        }
        // Deleted records: their lists go, except what a record of the
        // same source the work keeps still gives, which that record
        // restates (2026-09-18).
        for ((source, _native), work) in self.deleted_in.clone() {
            let by: Sources = [source].into_iter().collect();
            let kept = twins.get(&(work, source)).map_or_else(
                || (BTreeSet::new(), Vec::new(), Vec::new()),
                |records| kept_of(records, work, &mut beside, &mut landings),
            );
            let (kept_arcs, kept_relations, kept_rows) = kept;
            let old_arcs: Vec<WorkId> = self
                .serving
                .references(work)
                .filter(|a| a.by.contains(source))
                .map(|a| a.work)
                .collect();
            // The arcs the graph holds are in the ids it had before this
            // batch; what the kept record gives is resolved against the
            // ids after it. An arc whose target this batch retired is the
            // same arc under a new id, and the retirement leaves it to
            // this loop, the pair's record having been withdrawn.
            let mut standing: BTreeSet<WorkId> = BTreeSet::new();
            for to in old_arcs {
                let now = self.survivor_of(to);
                if kept_arcs.contains(&now) {
                    standing.insert(now);
                    if now != to {
                        lists.remove(work, to, by);
                        lists.add(work, now, by);
                        self.applied.arcs_moved += 1;
                    }
                    continue;
                }
                lists.remove(work, to, by);
                if self.serving.is_stub(to) {
                    stale.push((to, (work, source)));
                }
            }
            for to in kept_arcs.difference(&standing) {
                lists.add(work, *to, by);
            }
            lists.relations.push(RelationLine::Put {
                batch: batch_number,
                from: work,
                by: source,
                relations: kept_relations,
            });
            lists.unresolved.push(UnresolvedLine::Put {
                batch: batch_number,
                from: work,
                by: source,
                rows: kept_rows,
            });
            if twins.contains_key(&(work, source)) {
                put_cited_as.insert((work, source));
            }
        }
        // The cited-as rows, whole per citing work and source.
        for (from, by) in put_cited_as {
            let rows: Vec<OwnedCitedAs> = beside
                .iter()
                .filter(|row| (row.from, row.by) == (from, by))
                .map(|row| OwnedCitedAs {
                    sole: landings.get(&(from, by, row.to)) == Some(&1),
                    ..row.clone()
                })
                .collect();
            lists.cited_as.push(CitedAsLine::Put {
                batch: batch_number,
                from,
                by,
                rows,
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
        if let Some((x, _, from, _)) = self.moves.iter().find(|(x, ..)| retired.contains(x)) {
            return Err(Error::FullMerge(format!(
                "{x} cites stub {from} through a name its new work leaves, and is itself retired in this batch"
            )));
        }
        // A retired work's citations of stubs, restated under its survivor
        // for the sources that stay with it.
        let mut renamed: Vec<(WorkId, WorkId, WorkId, Sources)> = Vec::new();
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
                if arc.work != id && self.serving.is_stub(arc.work) {
                    renamed.push((arc.work, id, into, moved));
                }
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
                // A withdrawn record's citations are taken back by the
                // deletions loop, so they do not follow the retirement.
                // Every other citation does, a re-delivered record's
                // included, since its earlier copy's arcs stand
                // (2026-09-18).
                let theirs: Sources = SourceId::ALL
                    .into_iter()
                    .filter(|s| self.replaced.contains(&(arc.work, *s)))
                    .collect();
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
        self.write_stub_rows(landed, &stale, &renamed)?;
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

/// The copy of a record its row holds, when a live work holds the record.
fn row_record(
    rows: &mut Rows<'_>,
    source: SourceId,
    native: &OwnedName,
) -> Result<Option<crate::row::Record>, Error> {
    let Some(Resolved::One(id)) = rows.serving.resolve_name(native) else {
        return Ok(None);
    };
    let Some(row) = rows.get(id)? else {
        return Ok(None);
    };
    Ok(row
        .records
        .iter()
        .find(|x| x.source == source && x.native == *native)
        .cloned())
}

/// The delivery of the copy a record's row holds.
fn row_copy(
    rows: &mut Rows<'_>,
    source: SourceId,
    native: &OwnedName,
) -> Result<Option<Delivery>, Error> {
    Ok(row_record(rows, source, native)?.map(|x| Delivery::of(&x.view.borrow(), 0)))
}

/// The copies of a batch that speak for their records (merge plan §8),
/// ranked as the full merge ranks them: of a record's copies in the batch
/// and the one its row holds, the newest speaks; a deletion withdraws the
/// batch's copies in the sets before its own, and the row's copy when that
/// is before it too; a deletion a later copy overtakes withdraws nothing.
#[allow(clippy::too_many_lines)]
fn speaking(rows: &mut Rows<'_>, batch: Batch, applied: &mut Applied) -> Result<Batch, Error> {
    let Batch {
        records,
        cited_only: _,
        deletions,
        store_deletions,
        inputs,
    } = batch;
    let latest = |list: &[Deletion]| {
        let mut by: HashMap<(SourceId, OwnedName), Box<str>> = HashMap::new();
        for d in list {
            let slot = by
                .entry((d.source, d.native.clone()))
                .or_insert_with(|| d.set.clone());
            if **slot < *d.set {
                slot.clone_from(&d.set);
            }
        }
        by
    };
    let withdrawn_by = latest(&deletions);
    let mut any_withdrawn_by = latest(&store_deletions);
    for (key, set) in &withdrawn_by {
        let slot = any_withdrawn_by
            .entry(key.clone())
            .or_insert_with(|| set.clone());
        if **slot < **set {
            slot.clone_from(set);
        }
    }
    let mut order: Vec<(SourceId, OwnedName)> = Vec::new();
    let mut newest: HashMap<(SourceId, OwnedName), (Delivery, Incoming)> = HashMap::new();
    for (i, r) in records.into_iter().enumerate() {
        let key = (r.source, r.native.clone());
        if any_withdrawn_by
            .get(&key)
            .is_some_and(|set| **set > *r.view.set)
        {
            applied.withdrawn += 1;
            continue;
        }
        let delivery = Delivery::of(&r.view.borrow(), i as u64 + 1);
        match newest.get_mut(&key) {
            None => {
                order.push(key.clone());
                newest.insert(key, (delivery, r));
            }
            Some(current) => {
                applied.superseded += 1;
                // The citations of both copies stand.
                if current.0 < delivery {
                    let mut r = r;
                    r.references.append(&mut current.1.references);
                    r.absorb(&current.1);
                    *current = (delivery, r);
                } else {
                    let mut r = r;
                    current.1.references.append(&mut r.references);
                    current.1.absorb(&r);
                }
            }
        }
    }
    let mut kept = Vec::with_capacity(order.len());
    let mut cited_only = Vec::new();
    for key in order {
        let Some((delivery, r)) = newest.remove(&key) else {
            continue;
        };
        let mut r = r;
        if let Some(record) = row_record(rows, r.source, &r.native)? {
            if delivery.older_than(&Delivery::of(&record.view.borrow(), 0)) {
                applied.superseded += 1;
                cited_only.push(r);
                continue;
            }
            // This copy speaks over the row's: the row's names join it,
            // and the row's values fill what it has none of (§8).
            r.absorb_row(&record);
        }
        kept.push(r);
    }
    let in_batch: HashSet<(SourceId, OwnedName)> =
        kept.iter().map(|r| (r.source, r.native.clone())).collect();
    let mut applying = Vec::new();
    let mut done: HashSet<(SourceId, OwnedName)> = HashSet::new();
    for d in deletions {
        let key = (d.source, d.native.clone());
        if !done.insert(key.clone()) {
            continue;
        }
        let set = withdrawn_by.get(&key).cloned().unwrap_or(d.set);
        let overtaken = in_batch.contains(&key)
            || row_copy(rows, d.source, &d.native)?.is_some_and(|row| *row.set >= *set);
        if overtaken {
            applied.deletions_not_applying += 1;
            continue;
        }
        applying.push(Deletion {
            source: d.source,
            native: d.native,
            set,
        });
    }
    Ok(Batch {
        records: kept,
        cited_only,
        deletions: applying,
        store_deletions: Vec::new(),
        inputs,
    })
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
    apply_with(store, serving, batch, key, &NoRecords)
}

/// [`apply`], with a reader for records the batch does not carry (see
/// [`Records`]).
///
/// # Errors
///
/// As [`apply`].
#[allow(clippy::too_many_lines)]
pub fn apply_with(
    store: &Store,
    serving: &Serving,
    batch: Batch,
    key: Option<&Key>,
    layer: &dyn Records,
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
    let mut rows = Rows {
        serving,
        cache: HashMap::new(),
    };
    let batch = speaking(&mut rows, batch, &mut applied)?;
    applied.records = batch.records.len() as u64;
    applied.at.clone_from(&at);
    let mut engine = Engine {
        serving,
        batch: &batch,
        index: BatchIndex::of(&batch.records),
        older: BatchIndex::of(&batch.cited_only),
        layer,
        at: at.clone(),
        live: Live::new(serving),
        rows,
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
        replaced: HashSet::new(),
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
    let mut cited_as =
        Appender::open(&delta_dir.join(delta::CITED_AS)).map_err(|e| io(&delta_dir, e))?;
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
        if work.stub {
            // A held work that became a stub leaves the lexical index,
            // as a reader folding this line takes it out (2026-09-18).
            removes.push(work.id);
        } else {
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
    for line in &lists.cited_as {
        cited_as.put(line).map_err(|e| io(cited_as.path(), e))?;
    }
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
        (
            "cited_as",
            cited_as.finish().map_err(|e| io(&delta_dir, e))?,
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
