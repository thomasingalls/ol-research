//! Names interned for one run: every distinct name once, addressed by a
//! `u32`, so the identity pass unions and counts integers and holds each
//! DOI's text once.

use std::collections::HashMap;

use ingest::Key;

use crate::name::{Name, OwnedName};

/// An index into an [`Arena`].
pub type NameIx = u32;

/// The names of a run, each once.
#[derive(Debug, Default)]
pub struct Arena {
    names: Vec<OwnedName>,
    dois: HashMap<Box<str>, NameIx>,
    numbers: HashMap<(Key, u64), NameIx>,
}

impl Arena {
    #[allow(missing_docs)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// No names.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// The index of `name`, interning it if new: allocates once per
    /// distinct DOI and never for a name seen before.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 names in a run
    pub fn intern(&mut self, name: Name<'_>) -> NameIx {
        if let Some(ix) = self.find(name) {
            return ix;
        }
        let ix = self.names.len() as NameIx;
        match name {
            Name::Doi(d) => {
                self.dois.insert(d.into(), ix);
            }
            other => {
                self.numbers
                    .insert((other.key(), other.number().unwrap_or(0)), ix);
            }
        }
        self.names.push(name.to_owned());
        ix
    }

    /// The index of `name`, if interned.
    #[must_use]
    pub fn find(&self, name: Name<'_>) -> Option<NameIx> {
        match name {
            Name::Doi(d) => self.dois.get(d).copied(),
            other => self.numbers.get(&(other.key(), other.number()?)).copied(),
        }
    }

    /// The name at `ix`.
    ///
    /// # Panics
    ///
    /// An index this arena did not hand out.
    #[must_use]
    pub fn get(&self, ix: NameIx) -> &OwnedName {
        &self.names[ix as usize]
    }

    /// Every name, in index order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &OwnedName> {
        self.names.iter()
    }
}

#[cfg(test)]
mod tests {
    use model::Pmid;

    use super::Arena;
    use crate::name::{Name, OwnedName};

    #[test]
    fn names_intern_once() {
        let mut arena = Arena::new();
        let a = arena.intern(Name::Doi("10.1000/a"));
        let p = arena.intern(Name::Pmid(Pmid::from_digits("5").unwrap()));
        assert_eq!(arena.intern(Name::Doi("10.1000/a")), a);
        assert_eq!(arena.intern(Name::Pmid(Pmid::from_digits("5").unwrap())), p);
        assert_ne!(a, p);
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.find(Name::Doi("10.1000/b")), None);
        assert_eq!(*arena.get(a), OwnedName::Doi("10.1000/a".into()));
        assert_eq!(arena.iter().count(), 2);
    }
}
