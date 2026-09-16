//! The query core (serving plan §7): one open epoch — the names lookup,
//! the works index, the graph, the lexical index — and the five
//! questions, synchronous, speaking `WorkId` inside. The transport (SP4)
//! turns public ids into `WorkId`s at its boundary and back.

use std::collections::HashMap;

use graph::{Arcs, Base};
use ingest::Store;
use merge::public::{self, PREFIX};
use merge::{Name, WorkId};
use search::{Hits, Opened};

use crate::digest;
use crate::epoch::{self, Manifest};
use crate::error::Error;
use crate::names::{Names, Resolved as Named};
use crate::row::Row;
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

/// The open epoch.
pub struct Serving {
    epoch: Manifest,
    names: Names,
    works: Works,
    graph: Base,
    search: Opened,
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

impl Serving {
    /// Opens the epoch `current` names, each artifact checked against
    /// the digest the epoch recorded for its manifest.
    ///
    /// # Errors
    ///
    /// No epoch, an artifact missing or changed since the epoch, or any
    /// artifact refusing to open.
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
        let graph = Base::open(&dir_of("graph", graph::layout::MANIFEST)?)?;
        let search = Opened::open(&dir_of("search", search::index::MANIFEST)?)?;
        Ok(Self {
            epoch,
            names,
            works,
            graph,
            search,
        })
    }

    #[allow(missing_docs)]
    #[must_use]
    pub const fn epoch(&self) -> &Manifest {
        &self.epoch
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
        Ok(self.names.get(&key).map(|found| match found {
            Named::One(id) => match self.names.live(id) {
                Some(live) => Resolved::One(live),
                None => Resolved::Split(id),
            },
            Named::Shared(ids) => Resolved::Shared(ids),
        }))
    }

    /// Everything the merge knows about a work, and its text.
    ///
    /// # Errors
    ///
    /// A row that does not decode.
    pub fn work(&self, id: WorkId) -> Result<Option<Row>, Error> {
        self.works.get(id)
    }

    /// The works `id` cites, lazily, each with who asserted it.
    #[must_use]
    pub fn references(&self, id: WorkId) -> Arcs<'_> {
        self.graph.references(id)
    }

    /// The works citing `id`, lazily, each with who asserted it.
    #[must_use]
    pub fn citers(&self, id: WorkId) -> Arcs<'_> {
        self.graph.citers(id)
    }

    /// Works matching a query, with facet counts over the matches.
    ///
    /// # Errors
    ///
    /// A query the engine refuses.
    pub fn search(&self, text: &str, limit: usize) -> Result<Hits, Error> {
        Ok(self.search.search(text, limit, 5)?)
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
        let held = self.works.manifest().held;
        if self.search.documents() != held {
            return Err(broken(format!(
                "the lexical index holds {} documents, the works index {held} held works",
                self.search.documents()
            )));
        }
        let mut checked = Checked::default();
        for row in self.works.rows() {
            let row = row?;
            checked.rows += 1;
            for named in &row.names {
                if named.standing != merge::identity::Standing::Promoted {
                    continue;
                }
                let key = named.name.to_string();
                match self.names.get(&key) {
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
                match self.names.get(public_id) {
                    Some(Named::One(id)) if id == row.id => checked.public += 1,
                    other => {
                        return Err(broken(format!(
                            "{public_id} is {}'s public id but resolves to {other:?}",
                            row.id
                        )));
                    }
                }
            }
        }
        Ok(checked)
    }
}
