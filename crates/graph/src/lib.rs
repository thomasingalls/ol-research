//! The citation graph (graph plan §1): one node per work, one arc per
//! distinct edge of the merged layer's edges set, in two orientations —
//! forward for a work's references, transpose for its citers — each a
//! compressed sparse row over positions, memory-mapped, with a label
//! beside every arc saying who asserted it.
//!
//! A cache, as the merged sets are a cache of the registry: built from the
//! edges set and the works set and nothing else, byte-identical from the
//! same inputs, replaced whole by every build. Nothing here knows a source
//! crate; the merge's [`WorkId`] and [`Sources`] are what it speaks.
//!
//! [`nodes`] reads what the works set says about every id. [`build`] makes
//! the base in two passes — degrees, then placement — through [`layout`],
//! which is the files and the manifest. [`base`] is the reader: a map and a
//! slice per file. [`check`] is the invariants, and [`report`] the numbers
//! every build prints.

pub mod base;
pub mod build;
pub mod check;
pub mod error;
pub mod layout;
pub mod nodes;
pub mod report;

pub use base::{Arc, Arcs, Base};
pub use error::Error;
pub use layout::Manifest;
pub use merge::{Sources, WorkId};
pub use nodes::Nodes;

/// The graph's own version, beside the model's and the merge's: any change
/// to the files under `graph/` or to their meaning bumps it, and a base
/// under another version is rebuilt, never patched.
pub const VERSION: u32 = 1;
