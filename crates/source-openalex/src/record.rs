//! Native record types: what an OpenAlex work object contains, as observed
//! in one 10,873-work part file (plan §4a). Every object rejects unknown
//! keys. Documentation notes how many of the 10,873 carried a field where
//! that is informative.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use smallvec::SmallVec;

use crate::vocab::{
    AuthorPosition, Index, InstitutionType, License, OaStatus, Provenance, SourceType, Version,
    WorkType,
};

/// A list with room for its usual length inside the record itself (plan
/// §5, rule 1); only a longer one allocates. The lengths come from the
/// observed distributions in the plan's data, about the 90th percentile.
pub type Few<T, const N: usize> = SmallVec<[T; N]>;

/// A string borrowed from the input unless it contained an escape sequence.
pub use ingest::Text;

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
    pub title: Option<Text<'a>>,
    /// The same as `title` in the sample.
    #[serde(borrow)]
    pub display_name: Option<Text<'a>>,
    /// 1823..2026 in the sample.
    /// Null together with `publication_date` (32 DOAJ-indexed works in the
    /// 2025-10-17 part, e.g. `W7050100049`).
    pub publication_year: Option<u16>,
    /// `YYYY-MM-DD`.
    #[serde(borrow)]
    pub publication_date: Option<Text<'a>>,
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
    pub authorships: Few<Authorship<'a>, 8>,
    /// The snapshot always has it; the API (2026-09) does not.
    pub authors_count: Option<u32>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub corresponding_author_ids: Few<Text<'a>, 2>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub corresponding_institution_ids: Few<Text<'a>, 2>,
    #[allow(missing_docs)]
    pub countries_distinct_count: u32,
    #[allow(missing_docs)]
    pub institutions_distinct_count: u32,
    /// Always empty in the sample; an element would be a parse failure.
    pub institutions: Few<Unmodelled, 2>,
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
    /// API responses only: where the PDF and GROBID XML `has_content`
    /// promises are, each present exactly when its flag is and derivable
    /// from the id; absent from the snapshot, null when there is no
    /// content (34,762 works, 2026-09-15: 17,408 with both, 204 with a
    /// PDF alone, none with GROBID alone).
    #[serde(borrow)]
    pub content_urls: Option<ContentUrls<'a>>,
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
    /// Null on a paratext work (first seen 2025-07-29 part, `W4288097146`,
    /// which had no `title` or `display_name` either).
    pub is_retracted: Option<bool>,
    #[allow(missing_docs)]
    pub is_paratext: bool,
    /// Undocumented.
    pub is_xpac: bool,
    /// Absent on 6.
    #[serde(borrow)]
    pub primary_topic: Option<Topic<'a>>,
    /// Up to 3.
    #[serde(borrow)]
    pub topics: Few<Topic<'a>, 3>,
    /// 1 to 27.
    #[serde(borrow)]
    pub keywords: Few<Scored<'a>, 8>,
    /// Deprecated by OpenAlex in favour of topics; 1 to 34.
    #[serde(borrow)]
    pub concepts: Few<Concept<'a>, 16>,
    /// On 200 works, up to 120 headings.
    #[serde(borrow)]
    pub mesh: Few<Mesh<'a>, 4>,
    /// 1 to 17.
    #[serde(borrow)]
    pub locations: Few<Location<'a>, 4>,
    #[allow(missing_docs)]
    pub locations_count: u32,
    /// Never null in the snapshot, every field null when there is no OA
    /// copy (6,869); `null` from the API (2026-09).
    #[serde(borrow)]
    pub best_oa_location: Option<BestOaLocation<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub sustainable_development_goals: Few<Scored<'a>, 2>,
    /// Up to 62.
    #[serde(borrow)]
    pub awards: Few<Award<'a>, 2>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub funders: Few<Funder<'a>, 2>,
    /// Edges: OpenAlex ids of cited works; up to 581.
    #[serde(borrow)]
    pub referenced_works: Few<Text<'a>, 8>,
    #[allow(missing_docs)]
    pub referenced_works_count: u32,
    /// Up to 20.
    #[serde(borrow)]
    pub related_works: Few<Text<'a>, 20>,
    /// Word → positions, kept as the raw JSON slice; present on 8,831.
    #[serde(borrow)]
    pub abstract_inverted_index: Option<RawJson<'a>>,
    #[allow(missing_docs)]
    pub counts_by_year: Few<YearCount, 8>,
    /// `YYYY-MM-DDThh:mm:ss.fffZ` in the snapshot; the API (2026-09)
    /// writes six fraction digits and no zone.
    #[serde(borrow)]
    pub updated_date: Text<'a>,
    /// `YYYY-MM-DDThh:mm:ss.fffZ` in the snapshot; the API (2026-09)
    /// writes seconds, no fraction, no zone.
    #[serde(borrow)]
    pub created_date: Text<'a>,
    #[allow(missing_docs)]
    pub indexed_in: Few<Index, 4>,
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
    /// Absent on one location of one looked-up work (W2117692326,
    /// 2026-09-15), whose URLs read like test data.
    pub id: Option<Text<'a>>,
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
    /// The snapshot says where a location came from here and writes the
    /// bare id (digits, a DOI, `oai:…`); the API (2026-09) has no such
    /// field and says the namespace in the id instead: `doi:…`, `pmid:…`,
    /// `pmh:oai:…`.
    pub provenance: Option<Provenance>,
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
    pub issn: Option<Few<Text<'a>, 2>>,
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
    pub host_organization_lineage: Few<Text<'a>, 2>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub host_organization_lineage_names: Few<Text<'a>, 2>,
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
    pub institutions: Few<Institution<'a>, 2>,
    /// ISO 3166-1 alpha-2.
    #[serde(borrow)]
    pub countries: Few<Text<'a>, 2>,
    #[allow(missing_docs)]
    pub is_corresponding: bool,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_author_name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub raw_affiliation_strings: Few<Text<'a>, 2>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub affiliations: Few<Affiliation<'a>, 2>,
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
    /// `https://ror.org/…`; null for an institution without one (first
    /// seen 2025-07-29 part: `I4210137868`, together with a null `type`).
    #[serde(borrow)]
    pub ror: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub country_code: Option<Text<'a>>,
    #[serde(rename = "type")]
    #[allow(missing_docs)]
    pub r#type: Option<InstitutionType>,
    /// The institution and its parents; 1 to 28.
    #[serde(borrow)]
    pub lineage: Few<Text<'a>, 4>,
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
    pub institution_ids: Few<Text<'a>, 2>,
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
    /// Null when the currency has no conversion (`IRR`, 2025-07-29 part).
    pub value_usd: Option<u32>,
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

/// `content_urls`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentUrls<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub pdf: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub grobid_xml: Option<Text<'a>>,
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
