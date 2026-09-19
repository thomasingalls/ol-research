//! The query core (serving plan §7): one open epoch — the names lookup,
//! the works index, the graph, the lexical index — with everything the
//! merged layer appended since folded over it (§5), and the five
//! questions, synchronous, speaking `WorkId` inside. The transport (SP4)
//! turns public ids into `WorkId`s at its boundary and back. Every answer
//! is as of the watermark: the batch every artifact has folded to.

use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard};

use graph::Base;
use ingest::Store;
use merge::delta::WorkLine;
use merge::journal::{self, Line};
use merge::public::{self, PREFIX};
use merge::{Name, OwnedName, WorkId};
use search::{Hits, Opened};

use crate::cited::{Cited, Row as CitedRow};
use crate::digest;
use crate::epoch::{self, Manifest};
use crate::error::Error;
use crate::fold::{Advanced, Arcs, Fold, Slot};
use crate::names::{Names, Resolved as Named};
use crate::row::{self, Row, Text};
use crate::works::Works;

/// What a name or public id resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// One live work.
    One(WorkId),
    /// A shared name: every work it was accepted into.
    Shared(Vec<WorkId>),
    /// An id retired by a split: no survivor to answer for it.
    Split(WorkId),
}

/// The open epoch and its fold.
pub struct Serving {
    store: Store,
    epoch: Manifest,
    epoch_line: Line,
    names: Names,
    works: Works,
    cited: Cited,
    graph: Base,
    search: Opened,
    fold: RwLock<Fold>,
}

impl std::fmt::Debug for Serving {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Serving")
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

/// What the cross-check verified.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Checked {
    /// Rows read.
    pub rows: u64,
    /// Promoted names that resolved to their work.
    pub names: u64,
    /// Public ids that resolved to their work.
    pub public: u64,
}

/// The batch an answer is as of, and its timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watermark {
    #[allow(missing_docs)]
    pub batch: u64,
    #[allow(missing_docs)]
    pub at: String,
}

impl Serving {
    /// Opens the epoch `current` names, each artifact checked against
    /// the digest the epoch recorded for its manifest, and folds every
    /// batch the journal has past it that every artifact can answer at.
    ///
    /// # Errors
    ///
    /// No epoch, an artifact missing or changed since the epoch, any
    /// artifact refusing to open, or a journal that does not hold the
    /// epoch's batch as a full merge.
    pub fn open(store: &Store) -> Result<Self, Error> {
        let n = epoch::current(store)?.ok_or(Error::NoEpoch)?;
        let epoch = epoch::read(store, n)?;
        let dir_of = |name: &str, manifest: &str| -> Result<std::path::PathBuf, Error> {
            let artifact = epoch
                .artifacts
                .get(name)
                .ok_or_else(|| Error::Mismatch(format!("epoch {n} names no {name}")))?;
            let dir = store.root().join(&artifact.dir);
            let actual = digest::digest(&dir.join(manifest))?;
            if actual != artifact.manifest {
                return Err(Error::Mismatch(format!(
                    "{}: not the {name} epoch {n} was published with",
                    dir.display()
                )));
            }
            Ok(dir)
        };
        let names = Names::open(&dir_of("names", crate::names::MANIFEST)?)?;
        let works = Works::open(&dir_of("works", crate::works::MANIFEST)?)?;
        let cited = Cited::open(&dir_of("cited", crate::cited::MANIFEST)?)?;
        let graph = Base::open(&dir_of("graph", graph::layout::MANIFEST)?)?;
        let search = Opened::open(&dir_of("search", search::index::MANIFEST)?)?;
        let journal_path = store.journal();
        let lines = journal::read(&journal_path).map_err(|e| Error::io(&journal_path, e))?;
        let epoch_line = lines
            .iter()
            .find(|l| l.batch == epoch.batch)
            .cloned()
            .ok_or_else(|| {
                Error::Mismatch(format!(
                    "epoch {n} is at batch {}, which the journal does not have",
                    epoch.batch
                ))
            })?;
        let merged = store.root().join("merged").join(&epoch.merged);
        let fold = Fold::new(store, &merged, &epoch_line)?;
        let this = Self {
            store: store.clone(),
            epoch,
            epoch_line,
            names,
            works,
            cited,
            graph,
            search,
            fold: RwLock::new(fold),
        };
        this.refresh()?;
        Ok(this)
    }

    /// The references that gave `name` beside the work they landed on
    /// (merge plan §11, item 14), the base's and the fold's: what lets an
    /// incremental merge resolve those references again when the name
    /// becomes a work's (2026-09-18).
    #[must_use]
    pub fn cited_as(&self, name: &OwnedName) -> Vec<CitedRow> {
        let text = name.to_string();
        let fold = self
            .fold
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut rows: Vec<CitedRow> = self
            .cited
            .rows_of(&text)
            .into_iter()
            .filter(|row| !fold.cited.restated(row.from, row.by))
            .map(|row| CitedRow {
                from: fold.cited.survivor(row.from),
                to: fold.cited.survivor(row.to),
                ..row
            })
            .collect();
        rows.extend(fold.cited.rows_of(&text));
        rows.sort_unstable();
        rows.dedup();
        rows
    }

    /// Folds every batch the journal has gained that every artifact can
    /// answer at: what a server does on every poll.
    ///
    /// # Errors
    ///
    /// I/O, or a journal or a delta the fold refuses.
    pub fn refresh(&self) -> Result<Advanced, Error> {
        let path = self.store.journal();
        let lines = journal::read(&path).map_err(|e| Error::io(&path, e))?;
        let mut fold = self
            .fold
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        fold.advance(&self.graph, &self.search, &lines)
    }

    fn fold(&self) -> RwLockReadGuard<'_, Fold> {
        self.fold
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The batch every answer is as of, and its timestamp.
    #[must_use]
    pub fn watermark(&self) -> Watermark {
        let fold = self.fold();
        Watermark {
            batch: fold.watermark,
            at: fold.at.clone(),
        }
    }

    /// The store the epoch was opened from.
    #[must_use]
    pub const fn store(&self) -> &Store {
        &self.store
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn epoch(&self) -> &Manifest {
        &self.epoch
    }

    /// The journal line of the epoch's batch.
    #[must_use]
    pub const fn epoch_line(&self) -> &Line {
        &self.epoch_line
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn names(&self) -> &Names {
        &self.names
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn works(&self) -> &Works {
        &self.works
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn graph(&self) -> &Base {
        &self.graph
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn search_index(&self) -> &Opened {
        &self.search
    }

    /// Ids minted so far: the base's, or the tail's highest.
    #[must_use]
    pub fn minted(&self) -> u32 {
        self.names.manifest().nodes.max(self.fold().names.minted())
    }

    /// The batch the lexical index has absorbed, as of the last fold.
    #[must_use]
    pub fn lexical_batch(&self) -> u64 {
        self.fold().lexical
    }

    /// The bytes of each delta log folded so far, by the journal's keys.
    #[must_use]
    pub fn delta_offsets(&self) -> std::collections::BTreeMap<String, u64> {
        self.fold().offsets.clone()
    }

    /// What a key — a name's text form, or a public id — names, as
    /// stored: retired ids are not followed here; see [`live`](Self::live).
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Named> {
        if let Some(answer) = self.fold().names.get(key) {
            return answer;
        }
        self.names.get(key)
    }

    /// The live id an id stands for: itself, the id it was merged into,
    /// transitively, or `None` when it was split.
    #[must_use]
    pub fn live(&self, mut id: WorkId) -> Option<WorkId> {
        let fold = self.fold();
        loop {
            match fold
                .names
                .retired(id)
                .or_else(|| self.names.is_retired(id).then(|| self.names.live(id)))
            {
                None => return Some(id),
                Some(Some(into)) => id = into,
                Some(None) => return None,
            }
        }
    }

    /// Which work a name or a public id is, in any spelling: a merged
    /// work's id answers as its survivor.
    ///
    /// # Errors
    ///
    /// Text that is neither a name nor a public id.
    pub fn resolve(&self, text: &str) -> Result<Option<Resolved>, Error> {
        let key = if text
            .get(..PREFIX.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(PREFIX))
        {
            public::canonical(text).map_err(|e| Error::NotAName(e.to_string()))?
        } else {
            Name::parse(text)
                .map_err(|e| Error::NotAName(e.to_string()))?
                .to_string()
        };
        Ok(self.resolve_key(&key))
    }

    /// [`resolve`](Self::resolve) for a key already in canonical text.
    #[must_use]
    pub fn resolve_key(&self, key: &str) -> Option<Resolved> {
        self.get(key).map(|found| match found {
            Named::One(id) => match self.live(id) {
                Some(live) => Resolved::One(live),
                None => Resolved::Split(id),
            },
            Named::Shared(ids) => Resolved::Shared(ids),
        })
    }

    /// [`resolve`](Self::resolve) for a name.
    #[must_use]
    pub fn resolve_name(&self, name: &OwnedName) -> Option<Resolved> {
        self.resolve_key(&name.to_string())
    }

    /// Everything the merge knows about a work, and its text: the works
    /// delta's latest row for it, or the base's.
    ///
    /// # Errors
    ///
    /// A row that does not decode.
    pub fn work(&self, id: WorkId) -> Result<Option<Row>, Error> {
        let fold = self.fold();
        match fold.works.slot(id) {
            Some(Slot::Put(offset)) => {
                let line = fold.works.line(&fold.works_path(), offset)?;
                let WorkLine::Put { work, text, .. } = line else {
                    return Err(Error::Mismatch(format!(
                        "the works delta at byte {offset} is not the row of {id} it was indexed as"
                    )));
                };
                let title = row::title_of(&work.fields);
                let text = text.map(|t| Text::with_title(title, &t));
                Ok(Some(Row::from_owned(*work, self.public_of(id), text)))
            }
            Some(Slot::Retired(_) | Slot::Dropped) => Ok(None),
            None => self.works.get(id),
        }
    }

    /// Whether the fold has its own row, retirement or drop for an id.
    #[must_use]
    pub fn in_delta(&self, id: WorkId) -> bool {
        self.fold().works.slot(id).is_some()
    }

    /// Whether an id is a stub — names and no records — as of the
    /// watermark; false for a retired or unknown id.
    #[must_use]
    pub fn is_stub(&self, id: WorkId) -> bool {
        let fold = self.fold();
        match fold.works.slot(id) {
            Some(Slot::Put(_)) => fold.works.is_stub(id).unwrap_or(false),
            Some(Slot::Retired(_) | Slot::Dropped) => false,
            None => self.names.is_stub(id),
        }
    }

    /// The works `id` cites, lazily, each with who asserted it; a retired
    /// id answers as its survivor.
    #[must_use]
    pub fn references(&self, id: WorkId) -> Arcs<'_> {
        let id = self.live(id).unwrap_or(id);
        let delta = self.fold().edges.forward(id);
        Arcs::new(self.graph.references(id), delta)
    }

    /// The works citing `id`, lazily, each with who asserted it.
    #[must_use]
    pub fn citers(&self, id: WorkId) -> Arcs<'_> {
        let id = self.live(id).unwrap_or(id);
        let delta = self.fold().edges.transpose(id);
        Arcs::new(self.graph.citers(id), delta)
    }

    /// Works matching a query, with facet counts over the matches.
    ///
    /// # Errors
    ///
    /// A query the engine refuses.
    pub fn search(&self, text: &str, limit: usize) -> Result<Hits, Error> {
        self.search_facets(text, limit, 5)
    }

    /// As [`search`](Self::search), with the top `facets` values of each
    /// facet root.
    ///
    /// # Errors
    ///
    /// A query the engine refuses.
    pub fn search_facets(&self, text: &str, limit: usize, facets: usize) -> Result<Hits, Error> {
        Ok(self.search.search(text, limit, facets)?)
    }

    /// A work's public id: minted since the epoch, or in the base's array.
    #[must_use]
    pub fn public_of(&self, id: WorkId) -> Option<String> {
        if let Some(text) = self.fold().names.public_of(id) {
            return Some(text.to_owned());
        }
        self.names.public_of(id)
    }

    /// One hop out from a seed set: every work a seed cites or is cited
    /// by, the seeds themselves left out, with how many seeds it touches,
    /// most first, at most `budget` of them.
    #[must_use]
    pub fn expand(&self, seeds: &[WorkId], budget: usize) -> Vec<(WorkId, u32)> {
        let mut touched: HashMap<WorkId, u32> = HashMap::new();
        for seed in seeds {
            for arc in self.references(*seed).chain(self.citers(*seed)) {
                if seeds.contains(&arc.work) {
                    continue;
                }
                *touched.entry(arc.work).or_default() += 1;
            }
        }
        let mut out: Vec<(WorkId, u32)> = touched.into_iter().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.truncate(budget);
        out
    }

    /// Every row in id order, the base's with the fold's rows over them,
    /// given to `f` one at a time.
    ///
    /// # Errors
    ///
    /// A row that does not decode, or whatever `f` returns.
    pub fn each_row(&self, mut f: impl FnMut(Row) -> Result<(), Error>) -> Result<(), Error> {
        let fold = self.fold();
        let tail = fold.works.put_ids();
        let mut next_tail = 0usize;
        let mut emit_tail_up_to = |limit: Option<WorkId>,
                                   f: &mut dyn FnMut(Row) -> Result<(), Error>|
         -> Result<(), Error> {
            while next_tail < tail.len() && limit.is_none_or(|l| tail[next_tail] < l) {
                let id = tail[next_tail];
                next_tail += 1;
                if let Some(row) = self.work(id)? {
                    f(row)?;
                }
            }
            Ok(())
        };
        for row in self.works.rows() {
            let row = row?;
            if fold.works.slot(row.id).is_some() {
                continue;
            }
            emit_tail_up_to(Some(row.id), &mut f)?;
            f(row)?;
        }
        emit_tail_up_to(None, &mut f)
    }

    /// The artifacts against each other: every row's promoted names and
    /// public id resolve to it; the graph and the index hold the same
    /// ids; the lexical index holds exactly the held works.
    ///
    /// # Errors
    ///
    /// The first disagreement.
    pub fn cross_check(&self) -> Result<Checked, Error> {
        let broken = |what: String| Error::Invariant(what);
        if self.graph.nodes() != self.works.nodes() {
            return Err(broken(format!(
                "the graph has {} nodes, the works index {}",
                self.graph.nodes(),
                self.works.nodes()
            )));
        }
        let mut checked = Checked::default();
        let mut held = 0u64;
        self.each_row(|row| {
            checked.rows += 1;
            held += u64::from(!row.stub);
            for named in &row.names {
                if named.standing != merge::identity::Standing::Promoted {
                    continue;
                }
                let key = named.name.to_string();
                match self.get(&key) {
                    Some(Named::One(id)) if id == row.id => checked.names += 1,
                    other => {
                        return Err(broken(format!(
                            "{key} is promoted on {} but resolves to {other:?}",
                            row.id
                        )));
                    }
                }
            }
            if let Some(public_id) = &row.public {
                match self.get(public_id) {
                    Some(Named::One(id)) if id == row.id => checked.public += 1,
                    other => {
                        return Err(broken(format!(
                            "{public_id} is {}'s public id but resolves to {other:?}",
                            row.id
                        )));
                    }
                }
                if self.public_of(row.id).as_deref() != Some(public_id.as_str()) {
                    return Err(broken(format!(
                        "{}'s public id is {public_id} in its row and {:?} in the array",
                        row.id,
                        self.public_of(row.id)
                    )));
                }
            }
            Ok(())
        })?;
        if self.search.documents() != held {
            return Err(broken(format!(
                "the lexical index holds {} documents, the rows {held} held works",
                self.search.documents()
            )));
        }
        Ok(checked)
    }
}
