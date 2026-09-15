//! A listing: what a source offers, one entry per line, written by `list`
//! and read by `fetch`, under `listings/<source>/<date>.jsonl` in the
//! store (plan §3.5). Selection is done on the file — a grep of it is a
//! listing — so there is no header line, and every line stands alone.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

/// How an entry is fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A plain HTTPS download; `fetch` handles it.
    Https,
    /// A torrent; `fetch` refuses it and says what to run.
    Torrent,
}

/// One file the source offers, with what the source claims about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Where it is.
    pub url: String,
    /// Where it lands under `raw/<source>/`: the source's own path.
    pub path: String,
    #[allow(missing_docs)]
    pub kind: Kind,
    /// Its size, when the source says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// Its record count, when the source says (OpenAlex does).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub records: Option<u64>,
    /// A sidecar holding its MD5, when the source publishes one (NCBI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub md5_url: Option<String>,
    /// The release it belongs to, as the source names it: a snapshot
    /// date, a baseline year.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
}

/// An entry that could not be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadEntry {
    /// The entry's URL, for the message.
    pub url: String,
    #[allow(missing_docs)]
    pub reason: &'static str,
}

impl fmt::Display for BadEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.url, self.reason)
    }
}

impl Error for BadEntry {}

impl Entry {
    /// The entry is usable: the URL fits its kind, the path is relative
    /// and plain (no `..`, no root, no empty segment), and a sidecar URL
    /// is HTTPS.
    ///
    /// # Errors
    ///
    /// Why not.
    pub fn check(&self) -> Result<(), BadEntry> {
        let bad = |reason| BadEntry {
            url: self.url.clone(),
            reason,
        };
        match self.kind {
            Kind::Https if !self.url.starts_with("https://") => {
                return Err(bad("an https entry must have an https:// url"));
            }
            Kind::Torrent
                if !self.url.starts_with("https://") && !self.url.starts_with("magnet:") =>
            {
                return Err(bad("a torrent entry must have an https:// or magnet: url"));
            }
            _ => {}
        }
        if self.path.is_empty() {
            return Err(bad("empty path"));
        }
        if self.path.to_ascii_lowercase().ends_with(".part") {
            return Err(bad("a path may not end in .part"));
        }
        let plain = Path::new(&self.path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        if !plain {
            return Err(bad("path must be relative and plain"));
        }
        if self
            .md5_url
            .as_ref()
            .is_some_and(|u| !u.starts_with("https://"))
        {
            return Err(bad("md5_url must be https://"));
        }
        Ok(())
    }
}

/// Writes a listing, creating its directory. Listings are write-once:
/// an existing file is an error, not a target.
///
/// # Errors
///
/// An entry that fails [`Entry::check`], an existing file, or I/O.
pub fn write(path: &Path, entries: &[Entry]) -> io::Result<()> {
    for entry in entries {
        entry
            .check()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut out = io::BufWriter::new(fs::File::create_new(path)?);
    for entry in entries {
        serde_json::to_writer(&mut out, entry)?;
        out.write_all(b"\n")?;
    }
    out.flush()
}

/// Reads a listing: one entry per non-blank line, each checked.
///
/// # Errors
///
/// I/O, a line that is not an entry (with its number), or an entry that
/// fails [`Entry::check`].
pub fn read(path: &Path) -> io::Result<Vec<Entry>> {
    let file = BufReader::new(fs::File::open(path)?);
    let mut entries = Vec::new();
    for (index, line) in file.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Entry = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: line {}: {e}", path.display(), index + 1),
            )
        })?;
        entry.check().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: line {}: {e}", path.display(), index + 1),
            )
        })?;
        entries.push(entry);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::{Entry, Kind, read, write};

    fn entry(url: &str, path: &str) -> Entry {
        Entry {
            url: url.to_owned(),
            path: path.to_owned(),
            kind: Kind::Https,
            bytes: Some(20_094_169),
            records: Some(10_873),
            md5_url: None,
            release: Some("2026-06-26".to_owned()),
        }
    }

    #[test]
    fn entries_round_trip_through_a_listing_and_the_file_is_write_once() {
        let dir = std::env::temp_dir().join(format!("ol-rht-listing-{}", std::process::id()));
        let path = dir.join("2026-09-15.jsonl");
        let entries = vec![
            entry("https://x/a.gz", "works/a.gz"),
            Entry {
                md5_url: Some("https://x/b.gz.md5".to_owned()),
                bytes: None,
                records: None,
                ..entry("https://x/b.gz", "baseline/b.gz")
            },
        ];
        write(&path, &entries).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(!text.contains("md5_url\":null"), "{text}");
        assert_eq!(read(&path).unwrap(), entries);
        assert!(write(&path, &entries).is_err(), "write-once");
        std::fs::write(&path, format!("{text}\n{{\"url\":\"https://x/c\",\"path\":\"c\",\"kind\":\"https\",\"extra\":1}}\n")).unwrap();
        let err = read(&path).unwrap_err();
        assert!(err.to_string().contains("line 4"), "{err}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bad_entries_are_refused() {
        let bad = |e: Entry| e.check().unwrap_err().reason;
        assert!(bad(entry("http://x/a", "a")).contains("https://"));
        assert!(bad(entry("https://x/a", "")).contains("empty"));
        assert!(bad(entry("https://x/a", "../a")).contains("plain"));
        assert!(bad(entry("https://x/a", "/a")).contains("plain"));
        assert!(bad(entry("https://x/a", "a.part")).contains(".part"));
        assert!(
            bad(Entry {
                md5_url: Some("ftp://x".to_owned()),
                ..entry("https://x/a", "a")
            })
            .contains("md5_url")
        );
        assert!(
            Entry {
                kind: Kind::Torrent,
                ..entry("magnet:?xt=urn:btih:abc", "x.tar")
            }
            .check()
            .is_ok()
        );
        assert!(
            entry("https://x/a", "works/updated_date=2025-07-25/part_0000.gz")
                .check()
                .is_ok()
        );
    }
}
