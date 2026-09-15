//! A receipt: what was fetched, one per verified raw file, appended to
//! `receipts/<source>.jsonl` in the store (plan §3.5). A receipt's
//! existence is the completion marker for its raw file — what `_SUCCESS`
//! was to a Hadoop job. Only successes are recorded; a failure is the
//! run's exit code.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// What a fetch verified before it wrote the receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Check {
    /// The bytes received equal the response's `Content-Length`.
    ContentLength,
    /// The bytes received equal what the listing said.
    ListingBytes,
    /// The MD5 equals the source's sidecar.
    Md5Sidecar,
    /// The MD5 equals the response's `ETag` (S3, single-part objects).
    EtagMd5,
}

/// One verified raw file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    /// Under `raw/<source>/`.
    pub path: String,
    #[allow(missing_docs)]
    pub url: String,
    #[allow(missing_docs)]
    pub bytes: u64,
    /// Hex.
    pub sha256: String,
    /// Hex.
    pub md5: String,
    /// When the transfer began, UTC.
    pub fetched_at: String,
    /// How long it took.
    pub took_ms: u64,
    /// What was verified.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
    /// Bytes already on disk when the transfer began, when it resumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resumed_from: Option<u64>,
    #[allow(missing_docs)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[allow(missing_docs)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    /// The listing the entry came from, as given to `fetch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listing: Option<String>,
    /// Anything a person should know: how a backfilled file was verified,
    /// what an index document is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Appends one receipt as one line, creating the file and its directory
/// if needed. One `write` per line, flushed, so a crash loses at most the
/// transfer in flight.
///
/// # Errors
///
/// I/O.
pub fn append(path: &Path, receipt: &Receipt) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut line = serde_json::to_vec(receipt)?;
    line.push(b'\n');
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(&line)?;
    file.flush()
}

/// Every receipt in the file, in order; none when there is no file yet.
///
/// # Errors
///
/// I/O, or a line that is not a receipt (with its number).
pub fn read(path: &Path) -> io::Result<Vec<Receipt>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut receipts = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let receipt = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: line {}: {e}", path.display(), index + 1),
            )
        })?;
        receipts.push(receipt);
    }
    Ok(receipts)
}

/// The latest receipt for each path.
#[must_use]
pub fn latest(receipts: &[Receipt]) -> BTreeMap<&str, &Receipt> {
    receipts.iter().map(|r| (r.path.as_str(), r)).collect()
}

#[cfg(test)]
mod tests {
    use super::{Check, Receipt, append, latest, read};

    fn receipt(path: &str, bytes: u64) -> Receipt {
        Receipt {
            path: path.to_owned(),
            url: format!("https://x/{path}"),
            bytes,
            sha256: "ab".repeat(32),
            md5: "cd".repeat(16),
            fetched_at: "2026-09-15T08:00:00Z".to_owned(),
            took_ms: 12,
            checks: vec![Check::ContentLength, Check::EtagMd5],
            resumed_from: None,
            etag: Some("\"cdcd\"".to_owned()),
            last_modified: None,
            listing: Some("listings/x/2026-09-15.jsonl".to_owned()),
            note: None,
        }
    }

    #[test]
    fn receipts_append_read_back_and_the_latest_per_path_wins() {
        let dir = std::env::temp_dir().join(format!("ol-rht-receipts-{}", std::process::id()));
        let path = dir.join("x.jsonl");
        assert!(read(&path).unwrap().is_empty(), "no file yet");
        append(&path, &receipt("a.gz", 1)).unwrap();
        append(&path, &receipt("b.gz", 2)).unwrap();
        append(&path, &receipt("a.gz", 3)).unwrap();
        let all = read(&path).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], receipt("a.gz", 1));
        let latest = latest(&all);
        assert_eq!(latest["a.gz"].bytes, 3);
        assert_eq!(latest["b.gz"].bytes, 2);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"checks\":[\"content-length\",\"etag-md5\"]"),
            "{text}"
        );
        assert!(!text.contains("resumed_from"), "{text}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
