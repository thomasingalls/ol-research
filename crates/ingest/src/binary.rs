//! The binary sink (plan §5, rule 12), and the reader for what it writes.
//!
//! Every raw file becomes one output set of up to three files, one per
//! record kind the [`Sink`] receives: `<prefix>.works.zst`,
//! `<prefix>.references.zst`, `<prefix>.deletions.zst`, where the prefix
//! is named from the raw file (the runner's [`crate::run::mirror`]). Each
//! is a zstd stream of length-prefixed postcard values: first a [`Header`]
//! naming the source, the record kind and the model version, then the
//! records. Postcard is a compact, non-self-describing encoding, so a file
//! is readable only with the types of the model version in its header —
//! which is the point: output is a pure function of raw data and model
//! version, and a model change invalidates it (plan §6).
//!
//! One raw file, one set: a rebuild of one raw file replaces one set, and
//! workers write in parallel sharing nothing (plan §5, rule 4).
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

/// zstd level: fast; the level and the frame size are knobs not yet
/// turned (plan §5, second measurement).
const LEVEL: i32 = 3;

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

/// What one finished output set holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The set's prefix: its files are [`Kind::path`] of it.
    pub prefix: PathBuf,
    /// Records per kind, in [`Kind::ALL`] order.
    pub records: [u64; 3],
    /// Compressed bytes on disk, all three files.
    pub bytes: u64,
}

struct Stream {
    out: zstd::stream::write::Encoder<'static, BufWriter<File>>,
    records: u64,
}

/// One raw file's output set. Its files are removed when it is dropped
/// unfinished, so nothing partial survives an error.
struct Set {
    prefix: PathBuf,
    streams: Vec<Stream>,
    finished: bool,
}

impl Set {
    fn create(prefix: &Path, source: &str, model: u32, buf: &mut Vec<u8>) -> io::Result<Self> {
        if let Some(parent) = prefix.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let mut set = Self {
            prefix: prefix.to_path_buf(),
            streams: Vec::with_capacity(3),
            finished: false,
        };
        for kind in Kind::ALL {
            let file = File::create(kind.path(prefix))?;
            let out = zstd::stream::write::Encoder::new(BufWriter::new(file), LEVEL)?;
            set.streams.push(Stream { out, records: 0 });
        }
        for (i, kind) in Kind::ALL.into_iter().enumerate() {
            let header = Header {
                magic: Header::MAGIC,
                format: FORMAT,
                model,
                source: source.to_owned(),
                kind,
            };
            set.put(i, &header, buf)?;
            // Headers are not records.
            set.streams[i].records = 0;
        }
        Ok(set)
    }

    /// Encodes one value into the reusable buffer and writes it,
    /// length-prefixed, to stream `i`.
    fn put<T: Serialize + ?Sized>(
        &mut self,
        i: usize,
        value: &T,
        buf: &mut Vec<u8>,
    ) -> io::Result<()> {
        buf.clear();
        *buf = postcard::to_extend(value, std::mem::take(buf))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let stream = &mut self.streams[i];
        write_varint(&mut stream.out, buf.len() as u64)?;
        stream.out.write_all(buf)?;
        stream.records += 1;
        Ok(())
    }

    fn finish(mut self) -> io::Result<Written> {
        let streams = std::mem::take(&mut self.streams);
        let mut written = Written {
            prefix: self.prefix.clone(),
            records: [0; 3],
            bytes: 0,
        };
        for (i, s) in streams.into_iter().enumerate() {
            written.records[i] = s.records;
            let mut file = s.out.finish()?;
            file.flush()?;
            written.bytes += file.get_ref().metadata().map_or(0, |m| m.len());
        }
        self.finished = true;
        Ok(written)
    }

    fn remove_files(&self) {
        for kind in Kind::ALL {
            let _ = fs::remove_file(kind.path(&self.prefix));
        }
    }
}

impl Drop for Set {
    fn drop(&mut self) {
        if !self.finished {
            self.remove_files();
        }
    }
}

/// How a raw file's index and path name its output set.
type PrefixOf = Box<dyn Fn(usize, &Path) -> PathBuf + Send + Sync>;

/// A [`Sink`] writing the layout described in the module docs: one output
/// set per raw file, opened at [`Sink::begin_file`] and finished at
/// [`Sink::end_file`], named by a function of the raw file's index and
/// path.
///
/// An I/O error is remembered rather than raised — the record methods
/// cannot fail — and reported through [`Sink::failed`], which the runner
/// checks after every record, so a full disk stops the run at the record
/// it hit. Nothing is written after an error; [`Binary::finish`] then
/// removes every set this sink wrote, as does [`Binary::discard`] when a
/// run failed for another reason.
pub struct Binary {
    source: String,
    model: u32,
    prefix_of: PrefixOf,
    current: Option<Set>,
    written: Vec<Written>,
    buf: Vec<u8>,
    error: Option<io::Error>,
    poisoned: bool,
}

impl fmt::Debug for Binary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Binary")
            .field("source", &self.source)
            .field("model", &self.model)
            .field("open", &self.current.as_ref().map(|s| &s.prefix))
            .field("written", &self.written.len())
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl Binary {
    /// A sink for `source`, tagging every set with model version `model`
    /// and naming the set of the raw file at `index` with `prefix_of`.
    pub fn new(
        source: &str,
        model: u32,
        prefix_of: impl Fn(usize, &Path) -> PathBuf + Send + Sync + 'static,
    ) -> Self {
        Self {
            source: source.to_owned(),
            model,
            prefix_of: Box::new(prefix_of),
            current: None,
            written: Vec::new(),
            buf: Vec::with_capacity(1 << 16),
            error: None,
            poisoned: false,
        }
    }

    /// Opens the output set for raw file `index` at `path`: creates its
    /// files and their directory and writes the headers. What
    /// [`Sink::begin_file`] does.
    ///
    /// # Errors
    ///
    /// A set is already open, the sink has failed, or I/O.
    pub fn open_file(&mut self, path: &Path, index: usize) -> io::Result<()> {
        if self.poisoned || self.error.is_some() {
            return Err(io::Error::other("the sink failed earlier in the run"));
        }
        if self.current.is_some() {
            return Err(io::Error::other("a raw file is still open"));
        }
        let prefix = (self.prefix_of)(index, path);
        self.current = Some(Set::create(
            &prefix,
            &self.source,
            self.model,
            &mut self.buf,
        )?);
        Ok(())
    }

    /// Finishes the open set: ends the compression streams and flushes the
    /// files. What [`Sink::end_file`] does.
    ///
    /// # Errors
    ///
    /// A write error since the set was opened, no set open, or I/O while
    /// finishing; the set's files are removed on any of them.
    pub fn close_file(&mut self) -> io::Result<()> {
        if let Some(e) = self.error.take() {
            self.poisoned = true;
            self.current = None;
            return Err(e);
        }
        let set = self
            .current
            .take()
            .ok_or_else(|| io::Error::other("no raw file is open"))?;
        self.written.push(set.finish()?);
        Ok(())
    }

    /// The sets finished so far.
    #[must_use]
    pub fn written(&self) -> &[Written] {
        &self.written
    }

    /// Abandons the set of the raw file still open, if there is one: its
    /// files are removed and the sets before it stay. For a run that
    /// stopped inside a file — `ingest` cut short by the record limit, or
    /// a failure whose earlier files are complete and receipted.
    pub fn abandon_open(&mut self) {
        self.current = None;
    }

    fn put<T: Serialize + ?Sized>(&mut self, i: usize, value: &T) {
        if self.error.is_some() || self.poisoned {
            return;
        }
        match &mut self.current {
            Some(set) => {
                if let Err(e) = set.put(i, value, &mut self.buf) {
                    self.error = Some(e);
                }
            }
            None => self.error = Some(io::Error::other("a record outside a raw file")),
        }
    }

    /// Ends the run: the sets written, in the order their files finished.
    ///
    /// # Errors
    ///
    /// A set is still open, or an error was remembered; every set this
    /// sink wrote is removed first, so nothing partial is left behind.
    pub fn finish(mut self) -> io::Result<Vec<Written>> {
        if self.current.is_some() {
            self.discard();
            return Err(io::Error::other("a raw file is still open"));
        }
        if let Some(e) = self.error.take() {
            self.discard();
            return Err(e);
        }
        if self.poisoned {
            self.discard();
            return Err(io::Error::other("the sink failed earlier in the run"));
        }
        Ok(std::mem::take(&mut self.written))
    }

    /// Removes every set this sink wrote and the one open, if any: the run
    /// failed for a reason that was not the sink's, and nothing partial
    /// may remain (plan §6).
    pub fn discard(mut self) {
        self.current = None;
        for w in std::mem::take(&mut self.written) {
            for kind in Kind::ALL {
                let _ = fs::remove_file(kind.path(&w.prefix));
            }
        }
    }
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

    fn begin_file(&mut self, path: &Path, index: usize) -> io::Result<()> {
        self.open_file(path, index)
    }

    fn end_file(&mut self) -> io::Result<()> {
        self.close_file()
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
