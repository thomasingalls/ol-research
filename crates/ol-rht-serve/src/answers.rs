//! What the transport asks of whatever it serves: the serving layer's
//! core, or a fake in the tests.

use graph::Arc;
use merge::{Sources, WorkId};
use search::Hits;
use serve::epoch::Manifest;
use serve::{Resolved, Row, Serving, Watermark};

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
    /// A work's public id.
    fn public_of(&self, id: WorkId) -> Option<String>;
    /// `n` arcs of a list from `offset`, and the list's length.
    fn page(&self, id: WorkId, direction: Direction, offset: u64, n: usize) -> Page;
    /// Works matching a query.
    ///
    /// # Errors
    ///
    /// A query the engine refuses.
    fn search(&self, text: &str, limit: usize, facets: usize) -> Result<Hits, serve::Error>;
    /// One hop out from a seed set.
    fn expand(&self, seeds: &[WorkId], budget: usize) -> Vec<(WorkId, u32)>;
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

    fn public_of(&self, id: WorkId) -> Option<String> {
        Self::public_of(self, id)
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

    fn search(&self, text: &str, limit: usize, facets: usize) -> Result<Hits, serve::Error> {
        self.search_facets(text, limit, facets)
    }

    fn expand(&self, seeds: &[WorkId], budget: usize) -> Vec<(WorkId, u32)> {
        Self::expand(self, seeds, budget)
    }
}
