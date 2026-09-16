//! One row of the works index (serving plan §3): everything the merge
//! knows about a work — its names with their standings, its source
//! records with their evidence, its promoted fields with their
//! disagreements, its flags — plus the text its promoted records carry,
//! owned and self-contained so a row decodes alone. The merged record's
//! own owned types are used as they are, so a row is a merged record
//! with a public id and text beside it.

pub use merge::build::{OwnedAssert as Assert, OwnedFields as Fields, OwnedNamed as Named};
pub use merge::build::{OwnedRecord as Record, OwnedRejected as Rejected};
use merge::identity::Standing;
use merge::record::{Flags, Work};
use merge::{Document, OwnedName, OwnedWork, WorkId};
use model::symbol::Table;
use serde::{Deserialize, Serialize};

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

impl Text {
    /// From a document.
    #[must_use]
    pub fn of(d: &Document) -> Self {
        Self {
            title: d.title.clone(),
            abstract_text: d.abstract_text.clone(),
            authors: d.authors.clone(),
            venue: d.venue.clone(),
        }
    }

    /// From the merge's text of a record, under a title.
    #[must_use]
    pub fn with_title(title: Option<String>, text: &merge::view::Text) -> Self {
        Self {
            title,
            abstract_text: text.abstract_text.clone(),
            authors: text.authors.clone(),
            venue: text.venue.clone(),
        }
    }

    /// The merge's text: what the row carries beyond the title.
    #[must_use]
    pub fn body(&self) -> merge::view::Text {
        merge::view::Text {
            abstract_text: self.abstract_text.clone(),
            authors: self.authors.clone(),
            venue: self.venue.clone(),
        }
    }
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
        Self::from_owned(
            OwnedWork::of(work, table),
            public.map(str::to_owned),
            document.map(Text::of),
        )
    }

    /// A row from an owned merged work, as the works delta carries it.
    #[must_use]
    pub fn from_owned(work: OwnedWork, public: Option<String>, text: Option<Text>) -> Self {
        Self {
            id: work.id,
            stub: work.stub,
            public,
            names: work.names.into_vec(),
            records: work.records.into_vec(),
            rejected: work.rejected.into_vec(),
            fields: work.fields,
            flags: work.flags,
            text,
        }
    }

    /// The merged work the row was made from.
    #[must_use]
    pub fn into_work(self) -> OwnedWork {
        OwnedWork {
            id: self.id,
            stub: self.stub,
            records: self.records.into_iter().collect(),
            names: self.names.into_iter().collect(),
            rejected: self.rejected.into_iter().collect(),
            fields: self.fields,
            flags: self.flags,
        }
    }

    /// The merged work the row was made from, borrowed into a copy.
    #[must_use]
    pub fn work(&self) -> OwnedWork {
        self.clone().into_work()
    }

    /// The promoted name of a namespace, by its text form's prefix.
    #[must_use]
    pub fn promoted(&self, prefix: &str) -> Option<&OwnedName> {
        self.names
            .iter()
            .find(|n| n.standing == Standing::Promoted && n.name.to_string().starts_with(prefix))
            .map(|n| &n.name)
    }

    /// The document the lexical index holds for the row: none for a stub.
    #[must_use]
    pub fn document(&self) -> Option<Document> {
        if self.stub {
            return None;
        }
        let body = self.text.as_ref().map(Text::body);
        Some(Document::of(&self.work().borrow(), body.as_ref()))
    }
}

/// A promoted title with its markup stripped, for a row's text.
#[must_use]
pub fn title_of(fields: &Fields) -> Option<String> {
    fields
        .title
        .as_ref()
        .map(|t| merge::view::strip_markup(&t.value))
}
