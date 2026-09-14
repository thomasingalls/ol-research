//! Reading a gzipped baseline or update file into memory.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use flate2::read::MultiGzDecoder;

/// Decompresses a `.xml.gz` file into one UTF-8 string.
///
/// Multi-member gzip is handled: a single-member decoder would stop
/// silently after the first member and the run would "succeed" on a
/// fraction of the file (plan §5, rule 7). One file per worker, in memory,
/// is the intended shape — the parser borrows every record from this
/// string.
///
/// # Errors
///
/// Any I/O or gzip error, or bytes that are not valid UTF-8 (the files
/// declare `encoding="utf-8"`; anything else is a difference worth stopping
/// for).
pub fn read_to_string(path: &Path) -> io::Result<String> {
    let file = BufReader::new(File::open(path)?);
    let mut bytes = Vec::new();
    MultiGzDecoder::new(file).read_to_end(&mut bytes)?;
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
