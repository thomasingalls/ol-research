//! Opening a raw file as a byte stream: through the multi-member gzip
//! decoder when its extension is `gz`, as is otherwise. What a source's
//! [`records`](crate::Source::records) reads from.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use flate2::bufread::MultiGzDecoder;

/// How much of a gzipped file one read asks for.
const FILE_BUFFER: usize = 256 * 1024;

/// A raw file's text as the source serialized it, inflated on the way when
/// the file is gzipped.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // one per worker, never in a collection
pub enum Raw {
    /// Read as is; the framing reader on top buffers it.
    Plain(File),
    /// Inflated. Multi-member gzip is handled: a single-member decoder
    /// would stop silently after the first member and the run would
    /// "succeed" on a fraction of the file (plan §5, rule 7).
    Gz(MultiGzDecoder<BufReader<File>>),
}

impl Read for Raw {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(file) => file.read(buf),
            Self::Gz(decoder) => decoder.read(buf),
        }
    }
}

/// Opens `path`, choosing by its extension.
///
/// # Errors
///
/// The file cannot be opened.
pub fn open(path: &Path) -> io::Result<Raw> {
    let file = File::open(path)?;
    Ok(if path.extension().is_some_and(|e| e == "gz") {
        Raw::Gz(MultiGzDecoder::new(BufReader::with_capacity(
            FILE_BUFFER,
            file,
        )))
    } else {
        Raw::Plain(file)
    })
}
