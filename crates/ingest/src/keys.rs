//! `keys`: the identifiers a normalized set holds, one per line, as the
//! seed of a lookup listing (plan §8, item 8). A source says which of its
//! normalized values are keys of which kind — a work's own DOI, the PMID
//! a reference points at — and the command prints them, canonical and
//! distinct, in first-seen order.

use std::fmt;

use crate::source::Source;

/// Which identifier a seed holds, or a `keys` run asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    /// A DOI, lowercase, bare (`10.1000/x`).
    Doi,
    /// A PMID, digits.
    Pmid,
    /// A PMC id, `PMC` then digits. A name a work can carry (merge plan
    /// §2); no source looks up by it.
    Pmc,
    /// An OpenAlex work id, bare (`W123`).
    OpenAlex,
}

impl Key {
    /// Every key, in a fixed order.
    pub const ALL: [Self; 4] = [Self::Doi, Self::Pmid, Self::Pmc, Self::OpenAlex];

    /// `doi`, `pmid`, `pmc` or `openalex`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == s)
    }

    /// The name on the command line and in listing paths.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Doi => "doi",
            Self::Pmid => "pmid",
            Self::Pmc => "pmc",
            Self::OpenAlex => "openalex",
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A source whose normalized records yield seeds. Only values the model
/// accepted are keys: a claim that did not parse is not one.
pub trait Keys: Source {
    /// The `key` identifiers of a work, canonical text, each to `out`.
    fn work_keys(work: &Self::Work<'_>, key: Key, out: &mut dyn FnMut(String));

    /// The `key` identifiers a reference points at, each to `out`.
    fn reference_keys(reference: &Self::Reference<'_>, key: Key, out: &mut dyn FnMut(String));
}

#[cfg(test)]
mod tests {
    use super::Key;

    #[test]
    fn keys_have_names() {
        for key in Key::ALL {
            assert_eq!(Key::parse(key.name()), Some(key));
            assert_eq!(key.to_string(), key.name());
        }
        assert_eq!(Key::parse("DOI"), None);
    }
}
