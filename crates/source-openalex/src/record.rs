//! Native record types: what an OpenAlex work object contains, as observed
//! in one 10,873-work part file (plan §4a). Every object rejects unknown
//! keys. Documentation notes how many of the 10,873 carried a field where
//! that is informative.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::vocab::{
    AuthorPosition, Index, InstitutionType, License, OaStatus, Provenance, SourceType, Version,
    WorkType,
};

/// A string borrowed from the input unless it contained an escape sequence.
pub type Text<'a> = Cow<'a, str>;

/// One work.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // the source's four flags, mirrored
pub struct Work<'a> {
    /// `https://openalex.org/W…`.
    #[serde(borrow)]
    pub id: Text<'a>,
    /// URL form; absent on 1,737 of 10,873.
    #[serde(borrow)]
    pub doi: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub title: Text<'a>,
    /// The same as `title` in the sample.
    #[serde(borrow)]
    pub display_name: Text<'a>,
    /// 1823..2026 in the sample.
    pub publication_year: u16,
    /// `YYYY-MM-DD`.
    #[serde(borrow)]
    pub publication_date: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub ids: Ids<'a>,
    /// ISO 639-1; absent on 20.
    #[serde(borrow)]
    pub language: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub primary_location: Location<'a>,
    #[serde(rename = "type")]
    #[allow(missing_docs)]
    pub r#type: WorkType,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub open_access: OpenAccess<'a>,
    /// Up to 547.
    #[serde(borrow)]
    pub authorships: Vec<Authorship<'a>>,
    #[allow(missing_docs)]
    pub authors_count: u32,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub corresponding_author_ids: Vec<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub corresponding_institution_ids: Vec<Text<'a>>,
    #[allow(missing_docs)]
    pub countries_distinct_count: u32,
    #[allow(missing_docs)]
    pub institutions_distinct_count: u32,
    /// Always empty in the sample; an element would be a parse failure.
    pub institutions: Vec<Unmodelled>,
    /// Present on 778.
    #[serde(borrow)]
    pub apc_list: Option<Apc<'a>>,
    /// Present on 1, with float values where `apc_list` has integers.
    #[serde(borrow)]
    pub apc_paid: Option<ApcPaid<'a>>,
    /// Field-weighted citation impact; absent on 2,419.
    pub fwci: Option<f64>,
    #[allow(missing_docs)]
    pub has_fulltext: bool,
    #[allow(missing_docs)]
    pub has_content: HasContent,
    #[allow(missing_docs)]
    pub cited_by_count: u32,
    #[allow(missing_docs)]
    pub citation_normalized_percentile: Option<CitationNormalizedPercentile>,
    #[allow(missing_docs)]
    pub cited_by_percentile_year: Option<PercentileYear>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub biblio: Biblio<'a>,
    #[allow(missing_docs)]
    pub is_retracted: bool,
    #[allow(missing_docs)]
    pub is_paratext: bool,
    /// Undocumented.
    pub is_xpac: bool,
    /// Absent on 6.
    #[serde(borrow)]
    pub primary_topic: Option<Topic<'a>>,
    /// Up to 3.
    #[serde(borrow)]
    pub topics: Vec<Topic<'a>>,
    /// 1 to 27.
    #[serde(borrow)]
    pub keywords: Vec<Scored<'a>>,
    /// Deprecated by OpenAlex in favour of topics; 1 to 34.
    #[serde(borrow)]
    pub concepts: Vec<Concept<'a>>,
    /// On 200 works, up to 120 headings.
    #[serde(borrow)]
    pub mesh: Vec<Mesh<'a>>,
    /// 1 to 17.
    #[serde(borrow)]
    pub locations: Vec<Location<'a>>,
    #[allow(missing_docs)]
    pub locations_count: u32,
    /// Never null, but every field null when there is no OA copy (6,869).
    #[serde(borrow)]
    pub best_oa_location: BestOaLocation<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub sustainable_development_goals: Vec<Scored<'a>>,
    /// Up to 62.
    #[serde(borrow)]
    pub awards: Vec<Award<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub funders: Vec<Funder<'a>>,
    /// Edges: OpenAlex ids of cited works; up to 581.
    #[serde(borrow)]
    pub referenced_works: Vec<Text<'a>>,
    #[allow(missing_docs)]
    pub referenced_works_count: u32,
    /// Up to 20.
    #[serde(borrow)]
    pub related_works: Vec<Text<'a>>,
    /// Word → positions, kept as the raw JSON slice; present on 8,831.
    #[serde(borrow)]
    pub abstract_inverted_index: Option<RawJson<'a>>,
    #[allow(missing_docs)]
    pub counts_by_year: Vec<YearCount>,
    /// ISO 8601 with milliseconds.
    #[serde(borrow)]
    pub updated_date: Text<'a>,
    /// ISO 8601 with milliseconds.
    #[serde(borrow)]
    pub created_date: Text<'a>,
    #[allow(missing_docs)]
    pub indexed_in: Vec<Index>,
}

/// A JSON value kept exactly as serialized, borrowed and uninterpreted:
/// the abstract's inverted index.
#[derive(Clone, Copy, Deserialize)]
#[serde(transparent)]
pub struct RawJson<'a>(#[serde(borrow)] &'a RawValue);

impl<'a> RawJson<'a> {
    /// The JSON text.
    #[must_use]
    pub fn as_str(self) -> &'a str {
        self.0.get()
    }
}

impl PartialEq for RawJson<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}

impl Eq for RawJson<'_> {}

impl std::fmt::Debug for RawJson<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.0.get();
        let shown: String = text.chars().take(60).collect();
        write!(
            f,
            "RawJson({shown:?}{})",
            if text.len() > 60 { "…" } else { "" }
        )
    }
}

/// An element of a list that has always been empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unmodelled {}

/// External identifiers, all URL-form except `mag`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ids<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub openalex: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub doi: Option<Text<'a>>,
    /// `https://pubmed.ncbi.nlm.nih.gov/…`; 228 in the sample.
    #[serde(borrow)]
    pub pmid: Option<Text<'a>>,
    /// Documented; not in the sample.
    #[serde(borrow)]
    pub pmcid: Option<Text<'a>>,
    /// The Microsoft Academic Graph id, digits; 10,471 in the sample.
    #[serde(borrow)]
    pub mag: Option<Text<'a>>,
}

/// A location in `locations[]` or `primary_location`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location<'a> {
    /// `doi:…`, `oai:…`, a MAG id, …
    #[serde(borrow)]
    pub id: Text<'a>,
    #[allow(missing_docs)]
    pub is_oa: bool,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub landing_page_url: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub pdf_url: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub source: Option<Source<'a>>,
    #[allow(missing_docs)]
    pub license: Option<License>,
    /// `https://openalex.org/licenses/…`, matching `license`.
    #[serde(borrow)]
    pub license_id: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub version: Option<Version>,
    #[allow(missing_docs)]
    pub is_accepted: bool,
    #[allow(missing_docs)]
    pub is_published: Option<bool>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_type: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_source_name: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub provenance: Provenance,
}

/// `best_oa_location`: the same shape with every field nullable, because
/// OpenAlex writes an object of nulls rather than null.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BestOaLocation<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub is_oa: Option<bool>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub landing_page_url: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub pdf_url: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub source: Option<Source<'a>>,
    #[allow(missing_docs)]
    pub license: Option<License>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub license_id: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub version: Option<Version>,
    #[allow(missing_docs)]
    pub is_accepted: Option<bool>,
    #[allow(missing_docs)]
    pub is_published: Option<bool>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_type: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_source_name: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub provenance: Option<Provenance>,
}

/// A source: journal, repository, conference, …
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source<'a> {
    /// `https://openalex.org/S…`.
    #[serde(borrow)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub issn_l: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub issn: Option<Vec<Text<'a>>>,
    #[allow(missing_docs)]
    pub is_oa: bool,
    #[allow(missing_docs)]
    pub is_in_doaj: bool,
    #[allow(missing_docs)]
    pub is_core: bool,
    /// `https://openalex.org/I…` or `P…`.
    #[serde(borrow)]
    pub host_organization: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub host_organization_name: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub host_organization_lineage: Vec<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub host_organization_lineage_names: Vec<Text<'a>>,
    #[serde(rename = "type")]
    #[allow(missing_docs)]
    pub r#type: SourceType,
}

/// `open_access`: the Unpaywall facts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenAccess<'a> {
    #[allow(missing_docs)]
    pub is_oa: bool,
    #[allow(missing_docs)]
    pub oa_status: OaStatus,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub oa_url: Option<Text<'a>>,
    /// Absent on 474.
    pub any_repository_has_fulltext: Option<bool>,
}

/// An authorship: the author, their institutions, and the raw strings they
/// were resolved from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authorship<'a> {
    #[allow(missing_docs)]
    pub author_position: AuthorPosition,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub author: Author<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub institutions: Vec<Institution<'a>>,
    /// ISO 3166-1 alpha-2.
    #[serde(borrow)]
    pub countries: Vec<Text<'a>>,
    #[allow(missing_docs)]
    pub is_corresponding: bool,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_author_name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_affiliation_strings: Vec<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub affiliations: Vec<Affiliation<'a>>,
    /// The ORCID as deposited, before resolution; 6,931 of 30,825.
    #[serde(borrow)]
    pub raw_orcid: Option<Text<'a>>,
}

/// The resolved author.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Author<'a> {
    /// `https://openalex.org/A…`; null on 1,321 of 30,825.
    #[serde(borrow)]
    pub id: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
    /// `https://orcid.org/…`; 12,555 of 30,825.
    #[serde(borrow)]
    pub orcid: Option<Text<'a>>,
}

/// A resolved institution.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Institution<'a> {
    /// `https://openalex.org/I…`.
    #[serde(borrow)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
    /// `https://ror.org/…`, always present in the sample.
    #[serde(borrow)]
    pub ror: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub country_code: Option<Text<'a>>,
    #[serde(rename = "type")]
    #[allow(missing_docs)]
    pub r#type: InstitutionType,
    /// The institution and its parents; 1 to 28.
    #[serde(borrow)]
    pub lineage: Vec<Text<'a>>,
}

/// A raw affiliation string and the institutions it resolved to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Affiliation<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_affiliation_string: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub institution_ids: Vec<Text<'a>>,
}

/// `apc_list`: the list price, integers.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Apc<'a> {
    #[allow(missing_docs)]
    pub value: u32,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub currency: Text<'a>,
    #[allow(missing_docs)]
    pub value_usd: u32,
}

/// `apc_paid`: what was paid, floats in the one observed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApcPaid<'a> {
    #[allow(missing_docs)]
    pub value: f64,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub currency: Text<'a>,
    #[allow(missing_docs)]
    pub value_usd: f64,
}

/// `has_content`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HasContent {
    #[allow(missing_docs)]
    pub pdf: bool,
    #[allow(missing_docs)]
    pub grobid_xml: bool,
}

/// `citation_normalized_percentile`.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CitationNormalizedPercentile {
    /// `0.0..=1.0`.
    pub value: f64,
    #[allow(missing_docs)]
    pub is_in_top_1_percent: bool,
    #[allow(missing_docs)]
    pub is_in_top_10_percent: bool,
}

/// `cited_by_percentile_year`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PercentileYear {
    #[allow(missing_docs)]
    pub min: u8,
    #[allow(missing_docs)]
    pub max: u8,
}

/// `biblio`, all as text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Biblio<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub volume: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub issue: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub first_page: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub last_page: Option<Text<'a>>,
}

/// A topic with its subfield, field and domain.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topic<'a> {
    /// `https://openalex.org/T…`.
    #[serde(borrow)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
    #[allow(missing_docs)]
    pub score: f64,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub subfield: Named<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub field: Named<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub domain: Named<'a>,
}

/// An id and a name.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Named<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
}

/// An id, a name and a score: keywords, sustainable development goals.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scored<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
    #[allow(missing_docs)]
    pub score: f64,
}

/// A (deprecated) concept.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Concept<'a> {
    /// `https://openalex.org/C…`.
    #[serde(borrow)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub wikidata: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
    /// `0..=5`.
    pub level: u8,
    #[allow(missing_docs)]
    pub score: f64,
}

/// A MeSH heading, one qualifier per entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mesh<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub descriptor_ui: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub descriptor_name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub qualifier_ui: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub qualifier_name: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub is_major_topic: bool,
}

/// An award.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Award<'a> {
    /// `https://openalex.org/G…`.
    #[serde(borrow)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub funder_award_id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub funder_display_name: Text<'a>,
    /// `https://openalex.org/F…`.
    #[serde(borrow)]
    pub funder_id: Text<'a>,
}

/// A funder.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Funder<'a> {
    /// `https://openalex.org/F…`.
    #[serde(borrow)]
    pub id: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub display_name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub ror: Option<Text<'a>>,
}

/// One year's citations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YearCount {
    #[allow(missing_docs)]
    pub year: u16,
    #[allow(missing_docs)]
    pub cited_by_count: u32,
}
