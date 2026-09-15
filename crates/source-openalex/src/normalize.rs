//! Native work → normalized records.
//!
//! Value-level strictness (plan §6) over what the census of the part file
//! justified (plan §4a). Every identifier the snapshot writes is a strict
//! type: OpenAlex entity ids by letter, DOIs, PMIDs, ORCIDs (every one
//! passed its checksum), RORs (every one passed), ISSNs, MeSH UIs,
//! country and currency codes, ISO dates and millisecond timestamps. Four
//! kinds of value are [`Claim`]s because the sample showed the data itself
//! wrong in a known, countable way: fifteen DOIs that are the bare
//! `https://doi.org/`, four ISSNs with a wrong check digit, one deprecated
//! language code (`sh`), and — not a claim but a rule — Cebuano written as
//! `ceb` because ISO 639-1 has no code for it.
//!
//! Fields the census showed to be derivable from others are checked and
//! not stored: `ids.openalex`, `ids.doi`, `display_name`,
//! `publication_year`, `authors_count`, `corresponding_author_ids`,
//! `corresponding_institution_ids`, `countries_distinct_count`,
//! `institutions_distinct_count`, `locations_count`, `primary_location`,
//! `best_oa_location` (always the first open location), `primary_topic`,
//! `open_access.is_oa`, `license_id`, `is_accepted`, `is_published`,
//! `is_paratext`, `referenced_works_count`, each award's
//! `funder_display_name` and each source's `host_organization`. A record
//! that breaks one of those stops the run with its id, the field and the
//! raw text.
//!
//! Records are pushed to a [`Sink`] by reference as they are built; the
//! citation edges follow the work, one call each.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::fmt;
use std::num::NonZeroU32;

use ingest::Sink;
use model::id::{
    AuthorId, AwardId, ConceptId, EntityId, FunderId, HostOrganization, InstitutionId, SourceId,
    TopicId, strict_u32,
};
use model::{
    Claim, CodeError, CountryCode, CurrencyCode, Date, DateError, DateTime, Doi, DoiError,
    Interned, Issn, IssnError, Lang, LangError, MeshUi, MeshUiError, OpenAlexId, Orcid, OrcidError,
    ParseError, Pmid, Ror, RorError, Year,
};

use crate::OpenAlex;
use crate::record::{
    self, BestOaLocation, CitationNormalizedPercentile, HasContent, PercentileYear, Text,
    Work as Native,
};
use crate::vocab::{
    AuthorPosition, Index, InstitutionType, License, OaStatus, Provenance, SourceType, Version,
    WorkType,
};

/// A list with room for its usual length inside the record itself (plan
/// §5, rule 1); only a longer one allocates. The lengths come from the
/// observed distributions in the plan's data, about the 90th percentile.
pub type Few<T, const N: usize> = SmallVec<[T; N]>;

/// A DOI as the source asserted it.
pub type DoiClaim<'n> = Claim<'n, Doi<'n>, DoiError>;
/// An ISSN as the source asserted it.
pub type IssnClaim<'n> = Claim<'n, Issn, IssnError>;
/// A language code as the source asserted it.
pub type LangClaim<'n> = Claim<'n, Lang, LangError>;

const KEYWORD_PREFIX: &str = "https://openalex.org/keywords/";
const SUBFIELD_PREFIX: &str = "https://openalex.org/subfields/";
const FIELD_PREFIX: &str = "https://openalex.org/fields/";
const DOMAIN_PREFIX: &str = "https://openalex.org/domains/";
const LICENSE_PREFIX: &str = "https://openalex.org/licenses/";
const SDG_PREFIX: &str = "https://metadata.un.org/sdg/";
const WIKIDATA_PREFIX: &str = "https://www.wikidata.org/wiki/Q";

/// One OpenAlex work, normalized.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work<'n> {
    #[allow(missing_docs)]
    pub id: OpenAlexId,
    /// `doi`, already lowercase in the snapshot; 15 of 9,136 in the sample
    /// were the bare prefix with nothing after it.
    #[serde(borrow)]
    pub doi: Option<DoiClaim<'n>>,
    /// `ids.pmid`.
    pub pmid: Option<Pmid>,
    /// `ids.mag`: the Microsoft Academic Graph id, which is the `W` number
    /// on all but 2 of 10,471.
    pub mag: Option<NonZeroU32>,
    /// `title`, raw; one empty, two with inline markup in the sample;
    /// absent on a paratext work that has none (`W4288097146`, which has
    /// no `display_name` and no `is_retracted` either).
    pub title: Option<&'n str>,
    /// `publication_date`, a full date when there is one; 32 DOAJ-indexed
    /// works in the 2025-10-17 part had none, and no year either.
    pub published: Option<Date>,
    /// `language`: ISO 639-1, or ISO 639-2 for a language without a
    /// two-letter code.
    #[serde(borrow)]
    pub language: Option<LangClaim<'n>>,
    /// `type`.
    pub kind: WorkType,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub open_access: OpenAccess<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub authorships: Few<Authorship<'n>, 8>,
    /// `apc_list`.
    pub apc_list: Option<Apc>,
    /// `apc_paid`.
    pub apc_paid: Option<Apc>,
    /// Field-weighted citation impact.
    pub fwci: Option<f64>,
    /// Independent of `has_content` in 1,993 of 10,873.
    pub has_fulltext: bool,
    #[allow(missing_docs)]
    pub has_content: HasContent,
    #[allow(missing_docs)]
    pub cited_by_count: u32,
    #[allow(missing_docs)]
    pub citation_normalized_percentile: Option<CitationNormalizedPercentile>,
    #[allow(missing_docs)]
    pub cited_by_percentile_year: Option<PercentileYear>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub biblio: Biblio<'n>,
    /// Absent when OpenAlex did not say (a paratext work).
    pub is_retracted: Option<bool>,
    /// Undocumented.
    pub is_xpac: bool,
    /// `topics`, best first; `primary_topic` is the first.
    #[serde(borrow)]
    pub topics: Few<Topic<'n>, 3>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub keywords: Few<Keyword<'n>, 8>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub concepts: Few<Concept<'n>, 16>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub mesh: Few<Mesh<'n>, 4>,
    /// `locations`, the first being `primary_location`.
    #[serde(borrow)]
    pub locations: Few<Location<'n>, 4>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub sustainable_development_goals: Few<Sdg<'n>, 2>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub awards: Few<Award<'n>, 2>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub funders: Few<Funder<'n>, 2>,
    /// How many edges follow this work through [`Sink::reference`].
    pub referenced_works_count: u32,
    #[allow(missing_docs)]
    pub related_works: Few<OpenAlexId, 20>,
    /// The inverted index, raw JSON.
    #[serde(borrow)]
    pub abstract_inverted_index: Option<&'n str>,
    #[allow(missing_docs)]
    pub counts_by_year: Few<YearCount, 8>,
    /// `created_date`.
    pub created: DateTime,
    /// `updated_date`, never before `created`.
    pub updated: DateTime,
    #[allow(missing_docs)]
    pub indexed_in: IndexedIn,
}

impl<'n> Work<'n> {
    /// `best_oa_location`: the first open location, which is what the
    /// snapshot's field always was.
    #[must_use]
    pub fn best_oa_location(&self) -> Option<&Location<'n>> {
        self.locations.iter().find(|l| l.is_oa)
    }

    /// `is_paratext`, which is `type == paratext`.
    #[must_use]
    pub const fn is_paratext(&self) -> bool {
        matches!(self.kind, WorkType::Paratext)
    }
}

/// `open_access`, the Unpaywall facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAccess<'n> {
    #[allow(missing_docs)]
    pub status: OaStatus,
    /// One of the best open location's two URLs, when set.
    #[serde(borrow)]
    pub oa_url: Option<&'n str>,
    /// Absent on 474 gold works in the sample.
    pub any_repository_has_fulltext: Option<bool>,
}

impl OpenAccess<'_> {
    /// `is_oa`, which is `oa_status != closed`.
    #[must_use]
    pub const fn is_oa(&self) -> bool {
        !matches!(self.status, OaStatus::Closed)
    }
}

/// An authorship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authorship<'n> {
    /// Not derivable from the index: 30 single-author works say `last`.
    pub position: AuthorPosition,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub author: Author<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub institutions: Few<Institution<'n>, 2>,
    /// A superset of the institutions' countries.
    pub countries: Few<CountryCode, 2>,
    #[allow(missing_docs)]
    pub is_corresponding: bool,
    #[allow(missing_docs)]
    pub raw_author_name: &'n str,
    /// Not always the strings inside `affiliations` (84 of 30,825).
    pub raw_affiliation_strings: Few<&'n str, 2>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub affiliations: Few<Affiliation<'n>, 2>,
    /// The ORCID as deposited; differs from the resolved one on 5, and
    /// each occurs without the other.
    pub raw_orcid: Option<Orcid>,
}

/// The resolved author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author<'n> {
    /// Null on 1,321 of 30,825.
    pub id: Option<AuthorId>,
    #[allow(missing_docs)]
    pub name: &'n str,
    #[allow(missing_docs)]
    pub orcid: Option<Orcid>,
}

/// A resolved institution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Institution<'n> {
    #[allow(missing_docs)]
    pub id: InstitutionId,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
    /// Absent for an institution OpenAlex has not matched to ROR; the
    /// 2025-07-29 part had one such, with no `type` either.
    pub ror: Option<Ror>,
    #[allow(missing_docs)]
    pub country: Option<CountryCode>,
    /// Absent together with `ror`.
    pub kind: Option<InstitutionType>,
    /// The institution and its parents, the institution not always first.
    pub lineage: Few<InstitutionId, 4>,
}

/// A raw affiliation string and the institutions it resolved to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Affiliation<'n> {
    #[allow(missing_docs)]
    pub raw: &'n str,
    #[allow(missing_docs)]
    pub institution_ids: Few<InstitutionId, 2>,
}

/// An article processing charge, list or paid; the paid one arrives as
/// floats that have always been whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Apc {
    #[allow(missing_docs)]
    pub value: u32,
    #[allow(missing_docs)]
    pub currency: CurrencyCode,
    /// Absent when the currency has no conversion (`IRR`).
    pub value_usd: Option<u32>,
}

/// `biblio`, raw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Biblio<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issue: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub first_page: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub last_page: Option<&'n str>,
}

/// A topic with its subfield, field and domain.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Topic<'n> {
    #[allow(missing_docs)]
    pub id: TopicId,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
    #[allow(missing_docs)]
    pub score: f64,
    /// `subfields/NNNN`.
    #[serde(borrow)]
    pub subfield: (u16, Interned<'n>),
    /// `fields/NN`.
    #[serde(borrow)]
    pub field: (u8, Interned<'n>),
    /// `domains/N`.
    #[serde(borrow)]
    pub domain: (u8, Interned<'n>),
}

/// A keyword: the slug of `keywords/<slug>`, its name and score.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Keyword<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub slug: Interned<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
    #[allow(missing_docs)]
    pub score: f64,
}

/// A (deprecated) concept.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Concept<'n> {
    #[allow(missing_docs)]
    pub id: ConceptId,
    /// The `Q` number.
    pub wikidata: u32,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
    #[allow(missing_docs)]
    pub level: u8,
    #[allow(missing_docs)]
    pub score: f64,
}

/// A MeSH heading, one qualifier per entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mesh<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub descriptor: MeshUi<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub descriptor_name: Interned<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub qualifier: Option<Qualifier<'n>>,
    #[allow(missing_docs)]
    pub major: bool,
}

/// A MeSH qualifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qualifier<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub ui: MeshUi<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
}

/// A location: one copy of the work somewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub id: LocationId<'n>,
    #[allow(missing_docs)]
    pub is_oa: bool,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub landing_page_url: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub pdf_url: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub source: Option<Source<'n>>,
    #[allow(missing_docs)]
    pub license: Option<License>,
    /// `is_published` and `is_accepted` follow from it.
    pub version: Option<Version>,
    /// Sometimes the empty string.
    #[serde(borrow)]
    pub raw_type: Option<Interned<'n>>,
    /// Sometimes the empty string.
    #[serde(borrow)]
    pub raw_source_name: Option<Interned<'n>>,
    #[allow(missing_docs)]
    pub provenance: Provenance,
}

/// A location's `id`, whose form is its `provenance`'s. None of the three
/// typed forms is necessarily this work's own identifier: merged duplicates
/// keep their MAG id, PMID or DOI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LocationId<'n> {
    /// `provenance: mag`.
    Mag(NonZeroU32),
    /// `provenance: pubmed`.
    Pmid(Pmid),
    /// `provenance: crossref` or `datacite`, a bare DOI.
    Doi(#[serde(borrow)] Doi<'n>),
    /// `provenance: repo`: `doi:…`, `oai:…`, digits, or the repository's
    /// own string.
    Repo(&'n str),
}

/// A source: journal, repository, conference, …
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source<'n> {
    #[allow(missing_docs)]
    pub id: SourceId,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
    /// The linking ISSN, always among `issns` when present.
    pub issn_l: Option<Issn>,
    /// `issn`, null and `[]` both meaning none; four in the sample had a
    /// wrong check digit.
    #[serde(borrow)]
    pub issns: Few<IssnClaim<'n>, 2>,
    #[allow(missing_docs)]
    pub is_oa: bool,
    #[allow(missing_docs)]
    pub is_in_doaj: bool,
    #[allow(missing_docs)]
    pub is_core: bool,
    /// The host organization is the first of the lineage.
    pub host_organization_lineage: Few<HostOrganization, 2>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub host_organization_name: Option<Interned<'n>>,
    /// Not parallel to the lineage: usually shorter.
    #[serde(borrow)]
    pub host_organization_lineage_names: Few<Interned<'n>, 2>,
    #[allow(missing_docs)]
    pub kind: SourceType,
}

impl Source<'_> {
    /// `host_organization`.
    #[must_use]
    pub fn host_organization(&self) -> Option<HostOrganization> {
        self.host_organization_lineage.first().copied()
    }
}

/// A sustainable development goal, `1..=17`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sdg<'n> {
    #[allow(missing_docs)]
    pub goal: u8,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
    #[allow(missing_docs)]
    pub score: f64,
}

/// An award; its funder is always among the work's `funders`, with the
/// same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Award<'n> {
    #[allow(missing_docs)]
    pub id: AwardId,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Option<&'n str>,
    #[allow(missing_docs)]
    pub funder_award_id: &'n str,
    #[allow(missing_docs)]
    pub funder: FunderId,
}

/// A funder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Funder<'n> {
    #[allow(missing_docs)]
    pub id: FunderId,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
    #[allow(missing_docs)]
    pub ror: Option<Ror>,
}

/// One year's citations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearCount {
    #[allow(missing_docs)]
    pub year: Year,
    #[allow(missing_docs)]
    pub cited_by_count: u32,
}

/// `indexed_in`, as one bit per index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct IndexedIn(u8);

impl IndexedIn {
    const fn bit(index: Index) -> u8 {
        1 << index as u8
    }

    /// Whether the work is in that index.
    #[must_use]
    pub const fn contains(self, index: Index) -> bool {
        self.0 & Self::bit(index) != 0
    }

    /// How many indexes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// No index at all, as for 6,728 of 10,873.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The indexes, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = Index> {
        Index::ALL.into_iter().filter(move |i| self.contains(*i))
    }
}

/// A value that did not fit the model.
#[derive(Debug)]
pub struct Error {
    /// The work, once its id was readable.
    pub id: Option<OpenAlexId>,
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
    Id(ParseError),
    Doi(DoiError),
    Orcid(OrcidError),
    Issn(IssnError),
    Ror(RorError),
    Date(DateError),
    Mesh(MeshUiError),
    Code(CodeError),
    /// A number outside the range its documentation gives.
    Range(&'static str),
    /// Two fields that must agree do not.
    Inconsistent(&'static str),
    /// Present in the data, not modelled here yet.
    Unmodelled(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.id {
            Some(id) => write!(f, "{id}: ")?,
            None => f.write_str("id unknown: ")?,
        }
        write!(f, "{} = {:?}: ", self.field, self.raw)?;
        fmt::Display::fmt(&self.kind, f)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Id(e) => write!(f, "{e}"),
            ErrorKind::Doi(e) => write!(f, "{e}"),
            ErrorKind::Orcid(e) => write!(f, "{e}"),
            ErrorKind::Issn(e) => write!(f, "{e}"),
            ErrorKind::Ror(e) => write!(f, "{e}"),
            ErrorKind::Date(e) => write!(f, "{e}"),
            ErrorKind::Mesh(e) => write!(f, "{e}"),
            ErrorKind::Code(e) => write!(f, "{e}"),
            ErrorKind::Range(what) => write!(f, "out of range: {what}"),
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

/// OpenAlex writes ISO 639-1 where the language has a two-letter code and
/// ISO 639-2 where it does not (`ceb`, Cebuano). A three-letter code for a
/// language that has a two-letter one would be the form changing, and
/// fails; so does anything outside ISO 639-2, such as the deprecated `sh`.
fn language(code: &str) -> Result<Lang, LangError> {
    match Lang::from_alpha_2(code) {
        Ok(lang) => Ok(lang),
        Err(e) => {
            let lang = Lang::from_terminologic(code)?;
            if lang.alpha_2().is_some() {
                Err(e)
            } else {
                Ok(lang)
            }
        }
    }
}

/// True when `best_oa_location` is exactly this location. Both borrow
/// from the same record: an inline list is invariant in its lifetime, so
/// the two must be spelled with one.
fn best_matches<'a>(b: &BestOaLocation<'a>, l: &record::Location<'a>) -> bool {
    b.id.as_deref() == Some(&*l.id)
        && b.is_oa == Some(l.is_oa)
        && b.landing_page_url == l.landing_page_url
        && b.pdf_url == l.pdf_url
        && b.source == l.source
        && b.license == l.license
        && b.license_id == l.license_id
        && b.version == l.version
        && b.is_accepted == Some(l.is_accepted)
        && b.is_published == l.is_published
        && b.raw_type == l.raw_type
        && b.raw_source_name == l.raw_source_name
        && b.provenance == Some(l.provenance)
}

/// True when `best_oa_location` is the object of nulls.
fn best_is_null(b: &BestOaLocation<'_>) -> bool {
    b.id.is_none()
        && b.is_oa.is_none()
        && b.landing_page_url.is_none()
        && b.pdf_url.is_none()
        && b.source.is_none()
        && b.license.is_none()
        && b.license_id.is_none()
        && b.version.is_none()
        && b.is_accepted.is_none()
        && b.is_published.is_none()
        && b.raw_type.is_none()
        && b.raw_source_name.is_none()
        && b.provenance.is_none()
}

fn strs<'n, const N: usize>(list: &'n [Text<'_>]) -> Few<&'n str, N> {
    list.iter().map(|t| &**t).collect()
}

fn interned<'n, const N: usize>(list: &'n [Text<'_>]) -> Few<Interned<'n>, N> {
    list.iter().map(|t| Interned::Text(t)).collect()
}

struct Ctx {
    id: Option<OpenAlexId>,
}

impl Ctx {
    fn err(&self, field: &'static str, raw: &str, kind: ErrorKind) -> Error {
        Error {
            id: self.id,
            field,
            raw: raw.to_owned(),
            kind,
        }
    }

    fn inconsistent(&self, field: &'static str, raw: &str, what: &'static str) -> Error {
        self.err(field, raw, ErrorKind::Inconsistent(what))
    }

    fn entity<const K: char>(&self, field: &'static str, s: &str) -> Result<EntityId<K>, Error> {
        EntityId::from_url(s).map_err(|e| self.err(field, s, ErrorKind::Id(e)))
    }

    fn entities<const K: char, const N: usize>(
        &self,
        field: &'static str,
        list: &[Text<'_>],
    ) -> Result<Few<EntityId<K>, N>, Error> {
        list.iter().map(|s| self.entity(field, s)).collect()
    }

    /// The number after a fixed URL prefix.
    fn numbered(&self, field: &'static str, s: &str, prefix: &'static str) -> Result<u32, Error> {
        let rest = s
            .strip_prefix(prefix)
            .ok_or_else(|| self.err(field, s, ErrorKind::Id(ParseError::BadPrefix)))?;
        strict_u32(rest).map_err(|e| self.err(field, s, ErrorKind::Id(e.shift(prefix.len()))))
    }

    fn narrow<T: TryFrom<u32>>(
        &self,
        field: &'static str,
        s: &str,
        n: u32,
        what: &'static str,
    ) -> Result<T, Error> {
        T::try_from(n).map_err(|_| self.err(field, s, ErrorKind::Range(what)))
    }

    fn stamp(&self, field: &'static str, s: &str) -> Result<DateTime, Error> {
        DateTime::parse_utc_millis(s).map_err(|e| self.err(field, s, ErrorKind::Date(e)))
    }

    fn orcid(&self, field: &'static str, s: &str) -> Result<Orcid, Error> {
        Orcid::from_https_url(s).map_err(|e| self.err(field, s, ErrorKind::Orcid(e)))
    }

    fn whole(&self, field: &'static str, v: f64) -> Result<u32, Error> {
        if v.fract() != 0.0 || v < 0.0 || v > f64::from(u32::MAX) {
            return Err(self.err(
                field,
                &v.to_string(),
                ErrorKind::Unmodelled("an APC that is not a whole number"),
            ));
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // checked above
        let whole = v as u32;
        Ok(whole)
    }

    fn apc(
        &self,
        field: &'static str,
        value: f64,
        currency: &str,
        value_usd: Option<f64>,
    ) -> Result<Apc, Error> {
        Ok(Apc {
            value: self.whole(field, value)?,
            currency: CurrencyCode::parse(currency)
                .map_err(|e| self.err(field, currency, ErrorKind::Code(e)))?,
            value_usd: value_usd.map(|v| self.whole(field, v)).transpose()?,
        })
    }

    fn institution<'n>(&self, i: &'n record::Institution<'_>) -> Result<Institution<'n>, Error> {
        let id = self.entity("institutions.id", &i.id)?;
        let lineage = self.entities("institutions.lineage", &i.lineage)?;
        if !lineage.contains(&id) {
            return Err(self.inconsistent(
                "institutions.lineage",
                &i.id,
                "lineage does not contain the institution",
            ));
        }
        Ok(Institution {
            id,
            name: Interned::Text(&i.display_name),
            ror: i
                .ror
                .as_deref()
                .map(|r| {
                    Ror::from_url(r).map_err(|e| self.err("institutions.ror", r, ErrorKind::Ror(e)))
                })
                .transpose()?,
            country: i
                .country_code
                .as_deref()
                .map(|c| {
                    CountryCode::parse(c)
                        .map_err(|e| self.err("institutions.country_code", c, ErrorKind::Code(e)))
                })
                .transpose()?,
            kind: i.r#type,
            lineage,
        })
    }

    fn authorship<'n>(&self, a: &'n record::Authorship<'_>) -> Result<Authorship<'n>, Error> {
        let author = Author {
            id: a
                .author
                .id
                .as_deref()
                .map(|s| self.entity("author.id", s))
                .transpose()?,
            name: &a.author.display_name,
            orcid: a
                .author
                .orcid
                .as_deref()
                .map(|o| self.orcid("author.orcid", o))
                .transpose()?,
        };
        let countries: Few<CountryCode, 2> = a
            .countries
            .iter()
            .map(|c| {
                CountryCode::parse(c).map_err(|e| self.err("countries", c, ErrorKind::Code(e)))
            })
            .collect::<Result<_, _>>()?;
        let institutions: Few<Institution<'n>, 2> = a
            .institutions
            .iter()
            .map(|i| self.institution(i))
            .collect::<Result<_, _>>()?;
        for i in &institutions {
            if let Some(c) = i.country
                && !countries.contains(&c)
            {
                return Err(self.inconsistent(
                    "countries",
                    c.as_str(),
                    "an institution's country is not among the authorship's",
                ));
            }
        }
        let affiliations = a
            .affiliations
            .iter()
            .map(|x| {
                Ok(Affiliation {
                    raw: &x.raw_affiliation_string,
                    institution_ids: self
                        .entities("affiliations.institution_ids", &x.institution_ids)?,
                })
            })
            .collect::<Result<_, Error>>()?;
        Ok(Authorship {
            position: a.author_position,
            author,
            institutions,
            countries,
            is_corresponding: a.is_corresponding,
            raw_author_name: &a.raw_author_name,
            raw_affiliation_strings: strs(&a.raw_affiliation_strings),
            affiliations,
            raw_orcid: a
                .raw_orcid
                .as_deref()
                .map(|o| self.orcid("raw_orcid", o))
                .transpose()?,
        })
    }

    fn source<'n>(&self, s: &'n record::Source<'_>) -> Result<Source<'n>, Error> {
        let issn_l = s
            .issn_l
            .as_deref()
            .map(|x| Issn::parse(x).map_err(|e| self.err("source.issn_l", x, ErrorKind::Issn(e))))
            .transpose()?;
        let issns: Few<IssnClaim<'n>, 2> = s
            .issn
            .iter()
            .flatten()
            .map(|x| Claim::from_parse(x, Issn::parse(x)))
            .collect();
        if let Some(l) = issn_l
            && !issns.iter().any(|c| c.valid() == Some(&l))
        {
            return Err(self.inconsistent(
                "source.issn_l",
                &l.to_string(),
                "issn_l is not among issn",
            ));
        }
        let lineage = s
            .host_organization_lineage
            .iter()
            .map(|h| {
                HostOrganization::from_url(h)
                    .map_err(|e| self.err("source.host_organization_lineage", h, ErrorKind::Id(e)))
            })
            .collect::<Result<_, _>>()?;
        if s.host_organization.as_deref() != s.host_organization_lineage.first().map(|t| &**t) {
            return Err(self.inconsistent(
                "source.host_organization",
                s.host_organization.as_deref().unwrap_or(""),
                "host_organization is not the first of its lineage",
            ));
        }
        Ok(Source {
            id: self.entity("source.id", &s.id)?,
            name: Interned::Text(&s.display_name),
            issn_l,
            issns,
            is_oa: s.is_oa,
            is_in_doaj: s.is_in_doaj,
            is_core: s.is_core,
            host_organization_lineage: lineage,
            host_organization_name: s.host_organization_name.as_deref().map(Interned::Text),
            host_organization_lineage_names: interned(&s.host_organization_lineage_names),
            kind: s.r#type,
        })
    }

    fn location<'n>(&self, l: &'n record::Location<'_>) -> Result<Location<'n>, Error> {
        let id = match l.provenance {
            Provenance::Mag => strict_u32(&l.id)
                .ok()
                .and_then(NonZeroU32::new)
                .map(LocationId::Mag)
                .ok_or_else(|| {
                    self.err(
                        "locations.id",
                        &l.id,
                        ErrorKind::Id(strict_u32(&l.id).err().unwrap_or(ParseError::Zero)),
                    )
                })?,
            Provenance::Pubmed => LocationId::Pmid(
                Pmid::from_digits(&l.id)
                    .map_err(|e| self.err("locations.id", &l.id, ErrorKind::Id(e)))?,
            ),
            Provenance::Crossref | Provenance::Datacite => LocationId::Doi(
                Doi::parse(&l.id)
                    .map_err(|e| self.err("locations.id", &l.id, ErrorKind::Doi(e)))?,
            ),
            Provenance::Repo => LocationId::Repo(&l.id),
        };
        match (l.license, l.license_id.as_deref()) {
            (None, None) => {}
            (Some(license), Some(id))
                if id.strip_prefix(LICENSE_PREFIX) == Some(license.token()) => {}
            (_, id) => {
                return Err(self.inconsistent(
                    "locations.license_id",
                    id.unwrap_or(""),
                    "license_id is not licenses/ + license",
                ));
            }
        }
        if l.is_published != l.version.map(|v| v == Version::Published) {
            return Err(self.inconsistent(
                "locations.is_published",
                &format!("{:?}", l.is_published),
                "is_published disagrees with version",
            ));
        }
        if l.is_accepted != matches!(l.version, Some(Version::Accepted | Version::Published)) {
            return Err(self.inconsistent(
                "locations.is_accepted",
                &l.is_accepted.to_string(),
                "is_accepted disagrees with version",
            ));
        }
        Ok(Location {
            id,
            is_oa: l.is_oa,
            landing_page_url: l.landing_page_url.as_deref(),
            pdf_url: l.pdf_url.as_deref(),
            source: l.source.as_ref().map(|s| self.source(s)).transpose()?,
            license: l.license,
            version: l.version,
            raw_type: l.raw_type.as_deref().map(Interned::Text),
            raw_source_name: l.raw_source_name.as_deref().map(Interned::Text),
            provenance: l.provenance,
        })
    }

    fn topic<'n>(&self, t: &'n record::Topic<'_>) -> Result<Topic<'n>, Error> {
        let subfield = self.numbered("topics.subfield.id", &t.subfield.id, SUBFIELD_PREFIX)?;
        let field = self.numbered("topics.field.id", &t.field.id, FIELD_PREFIX)?;
        let domain = self.numbered("topics.domain.id", &t.domain.id, DOMAIN_PREFIX)?;
        Ok(Topic {
            id: self.entity("topics.id", &t.id)?,
            name: Interned::Text(&t.display_name),
            score: t.score,
            subfield: (
                self.narrow(
                    "topics.subfield.id",
                    &t.subfield.id,
                    subfield,
                    "subfield above u16",
                )?,
                Interned::Text(&t.subfield.display_name),
            ),
            field: (
                self.narrow("topics.field.id", &t.field.id, field, "field above u8")?,
                Interned::Text(&t.field.display_name),
            ),
            domain: (
                self.narrow("topics.domain.id", &t.domain.id, domain, "domain above u8")?,
                Interned::Text(&t.domain.display_name),
            ),
        })
    }

    fn mesh<'n>(&self, m: &'n record::Mesh<'_>) -> Result<Mesh<'n>, Error> {
        let qualifier = match (m.qualifier_ui.as_deref(), m.qualifier_name.as_deref()) {
            (None, None) => None,
            (Some(ui), Some(name)) => Some(Qualifier {
                ui: MeshUi::parse(ui)
                    .map_err(|e| self.err("mesh.qualifier_ui", ui, ErrorKind::Mesh(e)))?,
                name: Interned::Text(name),
            }),
            (ui, name) => {
                return Err(self.inconsistent(
                    "mesh.qualifier_ui",
                    ui.or(name).unwrap_or(""),
                    "qualifier_ui and qualifier_name must appear together",
                ));
            }
        };
        Ok(Mesh {
            descriptor: MeshUi::parse(&m.descriptor_ui).map_err(|e| {
                self.err("mesh.descriptor_ui", &m.descriptor_ui, ErrorKind::Mesh(e))
            })?,
            descriptor_name: Interned::Text(&m.descriptor_name),
            qualifier,
            major: m.is_major_topic,
        })
    }
}

/// Normalizes one work, emitting it and then its citation edges.
///
/// # Errors
///
/// The first value that does not fit the model; see [`ErrorKind`]. The
/// emitter may already have received the work when an edge fails.
#[allow(clippy::too_many_lines)] // one block per field group; the field set is the point
pub fn normalize<'n, K: Sink<'n, OpenAlex>>(w: &'n Native<'_>, emit: &mut K) -> Result<(), Error> {
    let mut ctx = Ctx { id: None };
    let id: OpenAlexId = ctx.entity("id", &w.id)?;
    ctx.id = Some(id);
    if w.ids.openalex != w.id {
        return Err(ctx.inconsistent("ids.openalex", &w.ids.openalex, "differs from id"));
    }
    if w.ids.doi != w.doi {
        return Err(ctx.inconsistent(
            "ids.doi",
            w.ids.doi.as_deref().unwrap_or(""),
            "differs from doi",
        ));
    }
    let doi = w
        .doi
        .as_deref()
        .map(|d| {
            d.strip_prefix(Doi::URL_PREFIX)
                .map(|bare| Claim::from_parse(bare, Doi::parse(bare)))
                .ok_or_else(|| ctx.err("doi", d, ErrorKind::Doi(DoiError::BadPrefix)))
        })
        .transpose()?;
    let pmid = w
        .ids
        .pmid
        .as_deref()
        .map(|p| Pmid::from_url(p).map_err(|e| ctx.err("ids.pmid", p, ErrorKind::Id(e))))
        .transpose()?;
    if let Some(pmcid) = w.ids.pmcid.as_deref() {
        return Err(ctx.err("ids.pmcid", pmcid, ErrorKind::Unmodelled("pmcid")));
    }
    let mag = w
        .ids
        .mag
        .as_deref()
        .map(|m| {
            strict_u32(m).ok().and_then(NonZeroU32::new).ok_or_else(|| {
                ctx.err(
                    "ids.mag",
                    m,
                    ErrorKind::Id(strict_u32(m).err().unwrap_or(ParseError::Zero)),
                )
            })
        })
        .transpose()?;

    let published = w
        .publication_date
        .as_deref()
        .map(|d| Date::parse_iso(d).map_err(|e| ctx.err("publication_date", d, ErrorKind::Date(e))))
        .transpose()?;
    match (published, w.publication_year) {
        (Some(date), Some(year)) if date.year().get() == year => {}
        (None, None) => {}
        (_, year) => {
            return Err(ctx.inconsistent(
                "publication_year",
                &year.map_or_else(|| "null".to_owned(), |y| y.to_string()),
                "differs from publication_date",
            ));
        }
    }
    if w.display_name != w.title {
        return Err(ctx.inconsistent(
            "display_name",
            w.display_name.as_deref().unwrap_or("null"),
            "differs from title",
        ));
    }
    let created = ctx.stamp("created_date", &w.created_date)?;
    let updated = ctx.stamp("updated_date", &w.updated_date)?;
    if updated < created {
        return Err(ctx.inconsistent("updated_date", &w.updated_date, "before created_date"));
    }
    let language = w
        .language
        .as_deref()
        .map(|l| Claim::from_parse(l, language(l)));
    if w.is_paratext != (w.r#type == WorkType::Paratext) {
        return Err(ctx.inconsistent(
            "is_paratext",
            &w.is_paratext.to_string(),
            "disagrees with type",
        ));
    }
    if !w.institutions.is_empty() {
        return Err(ctx.err(
            "institutions",
            "",
            ErrorKind::Unmodelled("a top-level institutions entry"),
        ));
    }

    // Authorships, and the five fields derived from them.
    if usize::try_from(w.authors_count).ok() != Some(w.authorships.len()) {
        return Err(ctx.inconsistent(
            "authors_count",
            &w.authors_count.to_string(),
            "differs from the authorships array",
        ));
    }
    let authorships = w
        .authorships
        .iter()
        .map(|a| ctx.authorship(a))
        .collect::<Result<_, _>>()?;
    let corresponding = w
        .authorships
        .iter()
        .filter(|a| a.is_corresponding)
        .filter_map(|a| a.author.id.as_deref());
    let mut n = 0;
    for aid in corresponding {
        n += 1;
        if !w.corresponding_author_ids.iter().any(|c| **c == *aid) {
            return Err(ctx.inconsistent(
                "corresponding_author_ids",
                aid,
                "a corresponding author is missing from it",
            ));
        }
    }
    if n != w.corresponding_author_ids.len() {
        return Err(ctx.inconsistent(
            "corresponding_author_ids",
            &w.corresponding_author_ids.len().to_string(),
            "differs from the corresponding authorships",
        ));
    }
    let mut distinct_corresponding_institutions = 0;
    let mut distinct_institutions = 0;
    let mut distinct_countries = 0;
    for (ai, a) in w.authorships.iter().enumerate() {
        for (ii, i) in a.institutions.iter().enumerate() {
            let earlier = |aj: usize, ij: usize| aj < ai || (aj == ai && ij < ii);
            let seen = |pred: &dyn Fn(&record::Authorship<'_>) -> bool| {
                w.authorships.iter().enumerate().any(|(aj, b)| {
                    pred(b)
                        && b.institutions
                            .iter()
                            .enumerate()
                            .any(|(ij, j)| earlier(aj, ij) && j.id == i.id)
                })
            };
            if !seen(&|_| true) {
                distinct_institutions += 1;
            }
            if a.is_corresponding {
                if !w.corresponding_institution_ids.contains(&i.id) {
                    return Err(ctx.inconsistent(
                        "corresponding_institution_ids",
                        &i.id,
                        "a corresponding author's institution is missing from it",
                    ));
                }
                if !seen(&|b| b.is_corresponding) {
                    distinct_corresponding_institutions += 1;
                }
            }
            if let Some(c) = &i.country_code
                && !w.authorships.iter().enumerate().any(|(aj, b)| {
                    b.institutions
                        .iter()
                        .enumerate()
                        .any(|(ij, j)| earlier(aj, ij) && j.country_code.as_ref() == Some(c))
                })
            {
                distinct_countries += 1;
            }
        }
    }
    if distinct_corresponding_institutions != w.corresponding_institution_ids.len() {
        return Err(ctx.inconsistent(
            "corresponding_institution_ids",
            &w.corresponding_institution_ids.len().to_string(),
            "differs from the corresponding authorships' institutions",
        ));
    }
    if usize::try_from(w.institutions_distinct_count).ok() != Some(distinct_institutions) {
        return Err(ctx.inconsistent(
            "institutions_distinct_count",
            &w.institutions_distinct_count.to_string(),
            "differs from the distinct institutions",
        ));
    }
    if usize::try_from(w.countries_distinct_count).ok() != Some(distinct_countries) {
        return Err(ctx.inconsistent(
            "countries_distinct_count",
            &w.countries_distinct_count.to_string(),
            "differs from the institutions' distinct countries",
        ));
    }

    // Locations, open access, and the fields derived from them.
    if usize::try_from(w.locations_count).ok() != Some(w.locations.len()) {
        return Err(ctx.inconsistent(
            "locations_count",
            &w.locations_count.to_string(),
            "differs from the locations array",
        ));
    }
    if w.locations.first() != Some(&w.primary_location) {
        return Err(ctx.inconsistent(
            "primary_location",
            &w.primary_location.id,
            "is not locations[0]",
        ));
    }
    let locations = w
        .locations
        .iter()
        .map(|l| ctx.location(l))
        .collect::<Result<_, _>>()?;
    let first_oa = w.locations.iter().find(|l| l.is_oa);
    let best_ok = match first_oa {
        Some(l) => best_matches(&w.best_oa_location, l),
        None => best_is_null(&w.best_oa_location),
    };
    if !best_ok {
        return Err(ctx.inconsistent(
            "best_oa_location",
            w.best_oa_location.id.as_deref().unwrap_or(""),
            "is not the first open location",
        ));
    }
    let oa = &w.open_access;
    if oa.is_oa != (oa.oa_status != OaStatus::Closed) {
        return Err(ctx.inconsistent(
            "open_access.is_oa",
            &oa.is_oa.to_string(),
            "disagrees with oa_status",
        ));
    }
    if oa.is_oa != first_oa.is_some() {
        return Err(ctx.inconsistent(
            "open_access.is_oa",
            &oa.is_oa.to_string(),
            "disagrees with the locations",
        ));
    }
    if let Some(url) = oa.oa_url.as_deref()
        && !first_oa.is_some_and(|l| {
            l.pdf_url.as_deref() == Some(url) || l.landing_page_url.as_deref() == Some(url)
        })
    {
        return Err(ctx.inconsistent(
            "open_access.oa_url",
            url,
            "is not a URL of the best open location",
        ));
    }

    // Subjects.
    if w.primary_topic.as_ref() != w.topics.first() {
        return Err(ctx.inconsistent(
            "primary_topic",
            w.primary_topic.as_ref().map_or("", |t| &t.id),
            "is not topics[0]",
        ));
    }
    let topics = w
        .topics
        .iter()
        .map(|t| ctx.topic(t))
        .collect::<Result<_, _>>()?;
    let keywords = w
        .keywords
        .iter()
        .map(|k| {
            Ok(Keyword {
                slug: Interned::Text(k.id.strip_prefix(KEYWORD_PREFIX).ok_or_else(|| {
                    ctx.err("keywords.id", &k.id, ErrorKind::Id(ParseError::BadPrefix))
                })?),
                name: Interned::Text(&k.display_name),
                score: k.score,
            })
        })
        .collect::<Result<_, Error>>()?;
    let concepts = w
        .concepts
        .iter()
        .map(|c| {
            Ok(Concept {
                id: ctx.entity("concepts.id", &c.id)?,
                wikidata: ctx.numbered("concepts.wikidata", &c.wikidata, WIKIDATA_PREFIX)?,
                name: Interned::Text(&c.display_name),
                level: c.level,
                score: c.score,
            })
        })
        .collect::<Result<_, Error>>()?;
    let mesh = w
        .mesh
        .iter()
        .map(|m| ctx.mesh(m))
        .collect::<Result<_, _>>()?;
    let sustainable_development_goals = w
        .sustainable_development_goals
        .iter()
        .map(|g| {
            let goal = ctx.numbered("sustainable_development_goals.id", &g.id, SDG_PREFIX)?;
            if !(1..=17).contains(&goal) {
                return Err(ctx.err(
                    "sustainable_development_goals.id",
                    &g.id,
                    ErrorKind::Range("goal outside 1..=17"),
                ));
            }
            Ok(Sdg {
                goal: ctx.narrow("sustainable_development_goals.id", &g.id, goal, "goal")?,
                name: Interned::Text(&g.display_name),
                score: g.score,
            })
        })
        .collect::<Result<_, Error>>()?;

    // Funding.
    let funders: Few<Funder<'_>, 2> = w
        .funders
        .iter()
        .map(|f| {
            Ok(Funder {
                id: ctx.entity("funders.id", &f.id)?,
                name: Interned::Text(&f.display_name),
                ror: f
                    .ror
                    .as_deref()
                    .map(|r| {
                        Ror::from_url(r).map_err(|e| ctx.err("funders.ror", r, ErrorKind::Ror(e)))
                    })
                    .transpose()?,
            })
        })
        .collect::<Result<_, Error>>()?;
    let awards = w
        .awards
        .iter()
        .map(|a| {
            let funder: FunderId = ctx.entity("awards.funder_id", &a.funder_id)?;
            let known = funders.iter().find(|f| f.id == funder);
            if known.map(|f| f.name) != Some(Interned::Text(&a.funder_display_name)) {
                return Err(ctx.inconsistent(
                    "awards.funder_id",
                    &a.funder_id,
                    "the award's funder is not among funders with that name",
                ));
            }
            Ok(Award {
                id: ctx.entity("awards.id", &a.id)?,
                name: a.display_name.as_deref(),
                funder_award_id: &a.funder_award_id,
                funder,
            })
        })
        .collect::<Result<_, Error>>()?;

    // Citations.
    if usize::try_from(w.referenced_works_count).ok() != Some(w.referenced_works.len()) {
        return Err(ctx.inconsistent(
            "referenced_works_count",
            &w.referenced_works_count.to_string(),
            "differs from the referenced_works array",
        ));
    }
    let related_works = ctx.entities("related_works", &w.related_works)?;
    let counts_by_year = w
        .counts_by_year
        .iter()
        .map(|c| {
            Ok(YearCount {
                year: Year::new(c.year).ok_or_else(|| {
                    ctx.err(
                        "counts_by_year.year",
                        &c.year.to_string(),
                        ErrorKind::Date(DateError::YearRange),
                    )
                })?,
                cited_by_count: c.cited_by_count,
            })
        })
        .collect::<Result<_, Error>>()?;
    let mut indexed_in = IndexedIn::default();
    for i in &w.indexed_in {
        if indexed_in.contains(*i) {
            return Err(ctx.inconsistent("indexed_in", &format!("{i:?}"), "an index listed twice"));
        }
        indexed_in.0 |= IndexedIn::bit(*i);
    }

    emit.work(&Work {
        id,
        doi,
        pmid,
        mag,
        title: w.title.as_deref(),
        published,
        language,
        kind: w.r#type,
        open_access: OpenAccess {
            status: oa.oa_status,
            oa_url: oa.oa_url.as_deref(),
            any_repository_has_fulltext: oa.any_repository_has_fulltext,
        },
        authorships,
        apc_list: w
            .apc_list
            .as_ref()
            .map(|a| {
                ctx.apc(
                    "apc_list",
                    f64::from(a.value),
                    &a.currency,
                    a.value_usd.map(f64::from),
                )
            })
            .transpose()?,
        apc_paid: w
            .apc_paid
            .as_ref()
            .map(|a| ctx.apc("apc_paid", a.value, &a.currency, Some(a.value_usd)))
            .transpose()?,
        fwci: w.fwci,
        has_fulltext: w.has_fulltext,
        has_content: w.has_content,
        cited_by_count: w.cited_by_count,
        citation_normalized_percentile: w.citation_normalized_percentile,
        cited_by_percentile_year: w.cited_by_percentile_year,
        biblio: Biblio {
            volume: w.biblio.volume.as_deref(),
            issue: w.biblio.issue.as_deref(),
            first_page: w.biblio.first_page.as_deref(),
            last_page: w.biblio.last_page.as_deref(),
        },
        is_retracted: w.is_retracted,
        is_xpac: w.is_xpac,
        topics,
        keywords,
        concepts,
        mesh,
        locations,
        sustainable_development_goals,
        awards,
        funders,
        referenced_works_count: w.referenced_works_count,
        related_works,
        abstract_inverted_index: w.abstract_inverted_index.map(record::RawJson::as_str),
        counts_by_year,
        created,
        updated,
        indexed_in,
    });

    for r in &w.referenced_works {
        let to: OpenAlexId = ctx.entity("referenced_works", r)?;
        emit.reference(&id, &to);
    }
    Ok(())
}
