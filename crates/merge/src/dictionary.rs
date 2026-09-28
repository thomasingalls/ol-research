//! Every name a full merge reads, once, in name order (merge plan §8; the
//! merge's memory, step 2b, 2026-09-24).
//!
//! The identity pass numbers names and unions the numbers. It used to
//! number them in the order it read them, keeping every name as a heap
//! object and three hash maps to find it again — on the PubMed baseline
//! most of 15 GB for 91 M names. A full merge can know every name before
//! it numbers any, because the copies read ([`crate::copies`]) opens every
//! works set first: it hands each standing copy's names to a [`Sorter`],
//! which writes them to sorted runs on the scratch disk and merges the runs
//! into a [`Dictionary`]. A name's number is then its rank in name order:
//! an FST from name to rank finds it, and a front-coded list of the names
//! in order gives it back.
//!
//! The byte form a name takes here is ours and never leaves the run: the
//! namespace first, as a byte in [`Key`]'s order, then a number big-endian
//! or the text — a publisher id's registrant big-endian, then its canonical
//! spelling — so that byte order is [`Name`]'s order and two spellings of
//! one publisher id are one entry.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use fst::{Map, MapBuilder};
use ingest::Key;
use model::{OpenAlexId, PmcId, Pmid, Registrant};
use smallvec::SmallVec;

use crate::arena::NameIx;
use crate::name::{Name, OwnedName};
use crate::publisher;

/// Bytes of names a run gathers before it is sorted and written.
const RUN: usize = 128 << 20;
/// Names in a block of the front-coded list: the first written whole, the
/// rest as what they share with the name before and what they add.
const BLOCK: usize = 16;

/// The namespace's byte: its place in [`Key::ALL`], which is [`Key`]'s
/// order.
#[allow(clippy::cast_possible_truncation)] // five namespaces
fn tag(key: Key) -> u8 {
    Key::ALL
        .iter()
        .position(|&k| k == key)
        .expect("every key is in Key::ALL") as u8
}

/// Writes `name`'s byte form to `out`.
fn encode(name: Name<'_>, out: &mut impl Extend<u8>) {
    out.extend([tag(name.key())]);
    match name {
        Name::Doi(d) => out.extend(d.bytes()),
        Name::Pmid(p) => out.extend(p.get().to_be_bytes()),
        Name::Pmc(p) => out.extend(p.get().to_be_bytes()),
        Name::OpenAlex(w) => out.extend(w.get().to_be_bytes()),
        Name::Publisher(registrant, value) => {
            out.extend(registrant.get().to_be_bytes());
            out.extend(publisher::canonical_bytes(value));
        }
    }
}

/// The name a byte form was made from; `None` for bytes [`encode`] did not
/// write.
fn decode(bytes: &[u8]) -> Option<OwnedName> {
    let (&tag, rest) = bytes.split_first()?;
    let u32_of = |b: &[u8]| Some(u32::from_be_bytes(b.try_into().ok()?));
    Some(match Key::ALL.get(usize::from(tag))? {
        Key::Doi => OwnedName::Doi(std::str::from_utf8(rest).ok()?.into()),
        Key::Pmid => OwnedName::Pmid(Pmid::new(u32_of(rest)?)?),
        Key::Pmc => OwnedName::Pmc(PmcId::new(u32_of(rest)?)?),
        Key::OpenAlex => {
            OwnedName::OpenAlex(OpenAlexId::new(u64::from_be_bytes(rest.try_into().ok()?))?)
        }
        Key::Publisher => {
            let (code, value) = rest.split_at_checked(4)?;
            OwnedName::Publisher(
                Registrant::from_code(u32_of(code)?)?,
                std::str::from_utf8(value).ok()?.into(),
            )
        }
    })
}

/// Names gathered into sorted runs on disk, to be merged into a
/// [`Dictionary`]. The directory is the sorter's own: made empty when it
/// begins, and removed with it.
pub struct Sorter {
    dir: PathBuf,
    keys: Vec<u8>,
    ends: Vec<u32>,
    runs: Vec<PathBuf>,
    pushed: u64,
}

impl fmt::Debug for Sorter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sorter")
            .field("dir", &self.dir)
            .field("runs", &self.runs.len())
            .field("pushed", &self.pushed)
            .finish_non_exhaustive()
    }
}

impl Sorter {
    /// A sorter writing its runs under `dir`.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn create(dir: &Path) -> io::Result<Self> {
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            keys: Vec::new(),
            ends: Vec::new(),
            runs: Vec::new(),
            pushed: 0,
        })
    }

    /// One name, as often as it is read.
    ///
    /// # Errors
    ///
    /// I/O, writing a run.
    #[allow(clippy::cast_possible_truncation)] // a run is under 4 GiB
    pub fn push(&mut self, name: Name<'_>) -> io::Result<()> {
        encode(name, &mut self.keys);
        self.ends.push(self.keys.len() as u32);
        self.pushed += 1;
        if self.keys.len() >= RUN {
            self.spill()?;
        }
        Ok(())
    }

    /// Names pushed, repeats included.
    #[must_use]
    pub const fn pushed(&self) -> u64 {
        self.pushed
    }

    /// The names gathered so far, sorted, each once, written as a run.
    #[allow(clippy::cast_possible_truncation)] // a run is under 2^32 names
    fn spill(&mut self) -> io::Result<()> {
        if self.ends.is_empty() {
            return Ok(());
        }
        let (keys, ends) = (&self.keys, &self.ends);
        let key = |i: usize| {
            let start = if i == 0 { 0 } else { ends[i - 1] as usize };
            &keys[start..ends[i] as usize]
        };
        let mut order: Vec<u32> = (0..ends.len() as u32).collect();
        order.sort_unstable_by(|&a, &b| key(a as usize).cmp(key(b as usize)));
        let path = self.dir.join(format!("run_{:04}", self.runs.len()));
        let mut out = BufWriter::with_capacity(1 << 20, File::create(&path)?);
        let mut last: Option<&[u8]> = None;
        for i in order {
            let k = key(i as usize);
            if last == Some(k) {
                continue;
            }
            write_entry(&mut out, k)?;
            last = Some(k);
        }
        out.flush()?;
        self.runs.push(path);
        self.keys.clear();
        self.ends.clear();
        Ok(())
    }

    /// The dictionary of every name pushed.
    ///
    /// # Errors
    ///
    /// I/O, or a run that does not read back.
    pub fn finish(mut self) -> io::Result<Dictionary> {
        self.spill()?;
        self.keys = Vec::new();
        self.ends = Vec::new();
        let mut readers = self
            .runs
            .iter()
            .map(|p| Ok(BufReader::with_capacity(1 << 20, File::open(p)?)))
            .collect::<io::Result<Vec<_>>>()?;
        let mut heap: BinaryHeap<Reverse<(Vec<u8>, usize)>> = BinaryHeap::new();
        for (r, reader) in readers.iter_mut().enumerate() {
            if let Some(k) = read_entry(reader)? {
                heap.push(Reverse((k, r)));
            }
        }
        let mut builder = Builder::new();
        while let Some(Reverse((k, r))) = heap.pop() {
            if let Some(next) = read_entry(&mut readers[r])? {
                heap.push(Reverse((next, r)));
            }
            builder.push(&k)?;
        }
        builder.finish()
    }
}

impl Drop for Sorter {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// A run's entry: its length, four bytes little-endian, then its bytes.
fn write_entry(out: &mut impl Write, key: &[u8]) -> io::Result<()> {
    let len = u32::try_from(key.len()).map_err(io::Error::other)?;
    out.write_all(&len.to_le_bytes())?;
    out.write_all(key)
}

fn read_entry(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let mut key = vec![0u8; u32::from_le_bytes(len) as usize];
    reader.read_exact(&mut key)?;
    Ok(Some(key))
}

/// A dictionary made from byte forms given in order, repeats dropped.
struct Builder {
    map: MapBuilder<Vec<u8>>,
    blocks: Vec<u64>,
    data: Vec<u8>,
    last: Vec<u8>,
    len: u32,
    starts: [u32; Key::ALL.len() + 1],
}

impl Builder {
    fn new() -> Self {
        Self {
            map: MapBuilder::memory(),
            blocks: Vec::new(),
            data: Vec::new(),
            last: Vec::new(),
            len: 0,
            starts: [0; Key::ALL.len() + 1],
        }
    }

    fn push(&mut self, key: &[u8]) -> io::Result<()> {
        if self.len > 0 && key == self.last.as_slice() {
            return Ok(());
        }
        if self.len > 0 && key < self.last.as_slice() {
            return Err(io::Error::other("the names came out of order"));
        }
        let Some(&tag) = key.first() else {
            return Err(io::Error::other("an empty name"));
        };
        let rank = self.len;
        self.map
            .insert(key, u64::from(rank))
            .map_err(io::Error::other)?;
        if (rank as usize).is_multiple_of(BLOCK) {
            self.blocks.push(self.data.len() as u64);
            varint(&mut self.data, key.len());
            self.data.extend_from_slice(key);
        } else {
            let shared = key
                .iter()
                .zip(&self.last)
                .take_while(|(a, b)| a == b)
                .count();
            varint(&mut self.data, shared);
            varint(&mut self.data, key.len() - shared);
            self.data.extend_from_slice(&key[shared..]);
        }
        // The namespaces follow one another; each starts where the ones
        // before it end.
        for t in usize::from(tag) + 1..self.starts.len() {
            self.starts[t] = rank + 1;
        }
        self.last.clear();
        self.last.extend_from_slice(key);
        self.len = rank
            .checked_add(1)
            .ok_or_else(|| io::Error::other("more than 2^32 names"))?;
        Ok(())
    }

    fn finish(self) -> io::Result<Dictionary> {
        let bytes = self.map.into_inner().map_err(io::Error::other)?;
        let map = Map::new(bytes).map_err(io::Error::other)?;
        let mut data = self.data;
        data.shrink_to_fit();
        let mut blocks = self.blocks;
        blocks.shrink_to_fit();
        Ok(Dictionary {
            map,
            blocks,
            data,
            len: self.len,
            starts: self.starts,
        })
    }
}

/// `n` in seven-bit groups, low first, each but the last with its top
/// bit set.
#[allow(clippy::cast_possible_truncation)] // the low seven bits, each time
fn varint(out: &mut Vec<u8>, mut n: usize) {
    while n >= 0x80 {
        out.push((n as u8) | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}

fn read_varint(data: &[u8], at: &mut usize) -> usize {
    let mut n = 0usize;
    let mut shift = 0;
    loop {
        let b = data[*at];
        *at += 1;
        n |= usize::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return n;
        }
        shift += 7;
    }
}

/// Every name of a run, each once, numbered in name order.
pub struct Dictionary {
    map: Map<Vec<u8>>,
    blocks: Vec<u64>,
    data: Vec<u8>,
    len: u32,
    /// The rank each namespace starts at, in [`Key::ALL`] order, and the
    /// count at the end.
    starts: [u32; Key::ALL.len() + 1],
}

impl fmt::Debug for Dictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dictionary")
            .field("len", &self.len)
            .field("bytes", &self.bytes())
            .finish_non_exhaustive()
    }
}

impl Dictionary {
    /// A dictionary of `names`, in any order, repeats and all: for tests,
    /// and for callers that hold their names in memory.
    ///
    /// # Errors
    ///
    /// Never, for names that encode; the builder's own errors otherwise.
    pub fn of<'a>(names: impl IntoIterator<Item = Name<'a>>) -> io::Result<Self> {
        let mut keys: Vec<Vec<u8>> = names
            .into_iter()
            .map(|n| {
                let mut k = Vec::new();
                encode(n, &mut k);
                k
            })
            .collect();
        keys.sort_unstable();
        let mut builder = Builder::new();
        for k in &keys {
            builder.push(k)?;
        }
        builder.finish()
    }

    /// How many names.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// No names.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Bytes held: the FST, the list, and the list's block index.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.map.as_fst().as_bytes().len() + self.data.len() + self.blocks.len() * 8
    }

    /// The FST's share of [`Dictionary::bytes`].
    #[must_use]
    pub fn fst_bytes(&self) -> usize {
        self.map.as_fst().as_bytes().len()
    }

    /// `name`'s rank, if it is here.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // ranks are under 2^32
    pub fn find(&self, name: Name<'_>) -> Option<NameIx> {
        let mut key: SmallVec<[u8; 64]> = SmallVec::new();
        encode(name, &mut key);
        self.map.get(&key[..]).map(|rank| rank as NameIx)
    }

    /// The namespace of the name at `ix`, without reading the name.
    ///
    /// # Panics
    ///
    /// An index past the end.
    #[must_use]
    pub fn key(&self, ix: NameIx) -> Key {
        assert!(ix < self.len, "{ix} is past the dictionary's end");
        let t = self.starts[1..]
            .iter()
            .position(|&end| ix < end)
            .unwrap_or(0);
        Key::ALL[t]
    }

    /// The name at `ix`.
    ///
    /// # Panics
    ///
    /// An index past the end.
    #[must_use]
    pub fn get(&self, ix: NameIx) -> OwnedName {
        assert!(ix < self.len, "{ix} is past the dictionary's end");
        let mut key: SmallVec<[u8; 64]> = SmallVec::new();
        let block = ix as usize / BLOCK;
        #[allow(clippy::cast_possible_truncation)] // the list is in memory
        let mut at = self.blocks[block] as usize;
        let len = read_varint(&self.data, &mut at);
        key.extend_from_slice(&self.data[at..at + len]);
        at += len;
        for _ in 0..ix as usize % BLOCK {
            let shared = read_varint(&self.data, &mut at);
            let more = read_varint(&self.data, &mut at);
            key.truncate(shared);
            key.extend_from_slice(&self.data[at..at + more]);
            at += more;
        }
        decode(&key).expect("the list holds only what encode wrote")
    }

    /// Every name, in order.
    #[must_use = "an iterator does nothing unless consumed"]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = OwnedName> + '_ {
        (0..self.len).map(|ix| self.get(ix))
    }
}

#[cfg(test)]
#[allow(clippy::unreadable_literal, clippy::cast_possible_truncation)] // real ids; small tests
mod tests {
    use model::{OpenAlexId, PmcId, Pmid, Registrant};

    use super::{Dictionary, Sorter, decode, encode};
    use crate::name::{Name, OwnedName};

    fn names() -> Vec<OwnedName> {
        let elsevier = Registrant::parse("10.1016").unwrap();
        let wiley = Registrant::parse("10.1111").unwrap();
        vec![
            OwnedName::Doi("10.1000/b".into()),
            OwnedName::Doi("10.1000/a".into()),
            OwnedName::Doi("10.1000/ab".into()),
            OwnedName::Doi("10.1000/a(1)".into()),
            OwnedName::Pmid(Pmid::new(9).unwrap()),
            OwnedName::Pmid(Pmid::new(10).unwrap()),
            OwnedName::Pmid(Pmid::new(41608527).unwrap()),
            OwnedName::Pmc(PmcId::new(7).unwrap()),
            OwnedName::OpenAlex(OpenAlexId::new(4_400_000_000).unwrap()),
            OwnedName::OpenAlex(OpenAlexId::new(12).unwrap()),
            OwnedName::Publisher(wiley, "JEU342".into()),
            OwnedName::Publisher(elsevier, "S0006-3495(25)00123-4".into()),
            OwnedName::Publisher(elsevier, "S0006349525001234".into()),
            OwnedName::Publisher(elsevier, "S0006349525001235".into()),
        ]
    }

    /// Byte order is name order, across and within namespaces — numbers
    /// by value, not by their digits — and a byte form reads back as the
    /// name, a publisher id in its canonical spelling.
    #[test]
    fn byte_order_is_name_order() {
        let names = names();
        for a in &names {
            let mut ka = Vec::new();
            encode(a.as_name(), &mut ka);
            assert_eq!(
                decode(&ka).unwrap().to_string(),
                a.to_string(),
                "{a} reads back"
            );
            for b in &names {
                let mut kb = Vec::new();
                encode(b.as_name(), &mut kb);
                assert_eq!(ka.cmp(&kb), a.cmp(b), "{a} against {b}");
            }
        }
    }

    /// Every name once, ranked in name order, found by either spelling, and
    /// given back; runs spilled and merged give the same dictionary as one
    /// held in memory.
    #[test]
    fn a_dictionary_ranks_names_in_order() {
        let names = names();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len() - 1, "two spellings, one name");
        let whole = Dictionary::of(names.iter().map(OwnedName::as_name)).unwrap();
        let dir = std::env::temp_dir().join(format!("ol-rht-dictionary-{}", std::process::id()));
        let mut gathering = Sorter::create(&dir).unwrap();
        // Every name three times, over runs of a few names each.
        for (i, n) in names.iter().cycle().take(names.len() * 3).enumerate() {
            gathering.push(n.as_name()).unwrap();
            if i % 5 == 4 {
                gathering.spill().unwrap();
            }
        }
        assert!(gathering.runs.len() > 3);
        let spilled = gathering.finish().unwrap();
        assert!(!dir.exists(), "a sorter takes its runs with it");
        for d in [&whole, &spilled] {
            assert_eq!(d.len(), sorted.len());
            let back: Vec<String> = d.iter().map(|n| n.to_string()).collect();
            let want: Vec<String> = sorted.iter().map(ToString::to_string).collect();
            assert_eq!(back, want);
            for (rank, n) in sorted.iter().enumerate() {
                assert_eq!(d.find(n.as_name()), Some(rank as u32), "{n}");
                assert_eq!(d.key(rank as u32), n.key(), "{n}");
            }
            let elsevier = Registrant::parse("10.1016").unwrap();
            assert_eq!(
                d.find(Name::Publisher(elsevier, "S0006-3495(25)00123-4")),
                d.find(Name::Publisher(elsevier, "S0006349525001234")),
            );
            assert_eq!(d.find(Name::Doi("10.1000/c")), None);
        }
    }

    /// Past one block of the front-coded list, names still come back whole.
    #[test]
    fn names_come_back_across_blocks() {
        let dois: Vec<String> = (0..100)
            .map(|i| format!("10.1000/journal.{i:03}"))
            .collect();
        let d = Dictionary::of(dois.iter().map(|s| Name::Doi(s))).unwrap();
        for (rank, doi) in dois.iter().enumerate() {
            assert_eq!(d.get(rank as u32), OwnedName::Doi(doi.as_str().into()));
        }
    }
}
