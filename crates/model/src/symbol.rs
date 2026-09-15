//! Symbol tables (plan §5, rule 4): repeated open values — venue names,
//! publishers, descriptors, affiliations — written once per output file
//! and referred to by a `u32` symbol, so the same forty-byte journal name
//! is stored once per file rather than ten thousand times.
//!
//! [`Interned`] is text while a record is being normalized, and a symbol
//! once it has been written. A binary serializer interns the text through
//! the [`Interner`] the sink installs on its thread for the file it is
//! writing; a human-readable serializer writes the text itself. Read back
//! from binary the value is a symbol, resolved through the file's
//! [`Table`] by [`Interned::resolve`], or, for a human-readable
//! serializer, through the table installed for reading. The thread-local
//! is the whole coupling: a worker writes one file at a time on one
//! thread, and a reader resolves one file's records at a time.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

use serde::de::Deserializer;
use serde::ser::Error as _;
use serde::{Deserialize, Serialize, Serializer};

/// A value that is interned when written to binary: text before, a
/// symbol after.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Interned<'a> {
    /// The text, as the normalizer produced it.
    Text(&'a str),
    /// An index into the file's [`Table`], as read back from binary.
    Symbol(u32),
}

impl<'a> Interned<'a> {
    /// The text, when this is text.
    #[must_use]
    pub const fn as_text(&self) -> Option<&'a str> {
        match self {
            Self::Text(t) => Some(t),
            Self::Symbol(_) => None,
        }
    }

    /// The text, resolving a symbol through `table`; `None` for a symbol
    /// the table does not have.
    #[must_use]
    pub fn resolve<'s>(&'s self, table: &'s Table) -> Option<&'s str> {
        match self {
            Self::Text(t) => Some(t),
            Self::Symbol(n) => table.get(*n),
        }
    }
}

impl fmt::Debug for Interned<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(t) => fmt::Debug::fmt(t, f),
            Self::Symbol(n) => write!(f, "#{n}"),
        }
    }
}

impl PartialEq<str> for Interned<'_> {
    fn eq(&self, other: &str) -> bool {
        self.as_text() == Some(other)
    }
}

impl PartialEq<&str> for Interned<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_text() == Some(*other)
    }
}

impl<'a> From<&'a str> for Interned<'a> {
    fn from(s: &'a str) -> Self {
        Self::Text(s)
    }
}

/// A file's symbols, in symbol order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table(Arc<[Box<str>]>);

impl Table {
    /// The string of symbol `n`.
    #[must_use]
    pub fn get(&self, n: u32) -> Option<&str> {
        self.0.get(n as usize).map(|s| &**s)
    }

    /// How many symbols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// No symbols.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The strings, in symbol order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &str> {
        self.0.iter().map(|s| &**s)
    }
}

impl From<Vec<Box<str>>> for Table {
    fn from(strings: Vec<Box<str>>) -> Self {
        Self(strings.into())
    }
}

/// `FxHash`: a few multiplies per word; the keys are short and many.
#[derive(Default)]
struct Fx(u64);

impl Hasher for Fx {
    fn write(&mut self, bytes: &[u8]) {
        const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        let (chunks, tail) = bytes.as_chunks::<8>();
        for c in chunks {
            let word = u64::from_le_bytes(*c);
            self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(K);
        }
        if !tail.is_empty() {
            let mut rest = [0u8; 8];
            rest[..tail.len()].copy_from_slice(tail);
            let word = u64::from_le_bytes(rest);
            self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(K);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// Builds a [`Table`] one string at a time, each seen string kept once.
#[derive(Default)]
pub struct Interner {
    index: HashMap<Box<str>, u32, BuildHasherDefault<Fx>>,
}

impl fmt::Debug for Interner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Interner")
            .field("symbols", &self.index.len())
            .finish_non_exhaustive()
    }
}

impl Interner {
    /// The symbol of `s`, new if unseen. The table is bounded by the
    /// vocabulary, not the corpus: this allocates once per distinct
    /// string, and never for one seen before.
    #[allow(clippy::cast_possible_truncation)] // a file has fewer than 2^32 distinct strings
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&n) = self.index.get(s) {
            return n;
        }
        let n = self.index.len() as u32;
        self.index.insert(s.into(), n);
        n
    }

    /// The table so far, in symbol order.
    ///
    /// # Panics
    ///
    /// Never: every symbol handed out has its string.
    #[must_use]
    pub fn finish(self) -> Table {
        let mut strings: Vec<Option<Box<str>>> = vec![None; self.index.len()];
        for (s, n) in self.index {
            strings[n as usize] = Some(s);
        }
        Table::from(
            strings
                .into_iter()
                .map(|s| s.expect("every symbol has its string"))
                .collect::<Vec<_>>(),
        )
    }
}

enum Slot {
    Writing(Interner),
    Reading(Table),
}

thread_local! {
    static SLOT: RefCell<Option<Slot>> = const { RefCell::new(None) };
}

/// Something is already installed on this thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installed;

impl fmt::Display for Installed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a symbol table is already installed on this thread")
    }
}

impl std::error::Error for Installed {}

/// Installs a fresh [`Interner`] on this thread: [`Interned`] text
/// serialized to binary is interned through it until [`end_writing`].
///
/// # Errors
///
/// A table is already installed for writing or reading.
pub fn begin_writing() -> Result<(), Installed> {
    SLOT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(Installed);
        }
        *slot = Some(Slot::Writing(Interner::default()));
        Ok(())
    })
}

/// Takes the interner installed by [`begin_writing`] as the file's table;
/// `None` when nothing was being written.
#[must_use]
pub fn end_writing() -> Option<Table> {
    SLOT.with(|slot| {
        let taken = slot.borrow_mut().take();
        match taken {
            Some(Slot::Writing(i)) => Some(i.finish()),
            Some(reading @ Slot::Reading(_)) => {
                *slot.borrow_mut() = Some(reading);
                None
            }
            None => None,
        }
    })
}

/// Installs `table` on this thread for reading: a symbol serialized to a
/// human-readable form resolves through it until [`end_reading`].
///
/// # Errors
///
/// A table is already installed for writing or reading.
pub fn begin_reading(table: Table) -> Result<(), Installed> {
    SLOT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return Err(Installed);
        }
        *slot = Some(Slot::Reading(table));
        Ok(())
    })
}

/// Removes the table installed by [`begin_reading`].
#[must_use]
pub fn end_reading() -> Option<Table> {
    SLOT.with(|slot| {
        let taken = slot.borrow_mut().take();
        match taken {
            Some(Slot::Reading(t)) => Some(t),
            Some(writing @ Slot::Writing(_)) => {
                *slot.borrow_mut() = Some(writing);
                None
            }
            None => None,
        }
    })
}

/// Runs `f` with `table` installed for reading.
///
/// # Panics
///
/// A table is already installed on this thread.
pub fn with_reading<R>(table: &Table, f: impl FnOnce() -> R) -> R {
    begin_reading(table.clone()).expect("no table installed");
    let r = f();
    let _ = end_reading();
    r
}

impl Serialize for Interned<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            match self {
                Self::Text(t) => serializer.serialize_str(t),
                Self::Symbol(n) => SLOT.with(|slot| match &*slot.borrow() {
                    Some(Slot::Reading(table)) => match table.get(*n) {
                        Some(t) => serializer.serialize_str(t),
                        None => Err(S::Error::custom(format!("symbol {n} beyond the table"))),
                    },
                    _ => Err(S::Error::custom(
                        "a symbol needs its file's table installed to be written as text",
                    )),
                }),
            }
        } else {
            match self {
                Self::Text(t) => SLOT.with(|slot| match &mut *slot.borrow_mut() {
                    Some(Slot::Writing(interner)) => serializer.serialize_u32(interner.intern(t)),
                    _ => Err(S::Error::custom(
                        "text interned outside a file being written",
                    )),
                }),
                Self::Symbol(n) => SLOT.with(|slot| match &*slot.borrow() {
                    Some(Slot::Reading(_)) => serializer.serialize_u32(*n),
                    _ => Err(S::Error::custom(
                        "a symbol written to binary outside its file",
                    )),
                }),
            }
        }
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Interned<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            <&'de str>::deserialize(deserializer).map(Interned::Text)
        } else {
            u32::deserialize(deserializer).map(Interned::Symbol)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Interned, Table, begin_reading, begin_writing, end_reading, end_writing, with_reading,
    };

    #[test]
    fn text_is_interned_on_the_way_to_binary_and_resolved_on_the_way_back() {
        begin_writing().unwrap();
        let record = [
            Interned::Text("Nature"),
            Interned::Text("Science"),
            Interned::Text("Nature"),
        ];
        let bytes = postcard::to_allocvec(&record).unwrap();
        let table = end_writing().unwrap();
        assert_eq!(table.len(), 2);
        assert_eq!(table.iter().collect::<Vec<_>>(), ["Nature", "Science"]);
        assert_eq!(
            bytes,
            [0, 1, 0],
            "a varint symbol each; a fixed array has no length"
        );
        let back: [Interned<'_>; 3] = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(
            back,
            [
                Interned::Symbol(0),
                Interned::Symbol(1),
                Interned::Symbol(0)
            ]
        );
        assert_eq!(back[1].resolve(&table), Some("Science"));
        assert_eq!(back[1].as_text(), None);
        assert_eq!(format!("{:?}", back[1]), "#1");
        let json = with_reading(&table, || serde_json::to_string(&back).unwrap());
        assert_eq!(json, r#"["Nature","Science","Nature"]"#);
        assert_eq!(serde_json::to_string(&record).unwrap(), json);
        let text: Vec<Interned<'_>> = serde_json::from_str(&json).unwrap();
        assert_eq!(text, record);
        assert_eq!(record[0], "Nature");
        assert!(end_reading().is_none(), "nothing left installed");
    }

    #[test]
    fn writing_and_reading_need_their_table_and_refuse_to_nest() {
        assert!(
            postcard::to_allocvec(&Interned::Text("x")).is_err(),
            "no interner installed"
        );
        assert!(
            serde_json::to_string(&Interned::Symbol(0)).is_err(),
            "no table installed"
        );
        begin_writing().unwrap();
        assert!(begin_writing().is_err());
        assert!(begin_reading(Table::default()).is_err());
        assert!(end_reading().is_none(), "writing stays installed");
        end_writing().unwrap();
        assert!(end_writing().is_none());
    }
}
