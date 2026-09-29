//! Native work → normalized records.
//!
//! Value-level strictness (plan §6) over the fields the evidence has
//! justified so far (plan §4a). Every DOI, ORCID, ISSN and ISBN in the
//! sample passed its checksum, every timestamp agreed with itself three
//! ways, and every `URL` was the DOI's — so those are strict types and
//! enforced invariants, and a record that breaks one stops the run with
//! its DOI, the field and the raw text. Reference DOIs are the exception,
//! as a [`Claim`]: one in five hundred carries a Unicode hyphen.
//!
//! Records are pushed to a [`Sink`] by reference as they are built.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::fmt;

use ingest::Sink;
use model::{
    Claim, Date, DateError, DateTime, Day, Doi, DoiError, Interned, Isbn, IsbnError, Isni,
    IsniError, Issn, IssnError, Lang, LangError, Month, Orcid, OrcidError, ParseError, PartialDate,
    Qid, Registrant, Ror, RorError,
};

use crate::Crossref;
use crate::date::{DateParts, PartialDate as PartialDateParts};
use crate::record::{
    Affiliation as NativeAffiliation, Contributor, Identifier, Institution as NativeInstitution,
    Serial, Text, Work as Native,
};
use crate::vocab::{
    AssertedBy, ContentVersion, IdKind, Meaning, RelationAssertedBy, RelationIdKind, Sequence,
    SerialKind, StatusType, UpdateType, UpdateWord, WorkType,
};

/// A list with room for its usual length inside the record itself (plan
/// §5, rule 1); only a longer one allocates. The lengths come from the
/// observed distributions in the plan's data, about the 90th percentile.
pub type Few<T, const N: usize> = SmallVec<[T; N]>;

/// A DOI as a source asserted it.
pub type DoiClaim<'n> = Claim<'n, Doi<'n>, DoiError>;
/// An ISSN as the source wrote it, or why its check digit is wrong: two
/// of 15,165 works looked up by DOI carried one (2026-09-15), the policy
/// §8 item 6 already applies at OpenAlex and PubMed.
pub type IssnClaim<'n> = Claim<'n, Issn, IssnError>;
/// A ROR id as the source wrote it, or why its checksum fails: one record
/// of the March 2026 file carries two copies of `03npvkt06` for the
/// Universidade Federal Fluminense, whose ROR is `02rjhbb08`
/// (10.25189/2675-4916.2023.v4.n1.id679, 2026-09-26). Kept as written
/// beside why, as an ISSN is, and never matched on.
pub type RorClaim<'n> = Claim<'n, Ror, RorError>;
/// An ISBN as the source wrote it, or why it is none: 1,290 records of the
/// March 2026 file carry one whose check digit fails or whose length is
/// wrong (2026-09-26) — Hanser's and AIAA's check digits, Blackwell's
/// ISBN-10 `X` on a thirteen-digit number, twelve digits, fourteen with a
/// stray `9` in front. Kept as written beside why, as an ISSN is (the
/// user's call), and never matched on.
pub type IsbnClaim<'n> = Claim<'n, Isbn, IsbnError>;
/// An ORCID iD as the source wrote it, or why it is none.
pub type OrcidClaim<'n> = Claim<'n, Orcid, OrcidError>;
/// An ISNI as the source wrote it, or why its check character is wrong.
pub type IsniClaim<'n> = Claim<'n, Isni, IsniError>;

/// One Crossref work, normalized: the fields with evidence behind them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work<'n> {
    /// The work's DOI, lowercased.
    #[serde(borrow)]
    pub doi: Doi<'n>,
    /// The DOI spelled in ASCII, where it has letters beyond ASCII
    /// ([`Doi::ascii_spelling`]): a name the merge gives the work beside
    /// its own, so a publisher's ASCII twin of it (`día` and `dia`) is the
    /// same work and a citation of either lands on it.
    #[serde(borrow)]
    pub ascii_doi: Option<Doi<'n>>,
    /// `prefix`: the DOI's current owner, which differs from the DOI's own
    /// registrant on 7 % of works.
    pub owner: Registrant,
    /// `member`; absent where the record does not say it, a DOI moved to a
    /// new owner's `prefix` (see [`crate::record::Work::member`]). Not
    /// looked up from the prefix: that is another source's word.
    pub member: Option<u32>,
    /// `type`; `None` where the record has none, which is unknown rather
    /// than guessed (the user's call, 2026-09-26).
    pub kind: Option<WorkType>,
    /// DOI registration.
    pub created: DateTime,
    /// Latest metadata deposit.
    pub deposited: DateTime,
    /// Latest indexing.
    pub indexed: DateTime,
    /// `issued`, `None` for `[[null]]`. `published` is checked equal to it
    /// and not kept.
    pub issued: Option<PartialDate>,
    /// `published-print`.
    pub published_print: Option<PartialDate>,
    /// `published-online`.
    pub published_online: Option<PartialDate>,
    /// The other dates the record gives, each with what it is.
    pub dates: Few<Dated, 1>,
    /// `title[0]`, raw markup included.
    #[serde(borrow)]
    pub title: Option<&'n str>,
    /// `title[1..]`: translations, in the sample.
    #[serde(borrow)]
    pub alternate_titles: Few<&'n str, 2>,
    /// `subtitle[0]`; empty strings occur.
    #[serde(borrow)]
    pub subtitle: Option<&'n str>,
    /// A JATS (usually) XML fragment.
    pub r#abstract: Option<&'n str>,
    /// `container-title`, every entry as written (model 13; model 12 kept
    /// the first). What a second entry is depends on the type: for a
    /// journal article, the same journal under another name (another
    /// language or script, other punctuation, an acronym: all 322 examples
    /// of `ol-rht-dump-crossref-v4`); for a book chapter, the book behind
    /// its series (citation fields plan §4).
    #[serde(borrow)]
    pub container_titles: Few<Interned<'n>, 1>,
    /// `short-container-title`, every entry as written: the journal's
    /// abbreviation, often in more than one style (`Med pregl`,
    /// `MED PREGLED`). Model 13.
    #[serde(borrow)]
    pub short_container_titles: Few<Interned<'n>, 1>,
    /// `journal-issue.published-print`: the issue's own print date, which
    /// the work's `published-print` need not be. Model 13.
    pub issue_published_print: Option<PartialDate>,
    /// `journal-issue.published-online`: the issue's own online date, its
    /// month or its year where the work's is a day. Model 13.
    pub issue_published_online: Option<PartialDate>,
    /// From `issn-type`, checked against `ISSN`.
    #[serde(borrow)]
    pub issns: Few<TypedIssn<'n>, 2>,
    /// From `isbn-type`, checked against `ISBN`.
    #[serde(borrow)]
    pub isbns: Few<TypedIsbn<'n>, 2>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issue: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub page: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub article_number: Option<&'n str>,
    /// `part-number`: which part of a multi-volume set, as written.
    #[serde(borrow)]
    pub part_number: Option<&'n str>,
    /// `version.version`: the work's version label, as written — free
    /// text, not a number.
    #[serde(borrow)]
    pub version: Option<&'n str>,
    /// `status`: posted content withdrawn or removed.
    #[serde(borrow)]
    pub status: Option<PostedStatus<'n>>,
    /// `aliases`: other DOIs the registry holds for this work — a
    /// figshare item's `.v1`, on 138 of the records the March 2026 file's
    /// survey named or stopped at (2026-09-26) — each a name the merge
    /// gives the work beside its own. Dropped until model 10.
    #[serde(borrow)]
    pub aliases: Few<DoiClaim<'n>, 1>,
    /// `publisher`; absent with `member`.
    #[serde(borrow)]
    pub publisher: Option<Interned<'n>>,
    /// `language`: ISO 639-1, or ISO 639-2's terminologic code on 303
    /// records of the March 2026 file (`spa` on the Universidad Nacional
    /// Mayor de San Marcos's and the Instituto Nacional Materno
    /// Perinatal's, `eng`; 2026-09-26). A bibliographic-only code (`fre`)
    /// has not been seen, and stops the run.
    pub language: Option<Lang>,
    /// `author[]`.
    #[serde(borrow)]
    pub authors: Few<Author<'n>, 8>,
    /// `institution[]`: the work's own, a dissertation's university or a
    /// report's issuer — where most Wikidata ids sit.
    #[serde(borrow)]
    pub institutions: Few<Institution<'n>, 1>,
    /// `references-count`, checked against the reference array.
    pub references_count: Option<u32>,
    /// `is-referenced-by-count`.
    pub cited_by_count: Option<u32>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub licenses: Few<License<'n>, 4>,
    /// Updates this work applies to others: it is the notice.
    #[serde(borrow)]
    pub updates_to: Few<Update<'n>, 2>,
    /// Updates applied to this work: it is the retracted, corrected, … one.
    #[serde(borrow)]
    pub updated_by: Few<Update<'n>, 2>,
    /// `relation`, flattened.
    #[serde(borrow)]
    pub relations: Few<Relation<'n>, 4>,
    /// `alternative-id`, often the DOI itself or a PII.
    #[serde(borrow)]
    pub alternative_ids: Few<&'n str, 2>,
}

/// An author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: AuthorName<'n>,
    /// A claim: 8 records of the March 2026 file write `http://orcid.org/`,
    /// read as the same iD, and one carries an iD whose check character
    /// fails (10.64522/v4i4.02), kept as written beside why (2026-09-26).
    #[serde(borrow)]
    pub orcid: Option<OrcidClaim<'n>>,
    /// `authenticated-orcid`.
    pub authenticated_orcid: Option<bool>,
    #[allow(missing_docs)]
    pub sequence: Sequence,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub affiliations: Few<Institution<'n>, 2>,
}

/// A person (`family`, maybe `given`) or an organisation (`name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorName<'n> {
    #[allow(missing_docs)]
    Person {
        #[allow(missing_docs)]
        #[serde(borrow)]
        given: Option<&'n str>,
        #[allow(missing_docs)]
        family: &'n str,
    },
    #[allow(missing_docs)]
    Org(&'n str),
    /// A given name and nothing else: a person known by one name
    /// (`Siddhartha`), a whole name written in the given-name field
    /// (`Nguyen Minh Duc`), initials (`F.`), or an organisation (`ASDEX
    /// Upgrade Team`) — which, the record does not say, so it is kept as
    /// written (81 records of the March 2026 file, the user's call,
    /// 2026-09-26).
    Given(&'n str),
}

/// An organisation as the deposit schema's `<institution>` describes it:
/// an author's affiliation, or the work's own institution. The schema asks
/// for a name or an id, and allows up to six each of acronyms, places and
/// departments; all are kept as written (the user's call, 2026-09-26), as
/// is who asserted each id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Institution<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Option<Interned<'n>>,
    /// `institution_acronym`.
    #[serde(borrow)]
    pub acronyms: Few<Interned<'n>, 1>,
    /// `institution_place`.
    #[serde(borrow)]
    pub places: Few<Interned<'n>, 1>,
    /// `institution_department`.
    #[serde(borrow)]
    pub departments: Few<Interned<'n>, 1>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub ror: Option<Asserted<RorClaim<'n>>>,
    /// Beside the ROR on 40 affiliations of 34,536 looked-up works
    /// (2026-09-15). A claim, as a ROR is: two records of the March 2026
    /// file carry one whose check character fails (10.46428/lbd.1, and
    /// 10.5555/diswikitest on Crossref's test prefix; 2026-09-26).
    #[serde(borrow)]
    pub isni: Option<Asserted<IsniClaim<'n>>>,
    /// Beside the ROR and ISNI on university repositories' deposits.
    pub wikidata: Option<Asserted<Qid>>,
}

/// An identifier and who asserted it: `publisher` on every institution id
/// among the records the March 2026 file's survey stopped at (16,044,
/// 2026-09-26), `crossref` on some of the funder DOIs beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asserted<T> {
    #[allow(missing_docs)]
    pub id: T,
    #[allow(missing_docs)]
    pub by: AssertedBy,
}

/// An ISSN with its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedIssn<'n> {
    #[allow(missing_docs)]
    pub kind: SerialKind,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issn: IssnClaim<'n>,
}

/// An ISBN with its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedIsbn<'n> {
    #[allow(missing_docs)]
    pub kind: SerialKind,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub isbn: IsbnClaim<'n>,
}

/// That posted content was withdrawn or removed, when, and the reasons
/// given, as written (the user's lean towards keeping what a record says,
/// 2026-09-26). The merge does not read it yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostedStatus<'n> {
    /// What became of it.
    pub kind: StatusType,
    /// When.
    pub updated: PartialDate,
    /// Why.
    #[serde(borrow)]
    pub reasons: Few<Note<'n>, 1>,
}

/// A text with its language when the record gives one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub text: &'n str,
    /// As written: `en`, `eng`, `fr`.
    #[serde(borrow)]
    pub language: Option<&'n str>,
}

/// A date beside the publication dates, with what it is. Crossref gives
/// these and they were dropped until model 10; for a dissertation or a
/// standard, `approved` is often the only date there is (the user's call,
/// 2026-09-26: keep them all).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dated {
    #[allow(missing_docs)]
    pub kind: DateKind,
    #[allow(missing_docs)]
    pub date: PartialDate,
}

/// What a [`Dated`] is: the field it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DateKind {
    /// `posted`: posted content went up.
    Posted,
    /// `accepted`.
    Accepted,
    /// `approved`: a dissertation's or a standard's approval.
    Approved,
    /// `published-other`.
    PublishedOther,
    /// `content-created`: a dataset's content was created.
    ContentCreated,
    /// `content-updated`: it last changed.
    ContentUpdated,
}

impl DateKind {
    /// The field, as Crossref spells it.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Posted => "posted",
            Self::Accepted => "accepted",
            Self::Approved => "approved",
            Self::PublishedOther => "published-other",
            Self::ContentCreated => "content-created",
            Self::ContentUpdated => "content-updated",
        }
    }
}

/// A license.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct License<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub url: Interned<'n>,
    #[allow(missing_docs)]
    pub version: ContentVersion,
    /// `start`, read ([`LicenseStart`]). `delay-in-days` is checked
    /// against it and dropped: it is Crossref's `max(0, start − issued)`,
    /// whose 0 means no delay, a start before publication, and nothing to
    /// measure from alike.
    #[serde(borrow)]
    pub start: LicenseStart<'n>,
}

/// When a license applies from.
///
/// Where a deposit gives no start date, Crossref writes the work's
/// `issued` as the start, padded to its first day (`1986` is 1986-01-01),
/// and where the work has no `issued`, writes none (M, over every local
/// license and the March 2026 file's dumped records, 2026-09-26). A
/// start on that day is the fill or a publisher's start on the day of
/// publication, which the record cannot tell apart and which say the same:
/// from publication (the user's call).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LicenseStart<'n> {
    /// From publication: no start, or the start is the publication date.
    FromPublication,
    /// From this day, before publication or after it.
    On(Date),
    /// A start written as a date that is none — `0001-01-01`, a date
    /// library's empty value, on one record of the March 2026 file —
    /// kept as written beside why.
    Unreadable {
        /// The `date-time` as written.
        #[serde(borrow)]
        raw: &'n str,
        #[allow(missing_docs)]
        error: DateError,
    },
}

/// A Crossmark update, either direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Update<'n> {
    /// The other work.
    #[serde(borrow)]
    pub doi: Doi<'n>,
    /// What the update is.
    pub kind: UpdateType,
    /// The word the record wrote, beside what it was read as, so a reading
    /// such as `err` for an erratum can be revisited (the user, 2026-09-26).
    pub word: UpdateWord,
    #[allow(missing_docs)]
    pub at: DateTime,
}

/// One relation to another object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation<'n> {
    /// `has-preprint`, `is-preprint-of`, …
    #[serde(borrow)]
    pub kind: Interned<'n>,
    #[allow(missing_docs)]
    pub id: &'n str,
    #[allow(missing_docs)]
    pub id_kind: RelationIdKind,
    #[allow(missing_docs)]
    pub asserted_by: RelationAssertedBy,
}

/// One entry of a work's reference list: an edge whose target is a DOI
/// (76 % of the sample), matching signals, or both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference<'n> {
    #[allow(missing_docs)]
    pub key: &'n str,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub doi: Option<DoiClaim<'n>>,
    /// Present exactly when `doi` is.
    pub asserted_by: Option<AssertedBy>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub unstructured: Option<&'n str>,
    /// Raw: `2019`, but also `nd` and `0`.
    #[serde(borrow)]
    pub year: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub article_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub journal_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub series_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub author: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issue: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub first_page: Option<&'n str>,
}

/// A value that did not fit the model.
#[derive(Debug)]
pub struct Error {
    /// The work, once its DOI was readable.
    pub doi: Option<String>,
    /// The field.
    pub field: &'static str,
    /// The value as the source wrote it.
    pub raw: String,
    /// Why.
    pub kind: ErrorKind,
}

/// Why a value did not fit.
#[derive(Debug)]
#[allow(missing_docs)]
pub enum ErrorKind {
    Doi(DoiError),
    Isni(IsniError),
    Issn(IssnError),
    Date(DateError),
    Lang(LangError),
    Id(ParseError),
    /// Two fields that must agree do not.
    Inconsistent(&'static str),
    /// Present in the data, not modelled here yet.
    Unmodelled(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.doi {
            Some(doi) => write!(f, "DOI {doi}: ")?,
            None => f.write_str("DOI unknown: ")?,
        }
        write!(f, "{} = {:?}: ", self.field, self.raw)?;
        fmt::Display::fmt(&self.kind, f)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Doi(e) => write!(f, "{e}"),
            ErrorKind::Isni(e) => write!(f, "{e}"),
            ErrorKind::Issn(e) => write!(f, "{e}"),
            ErrorKind::Date(e) => write!(f, "{e}"),
            ErrorKind::Lang(e) => write!(f, "{e}"),
            ErrorKind::Id(e) => write!(f, "{e}"),
            ErrorKind::Inconsistent(what) => write!(f, "inconsistent: {what}"),
            ErrorKind::Unmodelled(what) => write!(f, "not modelled yet: {what}"),
        }
    }
}

impl ingest::source::Signature for Error {
    fn signature(&self) -> String {
        format!("{}: {}", self.field, self.kind)
    }
}

impl std::error::Error for Error {}

struct Ctx {
    doi: Option<String>,
    /// What the record's reading set aside, by field and value, for the
    /// sink once the record is read ([`Sink::set_aside`]).
    set_aside: std::cell::RefCell<Vec<(&'static str, &'static str)>>,
    /// What it read as other than written, by field and how
    /// ([`Sink::corrected`]).
    corrected: std::cell::RefCell<Vec<(&'static str, &'static str)>>,
}

/// Where an `<institution>` sits: the field its failures name.
#[derive(Clone, Copy)]
enum Held {
    Affiliation,
    Institution,
}

impl Held {
    const fn field(self) -> &'static str {
        match self {
            Self::Affiliation => "affiliation",
            Self::Institution => "institution",
        }
    }

    const fn id_field(self) -> &'static str {
        match self {
            Self::Affiliation => "affiliation.id",
            Self::Institution => "institution.id",
        }
    }
}

/// The parts of an `<institution>`, from either place Crossref writes one.
struct Parts<'n, 'a> {
    name: Option<&'n Text<'a>>,
    acronym: &'n [Text<'a>],
    place: &'n [Text<'a>],
    department: &'n [Text<'a>],
    id: &'n [Identifier<'a>],
}

impl<'n, 'a> Parts<'n, 'a> {
    /// Nothing at all: `{}`.
    const fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.acronym.is_empty()
            && self.place.is_empty()
            && self.department.is_empty()
            && self.id.is_empty()
    }

    fn affiliation(a: &'n NativeAffiliation<'a>) -> Self {
        Self {
            name: a.name.as_ref(),
            acronym: a.acronym.as_deref().unwrap_or_default(),
            place: a.place.as_deref().unwrap_or_default(),
            department: a.department.as_deref().unwrap_or_default(),
            id: a.id.as_deref().unwrap_or_default(),
        }
    }

    fn institution(i: &'n NativeInstitution<'a>) -> Self {
        Self {
            name: i.name.as_ref(),
            acronym: i.acronym.as_deref().unwrap_or_default(),
            place: i.place.as_deref().unwrap_or_default(),
            department: i.department.as_deref().unwrap_or_default(),
            id: i.id.as_deref().unwrap_or_default(),
        }
    }
}

impl Ctx {
    /// A DOI, read ([`Doi::read`]), with what the reading changed noted
    /// for the sink under `field`.
    fn doi<'n>(&self, field: &'static str, raw: &'n str) -> Result<Doi<'n>, DoiError> {
        let (doi, readings) = Doi::read(raw)?;
        self.corrected
            .borrow_mut()
            .extend(readings.made().map(|how| (field, how)));
        Ok(doi)
    }

    fn err(&self, field: &'static str, raw: &str, kind: ErrorKind) -> Error {
        Error {
            doi: self.doi.clone(),
            field,
            raw: raw.to_owned(),
            kind,
        }
    }

    /// A full timestamp, its three representations checked against each
    /// other.
    fn stamp(
        &self,
        field: &'static str,
        parts: DateParts,
        date_time: &str,
        timestamp: i64,
    ) -> Result<DateTime, Error> {
        let at = DateTime::parse_utc(date_time)
            .map_err(|e| self.err(field, date_time, ErrorKind::Date(e)))?;
        let (Some(y), Some(m), Some(d)) = (parts.year, parts.month, parts.day) else {
            return Err(self.err(
                field,
                date_time,
                ErrorKind::Inconsistent("date-parts is not a full date"),
            ));
        };
        let date = Date::from_numbers(y, m, d)
            .map_err(|e| self.err(field, &format!("{y}-{m}-{d}"), ErrorKind::Date(e)))?;
        if date != at.date() {
            return Err(self.err(
                field,
                date_time,
                ErrorKind::Inconsistent("date-parts disagrees with date-time"),
            ));
        }
        if timestamp.div_euclid(1000) != at.epoch_seconds() {
            return Err(self.err(
                field,
                &timestamp.to_string(),
                ErrorKind::Inconsistent("timestamp disagrees with date-time"),
            ));
        }
        Ok(at)
    }

    /// A partial date; `None` when absent or `[[null]]`.
    fn partial(
        &self,
        field: &'static str,
        p: Option<&PartialDateParts>,
    ) -> Result<Option<PartialDate>, Error> {
        let Some(p) = p else { return Ok(None) };
        let d = p.date_parts;
        let Some(year) = d.year else { return Ok(None) };
        PartialDate::from_numbers(year, d.month, d.day)
            .map(Some)
            .map_err(|e| self.err(field, &format!("{d:?}"), ErrorKind::Date(e)))
    }

    /// `issn-type` / `isbn-type` entries, checked to be the same set as the
    /// plain `ISSN` / `ISBN` list.
    fn serials<'n, T>(
        &self,
        field: &'static str,
        typed: Option<&'n [Serial<'_>]>,
        plain: Option<&'n [Text<'_>]>,
        parse: impl Fn(&'n str) -> Result<T, ErrorKind>,
    ) -> Result<Vec<(SerialKind, T)>, Error> {
        let typed = typed.unwrap_or_default();
        let plain = plain.unwrap_or_default();
        if typed.len() != plain.len() || !typed.iter().all(|t| plain.contains(&t.value)) {
            return Err(self.err(
                field,
                "",
                ErrorKind::Inconsistent("typed list differs from plain list"),
            ));
        }
        typed
            .iter()
            .map(|t| {
                parse(&t.value)
                    .map(|v| (t.r#type, v))
                    .map_err(|e| self.err(field, &t.value, e))
            })
            .collect()
    }

    /// An author; `None` for one with nothing in it — no name of any kind,
    /// no ORCID, no affiliation, only its `sequence` — which says nothing
    /// and is set aside: university repositories' dissertations carry one
    /// beside their real authors (Drexel's, Wellington's, La Plata's), and
    /// the survey of the March 2026 file counted 303,155 records failing
    /// on an author with neither a family name nor a name (2026-09-26).
    fn author<'n>(&self, c: &'n Contributor<'_>) -> Result<Option<Author<'n>>, Error> {
        if c.given.is_none()
            && c.family.is_none()
            && c.name.is_none()
            && c.prefix.is_none()
            && c.suffix.is_none()
            && c.orcid.is_none()
            && c.authenticated_orcid.is_none()
            && c.affiliation.is_empty()
            && c.role.is_none()
        {
            self.set_aside.borrow_mut().push(("author", "{}"));
            return Ok(None);
        }
        let name = match (c.family.as_deref(), c.name.as_deref()) {
            (Some(family), None) => AuthorName::Person {
                given: c.given.as_deref(),
                family,
            },
            (None, Some(name)) => {
                if c.given.is_some() {
                    return Err(self.err(
                        "author",
                        name,
                        ErrorKind::Inconsistent("given with name"),
                    ));
                }
                AuthorName::Org(name)
            }
            (Some(family), Some(_)) => {
                return Err(self.err(
                    "author",
                    family,
                    ErrorKind::Inconsistent("both family and name"),
                ));
            }
            (None, None) => match c.given.as_deref() {
                Some(given) => AuthorName::Given(given),
                None => {
                    return Err(self.err(
                        "author",
                        "",
                        ErrorKind::Inconsistent("neither family nor name"),
                    ));
                }
            },
        };
        let orcid = c.orcid.as_deref().map(|o| {
            let read = match Orcid::from_https_url(o) {
                Err(OrcidError::BadPrefix) => Orcid::from_http_url(o),
                read => read,
            };
            Claim::from_parse(o, read)
        });
        let mut affiliations = Few::new();
        for a in &c.affiliation {
            // The schema gives a country to a grant investigator's
            // affiliation only.
            if let Some(country) = a.country.as_deref() {
                return Err(self.err(
                    "affiliation.country",
                    country,
                    ErrorKind::Unmodelled("a country on a contributor's affiliation"),
                ));
            }
            if let Some(i) = self.institution(Held::Affiliation, &Parts::affiliation(a))? {
                affiliations.push(i);
            }
        }
        Ok(Some(Author {
            name,
            orcid,
            authenticated_orcid: c.authenticated_orcid,
            sequence: c.sequence,
            affiliations,
        }))
    }

    /// A license's start, read against the work's publication date and
    /// Crossref's delay: the rules Crossref's own fill follows, checked
    /// ([`LicenseStart`]). Every one held on the March 2026 file's dumped
    /// records and every local license (2026-09-26); one that does not is
    /// a record Crossref wrote some other way, and stops the run.
    fn license_start<'n>(
        &self,
        l: &'n crate::record::License<'_>,
        issued: Option<PartialDate>,
    ) -> Result<LicenseStart<'n>, Error> {
        let field = "license.start";
        let delay = l.delay_in_days;
        let Some(s) = &l.start else {
            if issued.is_some() || delay != 0 {
                return Err(self.err(
                    field,
                    &delay.to_string(),
                    ErrorKind::Inconsistent("no start, but a publication date or a delay"),
                ));
            }
            return Ok(LicenseStart::FromPublication);
        };
        let at = match self.stamp(field, s.date_parts, &s.date_time, s.timestamp) {
            Ok(at) => at,
            Err(Error {
                kind: ErrorKind::Date(DateError::YearRange),
                ..
            }) => return self.unreadable_start(l, s),
            Err(e) => return Err(e),
        };
        if at.epoch_seconds().rem_euclid(86_400) != 0 {
            return Err(self.err(
                field,
                &s.date_time,
                ErrorKind::Inconsistent("a start that is not midnight"),
            ));
        }
        let start = at.date();
        let Some(issued) = issued else {
            if delay != 0 {
                return Err(self.err(
                    field,
                    &delay.to_string(),
                    ErrorKind::Inconsistent("a delay with no publication date"),
                ));
            }
            return Ok(LicenseStart::On(start));
        };
        let Some(published) = first_day(issued) else {
            return Err(self.err(
                "issued",
                &format!("{issued:?}"),
                ErrorKind::Unmodelled("a publication date that is not a year, month or day"),
            ));
        };
        let expected = (start.days_since_epoch() - published.days_since_epoch()).max(0);
        if delay != expected {
            return Err(self.err(
                field,
                &delay.to_string(),
                ErrorKind::Inconsistent("delay-in-days is not the start less the publication date"),
            ));
        }
        Ok(if start == published {
            LicenseStart::FromPublication
        } else {
            LicenseStart::On(start)
        })
    }

    /// A start whose year no date can hold: kept as written beside why,
    /// when its three forms agree with one another and its delay is 0.
    fn unreadable_start<'n>(
        &self,
        license: &crate::record::License<'_>,
        start: &'n crate::date::Stamp<'_>,
    ) -> Result<LicenseStart<'n>, Error> {
        let parts = start.date_parts;
        let (Some(year), Some(month), Some(day)) = (parts.year, parts.month, parts.day) else {
            return Err(self.err(
                "license.start",
                &start.date_time,
                ErrorKind::Inconsistent("date-parts is not a full date"),
            ));
        };
        let written = format!("{year:04}-{month:02}-{day:02}T00:00:00Z");
        let days = model::date::days_from_civil(i64::from(year), i64::from(month), i64::from(day));
        if start.date_time != written
            || start.timestamp != days * 86_400_000
            || license.delay_in_days != 0
        {
            return Err(self.err(
                "license.start",
                &start.date_time,
                ErrorKind::Inconsistent("an unreadable start whose forms disagree"),
            ));
        }
        Ok(LicenseStart::Unreadable {
            raw: &start.date_time,
            error: DateError::YearRange,
        })
    }

    /// An `<institution>`: its texts as written, and each id checked for
    /// its kind — a ROR and an ISNI as claims, a Wikidata item strictly.
    /// `None` for an empty one, `{}`, which says nothing and is set aside:
    /// 5,806 records of the March 2026 file carry one, Maximum Academic
    /// Press's above all (2026-09-26). One with anything is kept, though
    /// the schema asks for a name or an id: 5 records give a place and no
    /// more (`Belgrade,Serbia`), kept as written (the user's call). A DOI,
    /// a second id of one kind or an id Retraction Watch asserted has not
    /// been seen, and stops the run.
    fn institution<'n>(
        &self,
        held: Held,
        p: &Parts<'n, '_>,
    ) -> Result<Option<Institution<'n>>, Error> {
        if p.is_empty() {
            self.set_aside.borrow_mut().push((held.field(), "{}"));
            return Ok(None);
        }
        let (mut ror, mut isni, mut wikidata) = (None, None, None);
        for id in p.id {
            let (raw, by): (&'n str, _) = (&id.id, id.asserted_by);
            let fail = |kind| self.err(held.id_field(), raw, kind);
            if by == AssertedBy::RetractionWatch {
                return Err(fail(ErrorKind::Unmodelled(
                    "an institution id asserted by Retraction Watch",
                )));
            }
            let taken = match id.id_type {
                IdKind::Ror => ror
                    .replace(Asserted {
                        id: Claim::from_parse(raw, Ror::from_url(raw)),
                        by,
                    })
                    .is_some(),
                IdKind::Isni => isni
                    .replace(Asserted {
                        id: Claim::from_parse(raw, Isni::from_https_url(raw)),
                        by,
                    })
                    .is_some(),
                IdKind::Wikidata => wikidata
                    .replace(Asserted {
                        id: Qid::from_entity_url(raw).map_err(|e| fail(ErrorKind::Id(e)))?,
                        by,
                    })
                    .is_some(),
                IdKind::Doi => {
                    return Err(fail(ErrorKind::Unmodelled("a DOI as an institution id")));
                }
            };
            if taken {
                return Err(fail(ErrorKind::Unmodelled(
                    "more than one institution id of one kind",
                )));
            }
        }
        let texts = |list: &'n [Text<'_>]| list.iter().map(|t| Interned::Text(t)).collect();
        Ok(Some(Institution {
            name: p.name.map(|n| Interned::Text(n)),
            acronyms: texts(p.acronym),
            places: texts(p.place),
            departments: texts(p.department),
            ror,
            isni,
            wikidata,
        }))
    }

    /// An update, read as what its word means ([`UpdateWord::meaning`]);
    /// `None` for a word that is no update, which the caller sets aside. A
    /// test deposit's word reaches here only on a record the exclusion list
    /// does not hold, and stops the run: the list needs it, or the word
    /// means something after all.
    fn update<'n>(
        &self,
        field: &'static str,
        u: &'n crate::record::Update<'_>,
    ) -> Result<Option<Update<'n>>, Error> {
        let kind = match u.r#type.meaning() {
            Meaning::Update(kind) => kind,
            Meaning::SetAside => return Ok(None),
            Meaning::TestDeposit => {
                return Err(self.err(
                    field,
                    u.r#type.as_str(),
                    ErrorKind::Unmodelled("a test deposit's update word on a record not listed"),
                ));
            }
        };
        Ok(Some(Update {
            doi: self
                .doi(field, &u.doi)
                .map_err(|e| self.err(field, &u.doi, ErrorKind::Doi(e)))?,
            kind,
            word: u.r#type,
            at: self.stamp(
                field,
                u.updated.date_parts,
                &u.updated.date_time,
                u.updated.timestamp,
            )?,
        }))
    }
}

/// The first day of a partial date as Crossref pads one: a year's first
/// day, a month's, or the day. `None` for the shapes Crossref never
/// writes.
fn first_day(p: PartialDate) -> Option<Date> {
    let one = Day::new(1)?;
    match p {
        PartialDate::Year(y) => Date::new(y, Month::January, one).ok(),
        PartialDate::YearMonth(y, m) => Date::new(y, m, one).ok(),
        PartialDate::Day(d) => Some(d),
        _ => None,
    }
}

fn first<'n>(list: Option<&'n [Text<'_>]>) -> Option<&'n str> {
    list.and_then(|l| l.first()).map(|t| &**t)
}

/// Every entry of a list of strings, as written.
fn all<'n>(list: Option<&'n [Text<'_>]>) -> Few<Interned<'n>, 1> {
    list.into_iter()
        .flatten()
        .map(|t| Interned::Text(t))
        .collect()
}

/// Normalizes one work, emitting it and then its references.
///
/// # Errors
///
/// The first value that does not fit the model; see [`ErrorKind`]. The
/// emitter may already have received the work when a reference fails.
#[allow(clippy::too_many_lines)] // one block per field group; the field set is the point
pub fn normalize<'n, K: Sink<'n, Crossref>>(w: &'n Native<'_>, emit: &mut K) -> Result<(), Error> {
    let mut ctx = Ctx {
        doi: None,
        set_aside: std::cell::RefCell::new(Vec::new()),
        corrected: std::cell::RefCell::new(Vec::new()),
    };
    let doi = ctx
        .doi("DOI", &w.doi)
        .map_err(|e| ctx.err("DOI", &w.doi, ErrorKind::Doi(e)))?;
    ctx.doi = Some(doi.as_str().to_owned());
    if w.url.strip_prefix(Doi::URL_PREFIX) != Some(&*w.doi) {
        return Err(ctx.err(
            "URL",
            &w.url,
            ErrorKind::Inconsistent("URL is not https://doi.org/ + DOI"),
        ));
    }
    let owner = Registrant::parse(&w.prefix)
        .map_err(|e| ctx.err("prefix", &w.prefix, ErrorKind::Doi(e)))?;
    let member = w
        .member
        .as_deref()
        .map(|m| model::id::strict_u32(m).map_err(|e| ctx.err("member", m, ErrorKind::Id(e))))
        .transpose()?;

    let created = ctx.stamp(
        "created",
        w.created.date_parts,
        &w.created.date_time,
        w.created.timestamp,
    )?;
    let deposited = ctx.stamp(
        "deposited",
        w.deposited.date_parts,
        &w.deposited.date_time,
        w.deposited.timestamp,
    )?;
    if let Some(reason) = crate::exclusions::reason(&doi, deposited) {
        emit.excluded(&doi, reason);
        return Ok(());
    }
    let indexed = ctx.stamp(
        "indexed",
        w.indexed.date_parts,
        &w.indexed.date_time,
        w.indexed.timestamp,
    )?;
    let issued = ctx.partial("issued", Some(&w.issued))?;
    if let Some(published) = &w.published
        && published.date_parts != w.issued.date_parts
    {
        return Err(ctx.err(
            "published",
            &format!("{:?}", published.date_parts),
            ErrorKind::Inconsistent("published differs from issued"),
        ));
    }
    let published_print = ctx.partial("published-print", w.published_print.as_ref())?;
    let published_online = ctx.partial("published-online", w.published_online.as_ref())?;
    let (issue_published_print, issue_published_online) = match &w.journal_issue {
        None => (None, None),
        Some(issue) => (
            ctx.partial(
                "journal-issue.published-print",
                issue.published_print.as_ref(),
            )?,
            ctx.partial(
                "journal-issue.published-online",
                issue.published_online.as_ref(),
            )?,
        ),
    };
    let status = match &w.status {
        None => None,
        Some(st) => Some(PostedStatus {
            kind: st.r#type,
            updated: ctx
                .partial("status.updated", Some(&st.updated))?
                .ok_or_else(|| {
                    ctx.err(
                        "status.updated",
                        "",
                        ErrorKind::Inconsistent("a status with no date"),
                    )
                })?,
            reasons: st
                .status_description
                .iter()
                .flatten()
                .map(|d| Note {
                    text: &d.description,
                    language: d.language.as_deref(),
                })
                .collect(),
        }),
    };
    let mut dates = Few::new();
    for (kind, p) in [
        (DateKind::Posted, &w.posted),
        (DateKind::Accepted, &w.accepted),
        (DateKind::Approved, &w.approved),
        (DateKind::PublishedOther, &w.published_other),
        (DateKind::ContentCreated, &w.content_created),
        (DateKind::ContentUpdated, &w.content_updated),
    ] {
        if let Some(date) = ctx.partial(kind.field(), p.as_ref())? {
            dates.push(Dated { kind, date });
        }
    }

    if let (Some(a), Some(b)) = (w.reference_count, w.references_count)
        && a != b
    {
        return Err(ctx.err(
            "reference-count",
            &a.to_string(),
            ErrorKind::Inconsistent("reference-count differs from references-count"),
        ));
    }
    let references = w.reference.as_deref().unwrap_or_default();
    if let Some(n) = w.references_count
        && usize::try_from(n).ok() != Some(references.len())
        && w.reference.is_some()
    {
        return Err(ctx.err(
            "references-count",
            &n.to_string(),
            ErrorKind::Inconsistent("references-count differs from the reference array"),
        ));
    }

    let journal_serials = ctx
        .serials(
            "issn-type",
            w.issn_type.as_deref(),
            w.issn.as_deref(),
            |s| Ok(Claim::from_parse(s, Issn::parse(s))),
        )?
        .into_iter()
        .map(|(kind, issn)| TypedIssn { kind, issn })
        .collect();
    let book_serials = ctx
        .serials(
            "isbn-type",
            w.isbn_type.as_deref(),
            w.isbn.as_deref(),
            |s| Ok(Claim::from_parse(s, Isbn::parse(s))),
        )?
        .into_iter()
        .map(|(kind, isbn)| TypedIsbn { kind, isbn })
        .collect();
    let language = w
        .language
        .as_deref()
        .map(|l| {
            Lang::from_alpha_2(l)
                .or_else(|_| Lang::from_terminologic(l))
                .map_err(|e| ctx.err("language", l, ErrorKind::Lang(e)))
        })
        .transpose()?;
    let authors = w
        .author
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|c| ctx.author(c).transpose())
        .collect::<Result<_, _>>()?;
    let institutions = w
        .institution
        .iter()
        .flatten()
        .filter_map(|i| {
            ctx.institution(Held::Institution, &Parts::institution(i))
                .transpose()
        })
        .collect::<Result<_, _>>()?;
    let licenses = w
        .license
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|l| {
            Ok(License {
                url: Interned::Text(&l.url),
                version: l.content_version,
                start: ctx.license_start(l, issued)?,
            })
        })
        .collect::<Result<_, Error>>()?;
    let mut updates = |field: &'static str, list: Option<&'n [crate::record::Update<'_>]>| {
        let mut kept = Few::new();
        for u in list.unwrap_or_default() {
            match ctx.update(field, u)? {
                Some(update) => kept.push(update),
                None => emit.set_aside(&doi, field, u.r#type.as_str()),
            }
        }
        Ok::<_, Error>(kept)
    };
    let updates_to = updates("update-to", w.update_to.as_deref())?;
    let updated_by = updates("updated-by", w.updated_by.as_deref())?;
    // Crossref copies a work's `updated-by` into its `relation` under the
    // update's word, so a `relation` type can be one too, and means what
    // it means there.
    let mut relations = Few::new();
    for (kind, targets) in w.relation.iter().flatten() {
        if let Some(word) = UpdateWord::from_word(kind) {
            match word.meaning() {
                Meaning::Update(_) => {}
                Meaning::SetAside => {
                    for _ in targets {
                        emit.set_aside(&doi, "relation", word.as_str());
                    }
                    continue;
                }
                Meaning::TestDeposit => {
                    return Err(ctx.err(
                        "relation",
                        kind,
                        ErrorKind::Unmodelled(
                            "a test deposit's update word on a record not listed",
                        ),
                    ));
                }
            }
        }
        relations.extend(targets.iter().map(|t| Relation {
            kind: Interned::Text(kind),
            id: &t.id,
            id_kind: t.id_type,
            asserted_by: t.asserted_by,
        }));
    }

    let aliases = w
        .aliases
        .iter()
        .flatten()
        .map(|a| Claim::from_parse(a, ctx.doi("aliases", a)))
        .collect();
    for (field, value) in ctx.set_aside.take() {
        emit.set_aside(&doi, field, value);
    }
    for (field, how) in ctx.corrected.take() {
        emit.corrected(&doi, field, how);
    }
    emit.work(&Work {
        owner,
        member,
        kind: w.r#type,
        created,
        deposited,
        indexed,
        issued,
        published_print,
        published_online,
        dates,
        title: first(w.title.as_deref()),
        alternate_titles: w
            .title
            .iter()
            .flat_map(|t| t.iter().skip(1))
            .map(|t| &**t)
            .collect(),
        subtitle: first(w.subtitle.as_deref()),
        r#abstract: w.r#abstract.as_deref(),
        container_titles: all(w.container_title.as_deref()),
        short_container_titles: all(w.short_container_title.as_deref()),
        issue_published_print,
        issue_published_online,
        issns: journal_serials,
        isbns: book_serials,
        volume: w.volume.as_deref(),
        issue: w.issue.as_deref(),
        page: w.page.as_deref(),
        article_number: w.article_number.as_deref(),
        part_number: w.part_number.as_deref(),
        version: w.version.as_ref().map(|v| &*v.version),
        status,
        aliases,
        publisher: w.publisher.as_deref().map(Interned::Text),
        language,
        authors,
        institutions,
        references_count: w.references_count,
        cited_by_count: w.is_referenced_by_count,
        licenses,
        updates_to,
        updated_by,
        relations,
        alternative_ids: w
            .alternative_id
            .iter()
            .flat_map(|a| a.iter())
            .map(|t| &**t)
            .collect(),
        ascii_doi: doi.ascii_spelling(),
        doi: doi.clone(),
    });

    for r in references {
        if r.doi.is_some() != r.doi_asserted_by.is_some() {
            return Err(ctx.err(
                "reference",
                &r.key,
                ErrorKind::Inconsistent("DOI and doi-asserted-by must appear together"),
            ));
        }
        let cited = r.doi.as_deref().map(|d| {
            Claim::from_parse(
                d,
                Doi::read(d).map(|(cited, readings)| {
                    for how in readings.made() {
                        emit.corrected(&doi, "reference.DOI", how);
                    }
                    cited
                }),
            )
        });
        emit.reference(
            &doi,
            &Reference {
                key: &r.key,
                doi: cited,
                asserted_by: r.doi_asserted_by,
                unstructured: r.unstructured.as_deref(),
                year: r.year.as_deref(),
                article_title: r.article_title.as_deref(),
                journal_title: r.journal_title.as_deref(),
                volume_title: r.volume_title.as_deref(),
                series_title: r.series_title.as_deref(),
                author: r.author.as_deref(),
                volume: r.volume.as_deref(),
                issue: r.issue.as_deref(),
                first_page: r.first_page.as_deref(),
            },
        );
    }
    Ok(())
}
