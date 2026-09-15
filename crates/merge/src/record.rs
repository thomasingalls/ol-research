//! The rows of the merged sets (merge plan §5, §6, §8): the merged record
//! of a work, an edge, a name, a relation, an unresolved reference. Each
//! borrows its text from the set it was read from and serializes in the
//! normalized layout's framing; `merge dump` prints them as JSON.

use model::date::Bound;
use model::symbol::Interned;
use model::{Date, Lang};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::identity::Standing;
use crate::kind::{Form, Genre};
use crate::name::Name;
use crate::relation;
use crate::view::Venue;
use crate::work::{SourceId, Sources, WorkId};

/// One work: a summary and an index over the normalized layer, not a
/// superset of it (§5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Work<'a> {
    #[allow(missing_docs)]
    pub id: WorkId,
    /// Names and no records: something we hold cites it (§2).
    pub stub: bool,
    /// The source records the work is made of.
    #[serde(borrow)]
    pub records: SmallVec<[Record<'a>; 3]>,
    /// Every accepted name with its standing.
    #[serde(borrow)]
    pub names: SmallVec<[Named<'a>; 4]>,
    /// Every rejected assertion.
    #[serde(borrow)]
    pub rejected: SmallVec<[Rejected<'a>; 1]>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub fields: Fields<'a>,
    #[allow(missing_docs)]
    pub flags: Flags,
}

/// One source record of a work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record<'a> {
    #[allow(missing_docs)]
    pub source: SourceId,
    /// The source's own id for it, by which the normalized layer is
    /// reached.
    #[serde(borrow)]
    pub native: Name<'a>,
    /// The source's own revision stamp.
    pub revised: Option<Date>,
    /// The promoted record of its source: the one whose native id is the
    /// promoted name of its namespace, which matters only when a source
    /// holds two records of one work.
    pub promoted: bool,
}

/// An accepted name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named<'a> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Name<'a>,
    #[allow(missing_docs)]
    pub standing: Standing,
    /// Who asserted it; for a stub's name, who cited it.
    pub by: Sources,
    /// Whether a record of the work holds it natively.
    pub native: bool,
}

/// A rejected assertion: `by`'s record claimed `name`, which `holder`'s
/// work holds natively, and their titles disagree — a wrong link (§3.3),
/// the one reason there is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected<'a> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Name<'a>,
    #[allow(missing_docs)]
    pub by: SourceId,
    /// The native name of the record that holds it.
    #[serde(borrow)]
    pub holder: Name<'a>,
}

/// The flags of §5.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // the plan's flags, one each
pub struct Flags {
    /// Sources holding more than one record of the work.
    pub duplicates: Sources,
    /// One of its names is shared with another work.
    pub shared: bool,
    /// Joined on an untestable title (§3.3).
    pub uncorroborated: bool,
    /// The promoted genre cannot live in the promoted form (§5).
    pub kind_conflict: bool,
}

/// A promoted value, who it came from, and the other sources' values
/// where they differ under the field's rule — empty when consistent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field<T> {
    #[allow(missing_docs)]
    pub value: T,
    #[allow(missing_docs)]
    pub from: SourceId,
    #[allow(missing_docs)]
    pub others: SmallVec<[Other<T>; 2]>,
}

/// Another source's value for a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Other<T> {
    #[allow(missing_docs)]
    pub source: SourceId,
    #[allow(missing_docs)]
    pub value: T,
}

impl<T> Field<T> {
    /// The same field with every value mapped, borrowing from this one.
    pub fn map<'a, U>(&'a self, f: impl Fn(&'a T) -> U) -> Field<U> {
        Field {
            value: f(&self.value),
            from: self.from,
            others: self
                .others
                .iter()
                .map(|o| Other {
                    source: o.source,
                    value: f(&o.value),
                })
                .collect(),
        }
    }

    /// Whether any other source differs.
    #[must_use]
    pub fn disputed(&self) -> bool {
        !self.others.is_empty()
    }
}

/// Status by who asserts it: the union is the work's status, and the
/// holders missing from a set are the disagreement (§5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusBy {
    #[allow(missing_docs)]
    pub retracted: Sources,
    #[allow(missing_docs)]
    pub corrected: Sources,
    #[allow(missing_docs)]
    pub concern: Sources,
}

/// The promoted fields of §5 and their disagreements.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fields<'a> {
    /// Raw markup and all; others where the §3.3 comparison fails.
    #[serde(borrow)]
    pub title: Option<Field<&'a str>>,
    /// Who has an abstract: presence only.
    pub abstract_in: Sources,
    /// The author count; others where it differs.
    pub authors: Option<Field<u32>>,
    /// Others where the ISSN sets are disjoint.
    #[serde(borrow)]
    pub venue: Option<Field<Venue<'a>>>,
    /// The earliest date any source gives; others where the year differs.
    pub published: Option<Field<Bound>>,
    #[allow(missing_docs)]
    pub language: Option<Field<Lang>>,
    #[allow(missing_docs)]
    pub form: Option<Field<Form>>,
    #[allow(missing_docs)]
    pub genre: Option<Field<Genre>>,
    #[allow(missing_docs)]
    pub status: StatusBy,
    /// Open access; others where the flag differs.
    pub access: Option<Field<bool>>,
    /// Reference counts, per source, never promoted.
    pub references: SmallVec<[Other<u32>; 3]>,
    /// Citation counts, per source, never promoted.
    pub cited_by: SmallVec<[Other<u32>; 3]>,
}

/// One edge: `from` cites `to`, said by `by`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Edge {
    #[allow(missing_docs)]
    pub from: WorkId,
    #[allow(missing_docs)]
    pub to: WorkId,
    #[allow(missing_docs)]
    pub by: Sources,
}

/// One row of the names set: a name and the work it belongs to. A shared
/// name has one row per work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameRow<'a> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Name<'a>,
    #[allow(missing_docs)]
    pub work: WorkId,
    #[allow(missing_docs)]
    pub standing: Standing,
    #[allow(missing_docs)]
    pub by: Sources,
    #[allow(missing_docs)]
    pub native: bool,
}

/// One typed relation (§6): `from`'s record says `kind` of `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Relation {
    #[allow(missing_docs)]
    pub from: WorkId,
    #[allow(missing_docs)]
    pub to: WorkId,
    #[allow(missing_docs)]
    pub kind: relation::Kind,
    #[allow(missing_docs)]
    pub by: SourceId,
}

/// Why a reference resolved to no edge (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// The reference carries no name, only text or fields.
    NoName,
    /// Every name it carries is shared between works.
    Shared,
    /// Its names resolve to different works; the edge followed the
    /// registry id (§6) and this row keeps the rest.
    Disagree,
}

impl Why {
    /// The spelling in reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoName => "no name",
            Self::Shared => "shared",
            Self::Disagree => "disagree",
        }
    }
}

/// What a source gives for a reference besides names, for the matching
/// stage: Crossref's structured fields or unstructured string, PubMed's
/// citation text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation<'a> {
    /// The unstructured citation, raw markup and all.
    #[serde(borrow)]
    pub text: Option<&'a str>,
    /// The citing record's own key for the reference.
    #[serde(borrow)]
    pub key: Option<&'a str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub year: Option<&'a str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub author: Option<&'a str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub title: Option<&'a str>,
    /// The journal, series or volume title.
    #[serde(borrow)]
    pub container: Option<Interned<'a>>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume: Option<&'a str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issue: Option<&'a str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub first_page: Option<&'a str>,
}

impl Citation<'_> {
    /// Nothing at all: the reference is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.year.is_none()
            && self.author.is_none()
            && self.title.is_none()
            && self.container.is_none()
            && self.volume.is_none()
            && self.issue.is_none()
            && self.first_page.is_none()
    }
}

/// One reference that made no edge, keyed by the citing work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unresolved<'a> {
    #[allow(missing_docs)]
    pub from: WorkId,
    #[allow(missing_docs)]
    pub by: SourceId,
    #[allow(missing_docs)]
    pub why: Why,
    /// The names it carried, if any.
    #[serde(borrow)]
    pub names: SmallVec<[Name<'a>; 2]>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub citation: Citation<'a>,
}

#[cfg(test)]
mod tests {
    use model::symbol::{Interned, begin_writing, end_writing, with_reading};
    use model::{Date, Pmid};
    use smallvec::SmallVec;

    use super::{Citation, Edge, Field, Fields, Flags, Named, Record, Unresolved, Why, Work};
    use crate::identity::Standing;
    use crate::kind::{Form, Genre};
    use crate::name::Name;
    use crate::work::{SourceId, Sources, WorkId};

    fn sample<'a>() -> Work<'a> {
        Work {
            id: WorkId::new(7).unwrap(),
            stub: false,
            records: [Record {
                source: SourceId::Pubmed,
                native: Name::Pmid(Pmid::new(123).unwrap()),
                revised: Some(Date::parse_iso("2026-01-23").unwrap()),
                promoted: true,
            }]
            .into_iter()
            .collect(),
            names: [Named {
                name: Name::Doi("10.1000/x"),
                standing: Standing::Promoted,
                by: [SourceId::Pubmed].into_iter().collect(),
                native: false,
            }]
            .into_iter()
            .collect(),
            rejected: SmallVec::default(),
            fields: Fields {
                title: Some(Field {
                    value: "A <i>title</i>",
                    from: SourceId::Pubmed,
                    others: SmallVec::default(),
                }),
                form: Some(Field {
                    value: Form::JournalArticle,
                    from: SourceId::Crossref,
                    others: SmallVec::default(),
                }),
                genre: Some(Field {
                    value: Genre::Review,
                    from: SourceId::OpenAlex,
                    others: SmallVec::default(),
                }),
                ..Fields::default()
            },
            flags: Flags::default(),
        }
    }

    #[test]
    fn a_work_row_survives_postcard_and_prints_as_json() {
        begin_writing().unwrap();
        let bytes = postcard::to_allocvec(&sample()).unwrap();
        let table = end_writing().unwrap();
        // Declared after `bytes`: a work's lists are invariant in their
        // lifetime, so the one compared with `back` must drop first.
        let work = sample();
        let back: Work<'_> = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back, work);
        let json = with_reading(&table, || serde_json::to_string(&back).unwrap());
        assert!(json.contains("\"doi:10.1000/x\""));
        assert!(json.contains("\"pmid:123\""));
        assert!(json.contains("\"journal-article\""));
        assert!(json.contains("\"promoted\""));
        let edge = Edge {
            from: WorkId::FIRST,
            to: WorkId::new(2).unwrap(),
            by: Sources::NONE,
        };
        assert_eq!(
            serde_json::to_string(&edge).unwrap(),
            "{\"from\":1,\"to\":2,\"by\":0}"
        );
    }

    #[test]
    fn an_unresolved_row_carries_its_citation() {
        let row = Unresolved {
            from: WorkId::FIRST,
            by: SourceId::Crossref,
            why: Why::NoName,
            names: SmallVec::default(),
            citation: Citation {
                key: Some("ref1"),
                year: Some("2019"),
                container: Some(Interned::Text("Nature")),
                ..Citation::default()
            },
        };
        assert!(!row.citation.is_empty());
        assert!(Citation::default().is_empty());
        begin_writing().unwrap();
        let bytes = postcard::to_allocvec(&row).unwrap();
        let table = end_writing().unwrap();
        assert_eq!(table.len(), 1, "the container is interned");
        let back: Unresolved<'_> = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back.citation.container, Some(Interned::Symbol(0)));
        assert_eq!(back.why, Why::NoName);
        assert_eq!(Why::Disagree.name(), "disagree");
    }
}
