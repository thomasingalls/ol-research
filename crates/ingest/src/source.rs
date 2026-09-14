//! The compile-time plugin boundary of plan §3.2: what a source crate
//! implements, and what a sink implements. Factored from the three
//! normalizers that existed before it, not designed ahead of them.

use std::error::Error;

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
    type Id<'n>;
    /// A normalized work.
    type Work<'n>;
    /// A normalized citation edge, without the citing id it is emitted
    /// with.
    type Reference<'n>;
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
