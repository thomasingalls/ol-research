//! Shared ingestion plumbing (plan §3.3): the two traits every source crate
//! implements against, the runner that drives them, and reading raw files.
//!
//! [`Source`] is a source: its native record type, how a raw file's text
//! parses into those, and how one normalizes into what a [`Sink`] receives —
//! works, citation edges, deletions. [`normalize`] runs every record of
//! every file through both and stops at the first failure with the file,
//! the record's ordinal, the stage and the source's own error (plan §6).
//! Nothing here knows any source; the source crates depend on this one,
//! and the binary registers each with one line.
//!
//! Still to come here: fetch with manifests, the binary sink, the counting
//! allocator, parallelism over files.

pub mod gz;
pub mod run;
pub mod source;

pub use run::{Failure, Report, Stage, normalize, parse, read_raw};
pub use source::{Count, Sink, Source};
