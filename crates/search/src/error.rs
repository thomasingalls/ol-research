//! What stops a build, an open or a query.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// A build, an open or a query that stopped.
#[derive(Debug)]
pub enum Error {
    /// I/O, with the path.
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: io::Error,
    },
    /// The engine.
    Engine(#[allow(missing_docs)] tantivy::TantivyError),
    /// A query the engine's parser refuses.
    Query(#[allow(missing_docs)] tantivy::query::QueryParserError),
    /// A facet path that is not one.
    Facet(#[allow(missing_docs)] String),
    /// The manifest beside the index.
    Manifest {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: serde_json::Error,
    },
    /// An index of another version, or a schema without the mapping's
    /// fields.
    Mismatch(#[allow(missing_docs)] String),
}

impl From<tantivy::TantivyError> for Error {
    fn from(e: tantivy::TantivyError) -> Self {
        Self::Engine(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Engine(e) => write!(f, "tantivy: {e}"),
            Self::Query(e) => write!(f, "query: {e}"),
            Self::Facet(what) => write!(f, "facet: {what}"),
            Self::Manifest { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Mismatch(what) => f.write_str(what),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Engine(e) => Some(e),
            Self::Query(e) => Some(e),
            Self::Manifest { source, .. } => Some(source),
            Self::Facet(_) | Self::Mismatch(_) => None,
        }
    }
}
