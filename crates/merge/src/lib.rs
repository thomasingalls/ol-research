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
//! references (§6) — which [`layout`] writes as sharded sets.

pub mod arena;
pub mod build;
pub mod contributor;
pub mod document;
pub mod identity;
pub mod journal;
pub mod kind;
pub mod layout;
pub mod name;
pub mod public;
pub mod record;
pub mod registry;
pub mod relation;
pub mod report;
pub mod sets;
pub mod title;
pub mod view;
pub mod work;

pub use build::{Builder, Ids, Merged};
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
pub const VERSION: u32 = 1;
