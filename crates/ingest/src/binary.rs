//! The binary sink (plan §5, rule 12), and the reader for what it writes.
//!
//! One run of one source produces up to three files, one per record kind
//! the [`Sink`] receives: `<prefix>.works.zst`, `<prefix>.references.zst`,
//! `<prefix>.deletions.zst`. Each is a zstd stream of length-prefixed
//! postcard values: first a [`Header`] naming the source, the record kind
//! and the model version, then the records. Postcard is a compact,
//! non-self-describing encoding, so a file is readable only with the types
//! of the model version in its header — which is the point: output is a
//! pure function of raw data and model version, and a model change
//! invalidates it (plan §6).
//!
//! Writing allocates nothing per record in the steady state: each value is
//! encoded into one reusable buffer and copied into the compressor.
//! Symbol tables for repeated strings (rule 4) are not here yet; this is
//! the layout to measure them against.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::source::{Sink, Source};

/// The layout version, independent of the model version.
pub const FORMAT: u8 = 1;

/// The three record kinds, one file each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// One record per work.
    Works,
    /// One record per citation edge: the citing id, then the edge.
    References,
    /// One record per withdrawn work: its id.
    Deletions,
}

impl Kind {
    /// Every kind, in file order.
    pub const ALL: [Self; 3] = [Self::Works, Self::References, Self::Deletions];

    /// The file name segment: `works` in `<prefix>.works.zst`.
    #[must_use]
    pub const fn segment(self) -> &'static str {
        match self {
            Self::Works => "works",
            Self::References => "references",
            Self::Deletions => "deletions",
        }
    }

    /// The path of this kind's file for a prefix.
    #[must_use]
    pub fn path(self, prefix: &Path) -> PathBuf {
        let mut name = prefix.as_os_str().to_owned();
        name.push(format!(".{}.zst", self.segment()));
        PathBuf::from(name)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.segment())
    }
}

/// The first value of every file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// [`Header::MAGIC`].
    pub magic: [u8; 5],
    /// [`FORMAT`].
    pub format: u8,
    /// The model version the records were written with.
    pub model: u32,
    /// [`Source::NAME`].
    pub source: String,
    #[allow(missing_docs)]
    pub kind: Kind,
}

impl Header {
    /// What every file starts with.
    pub const MAGIC: [u8; 5] = *b"OLRHT";
}

/// What a finished sink wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Written {
    /// Records per kind, in [`Kind::ALL`] order.
    pub records: [u64; 3],
    /// Compressed bytes on disk, all files.
    pub bytes: u64,
}

struct Stream {
    path: PathBuf,
    out: zstd::stream::write::Encoder<'static, BufWriter<File>>,
    records: u64,
}

/// A [`Sink`] writing the layout described in the module docs.
///
/// An I/O error is remembered rather than raised — the sink methods cannot
/// fail — and reported through [`Sink::failed`], which the runner checks
/// after every record, so a full disk stops the run at the record it hit.
/// Nothing is written after an error; [`Binary::finish`] then removes the
/// files.
pub struct Binary {
    prefix: PathBuf,
    streams: [Stream; 3],
    buf: Vec<u8>,
    error: Option<io::Error>,
    poisoned: bool,
}

impl fmt::Debug for Binary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Binary")
            .field("prefix", &self.prefix)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

/// zstd level: fast, and still several times smaller than gzip's ratio
/// would need to be measured against.
const LEVEL: i32 = 3;

impl Binary {
    /// Creates the three files, their parent directory if needed, and
    /// writes each header. `model` is the model version to tag them with.
    ///
    /// # Errors
    ///
    /// Any I/O error creating or writing.
    pub fn create(prefix: &Path, source: &str, model: u32) -> io::Result<Self> {
        if let Some(parent) = prefix.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let mut streams = Vec::with_capacity(3);
        for kind in Kind::ALL {
            let path = kind.path(prefix);
            let out =
                zstd::stream::write::Encoder::new(BufWriter::new(File::create(&path)?), LEVEL)?;
            streams.push(Stream {
                path,
                out,
                records: 0,
            });
        }
        let Ok(streams) = <[Stream; 3]>::try_from(streams) else {
            unreachable!("three kinds, three streams")
        };
        let mut sink = Self {
            prefix: prefix.to_path_buf(),
            streams,
            buf: Vec::with_capacity(1 << 16),
            error: None,
            poisoned: false,
        };
        for (i, kind) in Kind::ALL.into_iter().enumerate() {
            let header = Header {
                magic: Header::MAGIC,
                format: FORMAT,
                model,
                source: source.to_owned(),
                kind,
            };
            sink.put(i, &header);
            // Headers are not records.
            sink.streams[i].records = 0;
        }
        match sink.error.take() {
            Some(e) => {
                sink.remove();
                Err(e)
            }
            None => Ok(sink),
        }
    }

    /// The files this sink writes.
    #[must_use]
    pub fn paths(&self) -> [&Path; 3] {
        [
            &self.streams[0].path,
            &self.streams[1].path,
            &self.streams[2].path,
        ]
    }

    /// Encodes one value into the reusable buffer and writes it,
    /// length-prefixed, to stream `i`. After an error, nothing.
    fn put<T: Serialize + ?Sized>(&mut self, i: usize, value: &T) {
        if self.error.is_some() || self.poisoned {
            return;
        }
        self.buf.clear();
        let buf = std::mem::take(&mut self.buf);
        match postcard::to_extend(value, buf) {
            Ok(buf) => self.buf = buf,
            Err(e) => {
                self.error = Some(io::Error::new(io::ErrorKind::InvalidData, e));
                return;
            }
        }
        let stream = &mut self.streams[i];
        let result = write_varint(&mut stream.out, self.buf.len() as u64)
            .and_then(|()| stream.out.write_all(&self.buf));
        match result {
            Ok(()) => stream.records += 1,
            Err(e) => self.error = Some(e),
        }
    }

    fn remove(&self) {
        for s in &self.streams {
            let _ = fs::remove_file(&s.path);
        }
    }

    /// Finishes the compression streams and flushes the files.
    ///
    /// # Errors
    ///
    /// An earlier write error, or one from finishing; the files are
    /// removed in either case, so nothing partial is left behind.
    pub fn finish(mut self) -> io::Result<Written> {
        if let Some(e) = self.error.take() {
            self.remove();
            return Err(e);
        }
        if self.poisoned {
            self.remove();
            return Err(io::Error::other("the sink failed earlier in the run"));
        }
        let mut written = Written::default();
        let streams = std::mem::replace(
            &mut self.streams,
            // Placeholders are never written to; `self` is consumed.
            match Self::placeholders(&self.prefix) {
                Ok(p) => p,
                Err(e) => {
                    self.remove();
                    return Err(e);
                }
            },
        );
        for (i, s) in streams.into_iter().enumerate() {
            written.records[i] = s.records;
            let result = s.out.finish().and_then(|mut w| w.flush().map(|()| w));
            match result {
                Ok(_) => written.bytes += fs::metadata(&s.path).map_or(0, |m| m.len()),
                Err(e) => {
                    self.remove();
                    return Err(e);
                }
            }
        }
        self.poisoned = true; // the placeholders must not be removed
        Ok(written)
    }

    /// Removes the files: the run failed for a reason that was not the
    /// sink's, and nothing partial may remain (plan §6).
    pub fn discard(self) {
        self.remove();
    }

    fn placeholders(prefix: &Path) -> io::Result<[Stream; 3]> {
        let mut v = Vec::with_capacity(3);
        for kind in Kind::ALL {
            v.push(Stream {
                path: kind.path(prefix),
                out: zstd::stream::write::Encoder::new(BufWriter::new(tempfile()?), LEVEL)?,
                records: 0,
            });
        }
        let Ok(streams) = <[Stream; 3]>::try_from(v) else {
            unreachable!("three kinds, three streams")
        };
        Ok(streams)
    }
}

/// An anonymous file that vanishes when closed, for the placeholders.
fn tempfile() -> io::Result<File> {
    let path = std::env::temp_dir().join(format!(
        "ol-rht-placeholder-{}-{:p}",
        std::process::id(),
        &LEVEL
    ));
    let file = File::create(&path)?;
    let _ = fs::remove_file(&path);
    Ok(file)
}

impl<'n, S: Source> Sink<'n, S> for Binary {
    fn work(&mut self, work: &S::Work<'n>) {
        self.put(0, work);
    }

    fn reference(&mut self, from: &S::Id<'n>, reference: &S::Reference<'n>) {
        self.put(1, &(from, reference));
    }

    fn deletion(&mut self, id: &S::Id<'n>) {
        self.put(2, id);
    }

    fn failed(&mut self) -> Option<io::Error> {
        let e = self.error.take();
        if e.is_some() {
            self.poisoned = true;
        }
        e
    }
}

fn write_varint(out: &mut impl Write, mut n: u64) -> io::Result<()> {
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            return out.write_all(&[byte]);
        }
        out.write_all(&[byte | 0x80])?;
    }
}

fn read_varint(bytes: &[u8], pos: &mut usize) -> io::Result<u64> {
    let mut n = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes
            .get(*pos)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated length"))?;
        *pos += 1;
        n |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(n);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "length too long",
    ))
}

/// One file written by [`Binary`], inflated into memory; records are
/// decoded on demand, borrowing from it.
#[derive(Debug)]
pub struct Reader {
    bytes: Vec<u8>,
    header: Header,
    body: usize,
}

impl Reader {
    /// Reads and inflates the whole file and checks its header.
    ///
    /// # Errors
    ///
    /// I/O, a bad zstd stream, or a header that is not this layout.
    pub fn open(path: &Path) -> io::Result<Self> {
        let bytes = zstd::stream::decode_all(BufReader::new(File::open(path)?))?;
        let mut pos = 0;
        let header: Header = next_value(&bytes, &mut pos)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty file"))?;
        if header.magic != Header::MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not an ol-rht file",
            ));
        }
        if header.format != FORMAT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("layout version {} is not {FORMAT}", header.format),
            ));
        }
        Ok(Self {
            bytes,
            header,
            body: pos,
        })
    }

    /// The header.
    #[must_use]
    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// The records, decoded as `T`, borrowing from the file's bytes.
    pub fn records<'a, T: Deserialize<'a>>(&'a self) -> impl Iterator<Item = io::Result<T>> + 'a {
        let mut pos = self.body;
        std::iter::from_fn(move || next_value(&self.bytes, &mut pos).transpose())
    }
}

/// The next length-prefixed value at `pos`, or `None` at the end.
fn next_value<'a, T: Deserialize<'a>>(bytes: &'a [u8], pos: &mut usize) -> io::Result<Option<T>> {
    if *pos >= bytes.len() {
        return Ok(None);
    }
    let len = usize::try_from(read_varint(bytes, pos)?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "length too long"))?;
    let end = pos
        .checked_add(len)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated record"))?;
    let value = postcard::from_bytes(&bytes[*pos..end])
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    *pos = end;
    Ok(Some(value))
}

/// Reads the header alone, for tools that route on it.
///
/// # Errors
///
/// As [`Reader::open`].
pub fn header(path: &Path) -> io::Result<Header> {
    Reader::open(path).map(|r| r.header)
}

/// A convenience for owned record types.
///
/// # Errors
///
/// As [`Reader::open`], then any record that does not decode.
pub fn read_all<T: DeserializeOwned>(path: &Path) -> io::Result<Vec<T>> {
    let reader = Reader::open(path)?;
    reader.records::<T>().collect()
}

#[cfg(test)]
mod tests {
    use super::{Kind, read_varint, write_varint};

    #[test]
    fn varints_roundtrip() {
        for n in [0u64, 1, 127, 128, 300, 1 << 20, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, n).unwrap();
            let mut pos = 0;
            assert_eq!(read_varint(&buf, &mut pos).unwrap(), n);
            assert_eq!(pos, buf.len());
        }
        assert!(read_varint(&[0x80], &mut 0).is_err(), "truncated");
        assert!(read_varint(&[0x80; 11], &mut 0).is_err(), "too long");
    }

    #[test]
    fn kinds_name_their_files() {
        let p = Kind::References.path(std::path::Path::new("out/openalex/part_0000"));
        assert_eq!(p.to_str().unwrap(), "out/openalex/part_0000.references.zst");
    }
}
