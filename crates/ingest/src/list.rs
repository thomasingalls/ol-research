//! `list`: what a source offers right now, written to the store as a
//! listing (plan §3.5) — the input `fetch` takes. A source knows its own
//! index document and how to read it; this module fetches the document,
//! keeps it beside the listing, and writes the listing the source
//! derived from it.

use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::http::{self, Client};
use crate::listing::{self, Entry};
use crate::source::Source;
use crate::store::Store;
use crate::time;

/// A source that can say what it offers.
pub trait Lister: Source {
    /// Every file the source offers now, from its index document(s),
    /// each fetched through `index` so it is kept beside the listing.
    /// Strict: an index that does not look as expected is an error, not
    /// a shorter listing.
    ///
    /// # Errors
    ///
    /// Fetching an index document, or reading it.
    fn list(index: &mut dyn Index) -> Result<Vec<Entry>, Error>;
}

/// Fetches index documents for a [`Lister`].
pub trait Index {
    /// The text at `url`, kept as `<name>` beside the listing.
    ///
    /// # Errors
    ///
    /// Fetching or keeping it.
    fn document(&mut self, url: &str, name: &str) -> Result<String, Error>;
}

/// Why a listing could not be made.
#[derive(Debug)]
pub enum Error {
    #[allow(missing_docs)]
    Http(http::Error),
    /// I/O on a store path.
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        error: io::Error,
    },
    /// An index document that does not look as expected.
    Index {
        /// The document's name.
        name: String,
        #[allow(missing_docs)]
        reason: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(e) => write!(f, "{e}"),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Index { name, reason } => write!(f, "{name}: {reason}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Io { error, .. } => Some(error),
            Self::Index { .. } => None,
        }
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Self {
        Self::Http(e)
    }
}

/// A listing written: where, and what.
#[derive(Debug)]
pub struct Listed {
    /// `listings/<source>/<stamp>.jsonl`.
    pub path: PathBuf,
    #[allow(missing_docs)]
    pub entries: Vec<Entry>,
}

struct StoreIndex<'a> {
    client: &'a Client,
    store: &'a Store,
    source: &'a str,
    stamp: &'a str,
}

impl Index for StoreIndex<'_> {
    fn document(&mut self, url: &str, name: &str) -> Result<String, Error> {
        let text = self.client.text(url)?;
        let path = self.store.listing_document(self.source, self.stamp, name);
        let keep = || -> io::Result<()> {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&path, &text)
        };
        keep().map_err(|error| Error::Io {
            path: path.clone(),
            error,
        })?;
        Ok(text)
    }
}

/// Lists `L` into the store: its index documents and the listing, named
/// by the moment it was taken.
///
/// # Errors
///
/// The lister's, or writing the listing.
pub fn list<L: Lister>(store: &Store, client: &Client) -> Result<Listed, Error> {
    let stamp = time::now().replace(':', "-");
    let mut index = StoreIndex {
        client,
        store,
        source: L::NAME,
        stamp: &stamp,
    };
    let entries = L::list(&mut index)?;
    let path = store.listing(L::NAME, &stamp);
    listing::write(&path, &entries).map_err(|error| Error::Io {
        path: path.clone(),
        error,
    })?;
    Ok(Listed { path, entries })
}
