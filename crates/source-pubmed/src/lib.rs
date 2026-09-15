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
            ingest::Key::OpenAlex => {}
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
