//! Enumerated attribute vocabularies, one enum per `<!ATTLIST>` token list
//! in the DTD. A value outside the list is a parse failure; there is no
//! `Other`.

/// Declares an enum whose variants are exactly one DTD token list.
macro_rules! attr_enum {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $variant:ident = $token:literal),+ $(,)?
    }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// the data.
    pub enum AffiliationIdSource {
        Ror = "ROR",
        Grid = "GRID",
        Isni = "ISNI",
        Ringgold = "RINGGOLD",
    }
}

#[cfg(test)]
mod tests {
    use super::{IdType, RefType, Status, YesNo};

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
}
