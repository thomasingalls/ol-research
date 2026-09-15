//! A record's values for the promoted fields of merge plan §5, in one
//! source-agnostic shape, borrowed from the record: what a
//! [`Contributor`](crate::Contributor) answers for each of its records,
//! and what the second pass keeps, owned, until the work's records have
//! all been seen.

use std::cmp::Ordering;
use std::fmt;

use model::date::Bound;
use model::symbol::{Interned, Table};
use model::{Date, Issn, Lang, PartialDate};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::kind::{Form, Genre};
use crate::title::{OwnedTitles, Titles};

/// An OpenAlex source (a journal, a repository), by its `S` number.
pub type OpenAlexSource = model::id::SourceId;

/// The venue as ids only: whichever of the three the source gives. Names
/// stay in the normalized layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Venue<'a> {
    /// OpenAlex's source id.
    pub openalex: Option<OpenAlexSource>,
    /// The ISSNs, valid ones only, print and electronic and linking.
    pub issns: SmallVec<[Issn; 3]>,
    /// PubMed's `NlmUniqueID`.
    #[serde(borrow)]
    pub nlm: Option<Interned<'a>>,
}

impl Venue<'_> {
    /// No id at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.openalex.is_none() && self.issns.is_empty() && self.nlm.is_none()
    }

    /// The disagreement rule of §5: both give ISSNs and none is shared.
    #[must_use]
    pub fn disjoint(&self, other: &Self) -> bool {
        !self.issns.is_empty()
            && !other.issns.is_empty()
            && !self.issns.iter().any(|i| other.issns.contains(i))
    }
}

/// What one source says about a work's status; derived by the merge from
/// publication types, relations, Crossmark updates and flags (§5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    #[allow(missing_docs)]
    pub retracted: bool,
    #[allow(missing_docs)]
    pub corrected: bool,
    /// An expression of concern.
    pub concern: bool,
}

/// One record's values, borrowed. A field the source does not have is
/// `None`, which the precedence rules skip.
#[derive(Debug, Clone, Default)]
pub struct View<'a> {
    /// The source's own revision stamp: Crossref's `indexed`, PubMed's
    /// `DateRevised`, OpenAlex's `updated_date`.
    pub revised: Option<Date>,
    /// The titles, raw markup and all.
    pub titles: Titles<'a>,
    /// Whether the record carries an abstract in any form.
    pub has_abstract: bool,
    /// How many authors the record lists.
    pub authors: u32,
    #[allow(missing_docs)]
    pub venue: Venue<'a>,
    /// The earliest publication date the source gives, at its precision.
    pub published: Option<Bound>,
    #[allow(missing_docs)]
    pub language: Option<Lang>,
    /// The form, native for Crossref and mapped for the others.
    pub form: Option<Form>,
    /// The genre, native for OpenAlex and PubMed and mapped for Crossref.
    pub genre: Option<Genre>,
    #[allow(missing_docs)]
    pub status: Status,
    /// Open access, when the source says: OpenAlex's status, a Creative
    /// Commons license at Crossref, a PMC copy at PubMed.
    pub access: Option<bool>,
    /// The source's count of the record's references, when it keeps one.
    pub references: Option<u32>,
    /// The source's count of citations to the record, when it keeps one.
    pub cited_by: Option<u32>,
}

/// A value a source vocabulary has that the tables of [`crate::kind`] and
/// [`crate::relation`] do not, or a symbol its set's table does not: the
/// run stops, or `--survey` counts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmapped {
    /// The normalized field.
    pub field: &'static str,
    /// The value as the source wrote it.
    pub value: String,
}

impl fmt::Display for Unmapped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?} is not in the table", self.field, self.value)
    }
}

impl std::error::Error for Unmapped {}

/// An interned value's text through its set's table, for the contributors:
/// a symbol the table does not have is a corrupt set, reported as
/// [`Unmapped`] on the field.
///
/// # Errors
///
/// A symbol beyond the table.
pub fn text<'a>(
    value: &'a Interned<'a>,
    table: &'a Table,
    field: &'static str,
) -> Result<&'a str, Unmapped> {
    value.resolve(table).ok_or_else(|| Unmapped {
        field,
        value: format!("{value:?} beyond the set's symbol table"),
    })
}

/// A partial date at the precision the merge compares: a season, a
/// quarter or a span is its year, or a span's start.
#[must_use]
pub fn bound(date: &PartialDate) -> Bound {
    match *date {
        PartialDate::Year(y) | PartialDate::YearSeason(y, _) | PartialDate::YearQuarter(y, _) => {
            Bound {
                year: y,
                month: None,
                day: None,
            }
        }
        PartialDate::YearMonth(y, m) => Bound {
            year: y,
            month: Some(m),
            day: None,
        },
        PartialDate::Day(d) => Bound {
            year: d.year(),
            month: Some(d.month()),
            day: Some(d.day()),
        },
        PartialDate::Span(span) => span.start,
    }
}

/// Which of two publication dates is earlier (§5): by year, then by month
/// with a month before none, then by day likewise — so among dates of
/// one year the most precise wins, and among dates of one precision the
/// earliest.
#[must_use]
pub fn earlier(a: &Bound, b: &Bound) -> Ordering {
    fn part<T: Ord>(a: Option<T>, b: Option<T>) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
    a.year
        .cmp(&b.year)
        .then_with(|| part(a.month, b.month))
        .then_with(|| part(a.day, b.day))
}

/// A venue that owns its text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedVenue {
    #[allow(missing_docs)]
    pub openalex: Option<OpenAlexSource>,
    #[allow(missing_docs)]
    pub issns: SmallVec<[Issn; 3]>,
    #[allow(missing_docs)]
    pub nlm: Option<Box<str>>,
}

impl OwnedVenue {
    /// The venue, borrowed.
    #[must_use]
    pub fn borrow(&self) -> Venue<'_> {
        Venue {
            openalex: self.openalex,
            issns: self.issns.clone(),
            nlm: self.nlm.as_deref().map(Interned::Text),
        }
    }
}

/// A view that owns its text: what the second pass keeps per record. The
/// title is kept as text, for promotion, and as fingerprints, for the
/// comparison of §3.3.
#[derive(Debug, Clone, Default)]
pub struct Owned {
    #[allow(missing_docs)]
    pub revised: Option<Date>,
    #[allow(missing_docs)]
    pub title: Option<Box<str>>,
    #[allow(missing_docs)]
    pub titles: OwnedTitles,
    #[allow(missing_docs)]
    pub has_abstract: bool,
    #[allow(missing_docs)]
    pub authors: u32,
    #[allow(missing_docs)]
    pub venue: OwnedVenue,
    #[allow(missing_docs)]
    pub published: Option<Bound>,
    #[allow(missing_docs)]
    pub language: Option<Lang>,
    #[allow(missing_docs)]
    pub form: Option<Form>,
    #[allow(missing_docs)]
    pub genre: Option<Genre>,
    #[allow(missing_docs)]
    pub status: Status,
    #[allow(missing_docs)]
    pub access: Option<bool>,
    #[allow(missing_docs)]
    pub references: Option<u32>,
    #[allow(missing_docs)]
    pub cited_by: Option<u32>,
}

impl Owned {
    /// A copy of `view` that owns its text. A venue id given as a symbol
    /// rather than text is dropped: contributors resolve through [`text`].
    #[must_use]
    pub fn of(view: &View<'_>) -> Self {
        Self {
            revised: view.revised,
            title: view.titles.title.map(Box::from),
            titles: OwnedTitles::of(view.titles),
            has_abstract: view.has_abstract,
            authors: view.authors,
            venue: OwnedVenue {
                openalex: view.venue.openalex,
                issns: view.venue.issns.clone(),
                nlm: view
                    .venue
                    .nlm
                    .as_ref()
                    .and_then(Interned::as_text)
                    .map(Box::from),
            },
            published: view.published,
            language: view.language,
            form: view.form,
            genre: view.genre,
            status: view.status,
            access: view.access,
            references: view.references,
            cited_by: view.cited_by,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use model::date::Bound;
    use model::{Day, Issn, Month, Year};

    use super::{Venue, earlier};

    fn bound(y: u16, m: Option<u8>, d: Option<u8>) -> Bound {
        Bound {
            year: Year::new(y).unwrap(),
            month: m.map(|m| Month::from_number(m).unwrap()),
            day: d.map(|d| Day::new(d).unwrap()),
        }
    }

    #[test]
    fn the_earlier_date_is_the_earliest_then_the_most_precise() {
        let full = bound(2014, Some(6), Some(15));
        let month = bound(2014, Some(11), None);
        let year = bound(2014, None, None);
        let next = bound(2015, Some(1), Some(1));
        assert_eq!(earlier(&full, &month), Ordering::Less);
        assert_eq!(earlier(&month, &year), Ordering::Less, "precision wins");
        assert_eq!(earlier(&year, &next), Ordering::Less);
        assert_eq!(earlier(&full, &full), Ordering::Equal);
        assert_eq!(
            earlier(&bound(2014, Some(6), None), &full),
            Ordering::Greater
        );
    }

    #[test]
    fn venues_disagree_only_on_disjoint_issns() {
        let a = Venue {
            issns: [Issn::parse("0960-1481").unwrap()].into_iter().collect(),
            ..Venue::default()
        };
        let b = Venue {
            issns: [Issn::parse("1879-0682").unwrap()].into_iter().collect(),
            ..Venue::default()
        };
        let both = Venue {
            issns: [
                Issn::parse("0960-1481").unwrap(),
                Issn::parse("1879-0682").unwrap(),
            ]
            .into_iter()
            .collect(),
            ..Venue::default()
        };
        assert!(a.disjoint(&b));
        assert!(!a.disjoint(&both));
        assert!(!a.disjoint(&Venue::default()), "nothing to compare");
        assert!(Venue::default().is_empty());
    }
}
