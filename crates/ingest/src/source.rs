//! The compile-time plugin boundary of plan §3.2: what a source crate
//! implements, and what a sink implements. Factored from the three
//! normalizers that existed before it, not designed ahead of them.

use std::error::Error;
use std::io;

use serde::{Deserialize, Serialize};

/// A source of scholarly records: how its raw files parse into native
/// records, and how a native record normalizes into the records a
/// [`Sink`] receives.
///
/// The lifetimes are the point (plan §5). A native record borrows from the
/// text of one raw file; the normalized records borrow from the native
/// one; the sink sees each by reference, once. Nothing is copied on the
/// way from bytes to sink.
///
/// Parsing and normalizing are two methods of one trait because they share
/// the native type and nothing else needed a boundary between them: the
/// runner reads a file, asks for its records, and hands each to
/// [`Source::normalize`].
pub trait Source: Sized {
    /// The source's name in reports: `pubmed`, `crossref`, `openalex`.
    const NAME: &'static str;

    /// One record as the source serialized it, borrowed from a file's text.
    type Native<'a>;
    /// The source's native identifier (plan §4), which every normalized
    /// record and every edge carries.
    type Id<'n>: Serialize + Deserialize<'n>;
    /// A normalized work. Serializable, because the binary sink is what
    /// every normalized record is for; deserializable borrowing from the
    /// bytes it was read from, for the same reason it borrows from the
    /// native record.
    type Work<'n>: Serialize + Deserialize<'n>;
    /// A normalized citation edge, without the citing id it is emitted
    /// with.
    type Reference<'n>: Serialize + Deserialize<'n>;
    /// Why a record did not parse: structural strictness (plan §6).
    type ParseError: Error + Send + Sync + 'static;
    /// Why a value did not fit the model: value-level strictness (plan §6).
    type NormalizeError: Error + Send + Sync + 'static;

    /// The records of one raw file's text, each parsed on demand and
    /// borrowed from it. The first error ends the run; what follows it is
    /// never read.
    fn records(
        input: &str,
    ) -> impl Iterator<Item = Result<Self::Native<'_>, Self::ParseError>> + '_;

    /// Normalizes one record, pushing what it becomes to `sink`: a work and
    /// then each of its edges, or a deletion.
    ///
    /// # Errors
    ///
    /// The first value that does not fit the model. The sink may already
    /// have received part of the record.
    fn normalize<'n, K: Sink<'n, Self>>(
        native: &'n Self::Native<'_>,
        sink: &mut K,
    ) -> Result<(), Self::NormalizeError>;
}

/// Where normalized records go. Called by reference, once per record, in
/// the order the source emits them: a work, then each of its edges.
///
/// `'n` is the lifetime of the native record the values borrow from. The
/// runner requires a sink for every `'n`, so a sink that outlives one
/// file's records can keep nothing it was given: it serializes immediately
/// (plan §5, rule 3). A sink for one lifetime — a test collecting records —
/// is called through [`Source::normalize`] directly.
pub trait Sink<'n, S: Source> {
    /// One work.
    fn work(&mut self, work: &S::Work<'n>);
    /// One citation edge from the work most recently emitted.
    fn reference(&mut self, from: &S::Id<'n>, reference: &S::Reference<'n>);
    /// A record the source has withdrawn: PubMed's `DeleteCitation`.
    fn deletion(&mut self, id: &S::Id<'n>);

    /// The first I/O error since the last call, for sinks that write. The
    /// three methods above cannot fail, so a writing sink remembers its
    /// error and the runner asks after every record, stopping at the one
    /// the error hit. Sinks that do not write leave the default.
    fn failed(&mut self) -> Option<io::Error> {
        None
    }
}

/// Two sinks fed the same records, the first before the second: a stats
/// report beside the binary output.
impl<'n, S: Source, A: Sink<'n, S>, B: Sink<'n, S>> Sink<'n, S> for (A, B) {
    fn work(&mut self, work: &S::Work<'n>) {
        self.0.work(work);
        self.1.work(work);
    }

    fn reference(&mut self, from: &S::Id<'n>, reference: &S::Reference<'n>) {
        self.0.reference(from, reference);
        self.1.reference(from, reference);
    }

    fn deletion(&mut self, id: &S::Id<'n>) {
        self.0.deletion(id);
        self.1.deletion(id);
    }

    fn failed(&mut self) -> Option<io::Error> {
        self.0.failed().or_else(|| self.1.failed())
    }
}

/// A sink by mutable reference.
impl<'n, S: Source, K: Sink<'n, S>> Sink<'n, S> for &mut K {
    fn work(&mut self, work: &S::Work<'n>) {
        (**self).work(work);
    }

    fn reference(&mut self, from: &S::Id<'n>, reference: &S::Reference<'n>) {
        (**self).reference(from, reference);
    }

    fn deletion(&mut self, id: &S::Id<'n>) {
        (**self).deletion(id);
    }

    fn failed(&mut self) -> Option<io::Error> {
        (**self).failed()
    }
}

/// An optional sink: `None` receives nothing and never fails.
impl<'n, S: Source, K: Sink<'n, S>> Sink<'n, S> for Option<K> {
    fn work(&mut self, work: &S::Work<'n>) {
        if let Some(k) = self {
            k.work(work);
        }
    }

    fn reference(&mut self, from: &S::Id<'n>, reference: &S::Reference<'n>) {
        if let Some(k) = self {
            k.reference(from, reference);
        }
    }

    fn deletion(&mut self, id: &S::Id<'n>) {
        if let Some(k) = self {
            k.deletion(id);
        }
    }

    fn failed(&mut self) -> Option<io::Error> {
        self.as_mut().and_then(Sink::failed)
    }
}

/// A sink that counts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    #[allow(missing_docs)]
    pub works: u64,
    #[allow(missing_docs)]
    pub references: u64,
    #[allow(missing_docs)]
    pub deletions: u64,
}

impl<'n, S: Source> Sink<'n, S> for Count {
    fn work(&mut self, _: &S::Work<'n>) {
        self.works += 1;
    }

    fn reference(&mut self, _: &S::Id<'n>, _: &S::Reference<'n>) {
        self.references += 1;
    }

    fn deletion(&mut self, _: &S::Id<'n>) {
        self.deletions += 1;
    }
}
