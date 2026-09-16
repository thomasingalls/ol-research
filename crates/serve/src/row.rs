//! One row of the works index (serving plan §3): everything the merge
//! knows about a work — its names with their standings, its source
//! records, its promoted fields with their disagreements, its flags —
//! plus the text its promoted records carry, owned and self-contained
//! so a row decodes alone.

use merge::identity::Standing;
use merge::kind::{Form, Genre};
use merge::record::{Field, Flags, Other, StatusBy, Work};
use merge::view::{OwnedVenue, Venue};
use merge::{Document, OwnedName, SourceId, Sources, WorkId};
use model::date::Bound;
use model::symbol::Table;
use model::{Date, Lang};
use serde::{Deserialize, Serialize};

/// An accepted name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    #[allow(missing_docs)]
    pub name: OwnedName,
    #[allow(missing_docs)]
    pub standing: Standing,
    /// Who asserted it; for a stub's name, who cited it.
    pub by: Sources,
    /// Whether a record of the work holds it natively.
    pub native: bool,
}

/// One source record of the work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    #[allow(missing_docs)]
    pub source: SourceId,
    #[allow(missing_docs)]
    pub native: OwnedName,
    #[allow(missing_docs)]
    pub revised: Option<Date>,
    #[allow(missing_docs)]
    pub promoted: bool,
}

/// A rejected assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    #[allow(missing_docs)]
    pub name: OwnedName,
    #[allow(missing_docs)]
    pub by: SourceId,
    #[allow(missing_docs)]
    pub holder: OwnedName,
}

/// The promoted fields of the merge plan's §5, owned.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fields {
    /// Raw markup and all, as the merge keeps it.
    pub title: Option<Field<String>>,
    #[allow(missing_docs)]
    pub abstract_in: Sources,
    #[allow(missing_docs)]
    pub authors: Option<Field<u32>>,
    #[allow(missing_docs)]
    pub venue: Option<Field<OwnedVenue>>,
    #[allow(missing_docs)]
    pub published: Option<Field<Bound>>,
    #[allow(missing_docs)]
    pub language: Option<Field<Lang>>,
    #[allow(missing_docs)]
    pub form: Option<Field<Form>>,
    #[allow(missing_docs)]
    pub genre: Option<Field<Genre>>,
    #[allow(missing_docs)]
    pub status: StatusBy,
    #[allow(missing_docs)]
    pub access: Option<Field<bool>>,
    #[allow(missing_docs)]
    pub references: Vec<Other<u32>>,
    #[allow(missing_docs)]
    pub cited_by: Vec<Other<u32>>,
}

/// The text a held work's promoted records carry, as plain text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    /// The promoted title, markup stripped.
    pub title: Option<String>,
    #[allow(missing_docs)]
    pub abstract_text: Option<String>,
    #[allow(missing_docs)]
    pub authors: Vec<String>,
    #[allow(missing_docs)]
    pub venue: Option<String>,
}

/// One work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    #[allow(missing_docs)]
    pub id: WorkId,
    #[allow(missing_docs)]
    pub stub: bool,
    /// The public id, when the log has one.
    pub public: Option<String>,
    #[allow(missing_docs)]
    pub names: Vec<Named>,
    #[allow(missing_docs)]
    pub records: Vec<Record>,
    #[allow(missing_docs)]
    pub rejected: Vec<Rejected>,
    #[allow(missing_docs)]
    pub fields: Fields,
    #[allow(missing_docs)]
    pub flags: Flags,
    /// For a held work; a stub has none.
    pub text: Option<Text>,
}

fn owned_venue(v: &Venue<'_>, table: &Table) -> OwnedVenue {
    OwnedVenue {
        openalex: v.openalex,
        issns: v.issns.clone(),
        nlm: v.nlm.as_ref().and_then(|n| n.resolve(table)).map(Box::from),
    }
}

impl Row {
    /// A row from a merged work, its public id, and its document when it
    /// has one; `table` is the set's symbol table, for the venue's id.
    #[must_use]
    pub fn of(
        work: &Work<'_>,
        table: &Table,
        public: Option<&str>,
        document: Option<&Document>,
    ) -> Self {
        let f = &work.fields;
        Self {
            id: work.id,
            stub: work.stub,
            public: public.map(str::to_owned),
            names: work
                .names
                .iter()
                .map(|n| Named {
                    name: n.name.to_owned(),
                    standing: n.standing,
                    by: n.by,
                    native: n.native,
                })
                .collect(),
            records: work
                .records
                .iter()
                .map(|r| Record {
                    source: r.source,
                    native: r.native.to_owned(),
                    revised: r.revised,
                    promoted: r.promoted,
                })
                .collect(),
            rejected: work
                .rejected
                .iter()
                .map(|r| Rejected {
                    name: r.name.to_owned(),
                    by: r.by,
                    holder: r.holder.to_owned(),
                })
                .collect(),
            fields: Fields {
                title: f.title.as_ref().map(|t| t.map(|s| (*s).to_owned())),
                abstract_in: f.abstract_in,
                authors: f.authors.clone(),
                venue: f.venue.as_ref().map(|v| v.map(|x| owned_venue(x, table))),
                published: f.published.clone(),
                language: f.language.clone(),
                form: f.form.clone(),
                genre: f.genre.clone(),
                status: f.status,
                access: f.access.clone(),
                references: f.references.to_vec(),
                cited_by: f.cited_by.to_vec(),
            },
            flags: work.flags,
            text: document.map(|d| Text {
                title: d.title.clone(),
                abstract_text: d.abstract_text.clone(),
                authors: d.authors.clone(),
                venue: d.venue.clone(),
            }),
        }
    }

    /// The promoted name of a namespace, by its text form's prefix.
    #[must_use]
    pub fn promoted(&self, prefix: &str) -> Option<&OwnedName> {
        self.names
            .iter()
            .find(|n| n.standing == Standing::Promoted && n.name.to_string().starts_with(prefix))
            .map(|n| &n.name)
    }
}
