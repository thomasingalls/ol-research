//! Shared ingestion plumbing (plan §3.3): the two traits every source crate
//! implements against, the runner that drives them, and the record
//! framing that lets a worker hold one record's text instead of a file's.
//!
//! [`Source`] is a source: its native record type, how a raw file's byte
//! stream frames and parses into those, and how one normalizes into what
//! a [`Sink`] receives — works, citation edges, deletions. [`normalize`]
//! runs every record of every file through both and stops at the first
//! failure with the file, the record's ordinal, the stage and the source's
//! own error (plan §6). Nothing here knows any source; the source crates
//! depend on this one, and the binary registers each with one line.
//!
//! [`Binary`] is the first sink: the normalized layer on disk (plan §5,
//! rule 12), tagged with the model version, readable back through
//! [`Reader`]. [`alloc::Counting`] counts allocations for the report.
//!
//! [`Store`] is the store of record (plan §3.5): one root under which
//! raw bytes, listings, receipts and normalized output live. [`list`]
//! writes what a source offers as a [`listing`]; [`fetch`] brings the
//! bytes in, verified, and writes a [`receipt`] per file; [`stream`]
//! does both while normalizing, a download straight through the framer.
//!
//! Still to come here: symbol tables.

pub mod alloc;
pub mod binary;
pub mod fetch;
pub mod frame;
pub mod http;
pub mod list;
pub mod listing;
pub mod raw;
pub mod receipt;
pub mod run;
pub mod source;
pub mod store;
pub mod stream;
pub mod text;
pub mod time;

pub use binary::{Binary, Header, Kind, Reader, Written};
pub use frame::ReadError;
pub use http::Client;
pub use list::{Index, Lister};
pub use listing::Entry;
pub use receipt::Receipt;
pub use run::{Failure, Report, Seen, Stage, Survey, normalize, parse};
pub use source::{Count, Signature, Sink, Source};
pub use store::Store;
pub use text::Text;
