//! The serving layer (serving plan §1): everything a query touches,
//! built over the merged layer and the graph, every artifact a cache.
//! [`names`] is the lookup from any name or public id to its work;
//! [`works`] the read index from a `WorkId` to everything the merge
//! knows about it and the text its records carry; [`epoch`] the manifest
//! that publishes a set of bases together; [`core`] the query core over
//! those two, the graph and the lexical index, which answers the five
//! questions of the plan's §7 and speaks `WorkId` inside; [`fold`] is
//! what every reader folds past its epoch — the registry's tail, the
//! works and edges deltas — under one watermark (§5).

pub mod cited;
pub mod compare;
pub mod core;
pub mod digest;
pub mod epoch;
pub mod error;
#[cfg(test)]
pub(crate) mod fixture;
pub mod fold;
pub mod incremental;
#[cfg(test)]
mod incremental_tests;
pub mod mapped;
pub mod names;
pub mod row;
pub mod works;

pub use core::{Resolved, Serving, Watermark};
pub use error::Error;
pub use fold::Arcs;
pub use row::Row;

/// The layer's own version, beside the model's and the merge's: any
/// change to the names or works files or their meaning bumps it, and a
/// base under another version is rebuilt, never patched. Version 3
/// (2026-09-16): a row's records carry their evidence, as the merged
/// record's do. Version 4 (2026-09-17): a row's records name their set and
/// version, and a stub name's sources are its citers'.
pub const VERSION: u32 = 4;

/// The directory name for this binary's versions.
#[must_use]
pub fn version() -> String {
    format!("v{}.{}.{}", model::VERSION, merge::VERSION, VERSION)
}
