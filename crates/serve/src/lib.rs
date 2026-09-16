//! The serving layer (serving plan §1): everything a query touches,
//! built over the merged layer and the graph, every artifact a cache.
//! [`names`] is the lookup from any name or public id to its work;
//! [`works`] the read index from a `WorkId` to everything the merge
//! knows about it and the text its records carry; [`epoch`] the manifest
//! that publishes a set of bases together; [`core`] the query core over
//! those two, the graph and the lexical index, which answers the five
//! questions of the plan's §7 and speaks `WorkId` inside.

pub mod core;
pub mod digest;
pub mod epoch;
pub mod error;
pub mod mapped;
pub mod names;
pub mod row;
pub mod works;

pub use core::{Resolved, Serving};
pub use error::Error;
pub use row::Row;

/// The layer's own version, beside the model's and the merge's: any
/// change to the names or works files or their meaning bumps it, and a
/// base under another version is rebuilt, never patched.
pub const VERSION: u32 = 1;

/// The directory name for this binary's versions.
#[must_use]
pub fn version() -> String {
    format!("v{}.{}.{}", model::VERSION, merge::VERSION, VERSION)
}
