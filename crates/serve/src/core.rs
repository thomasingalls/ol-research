//! The query core (serving plan §7): one open epoch — the names lookup,
//! the works index, the graph, the lexical index — with everything the
//! merged layer appended since folded over it (§5), and the five
//! questions, synchronous, speaking `WorkId` inside. The transport (SP4)
//! turns public ids into `WorkId`s at its boundary and back. Every answer
//! is as of the watermark: the batch every artifact has folded to.

use std::collections::{HashMap, HashSet};
use std::sync::{RwLock, RwLockReadGuard};
use std::time::Instant;

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
    /// An id retired by a split: no survivor answers for it, and the
    /// parts its names went to answer in its place, as what was once so
    /// (serving plan §11, item 24). Every part is live.
    Split {
        #[allow(missing_docs)]
        id: WorkId,
        #[allow(missing_docs)]
        into: Vec<WorkId>,
    },
}

/// Which arcs one hop of an [expansion](Serving::expand) follows. The
/// two features that use it go opposite ways on both axes — co-citation
/// seeds with a work's *citers* and walks *references*, bibliographic
/// coupling seeds with its *references* and walks *citers* — so the
/// direction is the caller's and never both by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Walk {
    /// What the seeds cite.
    References,
    /// What cites the seeds.
    Citers,
    /// Both, which is what an expansion was before the directions were
    /// split.
    #[default]
    Both,
}

/// One work reached by an [expansion](Serving::expand), with both the
/// raw evidence and the score it was ranked by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Neighbour {
    #[allow(missing_docs)]
    pub work: WorkId,
    /// How many distinct seeds reached it: `|S ∩ N(v)|`, the interpretable
    /// number — "7 of its 9 citers also cite this" is this beside
    /// [`degree`](Self::degree).
    pub touches: u32,
    /// `|N(v)|`: how many works stand where the seeds did, which is the
    /// *opposite* orientation to the one walked. Walking references
    /// reaches `v` along `seed → v`, so the set the seeds are compared
    /// against is `citers(v)`; walking citers, `references(v)`; walking
    /// both, the two of them added, which double-counts only a work `v`
    /// cites and is cited by — at most 88 held works of 148,210 on the
    /// epoch of 2026-09-19, every work in any directed cycle at all.
    pub degree: u64,
    /// Salton's cosine, `|S ∩ N(v)| / sqrt(|S| · |N(v)|)`, in `0..=1`.
    pub score: f64,
}

/// What one hop out found (serving plan §11, item 7): the best of it
/// the budget allowed, and the numbers that say what that is a part of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Expansion {
    /// Best-scored first, at most the budget asked for.
    pub neighbours: Vec<Neighbour>,
    /// Works the hop reached, before the budget truncated: what
    /// [`neighbours`](Self::neighbours) is the head of, and a list's
    /// `total` is to its page.
    pub total: u64,
    /// Distinct seeds walked, which is the `|S|` the score divides by.
    pub seeds_used: u64,
    /// Seeds the question had before any cap: the same as
    /// [`seeds_used`](Self::seeds_used) for a caller's own seed set, and
    /// the whole list for [`Serving::related`], where the cap is the
    /// server's.
    pub seeds_total: u64,
}

/// Which of the two questions [`Serving::related`] asks. Both are one
/// hop out from a list the subject work owns, and they are mirrors of
/// each other on both axes at once (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// Bibliographic coupling: the seeds are the work's **references**
    /// and the walk is **citers** — "what else cites what this cites".
    CitesSame,
    /// Co-citation: the seeds are the work's **citers** and the walk is
    /// **references** — "what else is cited by what cites this".
    CitedAlongside,
}

impl By {
    /// Which arcs the hop follows, which is the opposite orientation to
    /// the one the seeds were gathered from.
    #[must_use]
    pub const fn walk(self) -> Walk {
        match self {
            Self::CitesSame => Walk::Citers,
            Self::CitedAlongside => Walk::References,
        }
    }
}

/// Arcs [`Serving::related`] will walk, at most: the cap is on the work
/// and not on the seeds, since a seed's list may be any length and a
/// count of seeds therefore bounds nothing. Measured on epoch 7 of
/// 2026-09-19 at 2,444 arcs a millisecond, so this is about a tenth of
/// a second — the same order as the path search's 250,000-work visit
/// guard (§7), and like it a resource guard rather than a question.
///
/// Nothing in this corpus comes near it: the most-cited work we hold,
/// the PRISMA statement, walks 94,098 arcs for its co-citation and 3,147
/// for its coupling, so every `related` answer today is the whole
/// question and the bound is there for the corpus.
pub const RELATED_ARCS: u64 = 250_000;

/// One hop of a retirement, as the lookups have it.
#[derive(Debug, Clone, Copy)]
enum Retired {
    /// Merged into an older id, which answers for it.
    Merged(WorkId),
    /// Split, with no one successor: see [`Serving::split_into`].
    Split,
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
        Self::open_epoch(store, n)
    }

    /// Opens epoch `n` as [`open`](Self::open) opens the current one: how
    /// a server built for one version keeps answering from the last epoch
    /// it can read while `current` names one built by another.
    ///
    /// # Errors
    ///
    /// As [`open`](Self::open), and an epoch `n` that was never published.
    pub fn open_epoch(store: &Store, n: u64) -> Result<Self, Error> {
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
        self.cited_as_key(&name.to_string())
    }

    /// Every name the base's or the fold's cited-as rows name, in order:
    /// what a comparison of two cores walks (serving plan §11, item 35).
    #[must_use]
    pub fn cited_as_names(&self) -> Vec<String> {
        let mut names: std::collections::BTreeSet<String> =
            self.cited.names().into_iter().collect();
        names.extend(self.fold().cited.names().map(str::to_owned));
        names.into_iter().collect()
    }

    /// [`cited_as`](Self::cited_as) for a name's text form.
    #[must_use]
    pub fn cited_as_key(&self, text: &str) -> Vec<CitedRow> {
        let fold = self
            .fold
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut rows: Vec<CitedRow> = self
            .cited
            .rows_of(text)
            .into_iter()
            .filter(|row| !fold.cited.restated(row.from, row.by))
            .map(|row| CitedRow {
                from: fold.cited.survivor(row.from),
                to: fold.cited.survivor(row.to),
                ..row
            })
            .collect();
        rows.extend(fold.cited.rows_of(text));
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
                None => Resolved::Split {
                    id,
                    into: self.split_into(id),
                },
            },
            Named::Shared(ids) => Resolved::Shared(ids),
        })
    }

    /// One hop of a retirement: the older id `id` was merged into, or a
    /// split with no successor; `None` when it is live. The tail speaks
    /// over the base, as everywhere.
    fn retired_into(&self, id: WorkId) -> Option<Retired> {
        self.fold()
            .names
            .retired(id)
            .or_else(|| self.names.retired_into(id))
            .map(|into| into.map_or(Retired::Split, Retired::Merged))
    }

    /// The parts a split recorded for `id`, before they are followed.
    fn split_parts(&self, id: WorkId) -> Vec<WorkId> {
        let tail = self.fold().names.split_into(id).to_vec();
        if tail.is_empty() {
            self.names.split_into(id).to_vec()
        } else {
            tail
        }
    }

    /// The live works a split retired `id` into, in the registry's
    /// order: what an id nobody may resolve to one work can still answer
    /// with, as what was once so (serving plan §11, item 24). An id
    /// merged before the split answers with the split's parts; a part
    /// merged since is followed to its survivor, and a part split again
    /// gives its own parts. Empty for every id still live and every id
    /// no split retired.
    #[must_use]
    pub fn split_into(&self, id: WorkId) -> Vec<WorkId> {
        // The id the split actually retired, at the end of the merges.
        let mut at = id;
        let split = loop {
            match self.retired_into(at) {
                None => return Vec::new(),
                Some(Retired::Merged(into)) => at = into,
                Some(Retired::Split) => break at,
            }
        };
        let mut out: Vec<WorkId> = Vec::new();
        let mut seen: HashSet<WorkId> = HashSet::new();
        let mut queue: Vec<WorkId> = self.split_parts(split);
        queue.reverse();
        while let Some(part) = queue.pop() {
            if !seen.insert(part) {
                continue;
            }
            if let Some(live) = self.live(part) {
                if !out.contains(&live) {
                    out.push(live);
                }
            } else {
                // A part a later split retired answers with its own.
                let mut next = self.split_parts(part);
                next.reverse();
                queue.extend(next);
            }
        }
        out
    }

    /// [`resolve`](Self::resolve) for a name.
    #[must_use]
    pub fn resolve_name(&self, name: &OwnedName) -> Option<Resolved> {
        self.resolve_key(&name.to_string())
    }

    /// The works whose rows refuse `name` — whose records claimed it and
    /// were refused — the base's where the works delta has no newer word
    /// on the id, and the delta's own, in id order: a refused name's other
    /// claimants, which no lookup of the name finds (merge plan §11, item
    /// 24). Ids are as the rows have them, retired since or not.
    #[must_use]
    pub fn refusing(&self, name: &OwnedName) -> Vec<WorkId> {
        let key = name.to_string();
        let fold = self.fold();
        let mut ids: Vec<WorkId> = self
            .names
            .refusing(&key)
            .into_iter()
            .filter(|id| fold.works.slot(*id).is_none())
            .chain(fold.names.refusing(&key).iter().copied())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
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
            Some(Slot::Put(offset)) => Ok(Some(self.delta_row(&fold, id, offset)?)),
            Some(Slot::Retired(_) | Slot::Dropped) => Ok(None),
            None => self.works.get(id),
        }
    }

    /// The rows of many works at once, which is what a page of anything
    /// hydrated wants: [`work`](Self::work) for each id, answered in one
    /// walk of the index instead of one lookup per id.
    ///
    /// A single lookup decompresses a whole block of 256 rows and keeps
    /// one (serving plan §11, item 13), and a page's ids are neither
    /// sorted nor distinct — a citation neighbourhood's ids cluster, so
    /// the same block is decompressed again and again, once per id that
    /// falls in it. Here the ids are deduplicated and sorted first, and
    /// each block is decompressed once however many of the page's works
    /// it holds.
    ///
    /// The answer is a map and not a list in the caller's order: the
    /// caller already holds that order — its page, its neighbours, its
    /// path — and wants the row *of a work* while walking it. A map also
    /// says absence once. An id with no row (retired by a merge or a
    /// split, dropped, or past the epoch) is simply not a key, which is
    /// exactly [`work`](Self::work) answering `None`; the delta's
    /// overrides are applied as `work` applies them, so a row a batch
    /// touched reads the same either way.
    ///
    /// # Errors
    ///
    /// A row that does not decode.
    pub fn rows(&self, ids: &[WorkId]) -> Result<HashMap<WorkId, Row>, Error> {
        let fold = self.fold();
        let mut out: HashMap<WorkId, Row> = HashMap::with_capacity(ids.len());
        // The delta's rows are read one line at a time, as `work` reads
        // them; only the base is walked by block, and a batch's works
        // are few beside a page.
        let mut base: Vec<WorkId> = Vec::with_capacity(ids.len());
        let mut seen: HashSet<WorkId> = HashSet::with_capacity(ids.len());
        for id in ids {
            if !seen.insert(*id) {
                continue;
            }
            match fold.works.slot(*id) {
                Some(Slot::Put(offset)) => {
                    out.insert(*id, self.delta_row(&fold, *id, offset)?);
                }
                Some(Slot::Retired(_) | Slot::Dropped) => {}
                None => base.push(*id),
            }
        }
        base.sort_unstable();
        for (id, row) in self.works.get_many(&base)? {
            out.insert(id, row);
        }
        Ok(out)
    }

    /// The row the works delta holds at `offset` for `id`, the fold
    /// already read-locked: the override both [`work`](Self::work) and
    /// [`rows`](Self::rows) apply.
    fn delta_row(&self, fold: &Fold, id: WorkId, offset: u64) -> Result<Row, Error> {
        let line = fold.works.line(&fold.works_path(), offset)?;
        let WorkLine::Put { work, text, .. } = line else {
            return Err(Error::Mismatch(format!(
                "the works delta at byte {offset} is not the row of {id} it was indexed as"
            )));
        };
        let title = row::title_of(&work.fields);
        let text = text.map(|t| Text::with_title(title, &t));
        Ok(Row::from_owned(*work, self.public_of(id), text))
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

    /// Works matching a query, as the query parser read it, with facet
    /// counts over the matches.
    ///
    /// # Errors
    ///
    /// The engine.
    pub fn search(&self, asked: &query_parser::Query, limit: usize) -> Result<Hits, Error> {
        self.search_facets(asked, limit, 5, None)
    }

    /// As [`search`](Self::search), with the top `facets` values of each
    /// facet root, and stopped at `until` when there is one.
    ///
    /// # Errors
    ///
    /// The engine, or `search::Error::Deadline` past `until`.
    pub fn search_facets(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        facets: usize,
        until: Option<Instant>,
    ) -> Result<Hits, Error> {
        Ok(self.search.search(asked, limit, facets, until)?)
    }

    /// The top `limit` candidates for a citation's words: searched in the
    /// fields a citation names a work by, with no facets (citation lookup
    /// plan, D3), stopped at `until` when there is one.
    ///
    /// # Errors
    ///
    /// The engine, or `search::Error::Deadline` past `until`.
    pub fn candidates(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        until: Option<Instant>,
    ) -> Result<Vec<search::Hit>, Error> {
        Ok(self.search.candidates(asked, limit, until)?)
    }

    /// A work's public id: minted since the epoch, or in the base's array.
    #[must_use]
    pub fn public_of(&self, id: WorkId) -> Option<String> {
        if let Some(text) = self.fold().names.public_of(id) {
            return Some(text.to_owned());
        }
        self.names.public_of(id)
    }

    /// How many works `id` cites as of the watermark. Two offsets and no
    /// list decoded wherever no batch has touched the work's arcs, which
    /// is every work but a recent batch's; a touched work's list is
    /// counted with the delta folded over it, since a delta both adds
    /// arcs and empties them.
    #[must_use]
    pub fn out_degree(&self, id: WorkId) -> u64 {
        let id = self.live(id).unwrap_or(id);
        let touched = !self.fold().edges.forward(id).is_empty();
        if touched {
            self.references(id).count() as u64
        } else {
            self.graph.out_degree(id)
        }
    }

    /// How many works cite `id` as of the watermark; see
    /// [`out_degree`](Self::out_degree) for what it costs.
    #[must_use]
    pub fn in_degree(&self, id: WorkId) -> u64 {
        let id = self.live(id).unwrap_or(id);
        let touched = !self.fold().edges.transpose(id).is_empty();
        if touched {
            self.citers(id).count() as u64
        } else {
            self.graph.in_degree(id)
        }
    }

    /// One hop out from a seed set along `walk`: every work a seed
    /// reaches that way, the seeds themselves left out, scored, best
    /// first, at most `budget` of them (serving plan §11, item 7).
    ///
    /// `touches` is the raw overlap — how many distinct seeds reach the
    /// work, a seed that reaches it both ways under [`Walk::Both`]
    /// counted once — and the ranking is **Salton's cosine**,
    /// `|S ∩ N(v)| / sqrt(|S| · |N(v)|)`: the co-citation literature's
    /// own normalisation, and the one whose square-root denominator
    /// discounts a hub instead of deleting it, where Jaccard's linear
    /// `|S ∪ N(v)|` all but removes any work with a long citer list from
    /// an answer whatever it shares with the seeds.
    ///
    /// **The normalisation is before the truncate**, which is the whole
    /// of its point: normalising a page already chosen by raw overlap
    /// would normalise a set the hubs had picked. Measured over 500
    /// co-citation expansions on the epoch of 2026-09-19, that is not a
    /// formality — the top twenty by raw overlap and by cosine share
    /// 8.5 works of 20, and the median citer count of the top hit falls
    /// from 14 to 4. The deeper reason is that the raw count is nearly
    /// no signal at this scale: those runs threw up 911,071 candidates
    /// touched by exactly one seed against 39,951 touched by two, so an
    /// unnormalised ranking is a tie over nearly the whole answer,
    /// broken by id order.
    ///
    /// `N(v)` is the opposite orientation to the one walked; see
    /// [`Neighbour::degree`].
    ///
    /// `about` is the work the question was asked about, left out of its
    /// own answer. Where the seeds are a work's own list, that work
    /// scores **exactly one** against itself and heads its own answer:
    /// every seed reaches it, so its `touches` is the size of the seed
    /// set, and the seed set *is* the orientation its degree counts —
    /// `|S| / sqrt(|S| · |S|)`. Nothing else can reach that, and on
    /// epoch 7 of 2026-09-19 nothing comes close: the PRISMA statement's
    /// best real co-citation neighbour scores 0.259 and its best
    /// coupling neighbour 0.983. A seed set with no subject behind it
    /// passes `None` and nothing is dropped. It is a parameter and not
    /// the caller's own filter because the truncate happens here: a
    /// caller dropping it afterwards would be one short of the budget it
    /// asked for, and would have no way to take it out of the count of
    /// what the hop reached.
    // A seed count and a degree, neither of which reaches 2^53 in a
    // graph whose nodes are `u32`.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn expand(
        &self,
        seeds: &[WorkId],
        walk: Walk,
        budget: usize,
        about: Option<WorkId>,
    ) -> Expansion {
        let distinct: HashSet<WorkId> = seeds.iter().copied().collect();
        let size = distinct.len() as f64;
        // `(touches, the last seed to touch it)`: a seed that reaches a
        // work along both orientations is one seed, not two, so that
        // `touches` stays the set intersection the score divides. The
        // walk is over the distinct seeds for the same reason — two
        // spellings of one work are one seed, and counting both would put
        // the score over one.
        let mut touched: HashMap<WorkId, (u32, usize)> = HashMap::new();
        let (references, citers) = match walk {
            Walk::References => (true, false),
            Walk::Citers => (false, true),
            Walk::Both => (true, true),
        };
        for (k, seed) in distinct.iter().enumerate() {
            let forward = references
                .then(|| self.references(*seed))
                .into_iter()
                .flatten();
            let back = citers.then(|| self.citers(*seed)).into_iter().flatten();
            for arc in forward.chain(back) {
                if distinct.contains(&arc.work) || about == Some(arc.work) {
                    continue;
                }
                let slot = touched.entry(arc.work).or_insert((0, usize::MAX));
                if slot.1 != k {
                    slot.0 += 1;
                    slot.1 = k;
                }
            }
        }
        let total = touched.len() as u64;
        let mut out: Vec<Neighbour> = touched
            .into_iter()
            .map(|(work, (touches, _))| {
                let degree = match walk {
                    Walk::References => self.in_degree(work),
                    Walk::Citers => self.out_degree(work),
                    Walk::Both => self.in_degree(work) + self.out_degree(work),
                };
                Neighbour {
                    work,
                    touches,
                    degree,
                    // A work reached is reached by some arc, so its
                    // degree in that orientation is at least one.
                    score: f64::from(touches) / (size * degree.max(1) as f64).sqrt(),
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.touches.cmp(&a.touches))
                .then(a.work.cmp(&b.work))
        });
        out.truncate(budget);
        Expansion {
            neighbours: out,
            total,
            seeds_used: distinct.len() as u64,
            seeds_total: distinct.len() as u64,
        }
    }

    /// The two questions a reader asks of a work's neighbourhood, each
    /// answered in one call: **what else cites what this cites**
    /// ([`By::CitesSame`], bibliographic coupling) and **what else is
    /// cited by what cites this** ([`By::CitedAlongside`], co-citation).
    ///
    /// Both are [`expand`](Self::expand) over a seed set the work owns,
    /// and the point of gathering it here is that the answer is then the
    /// work's and not a client's: a caller paging a list and handing the
    /// page back as seeds asks a question whose answer depends on which
    /// page it happened to hold — a work with 882 citers seeded with the
    /// first 50 of them is answering about those 50.
    ///
    /// **The seeding is bounded and says so.** The bound is
    /// [`RELATED_ARCS`], on the arcs the walk will read and not on the
    /// seed count — a seed's own list may be any length, so a count of
    /// seeds bounds no work at all — and the seeds are taken **smallest
    /// degree first** in the orientation about to be walked, ties by id,
    /// until the next seed would not fit. Both numbers come back,
    /// [`Expansion::seeds_used`] against [`Expansion::seeds_total`], so
    /// a bounded answer is legible as one rather than passing for the
    /// whole.
    ///
    /// Smallest first, because at equal cost it is the better answer and
    /// not merely the cheaper one. Measured on epoch 7 of 2026-09-19
    /// against the unbounded answer — the PRISMA statement's co-citation
    /// over all 882 of its citers — the top twenty taken smallest-degree
    /// first shares 8 works of 20 with it at a budget of 10,000 arcs, 13
    /// at 25,000 and 17 at 50,000, where the same budgets spent on the
    /// seeds in list order share 1, 7 and 10. The reason is arithmetic:
    /// a budget buys 256 short seeds where it buys 87 long ones, and a
    /// seed citing three hundred works puts the subject in almost no
    /// company — the same reason the cosine discounts a hub, applied a
    /// step earlier. What a cap drops is therefore the longest lists.
    ///
    /// One seed is always walked, however long its list: a work whose
    /// every seed is a hub answers with its cheapest seed and says
    /// `seeds_used: 1`, which is an answer a reader can weigh, where an
    /// empty list would not be.
    ///
    /// The subject is left out of its own answer; see
    /// [`expand`](Self::expand)'s `about`.
    #[must_use]
    pub fn related(&self, work: WorkId, by: By, budget: usize) -> Expansion {
        let work = self.live(work).unwrap_or(work);
        let walk = by.walk();
        let arcs = match by {
            By::CitesSame => self.references(work),
            By::CitedAlongside => self.citers(work),
        };
        let mut seeds: Vec<WorkId> = arcs.map(|arc| arc.work).collect();
        seeds.sort_unstable();
        seeds.dedup();
        // A work citing itself is its own seed, and a seed is dropped
        // from the answer anyway; walking it would only buy the arcs it
        // holds twice.
        seeds.retain(|seed| *seed != work);
        let seeds_total = seeds.len() as u64;
        // What each seed will cost to walk, which is two offsets on the
        // base for all but the works a recent batch touched.
        let mut by_degree: Vec<(u64, WorkId)> = seeds
            .iter()
            .map(|seed| {
                let degree = match walk {
                    Walk::References => self.out_degree(*seed),
                    Walk::Citers => self.in_degree(*seed),
                    Walk::Both => self.out_degree(*seed) + self.in_degree(*seed),
                };
                (degree, *seed)
            })
            .collect();
        by_degree.sort_unstable();
        let mut spent = 0u64;
        seeds = by_degree
            .into_iter()
            .enumerate()
            // Smallest first, so a seed that does not fit means no later
            // one would either; the first is taken whatever it costs.
            .take_while(|(k, (degree, _))| {
                spent = spent.saturating_add(*degree);
                *k == 0 || spent <= RELATED_ARCS
            })
            .map(|(_, (_, seed))| seed)
            .collect();
        let mut found = self.expand(&seeds, walk, budget, Some(work));
        found.seeds_total = seeds_total;
        found
    }

    /// The shortest directed paths from one work to another, bounded
    /// (graph plan §7): "does `from` cite something that cites … `to`".
    /// The other direction is the other question — ask it by swapping
    /// the ends — since an undirected walk up and down the arcs means
    /// nothing.
    ///
    /// The search is over this layer's own arcs, the delta folded, so
    /// the answer is as of the watermark like every other. **No path is
    /// the ordinary answer**: the subgraph a directed path moves through
    /// is the held-to-held one — a stub has no references, so it ends any
    /// forward walk and cannot be a path's interior — and on the epoch
    /// of 2026-09-19 that subgraph has 93,007 arcs over 148,210 works, a
    /// mean out-degree of 0.63. Of 200,000 random held pairs, 4 had a
    /// path within six hops. Hence [`graph::Found::None`], which says
    /// how far the search looked, rather than an empty list.
    #[must_use]
    pub fn path(&self, from: WorkId, to: WorkId, bounds: graph::Bounds) -> graph::Found {
        let live = |id: WorkId| self.live(id).unwrap_or(id);
        graph::path::find(
            live(from),
            live(to),
            bounds,
            |work, sink| {
                for arc in self.references(work) {
                    sink(arc.work, arc.by);
                }
            },
            |work, sink| {
                for arc in self.citers(work) {
                    sink(arc.work, arc.by);
                }
            },
        )
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
