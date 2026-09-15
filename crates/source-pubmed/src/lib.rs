//! PubMed baseline and update XML: the native record types and a strict,
//! zero-copy streaming parser.
//!
//! The native types in [`record`] mirror the DTD the files declare
//! (`pubmed_250101.dtd`, kept verbatim in `fixtures/`), which is PubMed's
//! normative description of its own serializer (plan §4a). The mapping is
//! mechanical: a `#PCDATA` leaf is a [`Text`]; mixed content —
//! `(%text; | mml:math)*` — is a [`RawXml`] span kept exactly as serialized;
//! an enumerated attribute is an enum in [`attr`]; cardinality is enforced.
//! Order of children is not enforced yet.
//!
//! Nothing is normalized in [`record`]. Dates stay text, identifiers stay
//! text; [`normalize`] turns them into `model` types and is where the strict
//! value rules bite. This layer's strictness is structural: an element, attribute
//! or attribute value the DTD does not allow stops the run with a byte
//! offset, an element path and the record's PMID.

use std::io::BufRead;
use std::ops::ControlFlow;

pub mod attr;
pub mod error;
pub mod list;
pub mod normalize;
mod parse;
pub mod record;
pub mod text;
pub mod view;

pub use error::{Error, ErrorKind};
pub use parse::{ArticleSet, records};
pub use text::{RawXml, Text};

/// The source, for the ingest traits: baseline and update files, each a
/// `PubmedArticleSet` of citations and, in update files, deletions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pubmed;

impl ingest::Keys for Pubmed {
    fn work_keys(work: &normalize::Citation<'_>, key: ingest::Key, out: &mut dyn FnMut(String)) {
        match key {
            ingest::Key::Pmid => out(work.pmid.to_string()),
            ingest::Key::Doi => {
                if let Some(doi) = work.ids.doi.as_ref().and_then(model::Claim::valid) {
                    out(doi.as_str().to_owned());
                }
            }
            ingest::Key::Pmc => {
                if let Some(pmc) = work.ids.pmc {
                    out(pmc.to_string());
                }
            }
            ingest::Key::OpenAlex => {}
        }
    }

    fn reference_keys(
        reference: &normalize::Reference<'_>,
        key: ingest::Key,
        out: &mut dyn FnMut(String),
    ) {
        match key {
            ingest::Key::Pmid => {
                if let Some(pmid) = reference.pmid.as_ref().and_then(model::Claim::valid) {
                    out(pmid.to_string());
                }
            }
            ingest::Key::Doi => {
                if let Some(doi) = reference.doi.as_ref().and_then(model::Claim::valid) {
                    out(doi.as_str().to_owned());
                }
            }
            ingest::Key::Pmc => {
                if let Some(pmc) = reference.pmc.as_ref().and_then(model::Claim::valid) {
                    out(pmc.to_string());
                }
            }
            ingest::Key::OpenAlex => {}
        }
    }
}

impl merge::Contributor for Pubmed {
    const SOURCE: merge::SourceId = merge::SourceId::Pubmed;

    fn names<'w>(work: &'w normalize::Citation<'_>, out: &mut dyn FnMut(merge::Assertion<'w>)) {
        out(merge::Assertion {
            name: merge::Name::Pmid(work.pmid),
            field: "pmid",
            native: true,
        });
        if let Some(doi) = work.ids.doi.as_ref().and_then(model::Claim::valid) {
            out(merge::Assertion {
                name: merge::Name::Doi(doi.as_str()),
                field: "ids.doi",
                native: false,
            });
        }
        for doi in work
            .ids
            .elocation_doi
            .iter()
            .filter_map(model::Claim::valid)
        {
            out(merge::Assertion {
                name: merge::Name::Doi(doi.as_str()),
                field: "ids.elocation_doi",
                native: false,
            });
        }
        if let Some(pmc) = work.ids.pmc {
            out(merge::Assertion {
                name: merge::Name::Pmc(pmc),
                field: "ids.pmc",
                native: false,
            });
        }
    }

    fn native(id: &model::Pmid) -> merge::Name<'_> {
        merge::Name::Pmid(*id)
    }

    fn titles<'w>(work: &'w normalize::Citation<'_>) -> merge::Titles<'w> {
        merge::Titles {
            title: Some(work.title).filter(|t| !t.trim().is_empty()),
            vernacular: work.vernacular_title,
        }
    }

    fn view<'w>(
        work: &'w normalize::Citation<'_>,
        table: &'w model::Table,
    ) -> Result<merge::View<'w>, merge::view::Unmapped> {
        view::view(work, table)
    }

    fn relations(
        work: &normalize::Citation<'_>,
        _: &model::Table,
        out: &mut dyn FnMut(merge::relation::Related),
    ) -> Result<(), merge::view::Unmapped> {
        for r in &work.relations {
            out(merge::relation::Related {
                kind: view::relation_kind(r.kind),
                target: r.pmid.map(merge::OwnedName::Pmid),
            });
        }
        Ok(())
    }

    fn targets<'w>(reference: &'w normalize::Reference<'_>, out: &mut dyn FnMut(merge::Name<'w>)) {
        if let Some(pmid) = reference.pmid.as_ref().and_then(model::Claim::valid) {
            out(merge::Name::Pmid(*pmid));
        }
        if let Some(doi) = reference.doi.as_ref().and_then(model::Claim::valid) {
            out(merge::Name::Doi(doi.as_str()));
        }
        if let Some(pmc) = reference.pmc.as_ref().and_then(model::Claim::valid) {
            out(merge::Name::Pmc(*pmc));
        }
    }

    fn citation<'w>(reference: &'w normalize::Reference<'_>) -> merge::record::Citation<'w> {
        merge::record::Citation {
            text: Some(reference.citation).filter(|t| !t.trim().is_empty()),
            ..merge::record::Citation::default()
        }
    }
}

impl ingest::Source for Pubmed {
    const NAME: &'static str = "pubmed";
    type Native<'a> = record::Record<'a>;
    type Id<'n> = model::Pmid;
    type Work<'n> = normalize::Citation<'n>;
    type Reference<'n> = normalize::Reference<'n>;
    type ParseError = Error;
    type NormalizeError = normalize::Error;

    fn records<R: BufRead, F>(input: R, each: F) -> Result<(), ingest::ReadError<Error>>
    where
        F: FnMut(&record::Record<'_>) -> ControlFlow<()>,
    {
        records(input, each)
    }

    fn normalize<'n, K: ingest::Sink<'n, Self>>(
        native: &'n record::Record<'_>,
        sink: &mut K,
    ) -> Result<(), normalize::Error> {
        match native {
            record::Record::Article(article) => normalize::normalize(article, sink),
            record::Record::DeleteCitation(deleted) => normalize::deletions(deleted, sink),
        }
    }
}
