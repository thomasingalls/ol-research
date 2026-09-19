//! The merged layer (merge plan §1): one work across sources, built from
//! the normalized layer and nothing else. Identity — which records are one
//! work — lives here because it needs every record at once, which is
//! exactly what ingestion never needs.
//!
//! The vocabulary is the plan's §2. A [`WorkId`] is ours: minted by the
//! [`registry`], dense, opaque, derived from nothing. A [`Name`] is what
//! the world calls a work — a DOI, a PMID, a PMC id, an OpenAlex id — and
//! a work has many, each with who asserted it and a verdict. A *record* is
//! one source's normalized record of the work; a *stub* is a work with
//! names and no records, cited by something we hold.
//!
//! Nothing here knows a source. Each source crate implements
//! [`Contributor`] for its own records — the names a work asserts, its
//! titles, the names a reference points at — and the binary registers
//! each with one line, the way it does for [`ingest::Source`].
//!
//! [`identity`] is the first pass of the plan's §8: every work's
//! assertions, the components they connect, the verdicts of §3.3, and the
//! works those leave. [`registry`] is the plan's §4: the one part of the
//! merged layer that is of record. [`build`] is the second pass: every
//! record's [`view`] joins its work, every reference resolves through
//! the names, and the rows of [`record`] are made — the merged record
//! with its promoted fields (§5), the edges, relations and unresolved
//! references (§6) — which [`layout`] writes as sharded sets. [`delta`] is
//! what an incremental merge appends beside them (serving plan §5), and
//! [`journal`] the clock every run advances.

pub mod arena;
pub mod build;
pub mod contributor;
pub mod copies;
pub mod delta;
pub mod document;
pub mod identity;
pub mod journal;
pub mod kind;
pub mod layout;
pub mod name;
pub mod public;
pub mod publisher;
pub mod record;
pub mod registry;
pub mod relation;
pub mod report;
pub mod sets;
pub mod title;
pub mod view;
pub mod work;

pub use build::{Builder, Ids, Merged, OwnedWork};
pub use contributor::Contributor;
pub use document::{Assembler, Document};
pub use identity::Identity;
pub use kind::{Form, Genre};
pub use name::{Assertion, Name, OwnedName};
pub use registry::Registry;
pub use title::Titles;
pub use view::View;
pub use work::{SourceId, Sources, WorkId};

/// The merge's own version, beside the model's: any change to the rules
/// of the plan's §3 or §5 or to the shapes written under `merged/` bumps
/// it, and merged sets under another version are rebuilt, never patched.
/// Version 2 (2026-09-16): every record in a merged row carries its
/// assertions and its view, the evidence the incremental merge re-runs
/// the rules on, and a row's lists are in canonical order. Version 3
/// (2026-09-17): the newest copy of a record speaks for all of it, its
/// view names its set and version, and a stub name's sources are its
/// citers'. Version 4 (2026-09-18): the newest copy still speaks for
/// names, fields, relations and text, but a record's citations are the
/// union over every copy a deletion has not withdrawn — PubMed replaces a
/// publisher's reference list with PMC's when an article lands there, and
/// the identifiers the publisher gave are evidence we already hold.
/// Version 5 (2026-09-18): absence is not an assertion. A record's names
/// are the union over its standing copies, and each promoted field is
/// the newest copy's value or, where the newest has none, the newest
/// copy that has one; its text follows the same rule. Deletions,
/// relations and a value that is merely shorter are unchanged.
/// Version 6 (2026-09-18): a reference whose names name different works
/// is settled by its own citation text where that text repeats one of
/// its DOIs, the order of §6 deciding only where nothing corroborates.
/// Version 7 (2026-09-19): a stub's name carries, with each citer,
/// whether that reference's own citation text repeats the name — what
/// lets a reader settle the reference again, by §6 and not by an
/// imitation of it, when a record takes over part of the stub.
/// Version 8 (2026-09-19): publisher ids are names. A publisher's own
/// identifier for an article — PubMed's two `pii` slots, Crossref's
/// non-DOI `alternative-id` — is a name in a namespace of its own,
/// `reg1016:S0006349525001234`, keyed by the registrant of the record's
/// DOI and written exactly as the publisher wrote it but for Elsevier's
/// printed PII, which is written bare ([`publisher`]). Every merged row,
/// the registry and the delta logs gain names of that namespace, and
/// extending `publisher::ELSEVIER` changes the canonical spelling of
/// names already written, so that too is a full merge. With them, a
/// rejected assertion carries which rule rejected it
/// ([`identity::Reason`]) and a holder only where a record holds the
/// name, because a publisher id one source claims for more than one of
/// its own records is no article's identifier and no record holds it.
pub const VERSION: u32 = 8;
