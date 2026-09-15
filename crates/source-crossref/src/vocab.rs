//! Enumerated string values. A value outside a list is a parse failure.
//!
//! Three lists have an authoritative source and are complete: [`WorkType`]
//! (`https://api.crossref.org/types`, 30 entries), [`UpdateType`] (the
//! Crossmark update types) and [`RelationIdKind`] (the identifier types of
//! Crossref's deposit schema). The others are exactly the values observed in
//! the first 1,000 works, which is the point.

use serde::{Deserialize, Serialize};

/// The 30 work types of `https://api.crossref.org/types`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum WorkType {
    Book,
    BookChapter,
    BookPart,
    BookSection,
    BookSeries,
    BookSet,
    BookTrack,
    Component,
    Database,
    Dataset,
    Dissertation,
    EditedBook,
    Grant,
    Journal,
    JournalArticle,
    JournalIssue,
    JournalVolume,
    Monograph,
    Other,
    PeerReview,
    PostedContent,
    Proceedings,
    ProceedingsArticle,
    ProceedingsSeries,
    ReferenceBook,
    ReferenceEntry,
    Report,
    ReportComponent,
    ReportSeries,
    Standard,
}

/// `source`: "currently always Crossref".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[allow(missing_docs)]
pub enum Source {
    Crossref,
}

/// Who asserted a DOI or identifier: `doi-asserted-by`, `asserted-by` on
/// identifier objects, `source` on updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum AssertedBy {
    Publisher,
    Crossref,
}

/// `asserted-by` on a relation: which side of the relation asserted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum RelationAssertedBy {
    Subject,
    Object,
}

/// `id-type` on affiliation, funder and institution identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[allow(missing_docs)]
pub enum IdKind {
    #[serde(rename = "ROR")]
    Ror,
    #[serde(rename = "DOI")]
    Doi,
}

/// `id-type` on a relation: the identifier types of the deposit schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum RelationIdKind {
    Doi,
    Issn,
    Isbn,
    Uri,
    Pmid,
    Pmcid,
    Purl,
    Arxiv,
    Ark,
    Handle,
    Uuid,
    Ecli,
    Accession,
    Other,
}

/// A contributor's `sequence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Sequence {
    First,
    Additional,
}

/// `role.role` on a contributor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum RoleName {
    Author,
    Editor,
    Chair,
    Translator,
}

/// `role.vocabulary` on a contributor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Vocabulary {
    Crossref,
}

/// `content-version` on licenses and links.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ContentVersion {
    Vor,
    Am,
    Tdm,
    Unspecified,
    StmAsf,
}

/// `intended-application` on a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum IntendedApplication {
    TextMining,
    SimilarityChecking,
    Syndication,
    Unspecified,
}

/// `type` on `issn-type` and `isbn-type` entries (and on references).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum SerialKind {
    Print,
    Electronic,
}

/// `type` on `update-to` and `updated-by`: the Crossmark update types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum UpdateType {
    Addendum,
    Clarification,
    Correction,
    Corrigendum,
    Erratum,
    ExpressionOfConcern,
    NewEdition,
    NewVersion,
    PartialRetraction,
    Removal,
    Retraction,
    Withdrawal,
}

/// `subtype` of posted content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum Subtype {
    Preprint,
    WorkingPaper,
    Letter,
    Dissertation,
    Report,
    Review,
    Other,
}

/// `review.stage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ReviewStage {
    PrePublication,
    PostPublication,
}

/// `review.recommendation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ReviewRecommendation {
    MajorRevision,
    MinorRevision,
    Reject,
    RejectWithResubmit,
    Accept,
}

/// `review.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ReviewType {
    RefereeReport,
    EditorReport,
    AuthorComment,
    CommunityComment,
    Aggregate,
}

/// `type` of a project's funding entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum FundingType {
    Grant,
}

#[cfg(test)]
mod tests {
    use super::{ContentVersion, IdKind, UpdateType, WorkType};

    #[test]
    fn spellings_follow_the_data() {
        assert_eq!(
            serde_json::from_str::<WorkType>("\"journal-article\"").unwrap(),
            WorkType::JournalArticle
        );
        assert_eq!(
            serde_json::from_str::<ContentVersion>("\"stm-asf\"").unwrap(),
            ContentVersion::StmAsf
        );
        assert_eq!(
            serde_json::from_str::<IdKind>("\"ROR\"").unwrap(),
            IdKind::Ror
        );
        assert_eq!(
            serde_json::from_str::<UpdateType>("\"expression_of_concern\"").unwrap(),
            UpdateType::ExpressionOfConcern
        );
        assert!(serde_json::from_str::<WorkType>("\"Journal-Article\"").is_err());
        assert!(serde_json::from_str::<IdKind>("\"ror\"").is_err());
    }
}
