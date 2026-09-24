//! Files digested as they are written and checked as they are read: a
//! manifest names every file's size and SHA-256.

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Error;

/// One file as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileInfo {
    #[allow(missing_docs)]
    pub bytes: u64,
    #[allow(missing_docs)]
    pub sha256: String,
}

/// Lowercase hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Reads a file through the digest.
///
/// # Errors
///
/// I/O.
pub fn digest(path: &Path) -> Result<FileInfo, Error> {
    let mut file = File::open(path).map_err(|e| Error::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|e| Error::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
    }
    Ok(FileInfo {
        bytes,
        sha256: hex(&hasher.finalize()),
    })
}

/// A file being written, digested on the way.
#[derive(Debug)]
pub struct Sink {
    path: PathBuf,
    file: BufWriter<File>,
    hasher: Sha256,
    bytes: u64,
}

impl Sink {
    /// Creates the file.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn create(path: &Path) -> Result<Self, Error> {
        let file = File::create(path).map_err(|e| Error::io(path, e))?;
        Ok(Self {
            path: path.to_path_buf(),
            file: BufWriter::with_capacity(1 << 20, file),
            hasher: Sha256::new(),
            bytes: 0,
        })
    }

    /// Appends bytes.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.file
            .write_all(bytes)
            .map_err(|e| Error::io(&self.path, e))?;
        self.hasher.update(bytes);
        self.bytes += bytes.len() as u64;
        Ok(())
    }

    /// Bytes written so far.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.bytes
    }

    /// Flushes, syncs and closes.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn finish(self) -> Result<FileInfo, Error> {
        let file = self
            .file
            .into_inner()
            .map_err(|e| Error::io(&self.path, e.into_error()))?;
        file.sync_all().map_err(|e| Error::io(&self.path, e))?;
        Ok(FileInfo {
            bytes: self.bytes,
            sha256: hex(&self.hasher.finalize()),
        })
    }
}

/// So that a builder that writes a stream — the cited-as transducer — can
/// write straight into a digested file.
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.file.write_all(bytes)?;
        self.hasher.update(bytes);
        self.bytes += bytes.len() as u64;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
