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
/// `None`, which the precedence rules skip. Written into the merged
/// record as the evidence its verdicts are drawn from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct View<'a> {
    /// The source's own revision stamp: Crossref's `indexed`, PubMed's
    /// `DateRevised`, OpenAlex's `updated_date`.
    pub revised: Option<Date>,
    /// The source's version of the record: PubMed's `PMID@Version`, which
    /// a versioned citation raises; 1 where a source has no versions.
    pub version: u32,
    /// The normalized set the record was read from, relative to its
    /// source's directory and without the kind (`updatefiles/pubmed26n1335`):
    /// with the stamp and the version, what ranks two copies of one record
    /// (merge plan §8). Empty until the reader sets it.
    #[serde(borrow)]
    pub set: &'a str,
    /// The titles, raw markup and all.
    #[serde(borrow)]
    pub titles: Titles<'a>,
    /// Whether the record carries an abstract in any form.
    pub has_abstract: bool,
    /// How many authors the record lists.
    pub authors: u32,
    #[allow(missing_docs)]
    #[serde(borrow)]
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

/// The text a record carries beyond its promoted fields (serving plan
/// §3): what the merged record indexes but does not hold, as plain text
/// for the serving layer's documents.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    /// The abstract as plain text: markup stripped, sections joined under
    /// their labels.
    pub abstract_text: Option<String>,
    /// One display name per author, in the record's order.
    pub authors: Vec<String>,
    /// The venue's name as the source gives it.
    pub venue: Option<String>,
}

/// Plain text from raw markup: tags dropped — a block tag as a space, an
/// inline one as nothing, so `CO<sub>2</sub>` stays `CO2` and two
/// paragraphs do not run together — the XML entities decoded, whitespace
/// runs collapsed. Not a parser: enough for a title or an abstract, whose
/// markup is inline.
#[must_use]
pub fn strip_markup(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            let after = &rest[1..];
            if let Some(end) = after.find('>') {
                if is_block(&after[..end]) {
                    push_space(&mut out);
                }
                rest = &after[end + 1..];
            } else {
                out.push('<');
                rest = after;
            }
        } else if c == '&' {
            let after = &rest[1..];
            if let Some(end) = after.find(';').filter(|&n| n <= 8) {
                let entity = &after[..end];
                if let Some(decoded) = decode_entity(entity) {
                    out.push(decoded);
                } else {
                    out.push('&');
                    out.push_str(entity);
                    out.push(';');
                }
                rest = &after[end + 1..];
            } else {
                out.push('&');
                rest = after;
            }
        } else {
            if c.is_whitespace() {
                push_space(&mut out);
            } else {
                out.push(c);
            }
            rest = &rest[c.len_utf8()..];
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// A tag whose contents stand apart from what follows.
fn is_block(tag: &str) -> bool {
    let name = tag
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/')
        .next()
        .unwrap_or("");
    let name = name.rsplit(':').next().unwrap_or(name);
    matches!(
        name.to_ascii_lowercase().as_str(),
        "p" | "title" | "sec" | "abstract" | "list" | "list-item" | "br" | "div" | "li" | "tr"
    )
}

fn push_space(out: &mut String) {
    if !out.is_empty() && !out.ends_with(' ') {
        out.push(' ');
    }
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "lt" => Some('<'),
        "gt" => Some('>'),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = entity.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// An abstract's sections as one text: each under its label when it has
/// one (`RESULTS: …`), markup stripped, one section a line.
pub fn join_sections<'a>(sections: impl Iterator<Item = (Option<&'a str>, &'a str)>) -> String {
    let mut out = String::new();
    for (label, text) in sections {
        let text = strip_markup(text);
        if text.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        if let Some(label) = label.map(str::trim).filter(|l| !l.is_empty()) {
            out.push_str(label);
            out.push_str(": ");
        }
        out.push_str(&text);
    }
    out
}

/// OpenAlex's abstract, an inverted index — each word to its positions —
/// back into text.
///
/// # Errors
///
/// JSON that is not a map of words to positions.
pub fn inverted_index(json: &str) -> Result<String, Unmapped> {
    let index: std::collections::HashMap<String, Vec<u32>> =
        serde_json::from_str(json).map_err(|e| Unmapped {
            field: "abstract_inverted_index",
            value: e.to_string(),
        })?;
    let mut words: Vec<(u32, &str)> = index
        .iter()
        .flat_map(|(word, positions)| positions.iter().map(move |p| (*p, word.as_str())))
        .collect();
    words.sort_unstable();
    let mut out = String::new();
    for (_, word) in words {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    Ok(out)
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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

    /// No id at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.openalex.is_none() && self.issns.is_empty() && self.nlm.is_none()
    }
}

/// A view that owns its text: what the second pass keeps per record, and
/// what a merged record carries per source record. The titles are kept
/// as text, for promotion and for the record, and as fingerprints, for
/// the comparison of §3.3; the fingerprints are recomputed on reading.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Raw")]
pub struct Owned {
    #[allow(missing_docs)]
    pub revised: Option<Date>,
    #[allow(missing_docs)]
    pub version: u32,
    #[allow(missing_docs)]
    pub set: Box<str>,
    #[allow(missing_docs)]
    pub title: Option<Box<str>>,
    #[allow(missing_docs)]
    pub vernacular: Option<Box<str>>,
    #[allow(missing_docs)]
    #[serde(skip_serializing)]
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

/// [`Owned`] as it is read back: the fingerprints are not stored.
#[derive(Deserialize)]
struct Raw {
    revised: Option<Date>,
    version: u32,
    set: Box<str>,
    title: Option<Box<str>>,
    vernacular: Option<Box<str>>,
    has_abstract: bool,
    authors: u32,
    venue: OwnedVenue,
    published: Option<Bound>,
    language: Option<Lang>,
    form: Option<Form>,
    genre: Option<Genre>,
    status: Status,
    access: Option<bool>,
    references: Option<u32>,
    cited_by: Option<u32>,
}

impl From<Raw> for Owned {
    fn from(r: Raw) -> Self {
        Self {
            revised: r.revised,
            version: r.version,
            set: r.set,
            titles: OwnedTitles::of(Titles {
                title: r.title.as_deref(),
                vernacular: r.vernacular.as_deref(),
            }),
            title: r.title,
            vernacular: r.vernacular,
            has_abstract: r.has_abstract,
            authors: r.authors,
            venue: r.venue,
            published: r.published,
            language: r.language,
            form: r.form,
            genre: r.genre,
            status: r.status,
            access: r.access,
            references: r.references,
            cited_by: r.cited_by,
        }
    }
}

impl Owned {
    /// A copy of `view` that owns its text. A venue id given as a symbol
    /// rather than text is dropped: contributors resolve through [`text`],
    /// and a merged row is read through [`Owned::resolve`].
    #[must_use]
    pub fn of(view: &View<'_>) -> Self {
        Self::with_nlm(
            view,
            view.venue
                .nlm
                .as_ref()
                .and_then(Interned::as_text)
                .map(Box::from),
        )
    }

    /// A copy of `view` that owns its text, a venue id given as a symbol
    /// resolved through `table`.
    #[must_use]
    pub fn resolve(view: &View<'_>, table: &Table) -> Self {
        Self::with_nlm(
            view,
            view.venue
                .nlm
                .as_ref()
                .and_then(|n| n.resolve(table))
                .map(Box::from),
        )
    }

    fn with_nlm(view: &View<'_>, nlm: Option<Box<str>>) -> Self {
        Self {
            revised: view.revised,
            version: view.version,
            set: view.set.into(),
            title: view.titles.title.map(Box::from),
            vernacular: view.titles.vernacular.map(Box::from),
            titles: OwnedTitles::of(view.titles),
            has_abstract: view.has_abstract,
            authors: view.authors,
            venue: OwnedVenue {
                openalex: view.venue.openalex,
                issns: view.venue.issns.clone(),
                nlm,
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

    /// The view, borrowed.
    #[must_use]
    pub fn borrow(&self) -> View<'_> {
        View {
            revised: self.revised,
            version: self.version,
            set: &self.set,
            titles: Titles {
                title: self.title.as_deref(),
                vernacular: self.vernacular.as_deref(),
            },
            has_abstract: self.has_abstract,
            authors: self.authors,
            venue: self.venue.borrow(),
            published: self.published,
            language: self.language,
            form: self.form,
            genre: self.genre,
            status: self.status,
            access: self.access,
            references: self.references,
            cited_by: self.cited_by,
        }
    }

    /// Whether `other` is a newer copy of the same record: the rank of
    /// merge plan §8 — stamp, then version, then set — without the place
    /// in the set, which a view does not keep. See
    /// [`Delivery::older_than`](crate::copies::Delivery::older_than),
    /// which ranks the same three.
    #[must_use]
    pub fn older_than(&self, other: &Self) -> bool {
        (self.revised, self.version, &self.set) < (other.revised, other.version, &other.set)
    }

    /// Absence is not an assertion (merge plan §8, 2026-09-18): every
    /// field this copy has none of, taken from `older`, a copy of the
    /// same record that a later delivery superseded. The stamp, the
    /// version and the set stay this copy's: they name the delivery, not
    /// the work. A value that is merely shorter is not an absence — a
    /// truncated title or a reference list PMC cut down still speaks,
    /// because it is there.
    ///
    /// The venue fills as one id, never field by field: its three ids
    /// name one journal, and a later copy that moved the article would
    /// otherwise leave a venue no journal has.
    pub fn fill_from(&mut self, older: &Self) {
        if self.title.is_none() || self.vernacular.is_none() {
            self.title = self.title.take().or_else(|| older.title.clone());
            self.vernacular = self.vernacular.take().or_else(|| older.vernacular.clone());
            self.titles = OwnedTitles::of(Titles {
                title: self.title.as_deref(),
                vernacular: self.vernacular.as_deref(),
            });
        }
        self.has_abstract |= older.has_abstract;
        if self.authors == 0 {
            self.authors = older.authors;
        }
        if self.venue.is_empty() {
            self.venue = older.venue.clone();
        }
        self.published = self.published.or(older.published);
        self.language = self.language.or(older.language);
        self.form = self.form.or(older.form);
        self.genre = self.genre.or(older.genre);
        self.status.retracted |= older.status.retracted;
        self.status.corrected |= older.status.corrected;
        self.status.concern |= older.status.concern;
        self.access = self.access.or(older.access);
        self.references = self.references.or(older.references);
        self.cited_by = self.cited_by.or(older.cited_by);
    }

    /// Two copies of one record as the rule leaves them: the newer's
    /// values, the older's where the newer has none.
    #[must_use]
    pub fn joined(self, other: Self) -> Self {
        let (mut newer, older) = if self.older_than(&other) {
            (other, self)
        } else {
            (self, other)
        };
        newer.fill_from(&older);
        newer
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn markup_becomes_text() {
        use super::{inverted_index, join_sections, strip_markup};
        assert_eq!(
            strip_markup("<i>Drosophila</i> larvae"),
            "Drosophila larvae"
        );
        assert_eq!(strip_markup("CO<sub>2</sub> uptake"), "CO2 uptake");
        assert_eq!(
            strip_markup("<jats:p>First.</jats:p><jats:p>Second.</jats:p>"),
            "First. Second."
        );
        assert_eq!(
            strip_markup("a &lt; b &amp; c &#8211; d &#x2014; e"),
            "a < b & c \u{2013} d \u{2014} e"
        );
        assert_eq!(
            strip_markup("  many   spaces\n\tand lines  "),
            "many spaces and lines"
        );
        assert_eq!(strip_markup("x < y"), "x < y");
        assert_eq!(strip_markup("&unknown; &#zz;"), "&unknown; &#zz;");
        assert_eq!(
            join_sections(
                [
                    (Some("BACKGROUND"), "One <b>thing</b>."),
                    (None, "Two."),
                    (Some(""), "")
                ]
                .into_iter()
            ),
            "BACKGROUND: One thing.\nTwo."
        );
        assert_eq!(
            inverted_index(r#"{"world":[1],"Hello":[0,2]}"#).unwrap(),
            "Hello world Hello"
        );
        assert!(inverted_index("[1,2]").is_err());
    }

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
