//! Names interned for one run: every distinct name once, addressed by a
//! `u32`, so the identity pass unions and counts integers and holds each
//! DOI's text once.

use std::collections::HashMap;

use ingest::Key;
use model::Registrant;

use crate::name::{Name, OwnedName};
use crate::publisher;

/// An index into an [`Arena`].
pub type NameIx = u32;

/// The names of a run, each once.
#[derive(Debug, Default)]
pub struct Arena {
    /// Each name once, in the spelling the record that first asserted it
    /// wrote — which for a publisher id may not be the canonical one.
    /// Nothing downstream cares: a name renders canonically wherever it
    /// is written (`Name`'s `Display`), so the rows are the same however
    /// the run was ordered.
    names: Vec<OwnedName>,
    dois: HashMap<Box<str>, NameIx>,
    numbers: HashMap<(Key, u64), NameIx>,
    /// Publisher ids by the registrant that scopes them, then by the
    /// canonical spelling of the value: a namespace per registrant
    /// (merge plan §3.1), which is what the nesting is.
    publishers: HashMap<Registrant, HashMap<Box<str>, NameIx>>,
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
            Name::Publisher(registrant, value) => {
                self.publishers
                    .entry(registrant)
                    .or_default()
                    .insert(publisher::canonical(registrant, value).into(), ix);
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
            // The one lookup that may allocate, and only for a value
            // written in Elsevier's punctuated PII.
            Name::Publisher(registrant, value) => self
                .publishers
                .get(&registrant)?
                .get(&*publisher::canonical(registrant, value))
                .copied(),
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
    use model::{Doi, Pmid};

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

    #[test]
    fn a_publisher_id_interns_under_its_registrant_and_spelling() {
        let mut arena = Arena::new();
        let elsevier = Doi::parse("10.1016/x").unwrap().registrant();
        let other = Doi::parse("10.5555/x").unwrap().registrant();
        let punctuated = arena.intern(Name::Publisher(elsevier, "S0006-3495(25)00123-4"));
        assert_eq!(
            arena.intern(Name::Publisher(elsevier, "S0006349525001234")),
            punctuated,
            "one article, one name, either spelling"
        );
        let elsewhere = arena.intern(Name::Publisher(other, "S0006349525001234"));
        assert_ne!(elsewhere, punctuated, "another publisher's namespace");
        assert_eq!(arena.len(), 2);
        assert_eq!(
            *arena.get(punctuated),
            OwnedName::Publisher(elsevier, "S0006349525001234".into()),
            "one name, either spelling"
        );
        // The punctuated spelling was first, so that is what is held;
        // what is written is the canonical one either way.
        assert_eq!(
            arena.get(punctuated).to_string(),
            "reg1016:S0006349525001234"
        );
        assert_eq!(arena.find(Name::Publisher(other, "cvj_12_1266")), None);
    }
}
