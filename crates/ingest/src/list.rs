//! `list`: what a source offers right now, written to the store as a
//! listing (plan §3.5) — the input `fetch` takes. A source knows its own
//! index document and how to read it; this module fetches the document,
//! keeps it beside the listing, and writes the listing the source
//! derived from it.
//!
//! A *lookup* listing is the other kind (plan §8, item 8): a seed of ids,
//! chunked into the requests that answer them, every URL known before
//! anything is fetched, the seed kept beside the listing. It exists to
//! seed the POC sample; a source's bulk files stay the main path.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use model::{Doi, OpenAlexId, PmcId, Pmid};

use crate::http::{self, Client};
use crate::keys::Key;
use crate::listing::{self, Entry, Kind};
use crate::source::Source;
use crate::store::Store;
use crate::time;

/// A source that can say what it offers, and answer a batch of ids.
pub trait Lister: Source {
    /// The least interval between two lookup requests to this source:
    /// gentle by construction (plan §8, item 8), well inside what the
    /// source publishes.
    const PACE: Duration;

    /// The query parameter that carries a contact address when the
    /// source rewards one (`mailto`, `email`). Added by `fetch --mailto`
    /// only; never written into a listing.
    const CONTACT: Option<&'static str>;

    /// Every file the source offers now, from its index document(s),
    /// each fetched through `index` so it is kept beside the listing.
    /// Strict: an index that does not look as expected is an error, not
    /// a shorter listing.
    ///
    /// # Errors
    ///
    /// Fetching an index document, or reading it.
    fn list(index: &mut dyn Index) -> Result<Vec<Entry>, Error>;

    /// How many ids of `key` one lookup request carries, or `None` when
    /// the source cannot look `key` up.
    fn batch(key: Key) -> Option<usize>;

    /// The request answering one batch of `key` ids, each canonical.
    ///
    /// # Errors
    ///
    /// A key the source cannot look up.
    fn lookup(key: Key, ids: &[&str]) -> Result<Lookup, Error>;
}

/// One lookup request: the page that answers a batch of ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    /// The request, without any contact parameter.
    pub url: String,
    /// The page's file extension: `json`, `xml`.
    pub extension: &'static str,
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
    /// A seed line that is not an id of the seed's key, or repeats one.
    Seed {
        #[allow(missing_docs)]
        line: usize,
        #[allow(missing_docs)]
        id: String,
        #[allow(missing_docs)]
        reason: String,
    },
    /// A seed with no ids in it.
    Empty(PathBuf),
    /// A key the source cannot look up.
    Unsupported(Key),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(e) => write!(f, "{e}"),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Index { name, reason } => write!(f, "{name}: {reason}"),
            Self::Seed { line, id, reason } => write!(f, "seed line {line}: {id:?}: {reason}"),
            Self::Empty(path) => write!(f, "{}: no ids in the seed", path.display()),
            Self::Unsupported(key) => write!(f, "this source cannot look up a {key}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Io { error, .. } => Some(error),
            Self::Index { .. } | Self::Seed { .. } | Self::Empty(_) | Self::Unsupported(_) => None,
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
        keep(&path, &text)?;
        Ok(text)
    }
}

/// Writes `text` at `path`, creating its directory.
fn keep(path: &Path, text: &str) -> Result<(), Error> {
    let write = || -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, text)
    };
    write().map_err(|error| Error::Io {
        path: path.to_path_buf(),
        error,
    })
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
    write::<L>(store, &stamp, entries)
}

/// Lists the lookups answering `seed` — a file of `key` ids, one per
/// line, blank lines skipped — into the store: one entry per batch of
/// `L::batch(key)` ids, at `lookup/<stamp>/<key>-<n>.<ext>`, and the seed
/// itself beside the listing, canonical. A line that is not an id of
/// `key`, or repeats one, is an error: a seed is made by `keys` and is
/// exact.
///
/// # Errors
///
/// A key `L` cannot look up, reading the seed, a bad line, or writing.
pub fn list_seed<L: Lister>(store: &Store, key: Key, seed: &Path) -> Result<Listed, Error> {
    let per = L::batch(key).ok_or(Error::Unsupported(key))?;
    let text = fs::read_to_string(seed).map_err(|error| Error::Io {
        path: seed.to_path_buf(),
        error,
    })?;
    let ids = canonical(key, &text)?;
    if ids.is_empty() {
        return Err(Error::Empty(seed.to_path_buf()));
    }
    let stamp = time::now().replace(':', "-");
    let mut entries = Vec::with_capacity(ids.len().div_ceil(per));
    for (n, batch) in ids.chunks(per).enumerate() {
        let refs: Vec<&str> = batch.iter().map(String::as_str).collect();
        let lookup = L::lookup(key, &refs)?;
        entries.push(Entry {
            url: lookup.url,
            path: format!("lookup/{stamp}/{key}-{:05}.{}", n + 1, lookup.extension),
            kind: Kind::Lookup,
            bytes: None,
            records: Some(batch.len() as u64),
            md5_url: None,
            release: None,
        });
    }
    let mut lines = ids.join("\n");
    lines.push('\n');
    keep(&store.listing_document(L::NAME, &stamp, "seed"), &lines)?;
    write::<L>(store, &stamp, entries)
}

fn write<L: Lister>(store: &Store, stamp: &str, entries: Vec<Entry>) -> Result<Listed, Error> {
    let path = store.listing(L::NAME, stamp);
    listing::write(&path, &entries).map_err(|error| Error::Io {
        path: path.clone(),
        error,
    })?;
    Ok(Listed { path, entries })
}

/// The ids of a seed's text, each canonical for `key`, in order.
fn canonical(key: Key, text: &str) -> Result<Vec<String>, Error> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut ids = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let id = parse(key, line).map_err(|reason| Error::Seed {
            line: index + 1,
            id: line.to_owned(),
            reason,
        })?;
        if let Some(first) = seen.insert(id.clone(), index + 1) {
            return Err(Error::Seed {
                line: index + 1,
                id,
                reason: format!("repeats line {first}"),
            });
        }
        ids.push(id);
    }
    Ok(ids)
}

/// One id in its canonical form, through the model's own parser.
fn parse(key: Key, s: &str) -> Result<String, String> {
    match key {
        Key::Doi => Doi::parse(s)
            .map(|d| d.as_str().to_owned())
            .map_err(|e| e.to_string()),
        Key::Pmid => Pmid::from_digits(s)
            .map(|p| p.to_string())
            .map_err(|e| e.to_string()),
        Key::Pmc => PmcId::from_prefixed(s)
            .map(|p| p.to_string())
            .map_err(|e| e.to_string()),
        Key::OpenAlex => OpenAlexId::from_bare(s)
            .map(|w| w.to_string())
            .map_err(|e| e.to_string()),
        // No source looks up by a publisher id (merge plan §3.1), and its
        // namespace is a registrant rather than a fixed word, so it is
        // never a seed and never a listing path.
        Key::Publisher => Err("no source looks up by a publisher id".to_owned()),
    }
}

/// `s` as a query-string value: RFC 3986 unreserved bytes, `/` and `:`
/// kept, everything else percent-encoded, so an id holding `,`, `|`, `&`,
/// `#`, `+` or a space cannot break the request that carries it.
#[must_use]
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                out.push(char::from(b));
            }
            _ => {
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Error, canonical, encode};
    use crate::keys::Key;

    #[test]
    fn a_seed_is_canonical_distinct_ids_of_one_key() {
        let ids = canonical(Key::Doi, "10.1000/A\n\n 10.1000/b \n").unwrap();
        assert_eq!(ids, ["10.1000/a", "10.1000/b"]);
        let Err(Error::Seed { line, id, reason }) = canonical(Key::Doi, "10.1000/a\n10.1000/A\n")
        else {
            panic!("a repeat")
        };
        assert_eq!((line, id.as_str()), (2, "10.1000/a"));
        assert!(reason.contains("line 1"), "{reason}");
        let Err(Error::Seed { line, .. }) = canonical(Key::Pmid, "123\nx\n") else {
            panic!("not a pmid")
        };
        assert_eq!(line, 2);
        assert_eq!(canonical(Key::OpenAlex, "W12\n").unwrap(), ["W12"]);
        assert!(canonical(Key::OpenAlex, "https://openalex.org/W12\n").is_err());
        assert!(canonical(Key::Pmid, "").unwrap().is_empty());
    }

    #[test]
    fn query_values_are_percent_encoded_except_what_ids_are_made_of() {
        assert_eq!(encode("10.1000/x.y_z~1:2"), "10.1000/x.y_z~1:2");
        assert_eq!(encode("a,b|c&d#e+f g%"), "a%2Cb%7Cc%26d%23e%2Bf%20g%25");
        assert_eq!(encode("(2020)"), "%282020%29");
        assert_eq!(encode("é"), "%C3%A9");
    }
}
