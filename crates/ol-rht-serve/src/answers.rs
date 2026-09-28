//! What the transport asks of whatever it serves: the serving layer's
//! core, or a fake in the tests.

use std::collections::HashMap;
use std::time::Instant;

use graph::Arc;
use merge::{Sources, WorkId};
use search::Hits;
use serve::epoch::Manifest;
use serve::{By, Expansion, Resolved, Row, Serving, Walk, Watermark};

/// Which list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// What a work cites.
    References,
    /// What cites a work.
    Citers,
}

/// One page of a list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    #[allow(missing_docs)]
    pub arcs: Vec<Arc>,
    /// The whole list's length.
    pub total: u64,
    /// The union of the page's asserters.
    pub by: Sources,
}

/// The counts an epoch answers with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counts {
    #[allow(missing_docs)]
    pub merged: String,
    #[allow(missing_docs)]
    pub nodes: u64,
    #[allow(missing_docs)]
    pub works: u64,
    #[allow(missing_docs)]
    pub arcs: u64,
}

/// The questions, answered synchronously.
pub trait Answers: Send + Sync + 'static {
    /// The epoch answering.
    fn epoch(&self) -> &Manifest;
    /// The batch every answer is as of: the epoch's, or a later one
    /// folded from the deltas (serving plan §5).
    fn watermark(&self) -> Watermark;
    /// Its counts.
    fn counts(&self) -> Counts;
    /// Which work a name or public id is.
    ///
    /// # Errors
    ///
    /// Text that is neither.
    fn resolve(&self, text: &str) -> Result<Option<Resolved>, serve::Error>;
    /// Everything the merge knows about a work.
    ///
    /// # Errors
    ///
    /// A row that does not decode.
    fn work(&self, id: WorkId) -> Result<Option<Row>, serve::Error>;
    /// The rows of many works in one call, which is how a hydrated page
    /// is hydrated: one walk of the index rather than a lookup per row
    /// (serving plan §11, item 31). An id with no row is absent from
    /// the map, as [`work`](Self::work) answers `None` for it.
    ///
    /// # Errors
    ///
    /// A row that does not decode.
    fn rows(&self, ids: &[WorkId]) -> Result<HashMap<WorkId, Row>, serve::Error>;
    /// A work's public id.
    fn public_of(&self, id: WorkId) -> Option<String>;
    /// The publisher namespaces names are held in (`reg1016`, …): where a
    /// publisher's id typed with no namespace is looked for.
    fn publisher_namespaces(&self) -> &[String];
    /// `n` arcs of a list from `offset`, and the list's length.
    fn page(&self, id: WorkId, direction: Direction, offset: u64, n: usize) -> Page;
    /// Works matching a query, as the query parser read it, stopped at
    /// `until`.
    ///
    /// # Errors
    ///
    /// The engine, or `search::Error::Deadline` past `until`.
    fn search(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        facets: usize,
        until: Instant,
    ) -> Result<Hits, serve::Error>;
    /// The top `limit` candidates for a citation's words, as the query
    /// parser read them, searched in the fields a citation names a work by
    /// (citation lookup plan, D3), stopped at `until`.
    ///
    /// # Errors
    ///
    /// The engine, or `search::Error::Deadline` past `until`.
    fn candidates(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        until: Instant,
    ) -> Result<Vec<search::Hit>, serve::Error>;
    /// One hop out from a seed set along `walk`, scored and ranked;
    /// `about` is the work the question is about, left out of its own
    /// answer.
    fn expand(
        &self,
        seeds: &[WorkId],
        walk: Walk,
        budget: usize,
        about: Option<WorkId>,
    ) -> Expansion;
    /// One hop out from a seed set the server gathers itself: the
    /// work's references or its citers, bounded and reported.
    fn related(&self, work: WorkId, by: By, budget: usize) -> Expansion;
    /// The shortest directed paths from one work to another, bounded.
    fn path(&self, from: WorkId, to: WorkId, bounds: graph::Bounds) -> graph::Found;
}

impl Answers for Serving {
    fn epoch(&self) -> &Manifest {
        Self::epoch(self)
    }

    fn watermark(&self) -> Watermark {
        Self::watermark(self)
    }

    fn counts(&self) -> Counts {
        Counts {
            merged: self.epoch().merged.clone(),
            nodes: u64::from(self.minted()),
            works: self.works().manifest().works,
            arcs: self.graph().arcs(),
        }
    }

    fn resolve(&self, text: &str) -> Result<Option<Resolved>, serve::Error> {
        Self::resolve(self, text)
    }

    fn work(&self, id: WorkId) -> Result<Option<Row>, serve::Error> {
        Self::work(self, id)
    }

    fn rows(&self, ids: &[WorkId]) -> Result<HashMap<WorkId, Row>, serve::Error> {
        Self::rows(self, ids)
    }

    fn public_of(&self, id: WorkId) -> Option<String> {
        Self::public_of(self, id)
    }

    fn publisher_namespaces(&self) -> &[String] {
        Self::publisher_namespaces(self)
    }

    fn page(&self, id: WorkId, direction: Direction, offset: u64, n: usize) -> Page {
        let list = |direction: Direction| match direction {
            Direction::References => self.references(id),
            Direction::Citers => self.citers(id),
        };
        let total = list(direction).count() as u64;
        let mut by = Sources::NONE;
        let arcs: Vec<Arc> = list(direction)
            .skip(usize::try_from(offset).unwrap_or(usize::MAX))
            .take(n)
            .inspect(|a| {
                for s in a.by.iter() {
                    by.insert(s);
                }
            })
            .collect();
        Page { arcs, total, by }
    }

    fn search(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        facets: usize,
        until: Instant,
    ) -> Result<Hits, serve::Error> {
        self.search_facets(asked, limit, facets, Some(until))
    }

    fn candidates(
        &self,
        asked: &query_parser::Query,
        limit: usize,
        until: Instant,
    ) -> Result<Vec<search::Hit>, serve::Error> {
        Self::candidates(self, asked, limit, Some(until))
    }

    fn expand(
        &self,
        seeds: &[WorkId],
        walk: Walk,
        budget: usize,
        about: Option<WorkId>,
    ) -> Expansion {
        Self::expand(self, seeds, walk, budget, about)
    }

    fn related(&self, work: WorkId, by: By, budget: usize) -> Expansion {
        Self::related(self, work, by, budget)
    }

    fn path(&self, from: WorkId, to: WorkId, bounds: graph::Bounds) -> graph::Found {
        Self::path(self, from, to, bounds)
    }
}
