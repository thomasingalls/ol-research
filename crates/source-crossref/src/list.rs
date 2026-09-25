//! What Crossref offers in bulk: the annual public data file, one torrent
//! carrying 35,908 gzipped JSON-lines shards and Crossref's conversion
//! script `a.py`, flat in one directory, 223,053,729,528 bytes (plan §2).
//! `fetch` does not do torrents; the entry is listed so the listing says
//! what exists, and `fetch` refuses it with the command to run. And, for
//! seeding the POC sample only (plan §8, item 8), the REST API's answer to
//! a batch of DOIs.

use std::time::Duration;

use ingest::keys::Key;
use ingest::list::{Error, Index, Lister, Lookup, encode};
use ingest::listing::{Entry, Kind};

use crate::Crossref;

/// The March 2026 public data file's torrent (plan §12).
pub const TORRENT: &str =
    "https://academictorrents.com/download/b5ee0e102689b3e67023dd024694c0f5f124646f.torrent";
/// The works API, for lookups.
pub const API: &str = "https://api.crossref.org/works";
/// DOIs per lookup request: 50 repeated `doi:` filters answered in one
/// `work-list` on 2026-09-15.
const BATCH: usize = 50;

impl Lister for Crossref {
    /// Half the one request a second its headers report for the
    /// anonymous pool.
    const PACE: Duration = Duration::from_secs(2);
    const CONTACT: Option<&'static str> = Some("mailto");

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

    fn batch(key: Key) -> Option<usize> {
        (key == Key::Doi).then_some(BATCH)
    }

    fn lookup(key: Key, ids: &[&str]) -> Result<Lookup, Error> {
        if key != Key::Doi {
            return Err(Error::Unsupported(key));
        }
        let filter: Vec<String> = ids.iter().map(|id| format!("doi:{}", encode(id))).collect();
        Ok(Lookup {
            url: format!("{API}?filter={}&rows={BATCH}", filter.join(",")),
            extension: "json",
        })
    }
}

#[cfg(test)]
mod tests {
    use ingest::keys::Key;
    use ingest::list::Lister;

    use crate::Crossref;

    #[test]
    fn a_lookup_repeats_the_doi_filter() {
        let lookup = Crossref::lookup(Key::Doi, &["10.1000/a", "10.1000/b,c"]).unwrap();
        assert_eq!(
            lookup.url,
            "https://api.crossref.org/works?filter=doi:10.1000/a,doi:10.1000/b%2Cc&rows=50"
        );
        assert_eq!(Crossref::batch(Key::Doi), Some(50));
        assert_eq!(Crossref::batch(Key::Pmid), None);
        assert!(Crossref::lookup(Key::Pmid, &["1"]).is_err());
    }
}
