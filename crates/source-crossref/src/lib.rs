//! Crossref works JSON: the native record types and a strict, zero-copy
//! reader.
//!
//! Crossref publishes no schema for its JSON; `api_format.md` is
//! descriptive and behind the data (it does not know `role`, `award-info`,
//! `resource`, `project`, or `stm-asf`, and calls `issn-type` values
//! `pissn`/`eissn` where the data says `print`/`electronic`). So this is the
//! discovery path of plan §4a: the types in [`record`] mirror what 1,000
//! random works actually contained — every object with
//! `deny_unknown_fields`, every enumerated string a closed enum — and a
//! field or value outside that stops the run with the item's DOI. The
//! vocabularies with an authoritative source (`/types`, the Crossmark update
//! types, the deposit schema's lists) are that source's whole list; the
//! rest are exactly what was observed ([`vocab`]). A few records are kept
//! out on purpose ([`exclusions`]).
//!
//! Two containers hold the same record: the REST API envelope
//! (`message.items[]`, the POC input) and the public data file's JSON lines.
//! [`normalize`] is where values become `model` types.

use std::io::BufRead;
use std::ops::ControlFlow;

pub mod date;
pub mod error;
pub mod exclusions;
pub mod list;
pub mod listed_corrections;
pub mod normalize;
pub mod parse;
pub mod record;
pub mod view;
pub mod vocab;

pub use error::Error;
pub use parse::{Page, jsonl, records};
pub use record::Work;

/// The source, for the ingest traits: API pages or public-data-file lines,
/// one record schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Crossref;

impl ingest::Keys for Crossref {
    fn work_keys(work: &normalize::Work<'_>, key: ingest::Key, out: &mut dyn FnMut(String)) {
        if key == ingest::Key::Doi {
            out(work.doi.as_str().to_owned());
        }
    }

    fn reference_keys(
        reference: &normalize::Reference<'_>,
        key: ingest::Key,
        out: &mut dyn FnMut(String),
    ) {
        if key == ingest::Key::Doi
            && let Some(doi) = reference.doi.as_ref().and_then(model::Claim::valid)
        {
            out(doi.as_str().to_owned());
        }
    }
}

impl merge::Contributor for Crossref {
    const SOURCE: merge::SourceId = merge::SourceId::Crossref;

    fn names<'w>(work: &'w normalize::Work<'_>, out: &mut dyn FnMut(merge::Assertion<'w>)) {
        out(merge::Assertion {
            name: merge::Name::Doi(work.doi.as_str()),
            field: "doi",
            native: true,
        });
        // `alternative-id` is the publisher's own item id where it is not
        // the DOI again (merge plan §3.1), scoped by the registrant of
        // the DOI the record is registered under. The DOI-shaped ones —
        // 13,659 of the census's — are refused by the scope, whether they
        // restate this record's DOI or name another (§11, item 9).
        // The DOI spelled in ASCII, where it has letters beyond ASCII: the
        // name of a publisher's ASCII twin, when there is one.
        if let Some(ascii) = &work.ascii_doi {
            out(merge::Assertion {
                name: merge::Name::Doi(ascii.as_str()),
                field: "doi_ascii",
                native: false,
            });
        }
        // `aliases`: other DOIs the registry holds for the work.
        for alias in work.aliases.iter().filter_map(model::Claim::valid) {
            out(merge::Assertion {
                name: merge::Name::Doi(alias.as_str()),
                field: merge::ALIASES,
                native: false,
            });
        }
        let scope = view::publisher_scope(work);
        for value in &work.alternative_ids {
            if let Ok(name) = scope.name(value) {
                out(merge::Assertion {
                    name,
                    field: "alternative_ids",
                    native: false,
                });
            }
        }
    }

    fn container(work: &normalize::Work<'_>) -> bool {
        work.kind
            .is_some_and(|kind| view::form(kind).is_container())
    }

    fn corrects_its_aliases(work: &normalize::Work<'_>) -> bool {
        listed_corrections::corrects_its_aliases(&work.doi, work.deposited)
    }

    fn native<'w>(id: &'w model::Doi<'_>) -> merge::Name<'w> {
        merge::Name::Doi(id.as_str())
    }

    fn revision(work: &Self::Work<'_>) -> (Option<model::Date>, u32) {
        (Some(work.indexed.date()), 1)
    }

    fn titles<'w>(work: &'w normalize::Work<'_>) -> merge::Titles<'w> {
        merge::Titles {
            title: work.title.filter(|t| !t.trim().is_empty()),
            vernacular: work.alternate_titles.first().copied(),
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
        table: &model::Table,
        out: &mut dyn FnMut(merge::relation::Related),
    ) -> Result<(), merge::view::Unmapped> {
        view::relations(work, table, out)
    }

    fn targets<'w>(reference: &'w normalize::Reference<'_>, out: &mut dyn FnMut(merge::Name<'w>)) {
        if let Some(doi) = reference.doi.as_ref().and_then(model::Claim::valid) {
            out(merge::Name::Doi(doi.as_str()));
        }
    }

    fn citation<'w>(reference: &'w normalize::Reference<'_>) -> merge::record::Citation<'w> {
        view::citation(reference)
    }

    fn text(
        work: &normalize::Work<'_>,
        table: &model::Table,
    ) -> Result<merge::view::Text, merge::view::Unmapped> {
        view::text_of(work, table)
    }
}

impl ingest::Source for Crossref {
    const NAME: &'static str = "crossref";
    type Native<'a> = Work<'a>;
    type Id<'n> = model::Doi<'n>;
    type Work<'n> = normalize::Work<'n>;
    type Reference<'n> = normalize::Reference<'n>;
    type ParseError = Error;
    type NormalizeError = normalize::Error;

    fn records<R: BufRead, F>(input: R, each: F) -> Result<(), ingest::ReadError<Error>>
    where
        F: FnMut(Result<&Work<'_>, Error>) -> ControlFlow<()>,
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
