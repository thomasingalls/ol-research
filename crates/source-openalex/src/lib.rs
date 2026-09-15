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

    fn normalize<'n, K: ingest::Sink<'n, Self>>(
        native: &'n Work<'_>,
        sink: &mut K,
    ) -> Result<(), normalize::Error> {
        normalize::normalize(native, sink)
    }
}
