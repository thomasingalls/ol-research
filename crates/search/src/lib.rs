//! The lexical index (serving plan §3, SP2): the serving layer's
//! documents in a search engine, behind one shape the query core calls —
//! build from documents, open, search with facets. Tantivy is the
//! proof-of-concept engine, embedded, so the sink is a library call and
//! the tests run under cargo.
//!
//! A search is asked as the query parser's tree ([`query_parser`]),
//! never as text in the engine's own query language: `translate` is
//! the one place the tree becomes a Tantivy query, and `until` the
//! collector that stops a search at its deadline.
//!
//! The mapping — which fields are indexed and how, which are facets, what
//! is stored — is the user's to set. Until it is, [`schema::provisional`]
//! is one function that exercises the pipeline and says so, and every
//! report prints what it did.

pub mod analysis;
pub mod analyzer;
pub mod documents;
pub mod error;
pub mod index;
pub mod report;
pub mod schema;
pub(crate) mod translate;
mod until;

pub use error::Error;
pub use index::{Absorbed, Built, Hit, Hits, Manifest, Opened, Subset};

/// The index's own version, beside the model's and the merge's: any
/// change to the mapping or the files bumps it, and an index under
/// another version is rebuilt, never patched.
pub const VERSION: u32 = 1;

/// The engine, which names the directory under `search/`.
pub const ENGINE: &str = "tantivy";

/// The directory name for this binary's versions.
#[must_use]
pub fn version() -> String {
    format!("v{}.{}.{}", model::VERSION, merge::VERSION, VERSION)
}

/// Where this binary's index lives under a store.
#[must_use]
pub fn dir(store: &ingest::Store) -> std::path::PathBuf {
    store.search(ENGINE, model::VERSION, merge::VERSION, VERSION)
}
