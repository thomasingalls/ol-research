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
//! sources' raw markup. Nothing is normalized here.

pub mod error;
pub mod parse;
pub mod record;
pub mod vocab;

/// Reading raw files; shared with the other sources.
pub use ingest::gz;

pub use error::Error;
pub use parse::jsonl;
pub use record::Work;
