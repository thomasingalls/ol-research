//! OpenAlex works: the native record types and a strict, zero-copy reader
//! for the snapshot's JSON lines.
//!
//! OpenAlex documents its Work object descriptively and the schema moves
//! between snapshots, so this is the discovery path of plan §4a: the types
//! in [`record`] mirror what one 10,873-work part file of the 2026-06-26
//! snapshot contained — every object with `deny_unknown_fields`, every
//! enumerated string a closed enum — and a field or value outside that stops
//! the run with the line number and the work's id. A snapshot upgrade that
//! adds a field is meant to fail here first.
//!
//! The abstract arrives as an inverted index (word → positions); it is kept
//! as the raw JSON slice, uninterpreted, in the same spirit as the other
//! sources' raw markup. [`normalize`] is the value-level layer over it.

use std::io::BufRead;
use std::ops::ControlFlow;

pub mod error;
pub mod list;
pub mod normalize;
pub mod parse;
pub mod record;
pub mod view;
pub mod vocab;

pub use error::Error;
pub use parse::{jsonl, line, records};
pub use record::Work;

/// The source, for the ingest traits: snapshot part files, one work per
/// line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OpenAlex;

impl ingest::Keys for OpenAlex {
    fn work_keys(work: &normalize::Work<'_>, key: ingest::Key, out: &mut dyn FnMut(String)) {
        match key {
            ingest::Key::OpenAlex => out(work.id.to_string()),
            ingest::Key::Doi => {
                if let Some(doi) = work.doi.as_ref().and_then(model::Claim::valid) {
                    out(doi.as_str().to_owned());
                }
            }
            ingest::Key::Pmid => {
                if let Some(pmid) = work.pmid {
                    out(pmid.to_string());
                }
            }
            ingest::Key::Pmc | ingest::Key::Publisher => {}
        }
    }

    fn reference_keys(
        reference: &model::OpenAlexId,
        key: ingest::Key,
        out: &mut dyn FnMut(String),
    ) {
        if key == ingest::Key::OpenAlex {
            out(reference.to_string());
        }
    }
}

impl merge::Contributor for OpenAlex {
    const SOURCE: merge::SourceId = merge::SourceId::OpenAlex;

    fn names<'w>(work: &'w normalize::Work<'_>, out: &mut dyn FnMut(merge::Assertion<'w>)) {
        out(merge::Assertion {
            name: merge::Name::OpenAlex(work.id),
            field: "id",
            native: true,
        });
        if let Some(doi) = work.doi.as_ref().and_then(model::Claim::valid) {
            out(merge::Assertion {
                name: merge::Name::Doi(doi.as_str()),
                field: "doi",
                native: false,
            });
        }
        if let Some(pmid) = work.pmid {
            out(merge::Assertion {
                name: merge::Name::Pmid(pmid),
                field: "pmid",
                native: false,
            });
        }
    }

    fn native(id: &model::OpenAlexId) -> merge::Name<'_> {
        merge::Name::OpenAlex(*id)
    }

    fn revision(work: &Self::Work<'_>) -> (Option<model::Date>, u32) {
        (Some(work.updated.date()), 1)
    }

    fn titles<'w>(work: &'w normalize::Work<'_>) -> merge::Titles<'w> {
        merge::Titles {
            title: work.title.filter(|t| !t.trim().is_empty()),
            vernacular: None,
        }
    }

    fn view<'w>(
        work: &'w normalize::Work<'_>,
        table: &'w model::Table,
    ) -> Result<merge::View<'w>, merge::view::Unmapped> {
        view::view(work, table)
    }

    fn relations(
        work: &normalize::Work<'_>,
        _: &model::Table,
        out: &mut dyn FnMut(merge::relation::Related),
    ) -> Result<(), merge::view::Unmapped> {
        view::relations(work, out);
        Ok(())
    }

    fn targets<'w>(reference: &'w model::OpenAlexId, out: &mut dyn FnMut(merge::Name<'w>)) {
        out(merge::Name::OpenAlex(*reference));
    }

    fn citation(_: &model::OpenAlexId) -> merge::record::Citation<'_> {
        merge::record::Citation::default()
    }

    fn text(
        work: &normalize::Work<'_>,
        table: &model::Table,
    ) -> Result<merge::view::Text, merge::view::Unmapped> {
        view::text_of(work, table)
    }
}

impl ingest::Source for OpenAlex {
    const NAME: &'static str = "openalex";
    type Native<'a> = Work<'a>;
    type Id<'n> = model::OpenAlexId;
    type Work<'n> = normalize::Work<'n>;
    type Reference<'n> = model::OpenAlexId;
    type ParseError = Error;
    type NormalizeError = normalize::Error;

    fn records<R: BufRead, F>(input: R, each: F) -> Result<(), ingest::ReadError<Error>>
    where
        F: FnMut(&Work<'_>) -> ControlFlow<()>,
    {
        records(input, each)
    }

    /// The work's native name as the merge writes it, for the set's index.
    fn native_key(work: &Self::Work<'_>) -> Option<String> {
        merge::copies::native_of::<Self>(work).map(|name| name.to_string())
    }

    fn normalize<'n, K: ingest::Sink<'n, Self>>(
        native: &'n Work<'_>,
        sink: &mut K,
    ) -> Result<(), normalize::Error> {
        normalize::normalize(native, sink)
    }
}
