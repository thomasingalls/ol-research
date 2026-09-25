//! A search that stops when its time is up. Tantivy has no deadline of
//! its own and nothing that cancels a search in flight, but it lets a
//! collector walk a segment's matches itself (`collect_segment`), so this
//! one walks them as Tantivy's own loop does and looks at the clock every
//! [`EVERY`] documents. Walking them itself costs no pruning the search
//! had: the count and the facet counts visit every match anyway.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tantivy::collector::{Collector, SegmentCollector};
use tantivy::query::Weight;
use tantivy::{DocSet, Score, SegmentOrdinal, SegmentReader, TERMINATED, TantivyError};

/// Documents between two looks at the clock.
const EVERY: u32 = 4096;

/// `inner`, stopped at `deadline`.
pub(crate) struct Until<C> {
    inner: C,
    deadline: Instant,
    expired: AtomicBool,
}

impl<C> Until<C> {
    pub(crate) const fn new(inner: C, deadline: Instant) -> Self {
        Self {
            inner,
            deadline,
            expired: AtomicBool::new(false),
        }
    }

    /// Whether the search stopped because its time was up, which is how
    /// the error it stopped with is told from the engine's own.
    pub(crate) fn expired(&self) -> bool {
        self.expired.load(Ordering::Relaxed)
    }

    fn check(&self) -> tantivy::Result<()> {
        if Instant::now() >= self.deadline {
            self.expired.store(true, Ordering::Relaxed);
            return Err(TantivyError::InvalidArgument(
                "the search ran past its time limit".to_owned(),
            ));
        }
        Ok(())
    }
}

impl<C: Collector> Collector for Until<C> {
    type Fruit = C::Fruit;
    type Child = C::Child;

    fn for_segment(
        &self,
        ord: SegmentOrdinal,
        reader: &SegmentReader,
    ) -> tantivy::Result<C::Child> {
        self.inner.for_segment(ord, reader)
    }

    fn requires_scoring(&self) -> bool {
        self.inner.requires_scoring()
    }

    fn merge_fruits(
        &self,
        fruits: Vec<<C::Child as SegmentCollector>::Fruit>,
    ) -> tantivy::Result<C::Fruit> {
        self.inner.merge_fruits(fruits)
    }

    fn collect_segment(
        &self,
        weight: &dyn Weight,
        ord: SegmentOrdinal,
        reader: &SegmentReader,
    ) -> tantivy::Result<<C::Child as SegmentCollector>::Fruit> {
        self.check()?;
        let mut child = self.for_segment(ord, reader)?;
        let scoring = self.requires_scoring();
        let alive = reader.alive_bitset();
        let mut scorer = weight.scorer(reader, 1.0)?;
        let mut doc = scorer.doc();
        let mut walked: u32 = 0;
        while doc != TERMINATED {
            if alive.is_none_or(|a| a.is_alive(doc)) {
                let score: Score = if scoring { scorer.score() } else { 0.0 };
                child.collect(doc, score);
            }
            walked = walked.wrapping_add(1);
            if walked.is_multiple_of(EVERY) {
                self.check()?;
            }
            doc = scorer.advance();
        }
        Ok(child.harvest())
    }
}
