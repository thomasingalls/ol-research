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
//! types) are complete; the rest are exactly what was observed.
//!
//! Two containers hold the same record: the REST API envelope
//! (`message.items[]`, the POC input) and the public data file's JSON lines.
//! [`normalize`] is where values become `model` types.

pub mod date;
pub mod error;
pub mod normalize;
pub mod parse;
pub mod record;
pub mod vocab;

/// Reading raw files; shared with the other sources.
pub use ingest::gz;

pub use error::Error;
pub use parse::{Page, jsonl};
pub use record::Work;
