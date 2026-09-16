//! A base file mapped into memory, and little-endian reads out of it.

use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::error::Error;

/// One file, mapped; an empty file maps to nothing.
#[derive(Debug)]
pub struct Mapped(Option<Mmap>);

impl Mapped {
    /// Maps the file, checking its size against `bytes`.
    ///
    /// # Errors
    ///
    /// I/O, or a file of another size.
    pub fn open(path: &Path, bytes: u64) -> Result<Self, Error> {
        let file = File::open(path).map_err(|e| Error::io(path, e))?;
        let len = file.metadata().map_err(|e| Error::io(path, e))?.len();
        if len != bytes {
            return Err(Error::Mismatch(format!(
                "{}: {len} bytes on disk, {bytes} in the manifest",
                path.display()
            )));
        }
        if len == 0 {
            return Ok(Self(None));
        }
        // SAFETY: a base file is written once and replaced whole with its
        // directory, never modified in place, and nothing opens it for
        // writing while it is mapped; every access is a bounds-checked
        // slice of plain bytes.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| Error::io(path, e))?;
        Ok(Self(Some(map)))
    }

    /// The bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.0.as_deref().unwrap_or(&[])
    }

    /// The map itself, for a reader that wants to own it.
    #[must_use]
    pub fn into_inner(self) -> Option<Mmap> {
        self.0
    }
}

/// The `i`th little-endian `u32`.
#[must_use]
pub fn u32_at(bytes: &[u8], i: usize) -> u32 {
    let b = &bytes[i * 4..i * 4 + 4];
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// The `i`th little-endian `u64`.
#[must_use]
pub fn u64_at(bytes: &[u8], i: usize) -> u64 {
    let b = &bytes[i * 8..i * 8 + 8];
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// A count as an index.
#[must_use]
pub fn at(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
