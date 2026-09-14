//! Enumerated string values. A value outside a list is a parse failure.
//!
//! [`OaStatus`] and [`Index`] are complete per the documentation; the rest
//! are the values observed in the first part file plus the documented ones
//! that were not, where the documentation lists them.

use serde::Deserialize;

/// `type`: OpenAlex's work types, from Crossref's with a few of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum WorkType {
    Article,
    BookChapter,
    Dataset,
    Preprint,
    Dissertation,
    Book,
    Review,
    Paratext,
    Libguides,
    Letter,
    Other,
    ReferenceEntry,
    Report,
    Editorial,
    PeerReview,
    Erratum,
    Standard,
    Grant,
    SupplementaryMaterials,
    Retraction,
}

/// `open_access.oa_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum OaStatus {
    Diamond,
    Gold,
    Green,
    Hybrid,
    Bronze,
    Closed,
}

/// `version` on a location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[allow(missing_docs)]
pub enum Version {
    #[serde(rename = "publishedVersion")]
    Published,
    #[serde(rename = "acceptedVersion")]
    Accepted,
    #[serde(rename = "submittedVersion")]
    Submitted,
}

/// `license` on a location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum License {
    CcBy,
    CcBySa,
    CcByNd,
    CcByNc,
    CcByNcSa,
    CcByNcNd,
    Cc0,
    PublicDomain,
    OtherOa,
    PublisherSpecificOa,
    UnspecifiedOa,
}

/// `provenance` on a location: which upstream supplied it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Provenance {
    Mag,
    Repo,
    Datacite,
    Crossref,
    Pubmed,
}

/// `indexed_in[]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Index {
    Arxiv,
    Crossref,
    Datacite,
    Doaj,
    Pubmed,
}

/// `source.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[allow(missing_docs)]
pub enum SourceType {
    #[serde(rename = "journal")]
    Journal,
    #[serde(rename = "repository")]
    Repository,
    #[serde(rename = "conference")]
    Conference,
    #[serde(rename = "ebook platform")]
    EbookPlatform,
    #[serde(rename = "book series")]
    BookSeries,
    #[serde(rename = "metadata")]
    Metadata,
    #[serde(rename = "other")]
    Other,
}

/// `institutions[].type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum InstitutionType {
    Education,
    Healthcare,
    Company,
    Archive,
    Nonprofit,
    Government,
    Facility,
    Funder,
    Other,
}

/// `author_position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum AuthorPosition {
    First,
    Middle,
    Last,
}

#[cfg(test)]
mod tests {
    use super::{License, SourceType, Version, WorkType};

    #[test]
    fn spellings_follow_the_data() {
        assert_eq!(
            serde_json::from_str::<WorkType>("\"book-chapter\"").unwrap(),
            WorkType::BookChapter
        );
        assert_eq!(
            serde_json::from_str::<Version>("\"publishedVersion\"").unwrap(),
            Version::Published
        );
        assert_eq!(
            serde_json::from_str::<License>("\"cc-by-nc-nd\"").unwrap(),
            License::CcByNcNd
        );
        assert_eq!(
            serde_json::from_str::<SourceType>("\"ebook platform\"").unwrap(),
            SourceType::EbookPlatform
        );
        assert!(serde_json::from_str::<Version>("\"published\"").is_err());
    }
}
