//! The second pass of merge plan §8: every record's view joins its work,
//! every reference resolves through the names table, and the rows of the
//! merged sets are made — held works and stubs with their promoted fields
//! (§5), edges, relations and the unresolved (§6) — then checked against
//! the invariants of §8.
//!
//! The pass keeps one owned [`view::Owned`] per record until every set
//! has been read, then promotes per work; the plan's §8 names the range
//! shape that bounds this at scale.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use ingest::{Key, Kind};
use model::Issn;
use model::date::Bound;
use model::symbol::{Interned, Table};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::arena::NameIx;
use crate::contributor::Contributor;
use crate::copies::{self, Copies, Fate};
use crate::identity::{RecordIx, Resolution, Standing, Work as Identified};
use crate::kind::{Form, Genre};
use crate::name::{Name, OwnedName};
use crate::record::{
    Assert, Citation, CitedAs, Citer, Edge, Field, Fields, Flags, NameRow, Named, Other, Record,
    Rejected, Relation, StatusBy, Unresolved, Why, Work,
};
use crate::relation::{self, Related};
use crate::sets;
use crate::title::{self, Agreement};
use crate::view::{self, Owned, OwnedVenue, Unmapped, Venue};
use crate::work::{SourceId, Sources, WorkId};

/// Why the pass stopped.
#[derive(Debug)]
pub enum Error {
    /// A set could not be read.
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        source: io::Error,
    },
    /// A set without its symbol table beside it.
    NoTable(PathBuf),
    /// A record's value outside the merge's tables (merge plan §5, §6).
    Unmapped {
        #[allow(missing_docs)]
        path: PathBuf,
        /// From one.
        ordinal: u64,
        #[allow(missing_docs)]
        source: Unmapped,
    },
    /// An invariant of merge plan §8 does not hold: the merge is wrong,
    /// not the data.
    Invariant(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::NoTable(path) => write!(f, "{}: no symbol table beside it", path.display()),
            Self::Unmapped {
                path,
                ordinal,
                source,
            } => write!(f, "{}: record {ordinal}: {source}", path.display()),
            Self::Invariant(what) => write!(f, "invariant broken: {what}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Unmapped { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The ids the registry gave: one per held work, aligned with
/// [`Resolution::works`], and one per stub, aligned with
/// [`Resolution::stub_works`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ids {
    #[allow(missing_docs)]
    pub held: Vec<WorkId>,
    #[allow(missing_docs)]
    pub stubs: Vec<WorkId>,
}

/// What the pass counted.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    /// Records viewed per source.
    pub viewed: BTreeMap<SourceId, u64>,
    /// Copies joined into a record's view, per source: the second and
    /// later copy of one record, whose values fill what the newest has
    /// none of (§8).
    pub filled: BTreeMap<SourceId, u64>,
    /// Per source: references by what became of them — `held`, `stub`,
    /// `self`, `no name`, `shared`, `disagree`, `empty`, `unknown`,
    /// `citing record unknown`.
    pub references: BTreeMap<(SourceId, &'static str), u64>,
    /// Per source: relation targets likewise — `held`, `stub`, `not a
    /// name`, `shared`, `unknown`.
    pub relations: BTreeMap<(SourceId, &'static str), u64>,
    /// Relations kept, by kind.
    pub relation_kinds: BTreeMap<relation::Kind, u64>,
    /// Under `--survey`: values outside the merge's tables, by source
    /// and `field: value`, that stopped nothing; the record has no view.
    pub unmapped: BTreeMap<(SourceId, String), u64>,
}

/// What one promoted field's rule found over every work.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FieldStat {
    /// Works with a value.
    pub present: u64,
    /// Works with two or more candidates.
    pub compared: u64,
    /// Works where another source differs under the rule.
    pub disputed: u64,
    /// Which source the value came from.
    pub from: BTreeMap<SourceId, u64>,
    /// Per pair of sources, the promoted one first: candidates compared,
    /// and how many differed.
    pub pairs: BTreeMap<(SourceId, SourceId), (u64, u64)>,
}

/// A source record of a work, owned, with its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedRecord {
    #[allow(missing_docs)]
    pub source: SourceId,
    #[allow(missing_docs)]
    pub native: OwnedName,
    #[allow(missing_docs)]
    pub promoted: bool,
    #[allow(missing_docs)]
    pub asserts: SmallVec<[OwnedAssert; 3]>,
    #[allow(missing_docs)]
    pub view: Owned,
}

impl OwnedRecord {
    /// The source's own revision stamp.
    #[must_use]
    pub fn revised(&self) -> Option<model::Date> {
        self.view.revised
    }
}

/// An assertion, owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedAssert {
    #[allow(missing_docs)]
    pub name: OwnedName,
    #[allow(missing_docs)]
    pub field: Box<str>,
    #[allow(missing_docs)]
    pub native: bool,
}

/// An accepted name, owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedNamed {
    #[allow(missing_docs)]
    pub name: OwnedName,
    #[allow(missing_docs)]
    pub standing: Standing,
    #[allow(missing_docs)]
    pub by: Sources,
    #[allow(missing_docs)]
    pub native: bool,
    /// For a stub's name, the references that gave it; see
    /// [`Named::cited_by`].
    pub cited_by: SmallVec<[Citer; 1]>,
}

/// A rejected assertion, owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedRejected {
    #[allow(missing_docs)]
    pub name: OwnedName,
    #[allow(missing_docs)]
    pub by: SourceId,
    #[allow(missing_docs)]
    pub holder: OwnedName,
}

/// The promoted fields, owned.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedFields {
    #[allow(missing_docs)]
    pub title: Option<Field<Box<str>>>,
    #[allow(missing_docs)]
    pub abstract_in: Sources,
    #[allow(missing_docs)]
    pub authors: Option<Field<u32>>,
    #[allow(missing_docs)]
    pub venue: Option<Field<OwnedVenue>>,
    #[allow(missing_docs)]
    pub published: Option<Field<Bound>>,
    #[allow(missing_docs)]
    pub language: Option<Field<model::Lang>>,
    #[allow(missing_docs)]
    pub form: Option<Field<Form>>,
    #[allow(missing_docs)]
    pub genre: Option<Field<Genre>>,
    #[allow(missing_docs)]
    pub status: StatusBy,
    #[allow(missing_docs)]
    pub access: Option<Field<bool>>,
    #[allow(missing_docs)]
    pub references: SmallVec<[Other<u32>; 3]>,
    #[allow(missing_docs)]
    pub cited_by: SmallVec<[Other<u32>; 3]>,
}

/// A merged work that owns its text: what the pass holds until the sets
/// are written, and what the works delta carries (serving plan §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedWork {
    #[allow(missing_docs)]
    pub id: WorkId,
    #[allow(missing_docs)]
    pub stub: bool,
    #[allow(missing_docs)]
    pub records: SmallVec<[OwnedRecord; 3]>,
    #[allow(missing_docs)]
    pub names: SmallVec<[OwnedNamed; 4]>,
    #[allow(missing_docs)]
    pub rejected: SmallVec<[OwnedRejected; 1]>,
    #[allow(missing_docs)]
    pub fields: OwnedFields,
    #[allow(missing_docs)]
    pub flags: Flags,
}

impl OwnedWork {
    /// The row, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Work<'_> {
        let f = &self.fields;
        Work {
            id: self.id,
            stub: self.stub,
            records: self
                .records
                .iter()
                .map(|r| Record {
                    source: r.source,
                    native: r.native.as_name(),
                    promoted: r.promoted,
                    asserts: r
                        .asserts
                        .iter()
                        .map(|a| Assert {
                            name: a.name.as_name(),
                            field: &a.field,
                            native: a.native,
                        })
                        .collect(),
                    view: r.view.borrow(),
                })
                .collect(),
            names: self
                .names
                .iter()
                .map(|n| Named {
                    name: n.name.as_name(),
                    standing: n.standing,
                    by: n.by,
                    native: n.native,
                    cited_by: n.cited_by.clone(),
                })
                .collect(),
            rejected: self
                .rejected
                .iter()
                .map(|r| Rejected {
                    name: r.name.as_name(),
                    by: r.by,
                    holder: r.holder.as_name(),
                })
                .collect(),
            fields: Fields {
                title: f.title.as_ref().map(|t| t.map(|s| &**s)),
                abstract_in: f.abstract_in,
                authors: f.authors.clone(),
                venue: f.venue.as_ref().map(|v| v.map(OwnedVenue::borrow)),
                published: f.published.clone(),
                language: f.language.clone(),
                form: f.form.clone(),
                genre: f.genre.clone(),
                status: f.status,
                access: f.access.clone(),
                references: f.references.clone(),
                cited_by: f.cited_by.clone(),
            },
            flags: self.flags,
        }
    }

    /// A copy of a row that owns its text, a venue id given as a symbol
    /// resolved through the set's `table`.
    #[must_use]
    pub fn of(work: &Work<'_>, table: &Table) -> Self {
        let f = &work.fields;
        Self {
            id: work.id,
            stub: work.stub,
            records: work
                .records
                .iter()
                .map(|r| OwnedRecord {
                    source: r.source,
                    native: r.native.to_owned(),
                    promoted: r.promoted,
                    asserts: r
                        .asserts
                        .iter()
                        .map(|a| OwnedAssert {
                            name: a.name.to_owned(),
                            field: a.field.into(),
                            native: a.native,
                        })
                        .collect(),
                    view: Owned::resolve(&r.view, table),
                })
                .collect(),
            names: work
                .names
                .iter()
                .map(|n| OwnedNamed {
                    name: n.name.to_owned(),
                    standing: n.standing,
                    by: n.by,
                    native: n.native,
                    cited_by: n.cited_by.clone(),
                })
                .collect(),
            rejected: work
                .rejected
                .iter()
                .map(|r| OwnedRejected {
                    name: r.name.to_owned(),
                    by: r.by,
                    holder: r.holder.to_owned(),
                })
                .collect(),
            fields: OwnedFields {
                title: f.title.as_ref().map(|t| t.map(|s| Box::from(*s))),
                abstract_in: f.abstract_in,
                authors: f.authors.clone(),
                venue: f.venue.as_ref().map(|v| {
                    v.map(|x| OwnedVenue {
                        openalex: x.openalex,
                        issns: x.issns.clone(),
                        nlm: x.nlm.as_ref().and_then(|n| n.resolve(table)).map(Box::from),
                    })
                }),
                published: f.published.clone(),
                language: f.language.clone(),
                form: f.form.clone(),
                genre: f.genre.clone(),
                status: f.status,
                access: f.access.clone(),
                references: f.references.clone(),
                cited_by: f.cited_by.clone(),
            },
            flags: work.flags,
        }
    }

    /// Every name's references, for a stub; empty lists for a held work.
    #[must_use]
    pub fn citers_by_name(&self) -> BTreeMap<OwnedName, Vec<Citer>> {
        self.names
            .iter()
            .map(|n| (n.name.clone(), n.cited_by.to_vec()))
            .collect()
    }

    /// The names set's rows for this work.
    pub fn name_rows(&self) -> impl Iterator<Item = NameRow<'_>> {
        self.names.iter().map(|n| NameRow {
            name: n.name.as_name(),
            work: self.id,
            standing: n.standing,
            by: n.by,
            native: n.native,
        })
    }
}

/// A citation that owns its text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedCitation {
    #[allow(missing_docs)]
    pub text: Option<Box<str>>,
    #[allow(missing_docs)]
    pub key: Option<Box<str>>,
    #[allow(missing_docs)]
    pub year: Option<Box<str>>,
    #[allow(missing_docs)]
    pub author: Option<Box<str>>,
    #[allow(missing_docs)]
    pub title: Option<Box<str>>,
    #[allow(missing_docs)]
    pub container: Option<Box<str>>,
    #[allow(missing_docs)]
    pub volume: Option<Box<str>>,
    #[allow(missing_docs)]
    pub issue: Option<Box<str>>,
    #[allow(missing_docs)]
    pub first_page: Option<Box<str>>,
}

impl OwnedCitation {
    /// A copy that owns its text; a container given as a symbol is
    /// resolved through `table` when one is given.
    #[must_use]
    pub fn of(c: &Citation<'_>, table: Option<&Table>) -> Self {
        Self {
            text: c.text.map(Box::from),
            key: c.key.map(Box::from),
            year: c.year.map(Box::from),
            author: c.author.map(Box::from),
            title: c.title.map(Box::from),
            container: c
                .container
                .as_ref()
                .and_then(|n| match table {
                    Some(table) => n.resolve(table),
                    None => n.as_text(),
                })
                .map(Box::from),
            volume: c.volume.map(Box::from),
            issue: c.issue.map(Box::from),
            first_page: c.first_page.map(Box::from),
        }
    }

    /// The citation, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Citation<'_> {
        Citation {
            text: self.text.as_deref(),
            key: self.key.as_deref(),
            year: self.year.as_deref(),
            author: self.author.as_deref(),
            title: self.title.as_deref(),
            container: self.container.as_deref().map(Interned::Text),
            volume: self.volume.as_deref(),
            issue: self.issue.as_deref(),
            first_page: self.first_page.as_deref(),
        }
    }
}

/// A name given beside a landing, owning its text ([`CitedAs`]).
// Four flags, each a fact §6 and §11 item 14 name about one row:
// what it was for, how it landed, and whether it can move.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedCitedAs {
    #[allow(missing_docs)]
    pub from: WorkId,
    #[allow(missing_docs)]
    pub by: SourceId,
    #[allow(missing_docs)]
    pub name: OwnedName,
    #[allow(missing_docs)]
    pub to: WorkId,
    #[allow(missing_docs)]
    pub landed: Key,
    #[allow(missing_docs)]
    pub sole: bool,
    #[allow(missing_docs)]
    pub relation: bool,
    #[allow(missing_docs)]
    pub beside: bool,
    #[allow(missing_docs)]
    pub by_text: bool,
    #[allow(missing_docs)]
    pub in_text: bool,
}

impl OwnedCitedAs {
    /// The row as the set holds it.
    #[must_use]
    pub fn borrow(&self) -> CitedAs<'_> {
        CitedAs {
            from: self.from,
            by: self.by,
            name: self.name.as_name(),
            to: self.to,
            landed: self.landed,
            sole: self.sole,
            relation: self.relation,
            beside: self.beside,
            by_text: self.by_text,
            in_text: self.in_text,
        }
    }
}

/// An unresolved reference that owns its text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedUnresolved {
    #[allow(missing_docs)]
    pub from: WorkId,
    #[allow(missing_docs)]
    pub by: SourceId,
    #[allow(missing_docs)]
    pub why: Why,
    #[allow(missing_docs)]
    pub names: SmallVec<[OwnedName; 2]>,
    #[allow(missing_docs)]
    pub citation: OwnedCitation,
}

impl OwnedUnresolved {
    /// A copy of a row that owns its text.
    #[must_use]
    pub fn of(row: &Unresolved<'_>, table: Option<&Table>) -> Self {
        Self {
            from: row.from,
            by: row.by,
            why: row.why,
            names: row.names.iter().map(Name::to_owned).collect(),
            citation: OwnedCitation::of(&row.citation, table),
        }
    }

    /// The row, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Unresolved<'_> {
        Unresolved {
            from: self.from,
            by: self.by,
            why: self.why,
            names: self.names.iter().map(OwnedName::as_name).collect(),
            citation: self.citation.borrow(),
        }
    }
}

/// What the pass leaves: every row of every merged set, sorted as the
/// layout writes them, and the counts the report prints.
#[derive(Debug)]
pub struct Merged {
    /// Held works and stubs, by id.
    pub works: Vec<OwnedWork>,
    /// By citing work, then cited.
    pub edges: Vec<Edge>,
    /// By asserting work.
    pub relations: Vec<Relation>,
    /// By citing work.
    pub unresolved: Vec<OwnedUnresolved>,
    /// By citing work: the names references gave beside the works they
    /// landed on (2026-09-18).
    pub cited_as: Vec<OwnedCitedAs>,
    #[allow(missing_docs)]
    pub stats: Stats,
    /// Per promoted field, by name.
    pub fields: BTreeMap<&'static str, FieldStat>,
    /// Works by promoted form and genre.
    pub kinds: BTreeMap<(Form, Genre), u64>,
    /// Works whose promoted genre cannot live in their form.
    pub kind_conflicts: u64,
    /// Works by status flag and the sources asserting it.
    pub status: BTreeMap<(&'static str, String), u64>,
    /// Distinct edges by the sources asserting them.
    pub edges_by: BTreeMap<Sources, u64>,
    /// Per pair of sources, over edges whose citing work both hold and
    /// whose target is held: asserted by both, by the first alone, by
    /// the second alone.
    pub agreement: BTreeMap<(SourceId, SourceId), [u64; 3]>,
    #[allow(missing_docs)]
    pub self_citations: u64,
    #[allow(missing_docs)]
    pub held: u64,
    #[allow(missing_docs)]
    pub stubs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Free,
    Work(WorkId),
    Shared,
}

/// What a name resolves to through a names table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A work, and whether it is a stub.
    Work(WorkId, bool),
    /// A name accepted into more than one work.
    Shared,
    /// A name no work has.
    Unknown,
}

/// What one reference resolved to (merge plan §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Referenced {
    /// No name and no citation text: nothing at all.
    Empty,
    /// No name; the citation goes to the unresolved stream.
    NoName,
    /// Names, none of them known.
    Unknown,
    /// Only shared names: an unresolved row.
    SharedOnly,
    /// An edge to `to`, and how the reference's names were settled; a
    /// disagreement is an unresolved row too.
    Edge {
        #[allow(missing_docs)]
        to: WorkId,
        #[allow(missing_docs)]
        stub: bool,
        #[allow(missing_docs)]
        settled: Settled,
    },
}

/// How a reference's names settled on one work (merge plan §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settled {
    /// Every name that resolved named the same work.
    Agreed,
    /// The names named different works and the registry's order chose:
    /// PMID, PMC id, OpenAlex id, then DOI.
    ByRank,
    /// The names named different works and the reference's own citation
    /// text repeats one of its DOIs, which wins (2026-09-18). The DOI and
    /// the text are the publisher's deposit travelling together, where a
    /// PMID beside them is PubMed's later match: of the 492 PubMed
    /// references whose PMID and DOI name records we hold with different
    /// DOIs, 449 have a text repeating the reference's DOI and not one
    /// repeats the PMID's.
    ByText,
}

impl Referenced {
    /// The word the report counts it under, `from` being the citing work.
    #[must_use]
    pub const fn outcome(self, from: WorkId) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::NoName => "no name",
            Self::Unknown => "unknown",
            Self::SharedOnly => "shared",
            Self::Edge {
                settled: Settled::ByRank,
                ..
            } => "disagree",
            Self::Edge {
                settled: Settled::ByText,
                ..
            } => "disagree, the text backing the doi",
            Self::Edge { to, stub, .. } => {
                if to.get() == from.get() {
                    "self"
                } else if stub {
                    "stub"
                } else {
                    "held"
                }
            }
        }
    }
}

/// One reference through a names table (merge plan §6): one work named
/// is an edge; only shared names, or none, is an unresolved row; names
/// naming different works are settled by the reference's own citation
/// text where it repeats one of its DOIs, and otherwise follow the
/// registry id — PMID, PMC id, OpenAlex id before DOI — and are kept as
/// an unresolved row either way.
pub fn resolve_reference(
    names: &[Name<'_>],
    citation: &Citation<'_>,
    resolve: impl Fn(Name<'_>) -> Target,
) -> Referenced {
    if names.is_empty() {
        return if citation.is_empty() {
            Referenced::Empty
        } else {
            Referenced::NoName
        };
    }
    let mut resolved: SmallVec<[(Name<'_>, WorkId, bool); 3]> = SmallVec::new();
    let mut shared = false;
    for &name in names {
        match resolve(name) {
            Target::Work(id, stub) => resolved.push((name, id, stub)),
            Target::Shared => shared = true,
            Target::Unknown => {}
        }
    }
    if resolved.is_empty() {
        return if shared {
            Referenced::SharedOnly
        } else {
            Referenced::Unknown
        };
    }
    if resolved.iter().all(|r| r.1 == resolved[0].1) {
        let (_, to, stub) = resolved[0];
        return Referenced::Edge {
            to,
            stub,
            settled: Settled::Agreed,
        };
    }
    // The names disagree. A DOI the reference's own citation text repeats
    // is the publisher's word about what it deposited, and beats the
    // registry's order (§6, 2026-09-18); two corroborated DOIs naming
    // different works settle nothing and leave it to the order.
    if let Some((to, stub)) = corroborated(&resolved, citation.text) {
        return Referenced::Edge {
            to,
            stub,
            settled: Settled::ByText,
        };
    }
    let (_, to, stub) = resolved
        .iter()
        .min_by_key(|(n, _, _)| rank(n.key()))
        .copied()
        .unwrap_or(resolved[0]);
    Referenced::Edge {
        to,
        stub,
        settled: Settled::ByRank,
    }
}

/// Whether a citation text repeats a DOI, case insensitively: the
/// publisher's deposit written out beside the identifier it deposited
/// (§6, 2026-09-18). A DOI is ASCII, so the comparison is too.
#[must_use]
pub fn repeats(text: &str, doi: &str) -> bool {
    let (text, doi) = (text.as_bytes(), doi.as_bytes());
    !doi.is_empty()
        && doi.len() <= text.len()
        && text.windows(doi.len()).any(|w| w.eq_ignore_ascii_case(doi))
}

/// The work of the one DOI among `resolved` that `text` repeats; `None`
/// where there is no text, no DOI in it, or two that name different works.
fn corroborated(
    resolved: &[(Name<'_>, WorkId, bool)],
    text: Option<&str>,
) -> Option<(WorkId, bool)> {
    let text = text?;
    let mut found: Option<(WorkId, bool)> = None;
    for (name, id, stub) in resolved {
        let Name::Doi(doi) = name else {
            continue;
        };
        if !repeats(text, doi) {
            continue;
        }
        match found {
            None => found = Some((*id, *stub)),
            Some((other, _)) if other != *id => return None,
            Some(_) => {}
        }
    }
    found
}

/// A stub name's citers in order, one entry per citing record: the same
/// record's references are one citer, and the text mark is true where
/// any of them repeated the name.
pub fn join_citers(citers: &mut Vec<Citer>) {
    citers.sort_unstable();
    let mut joined: Vec<Citer> = Vec::with_capacity(citers.len());
    for c in citers.drain(..) {
        match joined.last_mut() {
            Some(before) if before.pair() == c.pair() => {
                // The marks of a record's several references together:
                // one that writes the name out corroborates it, and one
                // that named no held work makes it a name of its own.
                before.in_text |= c.in_text;
                before.held_beside &= c.held_beside;
            }
            _ => joined.push(c),
        }
    }
    *citers = joined;
}

/// Where a namespace stands when a reference's names disagree: the
/// registry's own id first, the DOI last (merge plan §6).
#[must_use]
pub const fn rank(key: Key) -> u8 {
    match key {
        Key::Pmid => 0,
        Key::Pmc => 1,
        Key::OpenAlex => 2,
        Key::Doi => 3,
    }
}

/// Every name's work: one live work unless shared (merge plan §8), and
/// one promoted name per namespace per work.
fn names_table(res: &Resolution, ids: &Ids) -> Result<Vec<Slot>, Error> {
    let mut name_work = vec![Slot::Free; res.arena.len()];
    for (w, work) in res.works.iter().enumerate() {
        let id = ids.held[w];
        for key in Key::ALL {
            let promoted = work
                .names
                .iter()
                .filter(|n| n.standing == Standing::Promoted && res.arena.get(n.name).key() == key)
                .count();
            let held = work
                .names
                .iter()
                .filter(|n| n.standing != Standing::Shared && res.arena.get(n.name).key() == key)
                .count();
            if promoted > 1 || (held > 0 && promoted == 0) {
                return Err(Error::Invariant(format!(
                    "{id} has {promoted} promoted {key} names among {held}"
                )));
            }
        }
        for n in &work.names {
            let slot = &mut name_work[n.name as usize];
            *slot = match (n.standing, *slot) {
                (Standing::Shared, Slot::Free | Slot::Shared) => Slot::Shared,
                (Standing::Shared, Slot::Work(_)) | (_, Slot::Shared) => {
                    return Err(Error::Invariant(format!(
                        "{} is both shared and held",
                        res.arena.get(n.name)
                    )));
                }
                (_, Slot::Free) => Slot::Work(id),
                (_, Slot::Work(other)) => {
                    return Err(Error::Invariant(format!(
                        "{} is in {other} and {id}",
                        res.arena.get(n.name)
                    )));
                }
            };
        }
    }
    Ok(name_work)
}

/// Every record's work: exactly one (merge plan §8).
fn records_table(res: &Resolution) -> Result<Vec<usize>, Error> {
    let mut record_work = vec![usize::MAX; res.records.len()];
    for (w, work) in res.works.iter().enumerate() {
        for &rix in &work.records {
            if record_work[rix as usize] != usize::MAX {
                return Err(Error::Invariant(format!(
                    "{}={} is in two works",
                    res.records[rix as usize].source,
                    res.arena.get(res.records[rix as usize].native)
                )));
            }
            record_work[rix as usize] = w;
        }
    }
    if let Some(rix) = record_work.iter().position(|w| *w == usize::MAX) {
        return Err(Error::Invariant(format!(
            "{}={} is in no work",
            res.records[rix].source,
            res.arena.get(res.records[rix].native)
        )));
    }
    Ok(record_work)
}

/// The pass, accumulating over every set of every source.
#[derive(Debug)]
pub struct Builder {
    res: Resolution,
    ids: Ids,
    stub_works: Vec<Vec<OwnedName>>,
    name_work: Vec<Slot>,
    stub_name_work: Vec<Option<WorkId>>,
    record_work: Vec<usize>,
    by_native: HashMap<NameIx, RecordIx>,
    views: Vec<Option<Owned>>,
    edges: Vec<(WorkId, WorkId, SourceId)>,
    relations: Vec<Relation>,
    unresolved: Vec<OwnedUnresolved>,
    /// Names references gave beside the held works they landed on
    /// (2026-09-18).
    cited_as: Vec<OwnedCitedAs>,
    /// Per stub name, the works whose references or relations gave it,
    /// with the source of the record that did.
    stub_citers: HashMap<NameIx, Vec<Citer>>,
    survey: bool,
    /// Counts so far.
    pub stats: Stats,
}

impl Builder {
    /// The names table from a resolution and the ids the registry gave
    /// it, with the invariants that can be checked before reading: every
    /// name in one live work unless shared, every record in exactly one
    /// work, one promoted name per namespace, no id given twice.
    ///
    /// # Errors
    ///
    /// An invariant broken.
    pub fn new(res: Resolution, ids: Ids) -> Result<Self, Error> {
        let stub_works = res.stub_works();
        if ids.held.len() != res.works.len() || ids.stubs.len() != stub_works.len() {
            return Err(Error::Invariant(format!(
                "{} ids for {} works, {} for {} stubs",
                ids.held.len(),
                res.works.len(),
                ids.stubs.len(),
                stub_works.len()
            )));
        }
        let mut seen: HashSet<WorkId> = HashSet::with_capacity(ids.held.len() + ids.stubs.len());
        for id in ids.held.iter().chain(&ids.stubs) {
            if !seen.insert(*id) {
                return Err(Error::Invariant(format!("{id} given to two works")));
            }
        }
        let name_work = names_table(&res, &ids)?;
        let record_work = records_table(&res)?;
        let mut by_native: HashMap<NameIx, RecordIx> = HashMap::with_capacity(res.records.len());
        for (rix, r) in res.records.iter().enumerate() {
            let rix = RecordIx::try_from(rix)
                .map_err(|_| Error::Invariant("more than 2^32 records".to_owned()))?;
            by_native.insert(r.native, rix);
        }
        let mut stub_name_work = vec![None; res.stubs.len()];
        for (s, names) in stub_works.iter().enumerate() {
            for name in names {
                let ix = res.stubs.find(name.as_name()).ok_or_else(|| {
                    Error::Invariant(format!("stub name {name} is not in the stubs"))
                })?;
                stub_name_work[ix as usize] = Some(ids.stubs[s]);
            }
        }
        if stub_name_work.iter().any(Option::is_none) {
            return Err(Error::Invariant("a stub name is in no stub".to_owned()));
        }
        let views = vec![None; res.records.len()];
        Ok(Self {
            res,
            ids,
            stub_works,
            name_work,
            stub_name_work,
            record_work,
            by_native,
            views,
            edges: Vec::new(),
            relations: Vec::new(),
            cited_as: Vec::new(),
            unresolved: Vec::new(),
            stub_citers: HashMap::new(),
            survey: false,
            stats: Stats::default(),
        })
    }
    /// Under `--survey`, a value outside the merge's tables is counted
    /// in [`Stats::unmapped`] instead of stopping the run.
    pub fn set_survey(&mut self, on: bool) {
        self.survey = on;
    }

    /// The resolution the pass is over.
    #[must_use]
    pub fn resolution(&self) -> &Resolution {
        &self.res
    }

    /// The ids the registry gave it.
    #[must_use]
    pub fn ids(&self) -> &Ids {
        &self.ids
    }

    /// The work a name belongs to.
    fn resolve(&self, name: Name<'_>) -> Target {
        if let Some(ix) = self.res.arena.find(name) {
            return match self.name_work[ix as usize] {
                Slot::Work(id) => Target::Work(id, false),
                Slot::Shared => Target::Shared,
                Slot::Free => Target::Unknown,
            };
        }
        if let Some(ix) = self.res.stubs.find(name) {
            return match self.stub_name_work[ix as usize] {
                Some(id) => Target::Work(id, true),
                None => Target::Unknown,
            };
        }
        Target::Unknown
    }

    /// The record with `native` as its native name, and its work's id.
    fn record_of(&self, native: Name<'_>) -> Option<(RecordIx, WorkId)> {
        let ix = self.res.arena.find(native)?;
        let rix = *self.by_native.get(&ix)?;
        Some((rix, self.ids.held[self.record_work[rix as usize]]))
    }

    /// Reads one works set of `S`: every record's view, kept for its
    /// work, and its relations, resolved; at most `limit` records.
    ///
    /// # Errors
    ///
    /// The set cannot be read or has no symbol table, or a value is
    /// outside the merge's tables.
    pub fn works<S: Contributor>(
        &mut self,
        path: &Path,
        set: &str,
        limit: Option<usize>,
        copies: &Copies,
    ) -> Result<u64, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = sets::open::<S>(path, Kind::Works).map_err(io)?;
        let table = reader
            .table()
            .ok_or_else(|| Error::NoTable(path.to_path_buf()))?;
        let mut read = 0u64;
        reader.with_table(|| {
            for (i, work) in reader
                .records::<S::Work<'_>>()
                .take(limit.unwrap_or(usize::MAX))
                .enumerate()
            {
                let work = work.map_err(io)?;
                let ordinal = i as u64 + 1;
                let unmapped = |source| Error::Unmapped {
                    path: path.to_path_buf(),
                    ordinal,
                    source,
                };
                let Some(native) = copies::native_of::<S>(&work) else {
                    continue;
                };
                // A record delivered more than once: the newest copy
                // gives each field it has, and where it has none, the
                // newest copy that does (§8, 2026-09-18). Relations are
                // the speaking copy's alone, since the serving layer
                // does not read a work's relations back and so cannot
                // take their union on an incremental merge.
                let delivery = copies::delivery_of::<S>(&work, set, ordinal);
                let speaks = match copies.fate(S::SOURCE, native, &delivery) {
                    Fate::Withdrawn => continue,
                    Fate::Speaks => true,
                    Fate::Superseded => false,
                };
                let Some((rix, from)) = self.record_of(native) else {
                    continue;
                };
                match S::view(&work, table) {
                    Ok(view) => {
                        let mut owned = Owned::resolve(&view, table);
                        owned.set = set.into();
                        let slot = &mut self.views[rix as usize];
                        *slot = Some(match slot.take() {
                            None => {
                                *self.stats.viewed.entry(S::SOURCE).or_default() += 1;
                                owned
                            }
                            Some(seen) => {
                                *self.stats.filled.entry(S::SOURCE).or_default() += 1;
                                owned.joined(seen)
                            }
                        });
                    }
                    Err(source) if self.survey => {
                        *self
                            .stats
                            .unmapped
                            .entry((S::SOURCE, source.to_string()))
                            .or_default() += 1;
                    }
                    Err(source) => return Err(unmapped(source)),
                }
                if speaks {
                    match S::relations(&work, table, &mut |r| self.relate(from, S::SOURCE, &r)) {
                        Ok(()) => {}
                        Err(source) if self.survey => {
                            *self
                                .stats
                                .unmapped
                                .entry((S::SOURCE, source.to_string()))
                                .or_default() += 1;
                        }
                        Err(source) => return Err(unmapped(source)),
                    }
                }
                read += 1;
            }
            Ok(())
        })?;
        Ok(read)
    }

    /// One record's view given directly, for tests and for callers that
    /// hold records in memory: `native` names the record.
    pub fn view(&mut self, native: Name<'_>, view: &view::View<'_>) -> bool {
        let Some((rix, _)) = self.record_of(native) else {
            return false;
        };
        self.views[rix as usize] = Some(Owned::of(view));
        true
    }

    /// Notes that a reference of `from`'s record of `source` gave a stub
    /// `name`. Relations do not count: they are not arcs, and what the
    /// evidence serves is moving arcs.
    /// Rows for the names references gave beside the works they landed
    /// on, in the order the references were read.
    #[must_use]
    pub fn cited_as_rows(&self) -> &[OwnedCitedAs] {
        &self.cited_as
    }

    fn stub_cited(
        &mut self,
        name: Name<'_>,
        from: WorkId,
        source: SourceId,
        in_text: bool,
        held_beside: bool,
    ) {
        if let Some(ix) = self.res.stubs.find(name) {
            self.stub_citers.entry(ix).or_default().push(Citer {
                from,
                by: source,
                in_text,
                held_beside,
            });
        }
    }

    /// One relation, resolved: an edge in the relations stream when its
    /// target is a held work's or a stub's name.
    pub fn relate(&mut self, from: WorkId, source: SourceId, r: &Related) {
        let what = match r.target.as_ref().map(|t| self.resolve(t.as_name())) {
            None => "not a name",
            Some(Target::Work(to, stub)) => {
                self.relations.push(Relation {
                    from,
                    to,
                    kind: r.kind,
                    by: source,
                });
                *self.stats.relation_kinds.entry(r.kind).or_default() += 1;
                if !stub && let Some(name) = r.target.as_ref() {
                    // A relation names a held work by one of its names:
                    // kept beside the work like a reference's names, so
                    // that a batch knows the name is still named if it
                    // leaves the work (serving plan §11, item 28).
                    self.cited_as.push(OwnedCitedAs {
                        from,
                        by: source,
                        name: name.clone(),
                        to,
                        landed: name.as_name().key(),
                        sole: true,
                        relation: true,
                        beside: false,
                        by_text: false,
                        in_text: false,
                    });
                }
                if stub { "stub" } else { "held" }
            }
            Some(Target::Shared) => "shared",
            Some(Target::Unknown) => "unknown",
        };
        *self.stats.relations.entry((source, what)).or_default() += 1;
    }

    /// Reads one references set of `S`: every reference resolved through
    /// the names table into an edge or an unresolved row; at most
    /// `limit` references.
    ///
    /// # Errors
    ///
    /// The set cannot be read.
    pub fn references<S: Contributor>(
        &mut self,
        path: &Path,
        set: &str,
        limit: Option<usize>,
        copies: &Copies,
    ) -> Result<u64, Error> {
        let io = |source| Error::Io {
            path: path.to_path_buf(),
            source,
        };
        let reader = sets::open::<S>(path, Kind::References).map_err(io)?;
        let mut read = 0u64;
        reader.with_table(|| {
            for record in reader
                .records::<(S::Id<'_>, S::Reference<'_>)>()
                .take(limit.unwrap_or(usize::MAX))
            {
                let (id, reference) = record.map_err(io)?;
                if !copies.alive_in(S::SOURCE, S::native(&id), set) {
                    *self
                        .stats
                        .references
                        .entry((S::SOURCE, "a withdrawn copy"))
                        .or_default() += 1;
                    continue;
                }
                let Some((_, from)) = self.record_of(S::native(&id)) else {
                    *self
                        .stats
                        .references
                        .entry((S::SOURCE, "citing record unknown"))
                        .or_default() += 1;
                    continue;
                };
                let mut names: SmallVec<[Name<'_>; 3]> = SmallVec::new();
                S::targets(&reference, &mut |n| names.push(n));
                self.reference(from, S::SOURCE, &names, &S::citation(&reference));
                read += 1;
            }
            Ok(())
        })?;
        Ok(read)
    }

    /// One reference, resolved (merge plan §6): one work named is an
    /// edge; only shared names, or none, is an unresolved row; names
    /// naming different works follow the registry id — PMID, PMC id,
    /// OpenAlex id before DOI — and keep the rest as an unresolved row
    /// too. Returns what it was.
    pub fn reference(
        &mut self,
        from: WorkId,
        source: SourceId,
        names: &[Name<'_>],
        citation: &Citation<'_>,
    ) -> &'static str {
        let referenced = resolve_reference(names, citation, |n| self.resolve(n));
        match referenced {
            Referenced::Empty | Referenced::Unknown => {}
            Referenced::NoName => self.unresolved(from, source, Why::NoName, names, citation),
            Referenced::SharedOnly => self.unresolved(from, source, Why::Shared, names, citation),
            Referenced::Edge { to, settled, stub } => {
                self.edges.push((from, to, source));
                // Whether the citation text repeats a name: what §6 asks
                // first when names disagree, and so what a later reader
                // needs to ask it again. A stub's names carry it per
                // citer, a held work's per cited-as row.
                let text = citation.text;
                let backs = |name: Name<'_>| match name {
                    Name::Doi(doi) => text.is_some_and(|t| repeats(t, doi)),
                    _ => false,
                };
                if stub {
                    // A reference naming a held work makes no stub name
                    // (§11, item 14), even where §6 lands it on one.
                    let held_beside = names
                        .iter()
                        .any(|n| matches!(self.resolve(*n), Target::Work(_, false)));
                    for &name in names {
                        if self.resolve(name) == Target::Work(to, true) {
                            self.stub_cited(name, from, source, backs(name), held_beside);
                        }
                    }
                } else if let Some(landed) = names
                    .iter()
                    .filter(|n| self.resolve(**n) == Target::Work(to, false))
                    .map(Name::key)
                    .min_by_key(|k| rank(*k))
                {
                    // Every name the reference gave, beside the work it
                    // landed on: the one that landed it, the others the
                    // work holds, and the ones nothing holds. That is
                    // who cited each name, which is what a batch needs
                    // when a name leaves a work — and, for a name nothing
                    // holds, what lets it resolve the reference again if
                    // a later batch gives that name to a work (§11 item
                    // 14; serving plan §11, items 26 and 28).
                    // Whether the citation text backs where it landed:
                    // what says, later, if a name given to another work
                    // takes the arc with it (§6; serving plan §11,
                    // item 26).
                    let by_text = names
                        .iter()
                        .any(|n| backs(*n) && self.resolve(*n) == Target::Work(to, stub));
                    for &name in names {
                        self.cited_as.push(OwnedCitedAs {
                            from,
                            by: source,
                            name: name.to_owned(),
                            to,
                            landed,
                            // Settled in `finish`, where the edges say
                            // how many references made each arc.
                            sole: true,
                            relation: false,
                            beside: self.resolve(name) == Target::Unknown,
                            by_text,
                            in_text: backs(name),
                        });
                    }
                }
                if settled != Settled::Agreed {
                    self.unresolved(from, source, Why::Disagree, names, citation);
                }
            }
        }
        let outcome = referenced.outcome(from);
        *self.stats.references.entry((source, outcome)).or_default() += 1;
        outcome
    }

    fn unresolved(
        &mut self,
        from: WorkId,
        by: SourceId,
        why: Why,
        names: &[Name<'_>],
        citation: &Citation<'_>,
    ) {
        self.unresolved.push(OwnedUnresolved {
            from,
            by,
            why,
            names: names.iter().map(Name::to_owned).collect(),
            citation: OwnedCitation::of(citation, None),
        });
    }

    /// Promotes every work's fields, settles the edges and checks the
    /// invariants of merge plan §8 that need every row.
    ///
    /// # Errors
    ///
    /// An invariant broken.
    #[allow(clippy::too_many_lines)]
    pub fn finish(self) -> Result<Merged, Error> {
        let Self {
            res,
            ids,
            stub_works,
            views,
            edges,
            relations,
            mut unresolved,
            mut cited_as,
            stats,
            mut stub_citers,
            ..
        } = self;
        let mut tallies = Tallies::default();
        let mut works: Vec<OwnedWork> = Vec::with_capacity(res.works.len() + stub_works.len());
        for (w, work) in res.works.iter().enumerate() {
            works.push(held(&res, work, ids.held[w], &views, &mut tallies));
        }
        for (s, names) in stub_works.iter().enumerate() {
            let citers: Vec<Vec<Citer>> = names
                .iter()
                .map(|name| {
                    let mut list = res
                        .stubs
                        .find(name.as_name())
                        .and_then(|ix| stub_citers.remove(&ix))
                        .unwrap_or_default();
                    join_citers(&mut list);
                    list
                })
                .collect();
            works.push(stub(names, &citers, ids.stubs[s]));
        }
        works.sort_by_key(|w| w.id);
        let max = works
            .last()
            .map(|w| w.id.get())
            .ok_or_else(|| Error::Invariant("no works at all".to_owned()))?;
        let mut holds: Vec<Option<Sources>> = vec![None; max as usize + 1];
        for w in &works {
            holds[w.id.get() as usize] = Some(w.records.iter().map(|r| r.source).collect());
        }
        let live = |id: WorkId| holds.get(id.get() as usize).is_some_and(Option::is_some);
        let is_stub = |id: WorkId| holds[id.get() as usize].is_some_and(Sources::is_empty);
        // Edges: one per pair, the sources joined.
        let mut raw = edges;
        raw.sort_unstable();
        // A `cited_as` row's arc moves whole only if one reference made
        // it: how many did is what the raw list says (2026-09-18).
        if !cited_as.is_empty() {
            let mut twice: HashSet<(WorkId, WorkId, SourceId)> = HashSet::new();
            for pair in raw.windows(2) {
                if pair[0] == pair[1] {
                    twice.insert(pair[0]);
                }
            }
            for row in &mut cited_as {
                row.sole = !twice.contains(&(row.from, row.to, row.by));
            }
        }
        let mut edges: Vec<Edge> = Vec::with_capacity(raw.len());
        for (from, to, source) in raw {
            if let Some(last) = edges.last_mut()
                && last.from == from
                && last.to == to
            {
                last.by.insert(source);
                continue;
            }
            edges.push(Edge {
                from,
                to,
                by: [source].into_iter().collect(),
            });
        }
        let mut self_citations = 0u64;
        let mut edges_by: BTreeMap<Sources, u64> = BTreeMap::new();
        let mut agreement: BTreeMap<(SourceId, SourceId), [u64; 3]> = BTreeMap::new();
        let pairs = [
            (SourceId::Pubmed, SourceId::Crossref),
            (SourceId::Pubmed, SourceId::OpenAlex),
            (SourceId::Crossref, SourceId::OpenAlex),
        ];
        for e in &edges {
            if !live(e.from) || !live(e.to) {
                return Err(Error::Invariant(format!(
                    "edge {} -> {} has an end that is no work",
                    e.from, e.to
                )));
            }
            if e.from == e.to {
                self_citations += 1;
            }
            *edges_by.entry(e.by).or_default() += 1;
            let citing = holds[e.from.get() as usize].unwrap_or(Sources::NONE);
            if is_stub(e.to) {
                continue;
            }
            for (a, b) in pairs {
                if citing.contains(a) && citing.contains(b) {
                    let slot = agreement.entry((a, b)).or_default();
                    match (e.by.contains(a), e.by.contains(b)) {
                        (true, true) => slot[0] += 1,
                        (true, false) => slot[1] += 1,
                        (false, true) => slot[2] += 1,
                        (false, false) => {}
                    }
                }
            }
        }
        let mut relations = relations;
        relations.sort_unstable();
        relations.dedup();
        for r in &relations {
            if !live(r.from) || !live(r.to) {
                return Err(Error::Invariant(format!(
                    "relation {} -> {} has an end that is no work",
                    r.from, r.to
                )));
            }
        }
        unresolved.sort_by_key(|u| u.from);
        cited_as.sort_by(|a, b| (a.from, &a.name, a.by).cmp(&(b.from, &b.name, b.by)));
        cited_as.dedup();
        Ok(Merged {
            held: res.works.len() as u64,
            stubs: stub_works.len() as u64,
            works,
            edges,
            relations,
            unresolved,
            cited_as,
            stats,
            fields: tallies.fields,
            kinds: tallies.kinds,
            kind_conflicts: tallies.kind_conflicts,
            status: tallies.status,
            edges_by,
            agreement,
            self_citations,
        })
    }
}

/// What promoting every work counts.
#[derive(Debug, Default)]
pub struct Tallies {
    fields: BTreeMap<&'static str, FieldStat>,
    kinds: BTreeMap<(Form, Genre), u64>,
    kind_conflicts: u64,
    status: BTreeMap<(&'static str, String), u64>,
}

impl Tallies {
    /// Nothing counted yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// One record's view as a candidate for a work's fields.
#[derive(Clone, Copy)]
struct Cand<'v> {
    source: SourceId,
    promoted: bool,
    /// The record's native name, which orders a source's records the same
    /// way in both merges (2026-09-18); the resolution's own record order
    /// is the run's, not the work's.
    native: &'v OwnedName,
    view: &'v Owned,
}

/// The precedence orders of merge plan §5. Genre puts PubMed before
/// OpenAlex since the first report (2026-09-15): where the two differed,
/// PubMed's curated type was the specific one — review, letter,
/// editorial, news, guideline against OpenAlex's `article` — 7,070
/// times, and OpenAlex's 288.
const TITLE: [SourceId; 3] = [SourceId::Pubmed, SourceId::Crossref, SourceId::OpenAlex];
const AUTHORS: [SourceId; 3] = [SourceId::Crossref, SourceId::OpenAlex, SourceId::Pubmed];
const VENUE: [SourceId; 3] = [SourceId::OpenAlex, SourceId::Crossref, SourceId::Pubmed];
const LANGUAGE: [SourceId; 3] = TITLE;
const FORM: [SourceId; 3] = [SourceId::Crossref, SourceId::Pubmed, SourceId::OpenAlex];
const GENRE: [SourceId; 3] = [SourceId::Pubmed, SourceId::OpenAlex, SourceId::Crossref];
const ACCESS: [SourceId; 3] = VENUE;

/// Candidates in a field's precedence: the order's sources first, a
/// source's promoted record before its others, then by native name.
fn ordered<'v>(cands: &[Cand<'v>], order: [SourceId; 3]) -> SmallVec<[Cand<'v>; 3]> {
    let mut out: SmallVec<[Cand<'v>; 3]> = cands.iter().copied().collect();
    let key = |c: &Cand<'v>| {
        (
            order.iter().position(|s| *s == c.source).unwrap_or(3),
            !c.promoted,
        )
    };
    out.sort_by(|a, b| key(a).cmp(&key(b)).then_with(|| a.native.cmp(b.native)));
    out
}

/// One field by precedence (merge plan §5): the first candidate with a
/// value is promoted, and every later one that differs under the rule is
/// kept beside it.
fn promote<'v, T>(
    name: &'static str,
    cands: &[Cand<'v>],
    order: [SourceId; 3],
    get: impl Fn(&'v Owned) -> Option<T>,
    differs: impl Fn(&T, &T) -> bool,
    tallies: &mut Tallies,
) -> Option<Field<T>> {
    let mut values = ordered(cands, order)
        .into_iter()
        .filter_map(|c| get(c.view).map(|v| (c.source, v)));
    let (from, value) = values.next()?;
    let stat = tallies.fields.entry(name).or_default();
    stat.present += 1;
    *stat.from.entry(from).or_default() += 1;
    let mut others: SmallVec<[Other<T>; 2]> = SmallVec::new();
    let mut compared = false;
    for (source, other) in values {
        compared = true;
        let pair = stat.pairs.entry((from, source)).or_default();
        pair.0 += 1;
        if differs(&value, &other) {
            pair.1 += 1;
            others.push(Other {
                source,
                value: other,
            });
        }
    }
    stat.compared += u64::from(compared);
    stat.disputed += u64::from(!others.is_empty());
    Some(Field {
        value,
        from,
        others,
    })
}

/// The publication date (merge plan §5): the earliest any source gives,
/// at its precision; a year that differs is the disagreement.
fn published(cands: &[Cand<'_>], tallies: &mut Tallies) -> Option<Field<Bound>> {
    let dated: SmallVec<[(SourceId, Bound); 3]> = ordered(cands, TITLE)
        .into_iter()
        .filter_map(|c| c.view.published.map(|d| (c.source, d)))
        .collect();
    let (from, value) = dated.iter().copied().reduce(|best, next| {
        if view::earlier(&next.1, &best.1).is_lt() {
            next
        } else {
            best
        }
    })?;
    let stat = tallies.fields.entry("published").or_default();
    stat.present += 1;
    *stat.from.entry(from).or_default() += 1;
    let mut others: SmallVec<[Other<Bound>; 2]> = SmallVec::new();
    let mut compared = false;
    for (source, other) in dated {
        if source == from {
            continue;
        }
        compared = true;
        let pair = stat.pairs.entry((from, source)).or_default();
        pair.0 += 1;
        if other.year != value.year {
            pair.1 += 1;
            others.push(Other {
                source,
                value: other,
            });
        }
    }
    stat.compared += u64::from(compared);
    stat.disputed += u64::from(!others.is_empty());
    Some(Field {
        value,
        from,
        others,
    })
}

fn issns_disjoint(a: &[Issn], b: &[Issn]) -> bool {
    !a.is_empty() && !b.is_empty() && !a.iter().any(|i| b.contains(i))
}

/// One held work's row (merge plan §5): the records with their evidence,
/// the names with their verdicts, the fields promoted from the views,
/// each list in its canonical order so that a row does not depend on
/// the order its records arrived in.
#[allow(clippy::too_many_lines)]
pub fn held(
    res: &Resolution,
    work: &Identified,
    id: WorkId,
    views: &[Option<Owned>],
    tallies: &mut Tallies,
) -> OwnedWork {
    let arena = &res.arena;
    let is_promoted = |rix: RecordIx| {
        let native = res.records[rix as usize].native;
        work.names
            .iter()
            .any(|n| n.name == native && n.standing == Standing::Promoted)
    };
    let mut records: SmallVec<[OwnedRecord; 3]> = work
        .records
        .iter()
        .map(|&rix| {
            let record = &res.records[rix as usize];
            OwnedRecord {
                source: record.source,
                native: arena.get(record.native).clone(),
                promoted: is_promoted(rix),
                asserts: record
                    .asserts
                    .iter()
                    .map(|a| OwnedAssert {
                        name: arena.get(a.name).clone(),
                        field: a.field.into(),
                        native: a.native,
                    })
                    .collect(),
                view: views[rix as usize].clone().unwrap_or_default(),
            }
        })
        .collect();
    records.sort_by(|a, b| (a.source, &a.native).cmp(&(b.source, &b.native)));
    let mut cands: SmallVec<[Cand<'_>; 3]> = work
        .records
        .iter()
        .filter_map(|&rix| {
            views[rix as usize].as_ref().map(|view| Cand {
                source: res.records[rix as usize].source,
                promoted: is_promoted(rix),
                native: res.arena.get(res.records[rix as usize].native),
                view,
            })
        })
        .collect();
    // Canonical, so that the counts below and every tie in `ordered` read
    // the same in both merges: the run's record order is not the work's
    // (2026-09-18).
    cands.sort_by(|a, b| (a.source, !a.promoted, a.native).cmp(&(b.source, !b.promoted, b.native)));
    let title = promote(
        "title",
        &cands,
        TITLE,
        |v| v.title.as_ref().map(|t| (t.clone(), v.titles.clone())),
        |a, b| {
            let agreement = title::agree(&a.1, &b.1);
            !agreement.is_agreement() && agreement != Agreement::Untestable
        },
        tallies,
    )
    .map(|f| f.map(|(t, _)| t.clone()));
    let authors = promote(
        "authors",
        &cands,
        AUTHORS,
        |v| (v.authors > 0).then_some(v.authors),
        |a, b| a != b,
        tallies,
    );
    let venue = promote(
        "venue",
        &cands,
        VENUE,
        |v| (!v.venue.borrow().is_empty()).then(|| v.venue.clone()),
        |a, b| issns_disjoint(&a.issns, &b.issns),
        tallies,
    );
    let published = published(&cands, tallies);
    let language = promote(
        "language",
        &cands,
        LANGUAGE,
        |v| v.language,
        |a, b| a != b,
        tallies,
    );
    let form = promote("form", &cands, FORM, |v| v.form, |a, b| a != b, tallies);
    let genre = promote("genre", &cands, GENRE, |v| v.genre, |a, b| a != b, tallies);
    let access = promote(
        "access",
        &cands,
        ACCESS,
        |v| v.access,
        |a, b| a != b,
        tallies,
    );
    let mut abstract_in = Sources::NONE;
    let mut status = StatusBy::default();
    let mut references: SmallVec<[Other<u32>; 3]> = SmallVec::new();
    let mut cited_by: SmallVec<[Other<u32>; 3]> = SmallVec::new();
    for c in &cands {
        if c.view.has_abstract {
            abstract_in.insert(c.source);
        }
        if c.view.status.retracted {
            status.retracted.insert(c.source);
        }
        if c.view.status.corrected {
            status.corrected.insert(c.source);
        }
        if c.view.status.concern {
            status.concern.insert(c.source);
        }
        if let Some(n) = c.view.references {
            references.push(Other {
                source: c.source,
                value: n,
            });
        }
        if let Some(n) = c.view.cited_by {
            cited_by.push(Other {
                source: c.source,
                value: n,
            });
        }
    }
    if !cands.is_empty() {
        let stat = tallies.fields.entry("abstract").or_default();
        stat.present += u64::from(!abstract_in.is_empty());
        stat.compared += u64::from(cands.len() > 1);
        stat.disputed += u64::from(!abstract_in.is_empty() && abstract_in.len() < cands.len());
    }
    for (flag, by) in [
        ("retracted", status.retracted),
        ("corrected", status.corrected),
        ("concern", status.concern),
    ] {
        if !by.is_empty() {
            *tallies.status.entry((flag, by.to_string())).or_default() += 1;
        }
    }
    let kind_conflict = match (&form, &genre) {
        (Some(f), Some(g)) => {
            *tallies.kinds.entry((f.value, g.value)).or_default() += 1;
            let conflict = !f.value.admits(g.value);
            tallies.kind_conflicts += u64::from(conflict);
            conflict
        }
        _ => false,
    };
    let mut names: SmallVec<[OwnedNamed; 4]> = work
        .names
        .iter()
        .map(|n| OwnedNamed {
            name: arena.get(n.name).clone(),
            standing: n.standing,
            by: n.by,
            native: n.native,
            cited_by: SmallVec::new(),
        })
        .collect();
    names.sort_by(|a, b| a.name.cmp(&b.name));
    let mut rejected: SmallVec<[OwnedRejected; 1]> = work
        .rejected
        .iter()
        .map(|r| OwnedRejected {
            name: arena.get(r.name).clone(),
            by: res.records[r.record as usize].source,
            holder: arena.get(res.records[r.holder as usize].native).clone(),
        })
        .collect();
    rejected.sort_by(|a, b| (&a.name, a.by, &a.holder).cmp(&(&b.name, b.by, &b.holder)));
    OwnedWork {
        id,
        stub: false,
        records,
        flags: Flags {
            duplicates: work.duplicates,
            shared: work.names.iter().any(|n| n.standing == Standing::Shared),
            uncorroborated: work.uncorroborated,
            kind_conflict,
        },
        names,
        rejected,
        fields: OwnedFields {
            title,
            abstract_in,
            authors,
            venue,
            published,
            language,
            form,
            genre,
            status,
            access,
            references,
            cited_by,
        },
    }
}

/// One stub's row: names, one promoted per namespace, nothing else.
/// `names` is sorted, as [`Resolution::stub_works`] gives it, and
/// `citers` is aligned with it: per name, the citing works and sources.
/// A name's sources are its citers' (decided 2026-09-17), so that an
/// incremental merge removing a stale citer has the sources follow.
#[must_use]
pub fn stub(names: &[OwnedName], citers: &[Vec<Citer>], id: WorkId) -> OwnedWork {
    let mut seen: SmallVec<[Key; 4]> = SmallVec::new();
    let rows: SmallVec<[OwnedNamed; 4]> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let key = name.key();
            let standing = if seen.contains(&key) {
                Standing::Alias
            } else {
                seen.push(key);
                Standing::Promoted
            };
            let by: Sources = citers
                .get(i)
                .map_or(Sources::NONE, |c| c.iter().map(|c| c.by).collect());
            OwnedNamed {
                name: name.clone(),
                standing,
                by,
                native: false,
                cited_by: citers
                    .get(i)
                    .map(|c| c.iter().copied().collect())
                    .unwrap_or_default(),
            }
        })
        .collect();
    OwnedWork {
        id,
        stub: true,
        records: SmallVec::new(),
        names: rows,
        rejected: SmallVec::new(),
        fields: OwnedFields::default(),
        flags: Flags::default(),
    }
}

/// A venue's borrowed form is what the disagreement rule reads.
impl OwnedVenue {
    /// The disagreement rule of §5 on owned venues.
    #[must_use]
    pub fn disjoint(&self, other: &Self) -> bool {
        issns_disjoint(&self.issns, &other.issns)
    }
}

#[allow(dead_code)] // the borrowed rule, kept beside the owned one
fn venue_disjoint(a: &Venue<'_>, b: &Venue<'_>) -> bool {
    a.disjoint(b)
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)] // real ids, as the sample spells them
mod tests {
    use model::date::Bound;
    use model::{Day, Issn, Lang, Month, OpenAlexId, Pmid, Year};

    use super::{Builder, Ids};
    use crate::identity::{Identity, Standing};
    use crate::kind::{Form, Genre};
    use crate::name::{Assertion, Name};
    use crate::record::Citer;
    use crate::record::{Citation, Why};
    use crate::registry::Registry;
    use crate::relation::{Kind, Related};
    use crate::title::Titles;
    use crate::view::{Status, Venue, View};
    use crate::work::{SourceId, Sources, WorkId};

    fn pmid(n: u32) -> Pmid {
        Pmid::new(n).unwrap()
    }

    fn w(n: u64) -> OpenAlexId {
        OpenAlexId::from_bare(&format!("W{n}")).unwrap()
    }

    fn bound(y: u16, m: Option<u8>, d: Option<u8>) -> Bound {
        Bound {
            year: Year::new(y).unwrap(),
            month: m.map(|m| Month::from_number(m).unwrap()),
            day: d.map(|d| Day::new(d).unwrap()),
        }
    }

    /// Three records of one work, one of another, references between
    /// them and to a stub.
    fn scenario() -> (Builder, [WorkId; 3]) {
        let mut id = Identity::new();
        let t = |s: &'static str| Titles {
            title: Some(s),
            vernacular: None,
        };
        id.push(
            SourceId::Pubmed,
            &[
                Assertion {
                    name: Name::Pmid(pmid(1)),
                    field: "pmid",
                    native: true,
                },
                Assertion {
                    name: Name::Doi("10.1000/a"),
                    field: "ids.doi",
                    native: false,
                },
            ],
            t("Alpha"),
        )
        .unwrap();
        id.push(
            SourceId::Crossref,
            &[Assertion {
                name: Name::Doi("10.1000/a"),
                field: "doi",
                native: true,
            }],
            t("Alpha"),
        )
        .unwrap();
        id.push(
            SourceId::OpenAlex,
            &[
                Assertion {
                    name: Name::OpenAlex(w(1)),
                    field: "id",
                    native: true,
                },
                Assertion {
                    name: Name::Doi("10.1000/a"),
                    field: "doi",
                    native: false,
                },
            ],
            t("Alpha"),
        )
        .unwrap();
        id.push(
            SourceId::Crossref,
            &[Assertion {
                name: Name::Doi("10.1000/b"),
                field: "doi",
                native: true,
            }],
            t("Beta"),
        )
        .unwrap();
        id.mark();
        assert_eq!(
            id.cite(SourceId::Crossref, &[Name::Doi("10.1000/stub")]),
            "stub"
        );
        let res = id.resolve(false).unwrap();
        assert_eq!(res.works.len(), 2);
        let mut registry = Registry::default();
        let order = res.held_order();
        let all: Vec<_> = order
            .iter()
            .map(|(_, n)| n.clone())
            .chain(res.stub_works())
            .collect();
        let assignment = registry.assign(&all, "t", false);
        let (held_ids, stub_ids) = assignment.ids.split_at(order.len());
        let mut ids = Ids {
            held: vec![WorkId::FIRST; res.works.len()],
            stubs: stub_ids.to_vec(),
        };
        for ((w, _), id) in order.iter().zip(held_ids) {
            ids.held[*w] = *id;
        }
        let alpha = ids.held[res.works.iter().position(|w| w.records.len() == 3).unwrap()];
        let beta = ids.held[res.works.iter().position(|w| w.records.len() == 1).unwrap()];
        let stub = ids.stubs[0];
        (Builder::new(res, ids).unwrap(), [alpha, beta, stub])
    }

    #[test]
    #[allow(clippy::too_many_lines)] // one scenario, every rule
    fn fields_are_promoted_by_precedence_with_disagreements_kept() {
        let (mut b, [alpha, beta, stub]) = scenario();
        let base = View {
            titles: Titles {
                title: Some("Alpha"),
                vernacular: None,
            },
            ..View::default()
        };
        assert!(b.view(
            Name::Pmid(pmid(1)),
            &View {
                has_abstract: true,
                authors: 3,
                published: Some(bound(2014, Some(11), None)),
                language: Lang::from_bibliographic("fre").ok(),
                form: Some(Form::JournalArticle),
                genre: Some(Genre::Review),
                status: Status {
                    retracted: true,
                    ..Status::default()
                },
                venue: Venue {
                    issns: [Issn::parse("0960-1481").unwrap()].into_iter().collect(),
                    ..Venue::default()
                },
                ..base.clone()
            }
        ));
        assert!(b.view(
            Name::Doi("10.1000/a"),
            &View {
                titles: Titles {
                    title: Some("Alpha: a subtitle nobody else has"),
                    vernacular: None,
                },
                authors: 4,
                published: Some(bound(2014, Some(6), Some(15))),
                language: Lang::from_bibliographic("eng").ok(),
                form: Some(Form::JournalArticle),
                genre: Some(Genre::Article),
                venue: Venue {
                    issns: [Issn::parse("1879-0682").unwrap()].into_iter().collect(),
                    ..Venue::default()
                },
                access: Some(true),
                references: Some(12),
                ..base.clone()
            }
        ));
        assert!(b.view(
            Name::OpenAlex(w(1)),
            &View {
                authors: 4,
                published: Some(bound(2015, None, None)),
                form: Some(Form::PostedContent),
                genre: Some(Genre::Preprint),
                access: Some(false),
                cited_by: Some(7),
                ..base.clone()
            }
        ));
        assert!(!b.view(Name::Doi("10.1000/nobody"), &base));
        // Edges: alpha cites beta by two sources, beta cites the stub, a
        // reference with no name, one through disagreeing names.
        assert_eq!(
            b.reference(
                alpha,
                SourceId::Crossref,
                &[Name::Doi("10.1000/b")],
                &Citation::default()
            ),
            "held"
        );
        assert_eq!(
            b.reference(
                alpha,
                SourceId::Pubmed,
                &[Name::Doi("10.1000/b")],
                &Citation::default()
            ),
            "held"
        );
        assert_eq!(
            b.reference(
                beta,
                SourceId::Crossref,
                &[Name::Doi("10.1000/stub")],
                &Citation::default()
            ),
            "stub"
        );
        assert_eq!(
            b.reference(
                beta,
                SourceId::Crossref,
                &[],
                &Citation {
                    year: Some("1999"),
                    ..Citation::default()
                }
            ),
            "no name"
        );
        assert_eq!(
            b.reference(beta, SourceId::Crossref, &[], &Citation::default()),
            "empty"
        );
        assert_eq!(
            b.reference(
                beta,
                SourceId::Pubmed,
                &[Name::Pmid(pmid(1)), Name::Doi("10.1000/b")],
                &Citation::default()
            ),
            "disagree",
            "the PMID wins: an edge to alpha"
        );
        // The same shape of disagreement, with the reference's own
        // citation text repeating its DOI: the publisher's word wins
        // over the order (§6, 2026-09-18), and the case of the text does
        // not matter.
        assert_eq!(
            b.reference(
                beta,
                SourceId::Crossref,
                &[Name::Pmid(pmid(1)), Name::Doi("10.1000/stub")],
                &Citation {
                    text: Some("Stu B. 1999. https://doi.org/10.1000/STUB"),
                    ..Citation::default()
                }
            ),
            "disagree, the text backing the doi",
            "the DOI wins: an edge to the stub, not to the PMID's work"
        );
        // A text that repeats no DOI of its own leaves the order to it.
        assert_eq!(
            b.reference(
                beta,
                SourceId::Pubmed,
                &[Name::Pmid(pmid(1)), Name::Doi("10.1000/stub")],
                &Citation {
                    text: Some("Stu B. 1999."),
                    ..Citation::default()
                }
            ),
            "disagree",
            "no corroboration: the PMID wins again"
        );
        assert_eq!(
            b.reference(
                alpha,
                SourceId::OpenAlex,
                &[Name::OpenAlex(w(1))],
                &Citation::default()
            ),
            "self"
        );
        b.relate(
            alpha,
            SourceId::Crossref,
            &Related {
                kind: Kind::HasPreprint,
                target: Some(Name::Doi("10.1000/stub").to_owned()),
            },
        );
        b.relate(
            alpha,
            SourceId::Crossref,
            &Related {
                kind: Kind::IsSupplementedBy,
                target: None,
            },
        );
        let merged = b.finish().unwrap();
        assert_eq!(merged.works.len(), 3);
        assert_eq!((merged.held, merged.stubs), (2, 1));
        let a = merged.works.iter().find(|w| w.id == alpha).unwrap();
        let f = &a.fields;
        let title = f.title.as_ref().unwrap();
        assert_eq!((&*title.value, title.from), ("Alpha", SourceId::Pubmed));
        assert_eq!(title.others.len(), 1, "Crossref's subtitle differs");
        assert_eq!(title.others[0].source, SourceId::Crossref);
        let authors = f.authors.as_ref().unwrap();
        assert_eq!((authors.value, authors.from), (4, SourceId::Crossref));
        assert_eq!(
            authors
                .others
                .iter()
                .map(|o| (o.source, o.value))
                .collect::<Vec<_>>(),
            [(SourceId::Pubmed, 3)]
        );
        let published = f.published.as_ref().unwrap();
        assert_eq!(
            (published.value, published.from),
            (bound(2014, Some(6), Some(15)), SourceId::Crossref),
            "the earliest date"
        );
        assert_eq!(published.others.len(), 1, "OpenAlex's 2015 differs by year");
        let language = f.language.as_ref().unwrap();
        assert_eq!(language.from, SourceId::Pubmed);
        assert_eq!(language.others.len(), 1);
        assert_eq!(f.form.as_ref().unwrap().value, Form::JournalArticle);
        assert_eq!(
            f.form.as_ref().unwrap().others[0].value,
            Form::PostedContent
        );
        assert_eq!(
            f.genre.as_ref().unwrap().value,
            Genre::Review,
            "PubMed first"
        );
        assert_eq!(f.genre.as_ref().unwrap().others.len(), 2);
        assert!(!a.flags.kind_conflict, "a review in a journal article");
        assert_eq!(f.venue.as_ref().unwrap().from, SourceId::Crossref);
        assert_eq!(f.venue.as_ref().unwrap().others.len(), 1, "disjoint ISSNs");
        assert!(!f.access.as_ref().unwrap().value, "OpenAlex first");
        assert_eq!(f.access.as_ref().unwrap().others.len(), 1);
        assert_eq!(
            f.abstract_in,
            [SourceId::Pubmed].into_iter().collect::<Sources>()
        );
        assert_eq!(
            f.status.retracted,
            [SourceId::Pubmed].into_iter().collect::<Sources>()
        );
        assert_eq!(f.references[0].value, 12);
        assert_eq!(f.cited_by[0].value, 7);
        assert!(a.records.iter().all(|r| r.promoted));
        assert_eq!(a.names.len(), 3);
        let b_work = merged.works.iter().find(|w| w.id == beta).unwrap();
        assert!(b_work.fields.title.is_none(), "no view was given");
        let s = merged.works.iter().find(|w| w.id == stub).unwrap();
        assert!(s.stub && s.records.is_empty());
        assert_eq!(s.names[0].standing, Standing::Promoted);
        assert!(s.names[0].by.contains(SourceId::Crossref));
        assert_eq!(
            s.names[0].cited_by.to_vec(),
            vec![Citer {
                from: beta,
                by: SourceId::Crossref,
                in_text: true,
                held_beside: false,
            }],
            "beta's two references are one citer, and one writes the DOI out"
        );
        // Edges.
        let edges: Vec<_> = merged
            .edges
            .iter()
            .map(|e| (e.from, e.to, e.by.to_string()))
            .collect();
        assert_eq!(
            edges,
            [
                (alpha, alpha, "openalex".to_owned()),
                (alpha, beta, "pubmed+crossref".to_owned()),
                (beta, alpha, "pubmed".to_owned()),
                (beta, stub, "crossref".to_owned()),
            ]
        );
        assert_eq!(merged.self_citations, 1);
        // Four unresolved rows: the reference with no name, and the
        // three disagreements — a disagreement the citation text settles
        // is still kept as one (§6, 2026-09-18).
        assert_eq!(merged.unresolved.len(), 4);
        assert_eq!(merged.unresolved[0].why, Why::NoName);
        assert_eq!(merged.unresolved[0].citation.year.as_deref(), Some("1999"));
        assert!(
            merged.unresolved[1..]
                .iter()
                .all(|u| u.why == Why::Disagree && u.names.len() == 2)
        );
        assert_eq!(merged.relations.len(), 1);
        assert_eq!(merged.relations[0].kind, Kind::HasPreprint);
        assert_eq!(
            merged.stats.relations[&(SourceId::Crossref, "not a name")],
            1
        );
        assert_eq!(merged.fields["title"].disputed, 1);
        assert_eq!(merged.fields["title"].compared, 1);
        assert_eq!(merged.fields["published"].present, 1);
        assert_eq!(merged.kind_conflicts, 0);
        assert_eq!(merged.kinds[&(Form::JournalArticle, Genre::Review)], 1);
        assert_eq!(merged.status[&("retracted", "pubmed".to_owned())], 1);
        assert_eq!(
            merged.agreement[&(SourceId::Pubmed, SourceId::Crossref)],
            [1, 0, 0],
            "alpha holds both; alpha -> beta by both"
        );
        let json = serde_json::to_string(&a.borrow()).unwrap();
        assert!(json.contains("\"kind_conflict\":false"));
        assert!(json.contains("\"genre\":{\"value\":\"review\""));
    }

    #[test]
    fn ids_must_match_the_resolution() {
        let (b, _) = scenario();
        let res = b.res;
        let bad = Ids {
            held: vec![WorkId::FIRST],
            stubs: vec![],
        };
        assert!(Builder::new(res, bad).is_err());
    }
}
