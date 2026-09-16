//! What stops a build, a read or a check.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use merge::WorkId;
use merge::record::Edge;

/// A build, a read or a check that stopped.
#[derive(Debug)]
pub enum Error {
    /// I/O, with the path it happened on.
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
    /// An edge out of `(from, to)` order, or repeated, in the edges set.
    Unsorted {
        #[allow(missing_docs)]
        previous: Edge,
        #[allow(missing_docs)]
        edge: Edge,
    },
    /// An edge whose end is not a work.
    UnknownEnd {
        #[allow(missing_docs)]
        edge: Edge,
    },
    /// The edges set was not the same in the second pass.
    Changed {
        #[allow(missing_docs)]
        what: String,
    },
    /// More distinct asserter sets than a byte can index.
    TooManyAsserters {
        #[allow(missing_docs)]
        count: usize,
    },
    /// A source name in a manifest this binary does not know.
    UnknownSource {
        #[allow(missing_docs)]
        name: String,
    },
    /// A work seen twice in the works set.
    DuplicateWork {
        #[allow(missing_docs)]
        id: WorkId,
    },
    /// A base of another version, or a file not the size its manifest says.
    Mismatch {
        #[allow(missing_docs)]
        what: String,
    },
    /// An invariant of the plan's §8 that does not hold.
    Invariant {
        #[allow(missing_docs)]
        what: String,
    },
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Manifest { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Unsorted { previous, edge } => write!(
                f,
                "edges set out of order: {} -> {} after {} -> {}",
                edge.from, edge.to, previous.from, previous.to
            ),
            Self::UnknownEnd { edge } => write!(
                f,
                "edge {} -> {} has an end that is not a work",
                edge.from, edge.to
            ),
            Self::Changed { what } => {
                write!(f, "the edges set changed between the two passes: {what}")
            }
            Self::TooManyAsserters { count } => write!(
                f,
                "{count} distinct asserter sets: more than a byte can index"
            ),
            Self::UnknownSource { name } => {
                write!(
                    f,
                    "the manifest names a source this binary does not know: {name}"
                )
            }
            Self::DuplicateWork { id } => write!(f, "work {id} appears twice in the works set"),
            Self::Mismatch { what } => f.write_str(what),
            Self::Invariant { what } => write!(f, "invariant broken: {what}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Manifest { source, .. } => Some(source),
            _ => None,
        }
    }
}
