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
use crate::view::{Venue, View};
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

/// One source record of a work, with the evidence the rules read from
/// it: its assertions (§3.1) and its view (§5). The verdicts — the names'
/// standings, the promoted fields — are a function of every record's
/// evidence, which is what lets an incremental merge (serving plan §5)
/// re-run the rules on one work without the normalized layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record<'a> {
    #[allow(missing_docs)]
    pub source: SourceId,
    /// The source's own id for it, by which the normalized layer is
    /// reached.
    #[serde(borrow)]
    pub native: Name<'a>,
    /// The promoted record of its source: the one whose native id is the
    /// promoted name of its namespace, which matters only when a source
    /// holds two records of one work.
    pub promoted: bool,
    /// Every assertion, the native one included, each name once.
    #[serde(borrow)]
    pub asserts: SmallVec<[Assert<'a>; 3]>,
    /// The record's values for the fields of §5, its revision stamp and
    /// its titles among them.
    #[serde(borrow)]
    pub view: View<'a>,
}

impl Record<'_> {
    /// The source's own revision stamp.
    #[must_use]
    pub fn revised(&self) -> Option<Date> {
        self.view.revised
    }
}

/// One assertion of a record (§3.1): a name, the normalized field it came
/// from, and whether it is the record's native id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assert<'a> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Name<'a>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub field: &'a str,
    #[allow(missing_docs)]
    pub native: bool,
}

/// One reference that gave a stub's name: the work whose record cited
/// it, that record's source, and whether the reference's own citation
/// text repeats the name. The text is what settles the reference again
/// when a record arrives for some of the stub's names and the rest
/// become a stub of their own, since the names then name two works and
/// §6 asks the text first (serving plan §11, item 28, 2026-09-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Citer {
    /// The citing work.
    pub from: WorkId,
    /// The source of the record that cited it.
    pub by: SourceId,
    /// The citation text repeats this name. Only a DOI is ever repeated
    /// (§6), so this is false for every other namespace.
    pub in_text: bool,
    /// The reference named a held work too, so it makes no name of its
    /// own: its other names are assertions about that work, not a stub
    /// (§11, item 14). §6 can still land it here, over a held work the
    /// order would prefer, which is why the citer is kept at all. False
    /// where a record's other reference to the same stub named none.
    pub held_beside: bool,
}

impl Citer {
    /// The citing work and source, which say which record it was.
    #[must_use]
    pub const fn pair(self) -> (WorkId, SourceId) {
        (self.from, self.by)
    }
}

/// An accepted name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named<'a> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Name<'a>,
    #[allow(missing_docs)]
    pub standing: Standing,
    /// Who asserted it; for a stub's name, the sources of its citers.
    pub by: Sources,
    /// Whether a record of the work holds it natively.
    pub native: bool,
    /// For a stub's name: the references that gave it — the evidence an
    /// incremental merge needs when a record arrives for some of a
    /// stub's names and not others (serving plan §5). One entry per
    /// citing record. Empty on a held work's names, which records
    /// assert; relations are not counted, since they are not arcs.
    pub cited_by: SmallVec<[Citer; 1]>,
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

/// One name a reference gave beside the work it landed on, keyed by the
/// citing work: the reference named this id too and nothing held it
/// (merge plan §11, item 14). It is the evidence an incremental merge
/// needs to resolve that reference again when a later batch gives the id
/// to a work or a stub (serving plan §11, item 26), which is why it is
/// kept rather than counted, since 2026-09-18.
// Four flags, each a fact §6 and §11 item 14 name about one row:
// what it was for, how it landed, and whether it can move.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitedAs<'a> {
    /// The citing work.
    pub from: WorkId,
    /// The source whose record cited it.
    pub by: SourceId,
    /// The name nothing held.
    #[serde(borrow)]
    pub name: Name<'a>,
    /// The work the reference landed on.
    pub to: WorkId,
    /// The namespace of the name that landed it there: a reference whose
    /// names disagree follows PMID, PMC, OpenAlex, then DOI (§6), so this
    /// says whether the arc would move if `name` became a work's.
    pub landed: ingest::Key,
    /// Whether this was the citing work's only reference of that source
    /// landing on `to`: if the arc moves, it moves whole; if not, the arc
    /// stays and only the new one is added.
    pub sole: bool,
    /// A relation named it, not a reference (§6): evidence that the name
    /// is still named, and so still a work of its own if it leaves this
    /// one, but no arc and no citer — a relation makes neither.
    pub relation: bool,
    /// Whether the name named no work when the reference was read: only
    /// then can a batch giving it to one move this arc (serving plan §11,
    /// item 26). A name the work already held is evidence of who cited
    /// it (item 28) and moves nothing.
    pub beside: bool,
    /// The reference's own citation text repeats a DOI of the work it
    /// landed on (§6, 2026-09-18): the landing is the publisher's word,
    /// which the order alone does not overturn.
    pub by_text: bool,
    /// The citation text repeats *this* name, which is how a DOI a batch
    /// gives to another work takes the arc with it although the order
    /// puts a DOI last.
    pub in_text: bool,
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

    use super::{
        Assert, Citation, Edge, Field, Fields, Flags, Named, Record, Unresolved, Why, Work,
    };
    use crate::identity::Standing;
    use crate::kind::{Form, Genre};
    use crate::name::Name;
    use crate::view::View;
    use crate::work::{SourceId, Sources, WorkId};

    fn sample<'a>() -> Work<'a> {
        Work {
            id: WorkId::new(7).unwrap(),
            stub: false,
            records: [Record {
                source: SourceId::Pubmed,
                native: Name::Pmid(Pmid::new(123).unwrap()),
                promoted: true,
                asserts: [Assert {
                    name: Name::Pmid(Pmid::new(123).unwrap()),
                    field: "pmid",
                    native: true,
                }]
                .into_iter()
                .collect(),
                view: View {
                    revised: Some(Date::parse_iso("2026-01-23").unwrap()),
                    ..View::default()
                },
            }]
            .into_iter()
            .collect(),
            names: [Named {
                name: Name::Doi("10.1000/x"),
                standing: Standing::Promoted,
                by: [SourceId::Pubmed].into_iter().collect(),
                native: false,
                cited_by: SmallVec::new(),
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
        assert!(json.contains("\"revised\":\"2026-01-23\""), "{json}");
        assert_eq!(back.records[0].revised().unwrap().to_string(), "2026-01-23");
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
