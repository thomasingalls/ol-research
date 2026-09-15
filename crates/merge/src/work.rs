//! Our id for a work, and the sources a fact came from.

use std::fmt;
use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

/// Our id for a work (merge plan §2): minted by the registry, dense,
/// opaque, derived from nothing. It never leaves the system; the world is
/// reached through the work's names.
///
/// One-based, so `Option<WorkId>` is four bytes and the number zero can
/// never be mistaken for a work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkId(NonZeroU32);

impl WorkId {
    /// The first id a registry mints.
    pub const FIRST: Self = Self(NonZeroU32::MIN);

    /// `n` as an id; `None` for zero.
    #[must_use]
    pub const fn new(n: u32) -> Option<Self> {
        match NonZeroU32::new(n) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// The id after this one; `None` past `u32::MAX`, which the plan's
    /// scale never reaches.
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(n) => Some(Self(n)),
            None => None,
        }
    }
}

impl fmt::Display for WorkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// A source, for bitmaps, precedence and reports. The order is fixed —
/// a new source appends — because it is written into merged sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum SourceId {
    #[allow(missing_docs)]
    Pubmed = 0,
    #[allow(missing_docs)]
    Crossref = 1,
    #[allow(missing_docs)]
    OpenAlex = 2,
}

impl SourceId {
    /// Every source, in bit order.
    pub const ALL: [Self; 3] = [Self::Pubmed, Self::Crossref, Self::OpenAlex];

    /// The name in reports and in the store: [`ingest::Source::NAME`].
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pubmed => "pubmed",
            Self::Crossref => "crossref",
            Self::OpenAlex => "openalex",
        }
    }

    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A set of sources, one bit each: who asserted an edge, who holds a
/// record of a work.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Sources(u8);

impl Sources {
    /// No source.
    pub const NONE: Self = Self(0);

    /// Adds one source.
    pub const fn insert(&mut self, source: SourceId) {
        self.0 |= source.bit();
    }

    /// Whether `source` is in the set.
    #[must_use]
    pub const fn contains(self, source: SourceId) -> bool {
        self.0 & source.bit() != 0
    }

    /// How many sources.
    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// No source at all.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The sources, in bit order.
    pub fn iter(self) -> impl Iterator<Item = SourceId> {
        SourceId::ALL.into_iter().filter(move |s| self.contains(*s))
    }
}

impl FromIterator<SourceId> for Sources {
    fn from_iter<I: IntoIterator<Item = SourceId>>(iter: I) -> Self {
        let mut set = Self::NONE;
        for s in iter {
            set.insert(s);
        }
        set
    }
}

/// `pubmed+crossref`, or `none`.
impl fmt::Display for Sources {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("none");
        }
        for (i, s) in self.iter().enumerate() {
            if i > 0 {
                f.write_str("+")?;
            }
            f.write_str(s.name())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceId, Sources, WorkId};

    #[test]
    fn ids_start_at_one_and_count_up() {
        assert_eq!(WorkId::new(0), None);
        assert_eq!(WorkId::FIRST.get(), 1);
        assert_eq!(WorkId::FIRST.next().unwrap().get(), 2);
        assert_eq!(WorkId::new(u32::MAX).unwrap().next(), None);
        assert_eq!(WorkId::FIRST.to_string(), "#1");
        assert_eq!(serde_json::to_string(&WorkId::FIRST).unwrap(), "1");
        assert_eq!(size_of::<Option<WorkId>>(), 4);
    }

    #[test]
    fn sources_are_bits() {
        let mut set = Sources::NONE;
        assert!(set.is_empty());
        assert_eq!(set.to_string(), "none");
        set.insert(SourceId::OpenAlex);
        set.insert(SourceId::Pubmed);
        assert_eq!(set.len(), 2);
        assert!(set.contains(SourceId::Pubmed) && !set.contains(SourceId::Crossref));
        assert_eq!(set.to_string(), "pubmed+openalex");
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            [SourceId::Pubmed, SourceId::OpenAlex]
        );
        assert_eq!(SourceId::ALL.into_iter().collect::<Sources>().len(), 3);
        assert_eq!(
            serde_json::to_string(&SourceId::OpenAlex).unwrap(),
            "\"openalex\""
        );
    }
}
