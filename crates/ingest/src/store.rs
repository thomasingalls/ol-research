//! The store of record (plan §3.5): one root, given to every command,
//! under which the pipeline keeps everything it produces or depends on —
//! raw bytes, listings, receipts, normalized output. The code speaks root
//! plus relative path and nothing else, so the root can be a directory
//! today and a bucket later.
//!
//! ```text
//! <root>/raw/<source>/<path>            bytes as served, write-once; <path> is the source's own
//! <root>/raw/<source>/<path>.part       a transfer in progress; never data
//! <root>/listings/<source>/<date>.jsonl what the source offered that day, one entry per line
//! <root>/listings/<source>/<date>.<name> the index document(s) the listing was read from
//! <root>/receipts/<source>.jsonl        what was fetched, one receipt per line, append-only
//! <root>/normalized/v<model>/<source>/… rebuildable
//! ```

use std::path::{Path, PathBuf};

/// The store root and the paths under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// A store at `root`; nothing is created until something is written.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[allow(missing_docs)]
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A raw file: `raw/<source>/<path>`. `path` is a listing entry's,
    /// already checked to be relative and plain (see
    /// [`Entry::check`](crate::listing::Entry::check)).
    #[must_use]
    pub fn raw(&self, source: &str, path: &str) -> PathBuf {
        self.root.join("raw").join(source).join(path)
    }

    /// The raw directory of a source: `raw/<source>`.
    #[must_use]
    pub fn raw_dir(&self, source: &str) -> PathBuf {
        self.root.join("raw").join(source)
    }

    /// A day's listing: `listings/<source>/<date>.jsonl`.
    #[must_use]
    pub fn listing(&self, source: &str, date: &str) -> PathBuf {
        self.root
            .join("listings")
            .join(source)
            .join(format!("{date}.jsonl"))
    }

    /// An index document a listing was read from, kept beside it:
    /// `listings/<source>/<date>.<name>`.
    #[must_use]
    pub fn listing_document(&self, source: &str, date: &str, name: &str) -> PathBuf {
        self.root
            .join("listings")
            .join(source)
            .join(format!("{date}.{name}"))
    }

    /// A source's receipts: `receipts/<source>.jsonl`.
    #[must_use]
    pub fn receipts(&self, source: &str) -> PathBuf {
        self.root.join("receipts").join(format!("{source}.jsonl"))
    }

    /// The normalized layer of one source under one model version:
    /// `normalized/v<model>/<source>`.
    #[must_use]
    pub fn normalized(&self, model: u32, source: &str) -> PathBuf {
        self.root
            .join("normalized")
            .join(format!("v{model}"))
            .join(source)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::Store;

    #[test]
    fn paths_follow_the_layout() {
        let store = Store::new("data");
        assert_eq!(store.root(), Path::new("data"));
        assert_eq!(
            store.raw("openalex", "works/updated_date=2025-07-25/part_0000.gz"),
            Path::new("data/raw/openalex/works/updated_date=2025-07-25/part_0000.gz")
        );
        assert_eq!(store.raw_dir("pubmed"), Path::new("data/raw/pubmed"));
        assert_eq!(
            store.listing("pubmed", "2026-09-15"),
            Path::new("data/listings/pubmed/2026-09-15.jsonl")
        );
        assert_eq!(
            store.listing_document("openalex", "2026-09-15", "manifest.json"),
            Path::new("data/listings/openalex/2026-09-15.manifest.json")
        );
        assert_eq!(
            store.receipts("crossref"),
            Path::new("data/receipts/crossref.jsonl")
        );
        assert_eq!(
            store.normalized(1, "openalex"),
            Path::new("data/normalized/v1/openalex")
        );
    }
}
