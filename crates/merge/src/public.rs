//! The public id (serving plan §6): the id the world sees for a work,
//! minted beside the `WorkId` into `merged/public.jsonl`, which is of
//! record. Sixty-four bits of a keyed BLAKE3 hash of the `WorkId` under
//! a key held outside the store — opaque without the key, reproducible
//! with it, so a registry rebuilt from scratch reproduces this log —
//! hashed again with a counter on the rare collision; written as `ol:`,
//! thirteen Crockford base32 characters and a check character. Never
//! reused, never changed; a merged work's id resolves to its survivor
//! through the registry.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::work::WorkId;

/// The namespace, in the names vocabulary's text form.
pub const PREFIX: &str = "ol:";

/// Every line of the log at `path` as (id, public id), in the log's order,
/// read as a stream: for a reader that keeps less of the log than
/// [`Public`]'s two maps. Empty when there is no file.
///
/// # Errors
///
/// I/O, opening the file; each item, I/O or a line that is not a mint,
/// with its line number.
pub fn lines(path: &Path) -> io::Result<impl Iterator<Item = io::Result<(WorkId, String)>>> {
    let file = match File::open(path) {
        Ok(file) => Some(BufReader::new(file)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let path = path.to_path_buf();
    Ok(file
        .into_iter()
        .flat_map(BufRead::lines)
        .enumerate()
        .filter_map(move |(i, text)| {
            let bad = |what: String| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: line {}: {what}", path.display(), i + 1),
                )
            };
            match text {
                Err(e) => Some(Err(e)),
                Ok(text) if text.trim().is_empty() => None,
                Ok(text) => Some(
                    serde_json::from_str::<Line>(&text)
                        .map(|line| (line.id, line.public))
                        .map_err(|e| bad(e.to_string())),
                ),
            }
        }))
}

/// Every id's public id as its 64 bits, in a slot per id: what a reader
/// that only asks an id for its public id keeps of the log. Eight bytes and
/// a bit an id, where [`Public`]'s two maps of text take about 120 — on the
/// corpus's publish the works index's copy of them was about 5 GB (the
/// serving layer's memory, 2026-09-25).
#[derive(Debug, Default)]
pub struct ById {
    bits: Vec<u64>,
    minted: Vec<u64>,
    len: usize,
}

impl ById {
    /// Reads the log at `path` as a stream; empty when there is no file.
    ///
    /// # Errors
    ///
    /// I/O, a line that is not a mint, an id minted twice, or a public id
    /// not in its canonical spelling — which this could not give back as
    /// the log has it.
    pub fn read(path: &Path) -> io::Result<Self> {
        let bad = |what: String| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {what}", path.display()),
            )
        };
        let mut this = Self::default();
        for line in lines(path)? {
            let (id, text) = line?;
            let bits = decode(&text).map_err(|e| bad(format!("{id}: {e}")))?;
            if encode(bits) != text {
                return Err(bad(format!(
                    "{id}: {text:?} is not its public id's canonical spelling"
                )));
            }
            let slot = Self::slot(id);
            if slot >= this.bits.len() {
                this.bits.resize(slot + 1, 0);
                this.minted.resize((slot + 1).div_ceil(64), 0);
            }
            let mask = 1u64 << (slot % 64);
            if this.minted[slot / 64] & mask != 0 {
                return Err(bad(format!("{id} minted twice")));
            }
            this.minted[slot / 64] |= mask;
            this.bits[slot] = bits;
            this.len += 1;
        }
        this.bits.shrink_to_fit();
        this.minted.shrink_to_fit();
        Ok(this)
    }

    fn slot(id: WorkId) -> usize {
        usize::try_from(id.get() - 1).unwrap_or(usize::MAX)
    }

    /// A work's public id.
    #[must_use]
    pub fn of(&self, id: WorkId) -> Option<String> {
        let slot = Self::slot(id);
        let word = self.minted.get(slot / 64)?;
        (word & (1u64 << (slot % 64)) != 0).then(|| encode(self.bits[slot]))
    }

    /// Ids with a public id.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// No id has one.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Crockford's alphabet: no I, L, O or U.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The check alphabet: the 32 symbols and five more, for a value mod 37.
const CHECK: &[u8; 37] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ*~$=U";

/// Characters after the prefix: thirteen for 64 bits, one check.
pub const LEN: usize = 14;

/// The key: 32 bytes from a file the user holds outside the store.
#[derive(Clone)]
pub struct Key([u8; 32]);

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(…)")
    }
}

impl Key {
    /// Reads exactly 32 bytes.
    ///
    /// # Errors
    ///
    /// I/O, or a file that is not 32 bytes.
    pub fn read(path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        let key: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {} bytes, not the 32 of a key",
                    path.display(),
                    bytes.len()
                ),
            )
        })?;
        Ok(Self(key))
    }

    /// A key from bytes, for tests.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// One line of the log: a mint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    #[allow(missing_docs)]
    pub id: WorkId,
    #[allow(missing_docs)]
    pub public: String,
    #[allow(missing_docs)]
    pub at: String,
}

/// The log, folded: every id's public id and every public id's work.
#[derive(Debug, Default)]
pub struct Public {
    by_id: HashMap<WorkId, String>,
    by_text: HashMap<String, WorkId>,
    lines: u64,
}

impl Public {
    /// Reads and folds the log; empty when the file does not exist.
    ///
    /// # Errors
    ///
    /// I/O, a line that does not parse, an id or a public id given twice.
    pub fn read(path: &Path) -> io::Result<Self> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let mut this = Self::default();
        for (i, text) in BufReader::new(file).lines().enumerate() {
            let text = text?;
            if text.trim().is_empty() {
                continue;
            }
            let bad = |what: String| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: line {}: {what}", path.display(), i + 1),
                )
            };
            let line: Line = serde_json::from_str(&text).map_err(|e| bad(e.to_string()))?;
            this.apply(line).map_err(bad)?;
        }
        Ok(this)
    }

    fn apply(&mut self, line: Line) -> Result<(), String> {
        if self.by_id.contains_key(&line.id) {
            return Err(format!("{} minted twice", line.id));
        }
        if self.by_text.contains_key(&line.public) {
            return Err(format!("{} given twice", line.public));
        }
        self.by_text.insert(line.public.clone(), line.id);
        self.by_id.insert(line.id, line.public);
        self.lines += 1;
        Ok(())
    }

    /// Appends lines, then syncs.
    ///
    /// # Errors
    ///
    /// I/O.
    pub fn append(path: &Path, lines: &[Line]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let mut out = BufWriter::new(file);
        for line in lines {
            serde_json::to_writer(&mut out, line)?;
            out.write_all(b"\n")?;
        }
        out.flush()?;
        out.into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()
    }

    /// Ids with a public id.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// No id has one.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Lines folded.
    #[must_use]
    pub const fn lines(&self) -> u64 {
        self.lines
    }

    /// A work's public id.
    #[must_use]
    pub fn of(&self, id: WorkId) -> Option<&str> {
        self.by_id.get(&id).map(String::as_str)
    }

    /// The work a public id names, in any spelling.
    #[must_use]
    pub fn resolve(&self, text: &str) -> Option<WorkId> {
        let canonical = canonical(text).ok()?;
        self.by_text.get(&canonical).copied()
    }

    /// Every id and its public id, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (WorkId, &str)> {
        self.by_id.iter().map(|(id, text)| (*id, text.as_str()))
    }

    /// Mints for every id in `ids` that has none, in the order given;
    /// the lines to append, already folded in.
    pub fn mint(
        &mut self,
        ids: impl IntoIterator<Item = WorkId>,
        key: &Key,
        at: &str,
    ) -> Vec<Line> {
        let mut lines = Vec::new();
        for id in ids {
            if self.by_id.contains_key(&id) {
                continue;
            }
            let mut attempt = 0u32;
            let public = loop {
                let text = encode(derive(key, id, attempt));
                if !self.by_text.contains_key(&text) {
                    break text;
                }
                attempt += 1;
            };
            let line = Line {
                id,
                public,
                at: at.to_owned(),
            };
            if self.apply(line.clone()).is_ok() {
                lines.push(line);
            }
        }
        lines
    }
}

/// The 64 bits for an id, at an attempt: the keyed hash a mint tries
/// first at attempt zero, then again with the counter on a collision.
#[must_use]
pub fn derive(key: &Key, id: WorkId, attempt: u32) -> u64 {
    let mut input = [0u8; 8];
    input[..4].copy_from_slice(&id.get().to_le_bytes());
    input[4..].copy_from_slice(&attempt.to_le_bytes());
    let hash = blake3::keyed_hash(&key.0, &input);
    let mut bits = [0u8; 8];
    bits.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(bits)
}

fn symbol(alphabet: &[u8], value: u64) -> char {
    alphabet[usize::try_from(value).unwrap_or(0)] as char
}

/// `ol:`, thirteen characters for the bits, most significant first, and
/// the check character.
#[must_use]
pub fn encode(bits: u64) -> String {
    let mut out = String::with_capacity(PREFIX.len() + LEN);
    out.push_str(PREFIX);
    for i in (0..13).rev() {
        out.push(symbol(ALPHABET, (bits >> (5 * i)) & 31));
    }
    out.push(symbol(CHECK, bits % 37));
    out
}

/// What a public id failed to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsePublicError {
    #[allow(missing_docs)]
    pub text: String,
    #[allow(missing_docs)]
    pub reason: &'static str,
}

impl fmt::Display for ParsePublicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a public id: {}", self.text, self.reason)
    }
}

impl std::error::Error for ParsePublicError {}

fn value(b: u8) -> Option<u64> {
    match b.to_ascii_uppercase() {
        b'O' => Some(0),
        b'I' | b'L' => Some(1),
        c => ALPHABET
            .iter()
            .position(|&a| a == c)
            .and_then(|p| u64::try_from(p).ok()),
    }
}

/// The 64 bits of a public id from its text: the prefix in any case,
/// Crockford's confusables folded — `I` and `L` as 1, `O` as 0 — and the
/// check character verified.
///
/// # Errors
///
/// No prefix, the wrong length, a character outside the alphabet, or a
/// check character that does not match.
pub fn decode(text: &str) -> Result<u64, ParsePublicError> {
    let err = |reason| ParsePublicError {
        text: text.to_owned(),
        reason,
    };
    let body = text
        .get(..PREFIX.len())
        .filter(|p| p.eq_ignore_ascii_case(PREFIX))
        .and_then(|_| text.get(PREFIX.len()..))
        .ok_or_else(|| err("no ol: prefix"))?;
    if body.len() != LEN {
        return Err(err("not fourteen characters after the prefix"));
    }
    let bytes = body.as_bytes();
    let mut bits: u64 = 0;
    for (i, &b) in bytes[..13].iter().enumerate() {
        let v = value(b).ok_or_else(|| err("a character outside Crockford's alphabet"))?;
        if i == 0 && v >= 16 {
            return Err(err("more than 64 bits"));
        }
        bits = (bits << 5) | v;
    }
    let check = CHECK
        .iter()
        .position(|&c| c == bytes[13].to_ascii_uppercase())
        .and_then(|p| u64::try_from(p).ok())
        .ok_or_else(|| err("a check character outside the alphabet"))?;
    if check != bits % 37 {
        return Err(err("the check character does not match"));
    }
    Ok(bits)
}

/// A public id in its canonical spelling.
///
/// # Errors
///
/// As [`decode`].
pub fn canonical(text: &str) -> Result<String, ParsePublicError> {
    decode(text).map(encode)
}

#[cfg(test)]
mod tests {
    use super::{ById, Key, LEN, Line, PREFIX, Public, canonical, decode, derive, encode};
    use crate::work::WorkId;

    fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    #[test]
    fn text_round_trips_and_checks() {
        for bits in [0, 1, 31, 32, u64::MAX, 0x1234_5678_9abc_def0] {
            let text = encode(bits);
            assert_eq!(text.len(), PREFIX.len() + LEN);
            assert_eq!(decode(&text).unwrap(), bits, "{text}");
            assert_eq!(canonical(&text.to_lowercase()).unwrap(), text);
        }
        assert_eq!(encode(0), "ol:00000000000000");
        let text = encode(0x1234_5678_9abc_def0);
        let folded = text.replace('1', "I").replace('0', "O");
        assert_eq!(canonical(&folded).unwrap(), text);
        let mut wrong = text.clone();
        wrong.pop();
        wrong.push('0');
        assert!(decode(&wrong).unwrap_err().reason.contains("check"));
        assert!(decode("doi:10.1/x").is_err());
        assert!(decode("ol:ZZZZZZZZZZZZZ0").is_err(), "more than 64 bits");
        assert!(decode("ol:0000000000000U0").is_err(), "too long");
    }

    #[test]
    fn minting_is_keyed_deterministic_and_collision_free() {
        let key = Key::from_bytes([7; 32]);
        let other = Key::from_bytes([8; 32]);
        let mut a = Public::default();
        let lines = a.mint([id(1), id(2), id(3)], &key, "t");
        assert_eq!(lines.len(), 3);
        assert_eq!(
            a.mint([id(2), id(4)], &key, "t").len(),
            1,
            "2 already had one"
        );
        let mut b = Public::default();
        assert_eq!(b.mint([id(1), id(2), id(3)], &key, "t"), lines);
        let mut c = Public::default();
        assert_ne!(c.mint([id(1)], &other, "t")[0].public, lines[0].public);
        assert_eq!(a.resolve(&lines[0].public.to_lowercase()), Some(id(1)));
        assert_eq!(a.of(id(3)), Some(lines[2].public.as_str()));
        // A collision: the text id 9 would get at attempt 0 is taken.
        let taken = encode(derive(&key, id(9), 0));
        let mut d = Public::default();
        d.apply(Line {
            id: id(100),
            public: taken.clone(),
            at: "t".into(),
        })
        .unwrap();
        let minted = d.mint([id(9)], &key, "t");
        assert_eq!(minted[0].public, encode(derive(&key, id(9), 1)));
        assert_ne!(minted[0].public, taken);
    }

    #[test]
    fn the_log_reads_back() {
        let dir = std::env::temp_dir().join(format!("ol-rht-public-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("public.jsonl");
        let key = Key::from_bytes([1; 32]);
        let mut public = Public::read(&path).unwrap();
        let lines = public.mint([id(1), id(2)], &key, "t");
        Public::append(&path, &lines).unwrap();
        let again = Public::read(&path).unwrap();
        assert_eq!(again.len(), 2);
        assert_eq!(again.lines(), 2);
        assert_eq!(again.of(id(2)), public.of(id(2)));
        Public::append(&path, &lines[..1]).unwrap();
        assert!(Public::read(&path).is_err(), "minted twice");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The dense table answers every id as the two maps do — ids minted
    /// out of order, ids past its end, ids between with none — and refuses
    /// what the maps refuse of an id, and a spelling it could not give back.
    #[test]
    fn the_table_by_id_answers_as_the_maps_do() {
        let dir = std::env::temp_dir().join(format!("ol-rht-public-by-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("public.jsonl");
        assert!(ById::read(&path).unwrap().is_empty(), "no log, no ids");
        let key = Key::from_bytes([3; 32]);
        let mut public = Public::default();
        let order = [70, 3, 64, 1, 65, 200, 2, 63, 129];
        let lines = public.mint(order.map(id), &key, "t");
        Public::append(&path, &lines).unwrap();
        let by_id = ById::read(&path).unwrap();
        assert_eq!(by_id.len(), order.len());
        for n in 1..=260 {
            assert_eq!(by_id.of(id(n)).as_deref(), public.of(id(n)), "id {n}");
        }
        Public::append(&path, &lines[2..3]).unwrap();
        let err = ById::read(&path).unwrap_err().to_string();
        assert!(err.contains("minted twice"), "{err}");
        std::fs::remove_file(&path).unwrap();
        let lower = Line {
            public: lines[0].public.to_lowercase(),
            ..lines[0].clone()
        };
        Public::append(&path, &[lower]).unwrap();
        let err = ById::read(&path).unwrap_err().to_string();
        assert!(err.contains("canonical spelling"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
