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
#[cfg(test)]
mod rows_tests;
#[cfg(test)]
mod traversal_tests;
pub mod works;

pub use core::{By, Expansion, Neighbour, Resolved, Serving, Walk, Watermark};
pub use error::Error;
pub use fold::Arcs;
pub use row::Row;

/// The layer's own version, beside the model's and the merge's: any
/// change to the names or works files or their meaning bumps it, and a
/// base under another version is rebuilt, never patched. Version 3
/// (2026-09-16): a row's records carry their evidence, as the merged
/// record's do. Version 4 (2026-09-17): a row's records name their set and
/// version, and a stub name's sources are its citers'. Version 5
/// (2026-09-19): the names base keeps the parts of every id a split
/// retired, so an id with no survivor answers with them rather than not
/// at all (serving plan §11, item 24). Version 6 (2026-09-19): the
/// cited-as rows carry the merged layer's new mark, whether a held work
/// stood beside the landing (merge plan §11, item 14; §11, item 26).
/// Version 7 (2026-09-25): a row's rejected assertions name the record
/// that made each and its field, as merge version 14's do, and the names
/// base keeps every refused name with the works refusing it
/// ([`names::REFUSED`]), which is how a batch finds a refused name's other
/// claimants (merge plan §11, item 24).
pub const VERSION: u32 = 7;

/// The directory name for this binary's versions.
#[must_use]
pub fn version() -> String {
    format!("v{}.{}.{}", model::VERSION, merge::VERSION, VERSION)
}
