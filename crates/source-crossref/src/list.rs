//! What Crossref offers in bulk: the annual public data file, one torrent
//! carrying one tar of JSON lines (plan §2). `fetch` does not do torrents;
//! the entry is listed so the listing says what exists, and `fetch`
//! refuses it with the command to run.

use ingest::list::{Error, Index, Lister};
use ingest::listing::{Entry, Kind};

use crate::Crossref;

/// The March 2026 public data file's torrent (plan §12).
pub const TORRENT: &str =
    "https://academictorrents.com/download/b5ee0e102689b3e67023dd024694c0f5f124646f.torrent";

impl Lister for Crossref {
    fn list(_: &mut dyn Index) -> Result<Vec<Entry>, Error> {
        Ok(vec![Entry {
            url: TORRENT.to_owned(),
            path: "public-data-file-2026".to_owned(),
            kind: Kind::Torrent,
            bytes: None,
            records: None,
            md5_url: None,
            release: Some("2026".to_owned()),
        }])
    }
}
