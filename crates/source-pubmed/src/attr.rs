//! Enumerated attribute vocabularies, one enum per `<!ATTLIST>` token list
//! in the DTD. A value outside the list is a parse failure; there is no
//! `Other`.
//!
//! Two of the vocabularies here are not `<!ATTLIST>` token lists at all —
//! `PublicationStatus` and `AffiliationIdSource` — and at the end are two
//! tables that are not vocabularies but their opposite: the values a
//! `CDATA` field was found to carry that belong to no vocabulary, each
//! with what it turned out to be. A value in one of those is a wart we
//! have read and decided about; a value in neither still stops the run.

/// Declares an enum whose variants are exactly one DTD token list.
macro_rules! attr_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $variant:ident = $token:literal),+ $(,)?
    }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize)]
        $vis enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every token the DTD allows, in declaration order.
            pub const TOKENS: &'static [&'static str] = &[$($token),+];

            /// Parses exactly one of the DTD's tokens; case and whitespace
            /// are significant.
            #[must_use]
            pub fn parse(token: &str) -> Option<Self> {
                match token {
                    $($token => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// The DTD token.
            #[must_use]
            pub const fn token(self) -> &'static str {
                match self {
                    $(Self::$variant => $token),+
                }
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(self.token())
            }
        }
    };
}

attr_enum! {
    /// `(Y | N)` — `ValidYN`, `CompleteYN`, `MajorTopicYN`, `EqualContrib`.
    pub enum YesNo { Yes = "Y", No = "N" }
}

attr_enum! {
    /// `MedlineCitation Owner (NLM | NASA | PIP | KIE | HSR | HMD | NOTNLM) "NLM"`.
    pub enum Owner {
        Nlm = "NLM", Nasa = "NASA", Pip = "PIP", Kie = "KIE", Hsr = "HSR", Hmd = "HMD", NotNlm = "NOTNLM",
    }
}

attr_enum! {
    /// `GeneralNote Owner (NLM | NASA | PIP | KIE | HSR | HMD) "NLM"`.
    pub enum NoteOwner { Nlm = "NLM", Nasa = "NASA", Pip = "PIP", Kie = "KIE", Hsr = "HSR", Hmd = "HMD" }
}

attr_enum! {
    /// `MedlineCitation Status (...) #REQUIRED`.
    pub enum Status {
        Completed = "Completed",
        InProcess = "In-Process",
        PubMedNotMedline = "PubMed-not-MEDLINE",
        InDataReview = "In-Data-Review",
        Publisher = "Publisher",
        Medline = "MEDLINE",
        OldMedline = "OLDMEDLINE",
    }
}

attr_enum! {
    /// `Article PubModel (...) #REQUIRED`.
    pub enum PubModel {
        Print = "Print",
        PrintElectronic = "Print-Electronic",
        Electronic = "Electronic",
        ElectronicPrint = "Electronic-Print",
        ElectronicECollection = "Electronic-eCollection",
    }
}

attr_enum! {
    /// `AbstractText NlmCategory (...) #IMPLIED`.
    pub enum NlmCategory {
        Background = "BACKGROUND",
        Objective = "OBJECTIVE",
        Methods = "METHODS",
        Results = "RESULTS",
        Conclusions = "CONCLUSIONS",
        Unassigned = "UNASSIGNED",
    }
}

attr_enum! {
    /// `AuthorList Type (authors | editors) #IMPLIED`.
    pub enum AuthorListType { Authors = "authors", Editors = "editors" }
}

attr_enum! {
    /// `ISSN IssnType (Electronic | Print) #REQUIRED`.
    pub enum IssnType { Electronic = "Electronic", Print = "Print" }
}

attr_enum! {
    /// `JournalIssue CitedMedium (Internet | Print) #REQUIRED`.
    pub enum CitedMedium { Internet = "Internet", Print = "Print" }
}

attr_enum! {
    /// `ELocationID EIdType (doi | pii) #REQUIRED`.
    pub enum EIdType { Doi = "doi", Pii = "pii" }
}

attr_enum! {
    /// `CommentsCorrections RefType (...) #REQUIRED`.
    pub enum RefType {
        AssociatedDataset = "AssociatedDataset",
        AssociatedPublication = "AssociatedPublication",
        CommentIn = "CommentIn",
        CommentOn = "CommentOn",
        CorrectedAndRepublishedIn = "CorrectedandRepublishedIn",
        CorrectedAndRepublishedFrom = "CorrectedandRepublishedFrom",
        ErratumIn = "ErratumIn",
        ErratumFor = "ErratumFor",
        ExpressionOfConcernIn = "ExpressionOfConcernIn",
        ExpressionOfConcernFor = "ExpressionOfConcernFor",
        RepublishedIn = "RepublishedIn",
        RepublishedFrom = "RepublishedFrom",
        RetractedAndRepublishedIn = "RetractedandRepublishedIn",
        RetractedAndRepublishedFrom = "RetractedandRepublishedFrom",
        RetractionIn = "RetractionIn",
        RetractionOf = "RetractionOf",
        UpdateIn = "UpdateIn",
        UpdateOf = "UpdateOf",
        SummaryForPatientsIn = "SummaryForPatientsIn",
        OriginalReportIn = "OriginalReportIn",
        ReprintIn = "ReprintIn",
        ReprintOf = "ReprintOf",
        Cites = "Cites",
    }
}

attr_enum! {
    /// `DescriptorName Type (Geographic) #IMPLIED`.
    pub enum DescriptorType { Geographic = "Geographic" }
}

attr_enum! {
    /// `SupplMeshName Type (...) #REQUIRED`.
    pub enum SupplMeshType {
        Disease = "Disease",
        Protocol = "Protocol",
        Organism = "Organism",
        Anatomy = "Anatomy",
        Population = "Population",
    }
}

attr_enum! {
    /// `OtherID Source (...) #REQUIRED`.
    pub enum OtherIdSource {
        Nasa = "NASA", Kie = "KIE", Pip = "PIP", Pop = "POP", Arpl = "ARPL", Cpc = "CPC",
        Ind = "IND", Cpfh = "CPFH", Clml = "CLML", Nrcbl = "NRCBL", Nlm = "NLM", Qcim = "QCIM",
    }
}

attr_enum! {
    /// `OtherAbstract Type (...) #REQUIRED`.
    pub enum OtherAbstractType {
        Aamc = "AAMC",
        Aids = "AIDS",
        Kie = "KIE",
        Pip = "PIP",
        Nasa = "NASA",
        Publisher = "Publisher",
        PlainLanguageSummary = "plain-language-summary",
    }
}

attr_enum! {
    /// `KeywordList Owner (...) "NLM"`.
    pub enum KeywordOwner {
        Nlm = "NLM", NlmAuto = "NLM-AUTO", Nasa = "NASA", Pip = "PIP", Kie = "KIE", NotNlm = "NOTNLM", Hhs = "HHS",
    }
}

attr_enum! {
    /// `PubMedPubDate PubStatus (...) #REQUIRED`.
    pub enum PubStatus {
        Received = "received",
        Accepted = "accepted",
        Epublish = "epublish",
        Ppublish = "ppublish",
        Revised = "revised",
        AheadOfPrint = "aheadofprint",
        Retracted = "retracted",
        Ecollection = "ecollection",
        Pmc = "pmc",
        Pmcr = "pmcr",
        Pubmed = "pubmed",
        Pubmedr = "pubmedr",
        Premedline = "premedline",
        Medline = "medline",
        Medliner = "medliner",
        Entrez = "entrez",
        PmcRelease = "pmc-release",
    }
}

attr_enum! {
    /// `ArticleId IdType (...) "pubmed"`.
    pub enum IdType {
        Doi = "doi",
        Pii = "pii",
        Pmcpid = "pmcpid",
        Pmpid = "pmpid",
        Pmc = "pmc",
        Mid = "mid",
        Sici = "sici",
        Pubmed = "pubmed",
        Medline = "medline",
        Pmcid = "pmcid",
        Pmcbook = "pmcbook",
        BookAccession = "bookaccession",
    }
}

attr_enum! {
    /// The text of `PublicationStatus`: not an attribute, but a closed list
    /// all the same (NLM documents these three).
    pub enum PublicationStatus {
        PrintPublished = "ppublish",
        ElectronicPublished = "epublish",
        AheadOfPrint = "aheadofprint",
    }
}

attr_enum! {
    /// `AffiliationInfo/Identifier@Source`: `CDATA` in the DTD, these four in
    /// the data — the four organization registries. What else the data
    /// carries is in [`NOT_AN_ORGANIZATION_ID`].
    pub enum AffiliationIdSource {
        Ror = "ROR",
        Grid = "GRID",
        Isni = "ISNI",
        Ringgold = "RINGGOLD",
    }
}

/// `AffiliationInfo/Identifier@Source` values that name no organization
/// registry, each with what it turned out to be.
///
/// `Source` is `CDATA`, so the element carries whatever the publisher's own
/// XML carried, and the first survey of the whole 2026 baseline (2026-09-23)
/// found 253 records in ~37M whose token is none of the four. Every token it
/// sampled — three per share, over eight shares — is the data being wrong in
/// a way we can name, not a registry we had not met:
///
/// * `ORCID`, all but a handful of the 253: the author's own ORCID, filed
///   inside `<AffiliationInfo>` instead of on the `<Author>` beside it, bare
///   (`0000-0002-3142-0388`, PMID 34617426) or as a URL (PMID 28032483). A
///   person identifier in an organization's slot. Keeping it would have the
///   record assert that an institution has an ORCID, which is not something
///   any reader downstream should be able to be told. Moving it to the author
///   instead would be a repair, not a reading: `Author/Identifier` treats a
///   second ORCID as an inconsistency, and deciding that a publisher's
///   misfiling is that author's identifier is a decision for the model, not
///   for a vocabulary table.
/// * `RINGGOLD RINGGOLD`: a Ringgold identifier with the attribute and the
///   value each doubled — `Source="RINGGOLD RINGGOLD"` over `643660 643660`,
///   on six of the ten affiliations of PMID 38329060, whose other four carry
///   a plain `RINGGOLD`. Halving it would be a repair too, and one that
///   guesses at the shape of whatever produced the doubling.
/// * `ES-UID`: Elsevier's own key for the affiliation line, its PII and a
///   hash (`S2352077918300761-…`, PMID 37312292). It identifies a string in
///   Elsevier's system, resolves nowhere, and names no institution.
///
/// So the four registries stay the vocabulary, an identifier whose source is
/// one of these is dropped, and the affiliation text — which is the part
/// that says who — is kept either way. Three examples per share is a sample,
/// not the list, so a fourth token may be out there; it stops the run, which
/// is what a closed vocabulary is for.
pub const NOT_AN_ORGANIZATION_ID: [(&str, &str); 3] = [
    (
        "ES-UID",
        "Elsevier's own key for the affiliation line, not a registry entry",
    ),
    (
        "ORCID",
        "a person's ORCID, filed in the affiliation envelope instead of on the author",
    ),
    (
        "RINGGOLD RINGGOLD",
        "a Ringgold identifier with the attribute and the value each doubled",
    ),
];

/// What an `AffiliationInfo/Identifier@Source` token is, when it is not one
/// of [`AffiliationIdSource`]'s registries and [`NOT_AN_ORGANIZATION_ID`]
/// has read it. `None` for anything else, which stops the run.
#[must_use]
pub fn not_an_organization_id(token: &str) -> Option<&'static str> {
    NOT_AN_ORGANIZATION_ID
        .iter()
        .find(|(t, _)| *t == token)
        .map(|&(_, what)| what)
}

/// `Language` values that are not an ISO 639-2 code in the bibliographic
/// form NLM documents, each with what it turned out to be.
///
/// NLM's language table is 59 codes, all of them ISO 639-2/B, and every one
/// of them parses; a PubMed search for a record indexed under any language
/// outside that table returns nothing (both checked 2026-09-23). One record
/// in ~37M disagrees, and it is the only `Language` the baseline survey
/// could not read: PMID 40316125, an English paper in Acta Biomaterialia on
/// whether brain white matter behaves the same in every direction, carries
/// `<Language>eng</Language><Language>crg</Language>`. `crg` is not ISO
/// 639-2 and not in NLM's table; ISO 639-3 gives it to Michif, a Canadian
/// Métis language, which the paper is plainly not written in — `grc`,
/// Ancient Greek, transposed is as good a guess as any at where it came
/// from. It is one publisher's stray token, so the record keeps `eng` and
/// the token is dropped.
///
/// The ISO table is not the place to fix this. It is generated from the
/// registry (`tools/gen_lang_table.py`), a `Lang` is stored as its index
/// into that table, and admitting a code ISO never had would cost every
/// normalized artifact — for one record, about a language it is not in.
pub const NOT_A_LANGUAGE: [(&str, &str); 1] = [(
    "crg",
    "a stray token on an English paper; no ISO 639-2 code, and ISO 639-3's Michif is not it",
)];

/// What a `Language` is, when it is not an ISO 639-2 bibliographic code and
/// [`NOT_A_LANGUAGE`] has read it. `None` for anything else, which stops the
/// run.
#[must_use]
pub fn not_a_language(code: &str) -> Option<&'static str> {
    NOT_A_LANGUAGE
        .iter()
        .find(|(c, _)| *c == code)
        .map(|&(_, what)| what)
}

#[cfg(test)]
mod tests {
    use super::{
        AffiliationIdSource, IdType, NOT_A_LANGUAGE, NOT_AN_ORGANIZATION_ID, RefType, Status,
        YesNo, not_a_language, not_an_organization_id,
    };

    #[test]
    fn tokens_roundtrip_and_nothing_else_parses() {
        for token in Status::TOKENS {
            assert_eq!(Status::parse(token).map(Status::token), Some(*token));
        }
        assert_eq!(Status::parse("medline"), None, "case is significant");
        assert_eq!(Status::parse(" MEDLINE"), None, "whitespace is significant");
        assert_eq!(YesNo::parse("y"), None);
        assert_eq!(IdType::parse("pmcid"), Some(IdType::Pmcid));
        assert_eq!(IdType::parse("pmc"), Some(IdType::Pmc));
        assert_eq!(RefType::TOKENS.len(), 23);
        assert_eq!(RefType::Cites.to_string(), "Cites");
    }

    #[test]
    fn the_four_registries_parse_and_the_warts_are_told_apart_from_them() {
        for token in AffiliationIdSource::TOKENS {
            assert!(AffiliationIdSource::parse(token).is_some(), "{token}");
            assert_eq!(not_an_organization_id(token), None, "{token}");
        }
        for (token, what) in NOT_AN_ORGANIZATION_ID {
            assert_eq!(AffiliationIdSource::parse(token), None, "{token}");
            assert_eq!(not_an_organization_id(token), Some(what), "{token}");
        }
        // Neither: the token stops the run rather than being dropped.
        for token in ["Ringgold", "ror", "WIKIDATA", "", "RINGGOLD RINGGOLD "] {
            assert_eq!(AffiliationIdSource::parse(token), None, "{token:?}");
            assert_eq!(not_an_organization_id(token), None, "{token:?}");
        }
    }

    #[test]
    fn only_the_language_the_survey_read_is_not_a_language() {
        assert!(not_a_language("crg").is_some());
        for code in NOT_A_LANGUAGE {
            assert!(model::Lang::from_bibliographic(code.0).is_err(), "{code:?}");
        }
        for code in ["eng", "crg ", "CRG", "grc", "und", ""] {
            assert_eq!(not_a_language(code), None, "{code:?}");
        }
    }
}
