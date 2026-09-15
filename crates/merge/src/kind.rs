//! The two kinds of a work (merge plan §5): `form`, the DOI registry's
//! vocabulary for what was registered — a journal article, a book
//! chapter, posted content, a dataset — and `genre`, the aggregators' and
//! PubMed's vocabulary for what the item is — an article, a review, an
//! erratum, a letter. One form holds a dozen genres (§5), so they are two
//! fields. Both are closed; each source crate maps its own vocabulary
//! into them, and a value it cannot map stops the run.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The registry's word for what was registered: Crossref's 30 work
/// types, spelled as Crossref spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Form {
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

/// What the item is: OpenAlex's types, which are the genres an
/// aggregator can tell apart, and the four PubMed's publication types add
/// — a comment, an expression of concern, a guideline, a news item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum Genre {
    Article,
    Review,
    Letter,
    Editorial,
    Comment,
    News,
    Erratum,
    Retraction,
    ExpressionOfConcern,
    Preprint,
    Guideline,
    DataPaper,
    SoftwarePaper,
    PeerReview,
    BookReview,
    ConferencePaper,
    ConferenceAbstract,
    BookChapter,
    Book,
    Dataset,
    Dissertation,
    Report,
    Standard,
    Grant,
    ReferenceEntry,
    SupplementaryMaterials,
    Paratext,
    Libguides,
    Other,
}

impl Form {
    /// Every form, in Crossref's order.
    pub const ALL: [Self; 30] = [
        Self::Book,
        Self::BookChapter,
        Self::BookPart,
        Self::BookSection,
        Self::BookSeries,
        Self::BookSet,
        Self::BookTrack,
        Self::Component,
        Self::Database,
        Self::Dataset,
        Self::Dissertation,
        Self::EditedBook,
        Self::Grant,
        Self::Journal,
        Self::JournalArticle,
        Self::JournalIssue,
        Self::JournalVolume,
        Self::Monograph,
        Self::Other,
        Self::PeerReview,
        Self::PostedContent,
        Self::Proceedings,
        Self::ProceedingsArticle,
        Self::ProceedingsSeries,
        Self::ReferenceBook,
        Self::ReferenceEntry,
        Self::Report,
        Self::ReportComponent,
        Self::ReportSeries,
        Self::Standard,
    ];

    /// The spelling in reports and sets.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Book => "book",
            Self::BookChapter => "book-chapter",
            Self::BookPart => "book-part",
            Self::BookSection => "book-section",
            Self::BookSeries => "book-series",
            Self::BookSet => "book-set",
            Self::BookTrack => "book-track",
            Self::Component => "component",
            Self::Database => "database",
            Self::Dataset => "dataset",
            Self::Dissertation => "dissertation",
            Self::EditedBook => "edited-book",
            Self::Grant => "grant",
            Self::Journal => "journal",
            Self::JournalArticle => "journal-article",
            Self::JournalIssue => "journal-issue",
            Self::JournalVolume => "journal-volume",
            Self::Monograph => "monograph",
            Self::Other => "other",
            Self::PeerReview => "peer-review",
            Self::PostedContent => "posted-content",
            Self::Proceedings => "proceedings",
            Self::ProceedingsArticle => "proceedings-article",
            Self::ProceedingsSeries => "proceedings-series",
            Self::ReferenceBook => "reference-book",
            Self::ReferenceEntry => "reference-entry",
            Self::Report => "report",
            Self::ReportComponent => "report-component",
            Self::ReportSeries => "report-series",
            Self::Standard => "standard",
        }
    }

    /// Whether a genre can live in this form (merge plan §5): the
    /// disagreement worth storing is one that cannot — a dissertation
    /// registered as a journal article, a review inside a dataset. The
    /// table is generous where the sources are known to blur (a
    /// proceedings article is `article` to OpenAlex) and strict where a
    /// pairing says one source is wrong. `other` admits everything.
    #[must_use]
    #[allow(clippy::too_many_lines)] // a table
    pub const fn admits(self, genre: Genre) -> bool {
        use Genre as G;
        match self {
            Self::JournalArticle => matches!(
                genre,
                G::Article
                    | G::Review
                    | G::Letter
                    | G::Editorial
                    | G::Comment
                    | G::News
                    | G::Erratum
                    | G::Retraction
                    | G::ExpressionOfConcern
                    | G::Guideline
                    | G::DataPaper
                    | G::SoftwarePaper
                    | G::PeerReview
                    | G::BookReview
                    | G::ConferencePaper
                    | G::ConferenceAbstract
                    | G::Paratext
                    | G::Other
            ),
            Self::Journal | Self::JournalIssue | Self::JournalVolume => {
                matches!(genre, G::Paratext | G::Editorial | G::Other)
            }
            Self::BookChapter
            | Self::BookPart
            | Self::BookSection
            | Self::BookTrack
            | Self::ReferenceEntry => matches!(
                genre,
                G::BookChapter
                    | G::ReferenceEntry
                    | G::Article
                    | G::Review
                    | G::Editorial
                    | G::BookReview
                    | G::ConferencePaper
                    | G::Erratum
                    | G::Paratext
                    | G::Other
            ),
            Self::Book
            | Self::EditedBook
            | Self::Monograph
            | Self::ReferenceBook
            | Self::BookSeries
            | Self::BookSet => matches!(
                genre,
                G::Book
                    | G::Dissertation
                    | G::Report
                    | G::Review
                    | G::ReferenceEntry
                    | G::Paratext
                    | G::Other
            ),
            Self::Component => matches!(
                genre,
                G::SupplementaryMaterials | G::Dataset | G::Paratext | G::Other
            ),
            Self::Dataset | Self::Database => matches!(
                genre,
                G::Dataset | G::DataPaper | G::SupplementaryMaterials | G::Other
            ),
            Self::Dissertation => matches!(genre, G::Dissertation | G::Other),
            Self::Grant => matches!(genre, G::Grant | G::Other),
            Self::PeerReview => matches!(genre, G::PeerReview | G::Review | G::Other),
            Self::PostedContent => matches!(
                genre,
                G::Preprint
                    | G::Article
                    | G::Review
                    | G::Letter
                    | G::Report
                    | G::Dissertation
                    | G::DataPaper
                    | G::SoftwarePaper
                    | G::Other
            ),
            Self::Proceedings | Self::ProceedingsSeries => {
                matches!(genre, G::Paratext | G::Book | G::Other)
            }
            Self::ProceedingsArticle => matches!(
                genre,
                G::ConferencePaper
                    | G::ConferenceAbstract
                    | G::Article
                    | G::Review
                    | G::Letter
                    | G::Editorial
                    | G::Erratum
                    | G::Paratext
                    | G::Other
            ),
            Self::Report | Self::ReportComponent | Self::ReportSeries => matches!(
                genre,
                G::Report | G::Article | G::Review | G::Dataset | G::Paratext | G::Other
            ),
            Self::Standard => matches!(genre, G::Standard | G::Other),
            Self::Other => true,
        }
    }
}

impl Genre {
    /// Every genre.
    pub const ALL: [Self; 29] = [
        Self::Article,
        Self::Review,
        Self::Letter,
        Self::Editorial,
        Self::Comment,
        Self::News,
        Self::Erratum,
        Self::Retraction,
        Self::ExpressionOfConcern,
        Self::Preprint,
        Self::Guideline,
        Self::DataPaper,
        Self::SoftwarePaper,
        Self::PeerReview,
        Self::BookReview,
        Self::ConferencePaper,
        Self::ConferenceAbstract,
        Self::BookChapter,
        Self::Book,
        Self::Dataset,
        Self::Dissertation,
        Self::Report,
        Self::Standard,
        Self::Grant,
        Self::ReferenceEntry,
        Self::SupplementaryMaterials,
        Self::Paratext,
        Self::Libguides,
        Self::Other,
    ];

    /// The spelling in reports and sets.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Review => "review",
            Self::Letter => "letter",
            Self::Editorial => "editorial",
            Self::Comment => "comment",
            Self::News => "news",
            Self::Erratum => "erratum",
            Self::Retraction => "retraction",
            Self::ExpressionOfConcern => "expression-of-concern",
            Self::Preprint => "preprint",
            Self::Guideline => "guideline",
            Self::DataPaper => "data-paper",
            Self::SoftwarePaper => "software-paper",
            Self::PeerReview => "peer-review",
            Self::BookReview => "book-review",
            Self::ConferencePaper => "conference-paper",
            Self::ConferenceAbstract => "conference-abstract",
            Self::BookChapter => "book-chapter",
            Self::Book => "book",
            Self::Dataset => "dataset",
            Self::Dissertation => "dissertation",
            Self::Report => "report",
            Self::Standard => "standard",
            Self::Grant => "grant",
            Self::ReferenceEntry => "reference-entry",
            Self::SupplementaryMaterials => "supplementary-materials",
            Self::Paratext => "paratext",
            Self::Libguides => "libguides",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for Form {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl fmt::Display for Genre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::{Form, Genre};

    #[test]
    fn names_are_the_serde_spellings() {
        for form in Form::ALL {
            let json = serde_json::to_string(&form).unwrap();
            assert_eq!(json, format!("{:?}", form.name()));
            assert_eq!(serde_json::from_str::<Form>(&json).unwrap(), form);
        }
        for genre in Genre::ALL {
            let json = serde_json::to_string(&genre).unwrap();
            assert_eq!(json, format!("{:?}", genre.name()));
            assert_eq!(serde_json::from_str::<Genre>(&json).unwrap(), genre);
        }
    }

    #[test]
    fn every_form_admits_something_and_other_admits_all() {
        for form in Form::ALL {
            assert!(Genre::ALL.iter().any(|g| form.admits(*g)), "{form}");
            assert!(form.admits(Genre::Other), "{form}");
        }
        assert!(Genre::ALL.iter().all(|g| Form::Other.admits(*g)));
        assert!(Form::JournalArticle.admits(Genre::Review));
        assert!(!Form::JournalArticle.admits(Genre::Dissertation));
        assert!(!Form::JournalArticle.admits(Genre::Preprint));
        assert!(!Form::Dataset.admits(Genre::Review));
        assert!(Form::PostedContent.admits(Genre::Preprint));
        assert!(Form::ProceedingsArticle.admits(Genre::Article));
    }
}
