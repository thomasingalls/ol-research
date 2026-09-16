//! What stops a build, an open, a check or a query.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// A build, an open, a check or a query that stopped.
#[derive(Debug)]
pub enum Error {
    /// I/O, with the path.
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: io::Error,
    },
    /// A manifest that does not parse or serialize.
    Manifest {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: serde_json::Error,
    },
    /// Another version's base, a moved directory, a file of the wrong
    /// size, or an epoch naming what is not there.
    Mismatch(#[allow(missing_docs)] String),
    /// The names transducer.
    Fst(#[allow(missing_docs)] fst::Error),
    /// A row that does not decode.
    Postcard(#[allow(missing_docs)] postcard::Error),
    #[allow(missing_docs)]
    Graph(graph::Error),
    #[allow(missing_docs)]
    Search(search::Error),
    /// Assembling the documents.
    Document(#[allow(missing_docs)] merge::document::Error),
    /// A check that failed.
    Invariant(#[allow(missing_docs)] String),
    /// No epoch has been published.
    NoEpoch,
    /// Text that is neither a name nor a public id.
    NotAName(#[allow(missing_docs)] String),
}

impl Error {
    /// An I/O error at `path`.
    #[must_use]
    pub fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

impl From<fst::Error> for Error {
    fn from(e: fst::Error) -> Self {
        Self::Fst(e)
    }
}

impl From<postcard::Error> for Error {
    fn from(e: postcard::Error) -> Self {
        Self::Postcard(e)
    }
}

impl From<graph::Error> for Error {
    fn from(e: graph::Error) -> Self {
        Self::Graph(e)
    }
}

impl From<search::Error> for Error {
    fn from(e: search::Error) -> Self {
        Self::Search(e)
    }
}

impl From<merge::document::Error> for Error {
    fn from(e: merge::document::Error) -> Self {
        Self::Document(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Manifest { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Mismatch(what) | Self::NotAName(what) => f.write_str(what),
            Self::Fst(e) => write!(f, "names: {e}"),
            Self::Postcard(e) => write!(f, "a row that does not decode: {e}"),
            Self::Graph(e) => write!(f, "graph: {e}"),
            Self::Search(e) => write!(f, "search: {e}"),
            Self::Document(e) => write!(f, "documents: {e}"),
            Self::Invariant(what) => write!(f, "check failed: {what}"),
            Self::NoEpoch => f.write_str("no epoch published; run epoch publish"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Manifest { source, .. } => Some(source),
            Self::Fst(e) => Some(e),
            Self::Postcard(e) => Some(e),
            Self::Graph(e) => Some(e),
            Self::Search(e) => Some(e),
            Self::Document(e) => Some(e),
            Self::Mismatch(_) | Self::Invariant(_) | Self::NoEpoch | Self::NotAName(_) => None,
        }
    }
}
