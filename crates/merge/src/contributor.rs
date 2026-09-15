//! What a source crate implements for the merge (merge plan §8): the
//! three questions the merge asks of a record, answered by the crate that
//! knows the record's shape. The merge is written once against the
//! answers, the way the runner is written once against [`Source`].

use ingest::Source;

use crate::name::{Assertion, Name};
use crate::title::Titles;
use crate::work::SourceId;

/// A source's records, as the merge sees them.
pub trait Contributor: Source {
    /// Which source, for bitmaps and precedence.
    const SOURCE: SourceId;

    /// The record's assertions (merge plan §3.1): its native id first,
    /// then every foreign id it carries that parsed. A malformed claim is
    /// not a name and is not emitted.
    fn names<'w>(work: &'w Self::Work<'_>, out: &mut dyn FnMut(Assertion<'w>));

    /// The record's titles, raw markup and all, for the corroboration of
    /// merge plan §3.3.
    fn titles<'w>(work: &'w Self::Work<'_>) -> Titles<'w>;

    /// The names a reference points at, each that parsed; none for a
    /// reference that carries only text or structured fields.
    fn targets<'w>(reference: &'w Self::Reference<'_>, out: &mut dyn FnMut(Name<'w>));
}
