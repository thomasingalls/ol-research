//! Enumerated string values. A value outside a list is a parse failure.
//!
//! Some lists have an authoritative source and are that source's whole
//! list: [`WorkType`] (`https://api.crossref.org/types`, 30 entries), the
//! Crossmark update types of [`UpdateType`], [`IdKind`]'s institution
//! kinds (the deposit schema's `institution_id@type`), and, since the
//! survey of the March 2026 file (2026-09-26), [`ReferenceType`] (the
//! schema's `citation@type`), [`FundingType`] (the grant schema's funding
//! types) and [`ReviewRecommendation`] and [`ReviewType`] (the schema's
//! peer-review attributes). The data carries more than a source lists
//! where the source once allowed it: [`UpdateWord`] reads Crossmark's
//! free-text years, and [`RelationIdKind`] two id types the deposit schema
//! never had. The rest are exactly the values observed in the first 1,000
//! works, which is the point.

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
    /// `source` on an update: Retraction Watch's data, carried by Crossref
    /// since 2023 (plan §2); 24 of 34,536 looked-up works (2026-09-15).
    #[serde(rename = "retraction-watch")]
    RetractionWatch,
}

/// `asserted-by` on a relation: which side of the relation asserted it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum RelationAssertedBy {
    Subject,
    Object,
}

/// `id-type` on affiliation, funder and institution identifiers. An
/// institution's are the deposit schema's `institution_id@type`, the same
/// three from 5.3.1 to 5.5.0: ROR, ISNI and Wikidata. A funder's is a DOI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[allow(missing_docs)]
pub enum IdKind {
    #[serde(rename = "ROR")]
    Ror,
    #[serde(rename = "DOI")]
    Doi,
    /// 40 affiliation ids of 34,536 looked-up works (2026-09-15).
    #[serde(rename = "ISNI")]
    Isni,
    /// A Wikidata item, written in lower case where the other two are in
    /// capitals. 1,957 shards of the March 2026 file stopped at one
    /// (2026-09-26), mostly university repositories' dissertations.
    #[serde(rename = "wikidata")]
    Wikidata,
}

/// `id-type` on a relation: the identifier types of the deposit schema,
/// and two it has never had that the March 2026 file carries anyway.
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
    /// On Preprints.org's `is-preprint-of` and Scientific Data's
    /// `references`: 18 ids in 6 records where the survey stopped
    /// (2026-09-26), every one a whole DOI, not a shortDOI's `10/…`.
    #[serde(rename = "short-doi")]
    ShortDoi,
    /// A person's ORCID given as what a work `is-supplemented-by`, on 3
    /// records of Elsevier's Catena (2025): the publisher's mistake,
    /// kept as written.
    Orcid,
}

/// A contributor's `sequence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Sequence {
    First,
    Additional,
}

/// `role.role` on a contributor: Crossref's own, and the fourteen `CRediT`
/// roles, all seen with `vocabulary: credit` on the looked-up works of
/// 2026-09-15, with `reviewer` (on `contributor`) and
/// `corresponding-author`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum RoleName {
    Author,
    Editor,
    Chair,
    Translator,
    Reviewer,
    CorrespondingAuthor,
    Conceptualization,
    DataCuration,
    FormalAnalysis,
    FundingAcquisition,
    Investigation,
    Methodology,
    ProjectAdministration,
    Resources,
    Software,
    Supervision,
    Validation,
    Visualization,
    WritingOriginalDraft,
    WritingReviewEditing,
}

/// `role.vocabulary` on a contributor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum Vocabulary {
    Crossref,
    /// `CRediT`, the contributor roles taxonomy.
    Credit,
}

/// `type` on a reference: the citing publisher's word for what it cites,
/// not the 30 work types — `conference-proceedings`, `preprint`,
/// `web-resource` and `software` are none of those. The deposit schema's
/// `citation@type` (5.4.0, 5.5.0), 22 words, which the API writes with a
/// hyphen for the schema's underscore; `blog-post`, `peer-review` and
/// `registered-report` have not been seen yet, so their spelling is that
/// rule's. The survey of the March 2026 file stopped at `patent` (IOP,
/// ECS) on 1,070 shards and `journal` on 64 (2026-09-26); one publisher
/// writes `journal` for an article in one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ReferenceType {
    Blog,
    BlogPost,
    Book,
    BookChapter,
    Collection,
    ConferencePaper,
    ConferenceProceedings,
    Dataset,
    Dissertation,
    Journal,
    JournalArticle,
    Other,
    Patent,
    PeerReview,
    Poster,
    Preprint,
    Protocol,
    RegisteredReport,
    Report,
    Software,
    Standard,
    WebResource,
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

/// What an update is, as the normalized record says it: the twelve
/// Crossmark update types, and a comment, which the Crossmark list lacks
/// and publishers deposited in its free-text years ([`UpdateWord`]).
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
    /// The notice comments on the work, or answers a comment on it
    /// (`IUCr`, RSC, Medical Physics): "Comment on …", "Response to …".
    Comment,
}

/// `type` on `update-to` and `updated-by`, as written. Crossmark's update
/// type was free text until deposit schema 4.3.5 (9 October 2014: "The
/// attribute supports custom types as well" before it), and records
/// deposited then still carry their publishers' words. Each word here is
/// read for what it means ([`UpdateWord::meaning`], the user's decisions
/// of 2026-09-26), and a word not listed still fails the parse. The
/// counts are entries among the records the March 2026 file's survey
/// stopped at, update-to and updated-by together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[allow(missing_docs)]
pub enum UpdateWord {
    #[serde(rename = "addendum")]
    Addendum,
    #[serde(rename = "clarification")]
    Clarification,
    #[serde(rename = "correction")]
    Correction,
    #[serde(rename = "corrigendum")]
    Corrigendum,
    #[serde(rename = "erratum")]
    Erratum,
    #[serde(rename = "expression_of_concern")]
    ExpressionOfConcern,
    #[serde(rename = "new_edition")]
    NewEdition,
    #[serde(rename = "new_version")]
    NewVersion,
    #[serde(rename = "partial_retraction")]
    PartialRetraction,
    #[serde(rename = "removal")]
    Removal,
    #[serde(rename = "retraction")]
    Retraction,
    #[serde(rename = "withdrawal")]
    Withdrawal,
    /// 6, five small publishers, 2013–22.
    #[serde(rename = "Erratum")]
    ErratumCapitalized,
    /// 5, Korean societies, 2011–14.
    #[serde(rename = "Retraction")]
    RetractionCapitalized,
    /// 2, the South African Journal of Science, 2018.
    #[serde(rename = "Corrigendum")]
    CorrigendumCapitalized,
    /// 8, PLOS, 2014–16.
    #[serde(rename = "expression-of-concern")]
    ExpressionOfConcernHyphenated,
    /// 2, Elsevier 2011: a notice titled "RETRACTED: …".
    #[serde(rename = "retration")]
    Retration,
    /// 1, the Journal of Breast Cancer 2018.
    #[serde(rename = "retracion")]
    Retracion,
    /// 409, Elsevier 2012–16, labelled "Erratum" on every one: its word
    /// for a correction notice of any title.
    #[serde(rename = "err")]
    Err,
    /// 70, APS 2018, always the work itself: corrected in place.
    #[serde(rename = "corrected")]
    Corrected,
    /// 70, Cambridge 2011–15, labelled "Corrected Article" — the label
    /// names the work the notice corrects: 32 of the 33 notices are titled
    /// "<its title> – ERRATUM" or "– CORRIGENDUM", 29 of them one page.
    #[serde(rename = "corrected-article")]
    CorrectedArticle,
    /// 6, AIP 2012: "Publisher's Note: …".
    #[serde(rename = "publisher-note")]
    PublisherNote,
    /// 26, `IUCr`, RSC.
    #[serde(rename = "comment")]
    Comment,
    /// 2, Medical Physics 2015: a letter titled "Comment on …".
    #[serde(rename = "correspondence")]
    Correspondence,
    /// 5, BMJ: the publisher did not know.
    #[serde(rename = "unknown")]
    Unknown,
    /// 2, RSC 2013: when the print version appeared.
    #[serde(rename = "print")]
    Print,
    /// 42, Medical Physics and AIP, always the work itself: its section.
    #[serde(rename = "article")]
    Article,
    /// A section, as `article` is.
    #[serde(rename = "tg-report")]
    TgReport,
    /// A section, as `article` is.
    #[serde(rename = "book-review")]
    BookReview,
    /// A section, as `article` is.
    #[serde(rename = "point-counterpoint")]
    PointCounterpoint,
    /// F1000's test deposits (`10.3410/f1000devtestarticle…`, 2012).
    #[serde(rename = "sample_update")]
    SampleUpdate,
    /// F1000's test deposits.
    #[serde(rename = "this_is_some_update_23")]
    ThisIsSomeUpdate23,
    /// Crossref's test prefix, `10.5555`.
    #[serde(rename = "interesting_update")]
    InterestingUpdate,
    /// Retraction Watch's, not a publisher's: on one Spandidos notice and
    /// the article it updates (10.3892/etm.2024.12720, 10.3892/etm.2018.6349),
    /// `68818` as both type and label beside `record-id: 69356` — an update
    /// whose kind the record does not say (the survey of the March 2026
    /// file, 2026-09-26).
    #[serde(rename = "68818")]
    RetractionWatch68818,
}

/// What an [`UpdateWord`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meaning {
    /// An update, of this type.
    Update(UpdateType),
    /// Nothing the model can keep as an update — no update at all (a
    /// section heading, a print date, the publisher not knowing) or one
    /// whose kind the record does not say: set aside and counted, not
    /// kept (the user, 2026-09-26: "drop and count").
    SetAside,
    /// A test deposit's: only on a record the checked-in exclusion list
    /// keeps out ([`crate::exclusions`]).
    TestDeposit,
}

impl UpdateWord {
    /// Every word, for the tests that keep the table whole.
    pub const ALL: [Self; 34] = [
        Self::Addendum,
        Self::Clarification,
        Self::Correction,
        Self::Corrigendum,
        Self::Erratum,
        Self::ExpressionOfConcern,
        Self::NewEdition,
        Self::NewVersion,
        Self::PartialRetraction,
        Self::Removal,
        Self::Retraction,
        Self::Withdrawal,
        Self::ErratumCapitalized,
        Self::RetractionCapitalized,
        Self::CorrigendumCapitalized,
        Self::ExpressionOfConcernHyphenated,
        Self::Retration,
        Self::Retracion,
        Self::Err,
        Self::Corrected,
        Self::CorrectedArticle,
        Self::PublisherNote,
        Self::Comment,
        Self::Correspondence,
        Self::Unknown,
        Self::Print,
        Self::Article,
        Self::TgReport,
        Self::BookReview,
        Self::PointCounterpoint,
        Self::SampleUpdate,
        Self::ThisIsSomeUpdate23,
        Self::InterestingUpdate,
        Self::RetractionWatch68818,
    ];

    /// What the word means. The twelve Crossmark words are themselves;
    /// capitals, a hyphen and two misspellings are the Crossmark word they
    /// spell; `err` is an erratum, Elsevier's own label on every one;
    /// `corrected`, `corrected-article` and `publisher-note` are
    /// corrections; `correspondence` is a comment.
    #[must_use]
    pub const fn meaning(self) -> Meaning {
        use UpdateType as T;
        Meaning::Update(match self {
            Self::Addendum => T::Addendum,
            Self::Clarification => T::Clarification,
            Self::Correction | Self::Corrected | Self::CorrectedArticle | Self::PublisherNote => {
                T::Correction
            }
            Self::Corrigendum | Self::CorrigendumCapitalized => T::Corrigendum,
            Self::Erratum | Self::ErratumCapitalized | Self::Err => T::Erratum,
            Self::ExpressionOfConcern | Self::ExpressionOfConcernHyphenated => {
                T::ExpressionOfConcern
            }
            Self::NewEdition => T::NewEdition,
            Self::NewVersion => T::NewVersion,
            Self::PartialRetraction => T::PartialRetraction,
            Self::Removal => T::Removal,
            Self::Retraction | Self::RetractionCapitalized | Self::Retration | Self::Retracion => {
                T::Retraction
            }
            Self::Withdrawal => T::Withdrawal,
            Self::Comment | Self::Correspondence => T::Comment,
            Self::Unknown
            | Self::Print
            | Self::Article
            | Self::TgReport
            | Self::BookReview
            | Self::PointCounterpoint
            | Self::RetractionWatch68818 => return Meaning::SetAside,
            Self::SampleUpdate | Self::ThisIsSomeUpdate23 | Self::InterestingUpdate => {
                return Meaning::TestDeposit;
            }
        })
    }

    /// Whether the word is one of Crossmark's twelve as they are spelled
    /// today, rather than an older word read for what it means.
    #[must_use]
    pub const fn is_crossmark(self) -> bool {
        matches!(
            self,
            Self::Addendum
                | Self::Clarification
                | Self::Correction
                | Self::Corrigendum
                | Self::Erratum
                | Self::ExpressionOfConcern
                | Self::NewEdition
                | Self::NewVersion
                | Self::PartialRetraction
                | Self::Removal
                | Self::Retraction
                | Self::Withdrawal
        )
    }

    /// The word as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Addendum => "addendum",
            Self::Clarification => "clarification",
            Self::Correction => "correction",
            Self::Corrigendum => "corrigendum",
            Self::Erratum => "erratum",
            Self::ExpressionOfConcern => "expression_of_concern",
            Self::NewEdition => "new_edition",
            Self::NewVersion => "new_version",
            Self::PartialRetraction => "partial_retraction",
            Self::Removal => "removal",
            Self::Retraction => "retraction",
            Self::Withdrawal => "withdrawal",
            Self::ErratumCapitalized => "Erratum",
            Self::RetractionCapitalized => "Retraction",
            Self::CorrigendumCapitalized => "Corrigendum",
            Self::ExpressionOfConcernHyphenated => "expression-of-concern",
            Self::Retration => "retration",
            Self::Retracion => "retracion",
            Self::Err => "err",
            Self::Corrected => "corrected",
            Self::CorrectedArticle => "corrected-article",
            Self::PublisherNote => "publisher-note",
            Self::Comment => "comment",
            Self::Correspondence => "correspondence",
            Self::Unknown => "unknown",
            Self::Print => "print",
            Self::Article => "article",
            Self::TgReport => "tg-report",
            Self::BookReview => "book-review",
            Self::PointCounterpoint => "point-counterpoint",
            Self::SampleUpdate => "sample_update",
            Self::ThisIsSomeUpdate23 => "this_is_some_update_23",
            Self::InterestingUpdate => "interesting_update",
            Self::RetractionWatch68818 => "68818",
        }
    }

    /// The word written as `text`, if it is one: a `relation` type too
    /// can be an update word, since Crossref copies a work's `updated-by`
    /// into its `relation` under the update's word.
    #[must_use]
    pub fn from_word(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.as_str() == text)
    }
}

/// `status.type` on posted content: what became of it (Crossref's posted
/// content schema). eLife writes `withdrawn` on a retracted reviewed
/// preprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum StatusType {
    Withdrawn,
    Removed,
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

/// `review.recommendation`: the deposit schema's (5.4.0, 5.5.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ReviewRecommendation {
    MajorRevision,
    MinorRevision,
    Reject,
    RejectWithResubmit,
    Accept,
    /// Tsinghua University Press's reviews; 8 shards of the March 2026
    /// file stopped at it (2026-09-26).
    AcceptWithReservation,
}

/// `review.type`: the deposit schema's (5.4.0, 5.5.0); `manuscript` and
/// `recommendation` not seen yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum ReviewType {
    RefereeReport,
    EditorReport,
    AuthorComment,
    CommunityComment,
    Aggregate,
    Manuscript,
    Recommendation,
}

/// `type` of a project's funding entry: the grant schema's 18 (`grant_id`
/// 0.2.0). The survey of the March 2026 file saw `award` stop 3 shards,
/// and the records it stopped at carry seven kinds (2026-09-26); `APC` and
/// `BPC` are the schema's spelling, not yet seen in the data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum FundingType {
    #[serde(rename = "APC")]
    Apc,
    Award,
    #[serde(rename = "BPC")]
    Bpc,
    Contract,
    Crowdfunding,
    Endowment,
    Equipment,
    Facilities,
    Fellowship,
    Grant,
    Infrastructure,
    Loan,
    Other,
    Prize,
    SalaryAward,
    Secondment,
    SeedFunding,
    TrainingGrant,
}

#[cfg(test)]
mod tests {
    use super::{
        ContentVersion, FundingType, IdKind, Meaning, ReferenceType, RelationIdKind, UpdateType,
        UpdateWord, WorkType,
    };

    #[test]
    fn every_update_word_is_its_spelling_and_has_a_meaning() {
        let mut seen = std::collections::BTreeSet::new();
        for w in UpdateWord::ALL {
            assert!(seen.insert(w.as_str()), "{} twice", w.as_str());
            let json = format!("\"{}\"", w.as_str());
            assert_eq!(serde_json::from_str::<UpdateWord>(&json).unwrap(), w);
            assert_eq!(serde_json::to_string(&w).unwrap(), json);
            assert_eq!(UpdateWord::from_word(w.as_str()), Some(w));
        }
        assert_eq!(UpdateWord::from_word("Err"), None);
        assert!(serde_json::from_str::<UpdateWord>("\"retractoin\"").is_err());
    }

    #[test]
    fn the_meanings_the_user_decided() {
        let read = |w: UpdateWord| match w.meaning() {
            Meaning::Update(t) => Some(t),
            _ => None,
        };
        assert_eq!(read(UpdateWord::Err), Some(UpdateType::Erratum));
        assert_eq!(
            read(UpdateWord::PublisherNote),
            Some(UpdateType::Correction)
        );
        assert_eq!(read(UpdateWord::Corrected), Some(UpdateType::Correction));
        assert_eq!(read(UpdateWord::Retration), Some(UpdateType::Retraction));
        assert_eq!(read(UpdateWord::Retracion), Some(UpdateType::Retraction));
        assert_eq!(
            read(UpdateWord::ExpressionOfConcernHyphenated),
            Some(UpdateType::ExpressionOfConcern)
        );
        assert_eq!(read(UpdateWord::Correspondence), Some(UpdateType::Comment));
        assert_eq!(
            read(UpdateWord::CorrectedArticle),
            Some(UpdateType::Correction)
        );
        for w in [
            UpdateWord::Article,
            UpdateWord::Print,
            UpdateWord::Unknown,
            UpdateWord::RetractionWatch68818,
        ] {
            assert_eq!(w.meaning(), Meaning::SetAside, "{}", w.as_str());
        }
        assert_eq!(UpdateWord::SampleUpdate.meaning(), Meaning::TestDeposit);
        assert_eq!(
            UpdateWord::ALL.iter().filter(|w| w.is_crossmark()).count(),
            12
        );
        for w in &UpdateWord::ALL[..12] {
            assert!(w.is_crossmark());
            let t = read(*w).unwrap();
            assert_eq!(
                serde_json::to_string(&t).unwrap(),
                format!("\"{}\"", w.as_str()),
                "a Crossmark word is its own type"
            );
        }
    }

    #[test]
    fn the_schema_lists_spell_as_the_api_writes() {
        assert_eq!(
            serde_json::from_str::<ReferenceType>("\"patent\"").unwrap(),
            ReferenceType::Patent
        );
        assert_eq!(
            serde_json::from_str::<ReferenceType>("\"registered-report\"").unwrap(),
            ReferenceType::RegisteredReport
        );
        assert_eq!(
            serde_json::from_str::<FundingType>("\"training-grant\"").unwrap(),
            FundingType::TrainingGrant
        );
        assert_eq!(
            serde_json::from_str::<FundingType>("\"APC\"").unwrap(),
            FundingType::Apc
        );
        assert!(serde_json::from_str::<FundingType>("\"apc\"").is_err());
        assert_eq!(
            serde_json::from_str::<RelationIdKind>("\"short-doi\"").unwrap(),
            RelationIdKind::ShortDoi
        );
    }

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
