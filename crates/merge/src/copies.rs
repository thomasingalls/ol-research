//! Which copy of a record speaks (merge plan §8, decided 2026-09-17). A
//! record delivered more than once — a baseline file and the update files
//! after it, a snapshot and a later lookup, two versions of a versioned
//! citation — contributes one copy to everything: its names, its fields,
//! its references and its relations. That copy is the newest: the latest
//! revision stamp, then the higher version, then the later set in path
//! order, then the later place in its set. A deletions set withdraws the
//! copies in the sets before it in path order; a record with no live copy
//! contributes nothing.
//!
//! Two copies in one set are one delivery for references: a references
//! set names the record, not the copy, so both copies' rows count.
//!
//! The full merge reads every works set and deletions set into [`Copies`]
//! before its identity pass, and asks it about each copy in both passes;
//! the test fixture asks it about records held in memory; the incremental
//! merge ranks a batch's copy against a row's with [`Delivery`] alone.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use ingest::Kind;
use model::Date;

use crate::contributor::Contributor;
use crate::dictionary::Sorter;
use crate::name::{Name, OwnedName};
use crate::sets;
use crate::view::View;
use crate::work::SourceId;

/// When a source delivered one copy of a record, ordered the way the rule
/// ranks copies: the greater delivery is the newer.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Delivery {
    /// The source's revision stamp.
    pub revised: Option<Date>,
    /// The source's version of the record.
    pub version: u32,
    /// The set, relative to its source's directory, without the kind.
    pub set: Box<str>,
    /// The copy's place in its set, from 1; 0 where the place is unknown,
    /// as for a merged row's record.
    pub ordinal: u64,
}

impl Delivery {
    /// A copy's delivery from its view, which carries the stamp, the
    /// version and the set.
    #[must_use]
    pub fn of(view: &View<'_>, ordinal: u64) -> Self {
        Self {
            revised: view.revised,
            version: view.version,
            set: view.set.into(),
            ordinal,
        }
    }

    /// Whether `other` is a newer delivery of the same record, the place in
    /// a set not counted: how the incremental merge ranks a batch's copy
    /// against a merged row's, which does not keep its place.
    #[must_use]
    pub fn older_than(&self, other: &Self) -> bool {
        (self.revised, self.version, &self.set) < (other.revised, other.version, &other.set)
    }
}

/// What became of one copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// The record's newest live copy.
    Speaks,
    /// A newer live copy speaks for the record.
    Superseded,
    /// A deletions set after this copy's withdrew the record.
    Withdrawn,
}

/// Counts, by source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    /// Copies seen.
    pub copies: BTreeMap<SourceId, u64>,
    /// Copies a newer one superseded.
    pub superseded: BTreeMap<SourceId, u64>,
    /// Copies a later deletions set withdrew.
    pub withdrawn: BTreeMap<SourceId, u64>,
    /// Records with two copies in one set.
    pub within_one_set: BTreeMap<SourceId, u64>,
}

/// Why reading a set for the rule stopped.
#[derive(Debug)]
pub enum Error {
    /// A set could not be read.
    Io {
        #[allow(missing_docs)]
        path: std::path::PathBuf,
        #[allow(missing_docs)]
        source: std::io::Error,
    },
    /// A record with no native name.
    NoNativeName {
        #[allow(missing_docs)]
        path: std::path::PathBuf,
        /// From 1.
        ordinal: u64,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NoNativeName { path, ordinal } => {
                write!(
                    f,
                    "{}: record {ordinal} asserts no native name",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for Error {}

/// Every copy of every record a run reads, and the withdrawals.
///
/// A copy's delivery is kept as [`Kept`]: its set as a place in the list
/// of sets the table has seen, and its place in the set in four bytes, so
/// that a record costs no string of its own — on the corpus, 40 M of them
/// (the merge's memory, step 2c, 2026-09-24). Sets compare by their names,
/// as [`Delivery`] compares them, never by their places in the list.
#[derive(Debug, Default)]
pub struct Copies {
    newest: HashMap<(SourceId, OwnedName), Kept>,
    withdrawn: HashMap<(SourceId, OwnedName), u32>,
    sets: Vec<Box<str>>,
    set_ix: HashMap<Box<str>, u32>,
    #[allow(missing_docs)]
    pub stats: Stats,
}

/// A [`Delivery`] as [`Copies`] keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Kept {
    revised: Option<Date>,
    version: u32,
    set: u32,
    ordinal: u32,
}

impl Copies {
    #[allow(missing_docs)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The place of `set` in the list of sets, adding it if new.
    #[allow(clippy::cast_possible_truncation)] // fewer than 2^32 sets in a run
    fn intern(&mut self, set: &str) -> u32 {
        if let Some(&ix) = self.set_ix.get(set) {
            return ix;
        }
        let ix = self.sets.len() as u32;
        self.sets.push(set.into());
        self.set_ix.insert(set.into(), ix);
        ix
    }

    /// `delivery` as the table keeps it.
    ///
    /// # Panics
    ///
    /// A place in a set past 2^32, which no set comes near.
    fn keep(&mut self, delivery: &Delivery) -> Kept {
        Kept {
            revised: delivery.revised,
            version: delivery.version,
            set: self.intern(&delivery.set),
            ordinal: u32::try_from(delivery.ordinal).expect("fewer than 2^32 records in a set"),
        }
    }

    /// Whether `a` is an older delivery than `b`, by [`Delivery`]'s order.
    fn older(&self, a: &Kept, b: &Kept) -> bool {
        (a.revised, a.version, &*self.sets[a.set as usize], a.ordinal)
            < (b.revised, b.version, &*self.sets[b.set as usize], b.ordinal)
    }

    /// Whether the table keeps `delivery` as `kept`.
    fn same(&self, kept: &Kept, delivery: &Delivery) -> bool {
        kept.revised == delivery.revised
            && kept.version == delivery.version
            && *self.sets[kept.set as usize] == *delivery.set
            && u64::from(kept.ordinal) == delivery.ordinal
    }

    /// A deletions set's word that a record is withdrawn: every copy in a
    /// set before `set` in path order is dead. The latest such set counts.
    pub fn withdraw(&mut self, source: SourceId, native: OwnedName, set: &str) {
        let ix = self.intern(set);
        let slot = self.withdrawn.entry((source, native)).or_insert(ix);
        if *self.sets[*slot as usize] < *set {
            *slot = ix;
        }
    }

    fn dead(&self, key: &(SourceId, OwnedName), set: &str) -> bool {
        self.withdrawn
            .get(key)
            .is_some_and(|&deleted| *self.sets[deleted as usize] > *set)
    }

    /// One copy, as the first reading sees it; a dead copy is not a
    /// candidate. Call after every withdrawal.
    pub fn see(&mut self, source: SourceId, native: Name<'_>, delivery: &Delivery) {
        self.seen(source, native, delivery);
    }

    /// [`Copies::see`], saying whether the copy stands: not withdrawn.
    fn seen(&mut self, source: SourceId, native: Name<'_>, delivery: &Delivery) -> bool {
        let key = (source, native.to_owned());
        *self.stats.copies.entry(source).or_default() += 1;
        if self.dead(&key, &delivery.set) {
            return false;
        }
        let kept = self.keep(delivery);
        match self.newest.get(&key).copied() {
            None => {
                self.newest.insert(key, kept);
            }
            Some(current) => {
                if current.set == kept.set {
                    *self.stats.within_one_set.entry(source).or_default() += 1;
                }
                if self.older(&current, &kept) {
                    self.newest.insert(key, kept);
                }
            }
        }
        true
    }

    /// What became of the copy delivered as `delivery`, after every copy
    /// was seen.
    #[must_use]
    pub fn fate(&self, source: SourceId, native: Name<'_>, delivery: &Delivery) -> Fate {
        let key = (source, native.to_owned());
        if self.dead(&key, &delivery.set) {
            Fate::Withdrawn
        } else if self
            .newest
            .get(&key)
            .is_some_and(|kept| self.same(kept, delivery))
        {
            Fate::Speaks
        } else {
            Fate::Superseded
        }
    }

    /// [`Copies::fate`], counted.
    pub fn judge(&mut self, source: SourceId, native: Name<'_>, delivery: &Delivery) -> Fate {
        let fate = self.fate(source, native, delivery);
        match fate {
            Fate::Speaks => {}
            Fate::Superseded => *self.stats.superseded.entry(source).or_default() += 1,
            Fate::Withdrawn => *self.stats.withdrawn.entry(source).or_default() += 1,
        }
        fate
    }

    /// Whether the copy of the record in `set` is still standing: not
    /// withdrawn by a later deletions set. A record's references are the
    /// union over its standing copies (merge plan §8, 2026-09-18), since
    /// PubMed replaces a publisher's reference list with PMC's when an
    /// article lands there, and the ids the publisher gave are evidence we
    /// already have.
    #[must_use]
    pub fn alive_in(&self, source: SourceId, native: Name<'_>, set: &str) -> bool {
        !self.dead(&(source, native.to_owned()), set)
    }

    /// Whether the record's speaking copy is in `set`: what a set's names,
    /// fields, relations and text are asked.
    #[must_use]
    pub fn speaks_in(&self, source: SourceId, native: Name<'_>, set: &str) -> bool {
        let key = (source, native.to_owned());
        self.newest.get(&key).is_some_and(|kept| {
            let speaking = &self.sets[kept.set as usize];
            **speaking == *set && !self.dead(&key, speaking)
        })
    }

    /// Reads one deletions set of `S`.
    ///
    /// # Errors
    ///
    /// The set cannot be read.
    pub fn read_deletions<S: Contributor>(&mut self, path: &Path, set: &str) -> Result<u64, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = sets::open::<S>(path, Kind::Deletions).map_err(io)?;
        let mut read = 0u64;
        for record in reader.records::<S::Id<'_>>() {
            let id = record.map_err(io)?;
            self.withdraw(S::SOURCE, S::native(&id).to_owned(), set);
            read += 1;
        }
        Ok(read)
    }

    /// Reads one works set of `S` for every copy's delivery, and gives
    /// `names` every name a standing copy asserts — a full merge's
    /// dictionary is made of them (the merge's memory, step 2b), and they
    /// are exactly the names the identity pass reads, since it reads every
    /// copy a deletion did not withdraw. Call after every deletions set is
    /// read.
    ///
    /// # Errors
    ///
    /// The set cannot be read, or a record has no native name.
    pub fn read_works<S: Contributor>(
        &mut self,
        path: &Path,
        set: &str,
        limit: Option<usize>,
        mut names: Option<&mut Sorter>,
    ) -> Result<u64, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = sets::open::<S>(path, Kind::Works).map_err(io)?;
        let mut read = 0u64;
        reader.with_table(|| {
            for (i, work) in reader
                .records::<S::Work<'_>>()
                .take(limit.unwrap_or(usize::MAX))
                .enumerate()
            {
                let work = work.map_err(io)?;
                let ordinal = i as u64 + 1;
                let native = native_of::<S>(&work).ok_or_else(|| Error::NoNativeName {
                    path: path.to_path_buf(),
                    ordinal,
                })?;
                let delivery = delivery_of::<S>(&work, set, ordinal);
                let stands = self.seen(S::SOURCE, native, &delivery);
                if let Some(sorter) = names.as_deref_mut().filter(|_| stands) {
                    let mut failed = None;
                    S::names(&work, &mut |a| {
                        if failed.is_none()
                            && let Err(e) = sorter.push(a.name)
                        {
                            failed = Some(e);
                        }
                    });
                    if let Some(e) = failed {
                        return Err(io(e));
                    }
                }
                read += 1;
            }
            Ok(read)
        })
    }
}

/// A record's native name, if it asserts one.
pub fn native_of<'w, S: Contributor>(work: &'w S::Work<'_>) -> Option<Name<'w>> {
    let mut native = None;
    S::names(work, &mut |a| {
        if a.native && native.is_none() {
            native = Some(a.name);
        }
    });
    native
}

/// A copy's delivery from the record itself and where it was read.
pub fn delivery_of<S: Contributor>(work: &S::Work<'_>, set: &str, ordinal: u64) -> Delivery {
    let (revised, version) = S::revision(work);
    Delivery {
        revised,
        version,
        set: set.into(),
        ordinal,
    }
}

/// A set's name for the rule: its path relative to `dir`, the source's
/// normalized directory, without the kind — `updatefiles/pubmed26n1335`.
/// `None` for a path outside `dir` or not a set.
#[must_use]
pub fn set_name(dir: &Path, path: &Path) -> Option<Box<str>> {
    prefix_name(dir, &Kind::prefix_of(path)?)
}

/// [`set_name`] for a set's prefix, the path without the kind.
#[must_use]
pub fn prefix_name(dir: &Path, prefix: &Path) -> Option<Box<str>> {
    let relative = prefix.strip_prefix(dir).ok()?;
    let parts: Vec<&str> = relative
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<_>>()?;
    (!parts.is_empty()).then(|| parts.join("/").into())
}

#[cfg(test)]
mod tests {
    use super::{Copies, Delivery, Fate};
    use crate::name::OwnedName;
    use crate::work::SourceId;
    use model::Date;

    fn d(revised: Option<&str>, version: u32, set: &str, ordinal: u64) -> Delivery {
        Delivery {
            revised: revised.map(|r| Date::parse_iso(r).unwrap()),
            version,
            set: set.into(),
            ordinal,
        }
    }

    #[test]
    fn the_newest_copy_speaks_by_stamp_then_version_then_set_then_place() {
        let pmid = OwnedName::parse("pmid:1").unwrap();
        let name = pmid.as_name();
        let mut copies = Copies::new();
        let baseline = d(Some("2026-01-01"), 1, "baseline/pubmed26n1334", 7);
        let update = d(Some("2026-01-01"), 1, "updatefiles/pubmed26n1335", 3);
        let older_stamp_later_set = d(Some("2025-12-01"), 1, "updatefiles/pubmed26n1336", 1);
        for copy in [&baseline, &update, &older_stamp_later_set] {
            copies.see(SourceId::Pubmed, name, copy);
        }
        assert_eq!(copies.fate(SourceId::Pubmed, name, &update), Fate::Speaks);
        assert_eq!(
            copies.fate(SourceId::Pubmed, name, &baseline),
            Fate::Superseded
        );
        assert_eq!(
            copies.fate(SourceId::Pubmed, name, &older_stamp_later_set),
            Fate::Superseded
        );
        assert!(copies.speaks_in(SourceId::Pubmed, name, "updatefiles/pubmed26n1335"));
        assert!(!copies.speaks_in(SourceId::Pubmed, name, "baseline/pubmed26n1334"));
        // Two versions of a versioned citation in one set, the second
        // version first: the higher version speaks.
        let v2 = d(Some("2026-02-03"), 2, "updatefiles/pubmed26n1339", 10);
        let v1 = d(Some("2026-02-03"), 1, "updatefiles/pubmed26n1339", 11);
        let other = OwnedName::parse("pmid:34621506").unwrap();
        copies.see(SourceId::Pubmed, other.as_name(), &v2);
        copies.see(SourceId::Pubmed, other.as_name(), &v1);
        assert_eq!(
            copies.fate(SourceId::Pubmed, other.as_name(), &v2),
            Fate::Speaks
        );
        assert_eq!(
            copies.fate(SourceId::Pubmed, other.as_name(), &v1),
            Fate::Superseded
        );
        assert_eq!(copies.stats.within_one_set[&SourceId::Pubmed], 1);
    }

    #[test]
    fn a_deletion_withdraws_the_copies_before_it_and_not_after() {
        let pmid = OwnedName::parse("pmid:2").unwrap();
        let name = pmid.as_name();
        let mut copies = Copies::new();
        copies.withdraw(SourceId::Pubmed, pmid.clone(), "updatefiles/pubmed26n1336");
        let before = d(Some("2026-01-01"), 1, "baseline/pubmed26n1334", 1);
        let after = d(Some("2025-01-01"), 1, "updatefiles/pubmed26n1338", 1);
        copies.see(SourceId::Pubmed, name, &before);
        assert_eq!(
            copies.fate(SourceId::Pubmed, name, &before),
            Fate::Withdrawn
        );
        assert!(!copies.speaks_in(SourceId::Pubmed, name, "baseline/pubmed26n1334"));
        copies.see(SourceId::Pubmed, name, &after);
        assert_eq!(copies.fate(SourceId::Pubmed, name, &after), Fate::Speaks);
        // A later deletion withdraws that copy too.
        copies.withdraw(SourceId::Pubmed, pmid.clone(), "updatefiles/pubmed26n1339");
        assert_eq!(copies.fate(SourceId::Pubmed, name, &after), Fate::Withdrawn);
    }

    #[test]
    fn a_row_copy_is_ranked_without_its_place() {
        let row = d(Some("2026-01-01"), 1, "baseline/pubmed26n1334", 0);
        let batch = d(Some("2026-01-01"), 1, "updatefiles/pubmed26n1335", 9);
        assert!(row.older_than(&batch));
        assert!(!batch.older_than(&row));
        let same = d(Some("2026-01-01"), 1, "baseline/pubmed26n1334", 5);
        assert!(!row.older_than(&same) && !same.older_than(&row));
    }
}
