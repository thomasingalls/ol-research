//! What a source crate implements for the merge (merge plan §8): the
//! questions the merge asks of a record, answered by the crate that
//! knows the record's shape. The merge is written once against the
//! answers, the way the runner is written once against [`Source`].

use ingest::Source;
use model::symbol::Table;

use crate::name::{Assertion, Name};
use crate::record::Citation;
use crate::relation::Related;
use crate::title::Titles;
use crate::view::{Text, Unmapped, View};
use crate::work::SourceId;

/// A source's records, as the merge sees them.
pub trait Contributor: Source {
    /// Which source, for bitmaps and precedence.
    const SOURCE: SourceId;

    /// The record's assertions (merge plan §3.1): its native id first,
    /// then every foreign id it carries that parsed. A malformed claim is
    /// not a name and is not emitted.
    fn names<'w>(work: &'w Self::Work<'_>, out: &mut dyn FnMut(Assertion<'w>));

    /// Whether the record is a container of other works — a journal, a
    /// volume or issue, a series ([`crate::Form::is_container`]) — whose
    /// aliases are then not its names (merge plan §11 item 28). No, unless
    /// the source says so.
    fn container(_work: &Self::Work<'_>) -> bool {
        false
    }

    /// The native id a references set carries with each edge, as a name.
    fn native<'w>(id: &'w Self::Id<'_>) -> Name<'w>;

    /// The record's revision stamp and version, as its view gives them
    /// (see [`View::revised`] and [`View::version`]), without building the
    /// view: what ranks two copies of one record (merge plan §8).
    fn revision(work: &Self::Work<'_>) -> (Option<model::Date>, u32);

    /// The record's titles, raw markup and all, for the corroboration of
    /// merge plan §3.3.
    fn titles<'w>(work: &'w Self::Work<'_>) -> Titles<'w>;

    /// The record's values for the promoted fields of §5, interned text
    /// resolved through the set's `table` (see [`crate::view::text`]),
    /// the source's own vocabularies mapped into the merge's.
    ///
    /// # Errors
    ///
    /// A value the mapping tables do not have, or a symbol the table does
    /// not: the run stops on it, or `--survey` counts it.
    fn view<'w>(work: &'w Self::Work<'_>, table: &'w Table) -> Result<View<'w>, Unmapped>;

    /// The typed relations of §6 the record asserts, each with the other
    /// work's name when it has one in our namespaces.
    ///
    /// # Errors
    ///
    /// A relation type the table does not have, or a symbol the set's
    /// table does not.
    fn relations(
        work: &Self::Work<'_>,
        table: &Table,
        out: &mut dyn FnMut(Related),
    ) -> Result<(), Unmapped>;

    /// The names a reference points at, each that parsed; none for a
    /// reference that carries only text or structured fields.
    fn targets<'w>(reference: &'w Self::Reference<'_>, out: &mut dyn FnMut(Name<'w>));

    /// What a reference gives besides names, for the unresolved stream of
    /// §6; empty for a source whose references are ids alone.
    fn citation<'w>(reference: &'w Self::Reference<'_>) -> Citation<'w>;

    /// The text the record carries beyond its promoted fields — the
    /// abstract as plain text, the authors' display names, the venue's
    /// name — for the serving layer's documents (serving plan §3).
    ///
    /// # Errors
    ///
    /// A symbol the set's table does not have, or an abstract encoded in
    /// a way that does not decode.
    fn text(work: &Self::Work<'_>, table: &Table) -> Result<Text, Unmapped>;
}
