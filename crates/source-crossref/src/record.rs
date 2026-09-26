//! Native record types: what a Crossref work object contains, as observed in
//! 1,000 random works (plan §4a). Every object rejects unknown keys.
//!
//! Field documentation notes how many of the 1,000 works carried the field
//! where that is informative, and where the data disagrees with
//! `api_format.md`.

use std::collections::BTreeMap;

use serde::Deserialize;
use smallvec::SmallVec;

use crate::date::{IndexedStamp, PartialDate, Stamp};
use crate::vocab::{
    AssertedBy, ContentVersion, FundingType, IdKind, IntendedApplication, ReferenceType,
    RelationAssertedBy, RelationIdKind, ReviewRecommendation, ReviewStage, ReviewType, RoleName,
    Sequence, SerialKind, Source, Subtype, UpdateWord, Vocabulary, WorkType,
};

/// A list with room for its usual length inside the record itself (plan
/// §5, rule 1); only a longer one allocates. The lengths come from the
/// observed distributions in the plan's data, about the 90th percentile.
pub type Few<T, const N: usize> = SmallVec<[T; N]>;

/// A string borrowed from the input unless it contained an escape sequence.
pub use ingest::Text;

/// One work.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Work<'a> {
    /// The DOI, as deposited (mixed case is common).
    #[serde(rename = "DOI", borrow)]
    pub doi: Text<'a>,
    /// `https://doi.org/` + DOI.
    #[serde(rename = "URL", borrow)]
    pub url: Text<'a>,
    /// One of the 30 types; absent on 2,257 of the records the March 2026
    /// file's survey stopped at (2026-09-26) — EMBL-EBI's dataset pages,
    /// collections, videos, articles, Crossmark policy pages, all created
    /// 2006 to 2022 and six of six title-only `<database>` deposits. The
    /// type is unknown, not guessed.
    #[serde(rename = "type")]
    pub r#type: Option<WorkType>,
    /// Always `Crossref`.
    pub source: Source,
    /// The DOI prefix, `10.1016`.
    #[serde(borrow)]
    pub prefix: Text<'a>,
    /// The member id, digits as a string.
    #[serde(borrow)]
    pub member: Text<'a>,
    /// Query relevance; `0.0` outside searches.
    pub score: f64,
    /// DOI registration.
    #[serde(borrow)]
    pub created: Stamp<'a>,
    /// Latest metadata deposit.
    #[serde(borrow)]
    pub deposited: Stamp<'a>,
    /// Latest indexing.
    #[serde(borrow)]
    pub indexed: IndexedStamp<'a>,
    /// Earliest publication date; `[[null]]` on 34 of 1,000.
    pub issued: PartialDate,
    /// Present on 965 of 1,000.
    pub published: Option<PartialDate>,
    /// Present on 785.
    pub published_print: Option<PartialDate>,
    /// Present on 499.
    pub published_online: Option<PartialDate>,
    /// Present on 20.
    pub published_other: Option<PartialDate>,
    /// Posted content.
    pub posted: Option<PartialDate>,
    /// Present on 12.
    pub accepted: Option<PartialDate>,
    /// Present on 4.
    pub approved: Option<PartialDate>,
    /// Present on 2.
    pub content_created: Option<PartialDate>,
    /// When a dataset's or database's content last changed: the deposit
    /// schema's `database_date/update_date`, the sibling of
    /// `content-created`. 9,641 shards of the March 2026 file stopped at
    /// it (2026-09-26), the Protein Data Bank's, OECD's and APA's first;
    /// it can come before `content-created`.
    pub content_updated: Option<PartialDate>,
    /// A grant's own start (grant schema 0.2.0, `award-start-date` on the
    /// grant as a whole), beside each project's; 991 shards stopped at it.
    /// A grant's `issued` is the earliest of these starts. Read, not kept:
    /// grants are not normalized yet.
    pub award_start: Option<PartialDate>,
    /// Absent on one work (a grant).
    pub reference_count: Option<u32>,
    /// Absent on the same work.
    pub references_count: Option<u32>,
    /// Absent on the same work.
    pub is_referenced_by_count: Option<u32>,
    /// Titles; 980 of 1,000 have one, up to two.
    #[serde(borrow)]
    pub title: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub subtitle: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub short_title: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub original_title: Option<Few<Text<'a>, 1>>,
    /// Up to two.
    #[serde(borrow)]
    pub container_title: Option<Few<Text<'a>, 1>>,
    /// Up to eight.
    #[serde(borrow)]
    pub short_container_title: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub issue_title: Option<Few<Text<'a>, 1>>,
    /// Posted content's group, e.g. `SSRN`.
    #[serde(borrow)]
    pub group_title: Option<Text<'a>>,
    /// A JATS XML fragment; 275 of 1,000.
    #[serde(borrow)]
    pub r#abstract: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub publisher: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub publisher_location: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub volume: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub issue: Option<Text<'a>>,
    /// `89-95`, `e12821`.
    #[serde(borrow)]
    pub page: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub article_number: Option<Text<'a>>,
    /// `1`, `0`, `1/e`.
    #[serde(borrow)]
    pub edition_number: Option<Text<'a>>,
    /// `C`, `S1`, …
    #[serde(rename = "special_numbering", borrow)]
    pub special_numbering: Option<Text<'a>>,
    /// ISO 639-1, `en`.
    #[serde(borrow)]
    pub language: Option<Text<'a>>,
    /// Crossmark policy DOI URL.
    #[serde(borrow)]
    pub update_policy: Option<Text<'a>>,
    #[serde(rename = "ISSN", borrow)]
    #[allow(missing_docs)]
    pub issn: Option<Few<Text<'a>, 2>>,
    /// `print` / `electronic`, not the `pissn` / `eissn` of `api_format.md`.
    #[serde(borrow)]
    pub issn_type: Option<Few<Serial<'a>, 2>>,
    #[serde(rename = "ISBN", borrow)]
    #[allow(missing_docs)]
    pub isbn: Option<Few<Text<'a>, 2>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub isbn_type: Option<Few<Serial<'a>, 2>>,
    /// Publisher identifiers such as PIIs; 551 of 1,000.
    #[serde(borrow)]
    pub alternative_id: Option<Few<Text<'a>, 2>>,
    /// Other DOIs for the same work; 2 of 1,000.
    #[serde(borrow)]
    pub aliases: Option<Few<Text<'a>, 2>>,
    /// `Portico`, `CLOCKSS`, …
    #[serde(borrow)]
    pub archive: Option<Few<Text<'a>, 2>>,
    /// Documented, deprecated, not observed; kept so its return is noticed.
    #[serde(borrow)]
    pub subject: Option<Few<Text<'a>, 4>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub degree: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub description: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub proceedings_subject: Option<Text<'a>>,
    /// A grant's award number.
    #[serde(borrow)]
    pub award: Option<Text<'a>>,
    /// Posted content's subtype.
    pub subtype: Option<Subtype>,
    /// 898 of 1,000; up to 544 authors.
    #[serde(borrow)]
    pub author: Option<Few<Contributor<'a>, 8>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub editor: Option<Few<Contributor<'a>, 4>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub chair: Option<Few<Contributor<'a>, 2>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub translator: Option<Few<Contributor<'a>, 2>>,
    /// 2 of 1,000, with a `[null]` role.
    #[serde(borrow)]
    pub contributor: Option<Few<Contributor<'a>, 2>>,
    /// 733 of 1,000; up to 503 entries.
    #[serde(borrow)]
    pub reference: Option<Few<Reference<'a>, 16>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub funder: Option<Few<Funder<'a>, 4>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub license: Option<Few<License<'a>, 4>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub link: Option<Few<Link<'a>, 4>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub assertion: Option<Few<Assertion<'a>, 8>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub content_domain: Option<ContentDomain<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub resource: Resource<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub journal_issue: Option<JournalIssue<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub event: Option<Event<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub institution: Option<Few<Institution<'a>, 2>>,
    /// Keyed by relation type: `has-preprint`, `is-preprint-of`, …
    #[serde(borrow)]
    pub relation: Option<BTreeMap<&'a str, Vec<Relation<'a>>>>,
    /// Updates this work applies to others.
    #[serde(borrow)]
    pub update_to: Option<Few<Update<'a>, 2>>,
    /// Updates others apply to this work.
    #[serde(borrow)]
    pub updated_by: Option<Few<Update<'a>, 2>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub review: Option<Review<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub standards_body: Option<StandardsBody<'a>>,
    /// Grants.
    #[serde(borrow)]
    pub project: Option<Few<Project<'a>, 1>>,
    /// Documented; not observed.
    #[serde(borrow)]
    pub clinical_trial_number: Option<Few<ClinicalTrialNumber<'a>, 2>>,
}

/// An `issn-type` or `isbn-type` entry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Serial<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub value: Text<'a>,
    #[allow(missing_docs)]
    pub r#type: SerialKind,
}

/// An author, editor, chair, translator or contributor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Contributor<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub given: Option<Text<'a>>,
    /// Absent for organisations, which use `name`.
    #[serde(borrow)]
    pub family: Option<Text<'a>>,
    /// Organisational contributors.
    #[serde(borrow)]
    pub name: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub prefix: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub suffix: Option<Text<'a>>,
    /// URL form, `https://orcid.org/0000-…`.
    #[serde(rename = "ORCID", borrow)]
    pub orcid: Option<Text<'a>>,
    #[serde(rename = "authenticated-orcid")]
    #[allow(missing_docs)]
    pub authenticated_orcid: Option<bool>,
    #[allow(missing_docs)]
    pub sequence: Sequence,
    /// Always present, often empty.
    #[serde(borrow)]
    pub affiliation: Few<Affiliation<'a>, 2>,
    /// One role per contributor; `[null]` on `contributor` entries.
    pub role: Option<Few<Option<Role>, 1>>,
}

/// `role` on a contributor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    #[allow(missing_docs)]
    pub role: RoleName,
    #[allow(missing_docs)]
    pub vocabulary: Vocabulary,
}

/// An affiliation: the deposit schema's `<institution>`, the element a
/// work's own [`Institution`] is too, so it has the same parts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Affiliation<'a> {
    /// Absent on 4 of 1,249.
    #[serde(borrow)]
    pub name: Option<Text<'a>>,
    /// `institution_acronym`, up to six; 892 shards of the March 2026 file
    /// stopped at one (2026-09-26).
    #[serde(borrow)]
    pub acronym: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub department: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub place: Option<Few<Text<'a>, 1>>,
    /// ISO country code, on grant investigators.
    #[serde(borrow)]
    pub country: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Option<Few<Identifier<'a>, 1>>,
}

/// An identifier with its type and asserter: ROR ids, funder DOIs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Identifier<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Text<'a>,
    #[allow(missing_docs)]
    pub id_type: IdKind,
    #[allow(missing_docs)]
    pub asserted_by: AssertedBy,
}

/// One reference: a resolved DOI, structured fields, unstructured text, or
/// any mix.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Reference<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub key: Text<'a>,
    /// 18,392 of 24,344.
    #[serde(rename = "DOI", borrow)]
    pub doi: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub doi_asserted_by: Option<AssertedBy>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub unstructured: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub author: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub year: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub journal_title: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub article_title: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub series_title: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub volume_title: Option<Text<'a>>,
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
    pub edition: Option<Text<'a>>,
    /// URL form, `https://id.crossref.org/issn/0004-637X`.
    #[serde(rename = "ISSN", borrow)]
    pub issn: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub issn_type: Option<SerialKind>,
    /// URL form, `https://id.crossref.org/isbn/…`.
    #[serde(rename = "ISBN", borrow)]
    pub isbn: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub isbn_type: Option<SerialKind>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub component: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub standard_designator: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub standards_body: Option<Text<'a>>,
    /// Rare in the sample (44 references of one work); its own vocabulary.
    #[serde(rename = "type")]
    pub r#type: Option<ReferenceType>,
}

/// A funder.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Funder<'a> {
    /// Absent on 1 of 373.
    #[serde(borrow)]
    pub name: Option<Text<'a>>,
    /// Open Funder Registry DOI.
    #[serde(rename = "DOI", borrow)]
    pub doi: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub doi_asserted_by: Option<AssertedBy>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub award: Option<Few<Text<'a>, 2>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub award_info: Option<Few<AwardInfo<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Option<Few<Identifier<'a>, 1>>,
}

/// `award-info` on a funder.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct AwardInfo<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub award_number: Few<Text<'a>, 2>,
    /// An award DOI, on 4 of 34,536 looked-up works (2026-09-15).
    #[serde(borrow)]
    pub id: Option<Few<AwardId<'a>, 1>>,
}

/// An identifier of an award: `{id, id-type, asserted-by}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct AwardId<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Text<'a>,
    #[allow(missing_docs)]
    pub id_type: IdKind,
    #[allow(missing_docs)]
    pub asserted_by: AssertedBy,
}

/// A license.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct License<'a> {
    #[serde(rename = "URL", borrow)]
    #[allow(missing_docs)]
    pub url: Text<'a>,
    #[allow(missing_docs)]
    pub content_version: ContentVersion,
    /// Crossref's `max(0, start − issued)`, in days.
    pub delay_in_days: i64,
    /// Absent where the deposit gave none and the work has no `issued`
    /// for Crossref to fill it from: dissertations and standards above
    /// all (3,272 shards of the March 2026 file stopped at one).
    #[serde(borrow)]
    pub start: Option<Stamp<'a>>,
}

/// A full-text link.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Link<'a> {
    #[serde(rename = "URL", borrow)]
    #[allow(missing_docs)]
    pub url: Text<'a>,
    /// A MIME type or `unspecified`.
    #[serde(borrow)]
    pub content_type: Text<'a>,
    #[allow(missing_docs)]
    pub content_version: ContentVersion,
    #[allow(missing_docs)]
    pub intended_application: IntendedApplication,
}

/// A publisher assertion.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assertion<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub name: Text<'a>,
    /// Absent on 27 of 1,170.
    #[serde(borrow)]
    pub value: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub label: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub order: Option<u32>,
    #[serde(rename = "URL", borrow)]
    #[allow(missing_docs)]
    pub url: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub explanation: Option<Explanation<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub group: Option<AssertionGroup<'a>>,
}

/// `explanation` on an assertion.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Explanation<'a> {
    #[serde(rename = "URL", borrow)]
    #[allow(missing_docs)]
    pub url: Text<'a>,
}

/// `group` on an assertion.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionGroup<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub name: Text<'a>,
    /// Absent on a group of one looked-up work (10.18332/tpc/214725,
    /// 2026-09-15); always present in the sample.
    #[serde(borrow)]
    pub label: Option<Text<'a>>,
}

/// Crossmark domain.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ContentDomain<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub domain: Few<Text<'a>, 4>,
    /// Absent on 4 of 999.
    pub crossmark_restriction: Option<bool>,
}

/// Where the work lives.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub primary: ResourceUrl<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub secondary: Option<Few<LabelledUrl<'a>, 1>>,
}

/// `resource.primary`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceUrl<'a> {
    #[serde(rename = "URL", borrow)]
    #[allow(missing_docs)]
    pub url: Text<'a>,
}

/// `resource.secondary[]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelledUrl<'a> {
    #[serde(rename = "URL", borrow)]
    #[allow(missing_docs)]
    pub url: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub label: Text<'a>,
}

/// The issue a journal article appeared in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct JournalIssue<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub issue: Text<'a>,
    #[allow(missing_docs)]
    pub published_print: Option<PartialDate>,
    #[allow(missing_docs)]
    pub published_online: Option<PartialDate>,
}

/// A conference.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub location: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub acronym: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub number: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub sponsor: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub theme: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub start: Option<PartialDate>,
    #[allow(missing_docs)]
    pub end: Option<PartialDate>,
}

/// An institution.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Institution<'a> {
    /// Absent on 1 of 19.
    #[serde(borrow)]
    pub name: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub acronym: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub department: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub place: Option<Few<Text<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Option<Few<Identifier<'a>, 1>>,
}

/// One relation target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Relation<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Text<'a>,
    #[allow(missing_docs)]
    pub id_type: RelationIdKind,
    #[allow(missing_docs)]
    pub asserted_by: RelationAssertedBy,
}

/// A Crossmark update: retraction, correction, erratum, …
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update<'a> {
    #[serde(rename = "DOI", borrow)]
    #[allow(missing_docs)]
    pub doi: Text<'a>,
    /// As written; [`UpdateWord::meaning`] reads it.
    pub r#type: UpdateWord,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub label: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub source: Option<AssertedBy>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub updated: Stamp<'a>,
    /// Retraction Watch's record number, with `source: retraction-watch`.
    #[serde(rename = "record-id", borrow)]
    pub record_id: Option<Text<'a>>,
}

/// Peer-review metadata.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Review<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub language: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub recommendation: Option<ReviewRecommendation>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub running_number: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub revision_round: Option<Text<'a>>,
    #[allow(missing_docs)]
    pub stage: Option<ReviewStage>,
    #[serde(rename = "type")]
    #[allow(missing_docs)]
    pub r#type: Option<ReviewType>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub competing_interest_statement: Option<Text<'a>>,
}

/// The body behind a standard.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardsBody<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub acronym: Option<Text<'a>>,
}

/// A grant's project, from the one grant observed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Project<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub project_title: Option<Few<Titled<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub project_description: Option<Few<Described<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub award_amount: Option<AwardAmount<'a>>,
    #[allow(missing_docs)]
    pub award_start: Option<PartialDate>,
    #[allow(missing_docs)]
    pub award_end: Option<PartialDate>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub funding: Option<Few<Funding<'a>, 1>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub investigator: Option<Few<Investigator<'a>, 2>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub lead_investigator: Option<Few<Investigator<'a>, 1>>,
}

/// A grant investigator: a person without the `sequence` and `role` that
/// contributors carry.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Investigator<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub given: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub family: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub name: Option<Text<'a>>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub suffix: Option<Text<'a>>,
    #[serde(rename = "ORCID", borrow)]
    #[allow(missing_docs)]
    pub orcid: Option<Text<'a>>,
    #[serde(rename = "authenticated-orcid")]
    #[allow(missing_docs)]
    pub authenticated_orcid: Option<bool>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub affiliation: Option<Few<Affiliation<'a>, 2>>,
    /// When the person's part in the project began (grant schema 0.2.0,
    /// an investigator's `@start-date`); on 9 of the grants the March
    /// 2026 file's survey stopped at, behind `award-start`.
    pub role_start: Option<PartialDate>,
    /// When it ended.
    pub role_end: Option<PartialDate>,
}

/// `project-title[]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Titled<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub title: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub language: Option<Text<'a>>,
}

/// `project-description[]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Described<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub description: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub language: Option<Text<'a>>,
}

/// `award-amount`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AwardAmount<'a> {
    #[allow(missing_docs)]
    pub amount: f64,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub currency: Text<'a>,
}

/// `funding[]` on a project.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Funding<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub funder: FundingFunder<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub scheme: Option<Text<'a>>,
    #[serde(rename = "type")]
    #[allow(missing_docs)]
    pub r#type: Option<FundingType>,
}

/// `funding[].funder`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingFunder<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub name: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub id: Option<Few<Identifier<'a>, 1>>,
}

/// Documented; not yet observed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClinicalTrialNumber<'a> {
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub clinical_trial_number: Text<'a>,
    #[serde(borrow)]
    #[allow(missing_docs)]
    pub registry: Text<'a>,
    #[serde(rename = "type", borrow)]
    #[allow(missing_docs)]
    pub r#type: Option<Text<'a>>,
}
