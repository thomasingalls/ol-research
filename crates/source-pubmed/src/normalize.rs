//! Native record → normalized records.
//!
//! This is where value-level strictness lives (plan §6): identifiers,
//! dates and vocabularies become `model` types, and a value that does not
//! fit stops the run with the PMID, the field and the raw text. The field
//! set covers everything observed in the first baseline file; DTD content
//! the file did not contain (`GeneSymbolList`, `OtherID`, `ObjectList`, a
//! `Second` in a history date, …) is an error on first sight, which is how
//! it gets modelled.
//!
//! Mixed-content fields — title, abstract, keywords, affiliations, citation
//! text — hold the raw inner markup, borrowed, with no interpretation:
//! `<sub>`, `<sup>`, `<i>` and MathML are kept exactly as serialized. What
//! to make of them is deferred until something needs to (decided
//! 2026-09-15).
//!
//! Records are pushed to an [`Emit`] by reference as they are built.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::fmt;

use ingest::Sink;
use model::{
    Claim, Date, DateError, Day, Doi, DoiError, Interned, Issn, IssnError, Lang, LangError,
    MedlineDateError, MeshUi, MeshUiError, Month, Orcid, OrcidError, ParseError, PartialDate,
    PmcId, Pmid, Season, TimeOfDay, Year,
};

use crate::Pubmed;
use crate::attr::{
    AffiliationIdSource, CitedMedium, EIdType, IdType, IssnType, KeywordOwner, NlmCategory,
    NoteOwner, OtherAbstractType, OtherIdSource, Owner, PubModel, PubStatus, PublicationStatus,
    RefType, Status, SupplMeshType, YesNo,
};
use crate::record::{
    self, AbstractText, ArticleId, AuthorName as NativeAuthorName, Date as DateText,
    DeleteCitation, MedlineCitation, Pagination as NativePagination,
    PersonName as NativePersonName, PubDate, PubmedArticle, ReferenceList as ReferenceListRecord,
};
use crate::text::RawXml;

/// A list with room for its usual length inside the record itself (plan
/// §5, rule 1); only a longer one allocates. The lengths come from the
/// observed distributions in the plan's data, about the 90th percentile.
pub type Few<T, const N: usize> = SmallVec<[T; N]>;

/// A DOI as a source asserted it.
pub type DoiClaim<'n> = Claim<'n, Doi<'n>, DoiError>;
/// A PMID as a reference asserted it; pubmed26n1333 had a PMC id and a
/// DOI under `IdType="pubmed"`.
pub type PmidClaim<'n> = Claim<'n, Pmid, ParseError>;
/// A PMC id as a reference asserted it.
pub type PmcClaim<'n> = Claim<'n, PmcId, ParseError>;
/// An ISSN as the record asserted it; pubmed26n1333 had one with a wrong
/// check digit, the DOI-claim policy of plan §6 applied.
pub type IssnClaim<'n> = Claim<'n, Issn, IssnError>;
/// A language code as the source wrote it, or why it is not ISO 639-2:
/// `sp` on one `OtherAbstract` in 15,432 looked-up records (2026-09-15).
pub type LangClaim<'n> = Claim<'n, Lang, LangError>;
/// An ORCID as an author typed it: 17 of 7,607 in the first file were
/// truncated to fifteen characters.
pub type OrcidClaim<'n> = Claim<'n, Orcid, OrcidError>;

/// One PubMed record, normalized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation<'n> {
    /// The record's identifier.
    pub pmid: Pmid,
    /// `PMID@Version`.
    pub version: u32,
    /// Indexing status: `Publisher`, `In-Process`, `MEDLINE`, …
    pub status: Status,
    /// `MedlineCitation@Owner`.
    pub owner: Owner,
    /// `MedlineCitation@IndexingMethod`: `Automated` or `Curated`, free text
    /// in the DTD.
    #[serde(borrow)]
    pub indexing_method: Option<&'n str>,
    /// The record's other identifiers.
    #[serde(borrow)]
    pub ids: Identifiers<'n>,
    /// The record's dates.
    pub dates: Dates,
    /// The journal and issue.
    #[serde(borrow)]
    pub journal: Journal<'n>,
    /// `ArticleTitle`, raw markup; empty on 14 of 4,989.
    pub title: &'n str,
    /// `VernacularTitle`, raw markup.
    #[serde(borrow)]
    pub vernacular_title: Option<&'n str>,
    /// `Abstract`.
    pub r#abstract: Option<Abstract<'n>>,
    /// `OtherAbstract*`.
    #[serde(borrow)]
    pub other_abstracts: Few<OtherAbstract<'n>, 2>,
    /// `Language+`, ISO 639-2 bibliographic codes.
    pub languages: Few<Lang, 1>,
    /// `PublicationType+`.
    #[serde(borrow)]
    pub publication_types: Few<MeshTerm<'n>, 4>,
    /// `AuthorList/Author*`.
    #[serde(borrow)]
    pub authors: Few<Author<'n>, 8>,
    /// `InvestigatorList*`, each with the `ID` a `CollectiveName` may point at.
    #[serde(borrow)]
    pub investigator_lists: Few<InvestigatorList<'n>, 2>,
    /// `MeshHeadingList`.
    #[serde(borrow)]
    pub mesh: Few<MeshHeading<'n>, 16>,
    /// `SupplMeshList`.
    #[serde(borrow)]
    pub suppl_mesh: Few<SupplMesh<'n>, 2>,
    /// `ChemicalList`.
    #[serde(borrow)]
    pub chemicals: Few<Chemical<'n>, 8>,
    /// Every `Keyword` of every `KeywordList`, with its list's owner.
    #[serde(borrow)]
    pub keywords: Few<Keyword<'n>, 8>,
    /// `GrantList`.
    #[serde(borrow)]
    pub grants: Few<Grant<'n>, 4>,
    /// `DataBankList`.
    #[serde(borrow)]
    pub data_banks: Few<DataBank<'n>, 2>,
    /// `CoiStatement`, raw markup.
    #[serde(borrow)]
    pub coi_statement: Option<&'n str>,
    /// `NumberOfReferences`, on older MEDLINE records (362 of 15,432
    /// looked up on 2026-09-15).
    pub number_of_references: Option<u32>,
    /// `GeneSymbolList/GeneSymbol+`, on records of the 1990s (9 of 15,432).
    #[serde(borrow)]
    pub gene_symbols: Few<&'n str, 4>,
    /// `PersonalNameSubjectList`: who a biographical record is about (1 of
    /// 15,432).
    #[serde(borrow)]
    pub personal_name_subjects: Few<PersonName<'n>, 1>,
    /// `OtherID*`: identifiers from other databases, by source (59 of
    /// 15,432).
    #[serde(borrow)]
    pub other_ids: Few<OtherId<'n>, 1>,
    /// `GeneralNote*`, by owner, on records of the 1990s (6 of 15,432).
    #[serde(borrow)]
    pub general_notes: Few<GeneralNote<'n>, 1>,
    /// `CitationSubset*`: `IM` and friends.
    #[serde(borrow)]
    pub citation_subsets: Few<&'n str, 2>,
    /// `PubmedData/History`.
    pub history: Few<HistoryEntry, 8>,
    /// `PubmedData/PublicationStatus`.
    pub publication_status: Option<PublicationStatus>,
    /// `CommentsCorrections*`: errata, retractions, comments, by kind.
    pub relations: Few<Relation, 4>,
}

/// `GeneralNote`: free text with an owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralNote<'n> {
    /// `Owner`.
    pub owner: NoteOwner,
    #[allow(missing_docs)]
    pub text: &'n str,
}

/// `OtherID`: an identifier from another database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherId<'n> {
    /// `Source`.
    pub source: OtherIdSource,
    #[allow(missing_docs)]
    pub value: &'n str,
}

/// The record's identifiers beyond its PMID.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Identifiers<'n> {
    /// `ArticleId IdType="doi"` — the record's own DOI as it asserts it.
    #[serde(borrow)]
    pub doi: Option<DoiClaim<'n>>,
    /// `ELocationID EIdType="doi"` — the same identifier from the article
    /// block, once or, on 5 of 15,432 looked-up records, twice (a case
    /// variant, or a value that is not a DOI beside one that is). Every
    /// valid one agrees with `doi`.
    #[serde(borrow)]
    pub elocation_doi: Few<DoiClaim<'n>, 1>,
    /// `ArticleId IdType="pii"`, an opaque publisher item identifier.
    #[serde(borrow)]
    pub pii: Option<&'n str>,
    /// `mid`: a manuscript id on a record with an author manuscript —
    /// NIH's `NIHMS…` or Europe PMC's `EMS…` (both first seen in
    /// pubmed26n1333, PMIDs 41576088 and 41583386).
    pub mid: Option<&'n str>,
    /// `ELocationID EIdType="pii"`, which need not equal `pii`; twice on
    /// 87 of 15,432 looked-up records.
    #[serde(borrow)]
    pub elocation_pii: Few<&'n str, 1>,
    /// `ArticleId IdType="pmc"`.
    pub pmc: Option<PmcId>,
    /// `ArticleId IdType="medline"`: an old MEDLINE unique identifier (28
    /// of 15,432).
    pub medline: Option<&'n str>,
    /// `ArticleId IdType="sici"`: a Serial Item and Contribution
    /// Identifier (11 of 15,432).
    pub sici: Option<&'n str>,
}

/// The record's dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dates {
    /// `JournalIssue/PubDate`, at the precision the record gave.
    pub published: PartialDate,
    /// `ArticleDate`, the electronic publication date; at most one is
    /// modelled.
    pub electronic: Option<Date>,
    /// `DateRevised`.
    pub revised: Option<Date>,
    /// `DateCompleted`.
    pub completed: Option<Date>,
}

/// The journal and the issue the article appeared in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Journal<'n> {
    /// `Journal/Title`.
    #[serde(borrow)]
    pub title: Option<Interned<'n>>,
    /// `Journal/ISOAbbreviation`.
    #[serde(borrow)]
    pub iso_abbreviation: Option<Interned<'n>>,
    /// `MedlineJournalInfo/MedlineTA`.
    #[serde(borrow)]
    pub medline_ta: Interned<'n>,
    /// `MedlineJournalInfo/NlmUniqueID`: digits, sometimes with a trailing
    /// letter.
    #[serde(borrow)]
    pub nlm_unique_id: Option<Interned<'n>>,
    /// `Journal/ISSN` with its type, as claimed.
    #[serde(borrow)]
    pub issn: Option<TypedIssn<'n>>,
    /// `MedlineJournalInfo/ISSNLinking`, as claimed.
    #[serde(borrow)]
    pub issn_linking: Option<IssnClaim<'n>>,
    /// `MedlineJournalInfo/Country`, NLM's spelling (`United States`,
    /// `England`, `Korea (South)`).
    #[serde(borrow)]
    pub country: Option<Interned<'n>>,
    /// `Article@PubModel`.
    pub pub_model: PubModel,
    /// `JournalIssue@CitedMedium`.
    pub cited_medium: CitedMedium,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issue: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub pagination: Option<Pagination<'n>>,
}

/// An ISSN with its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedIssn<'n> {
    #[allow(missing_docs)]
    pub kind: IssnType,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issn: IssnClaim<'n>,
}

/// `Pagination`, as text: `1-8`, `e2025072830`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pagination<'n> {
    /// `StartPage`, `EndPage?`, `MedlinePgn?`.
    Pages {
        #[allow(missing_docs)]
        start: &'n str,
        #[allow(missing_docs)]
        #[serde(borrow)]
        end: Option<&'n str>,
        #[allow(missing_docs)]
        #[serde(borrow)]
        medline: Option<&'n str>,
    },
    /// `MedlinePgn` alone.
    Medline(&'n str),
}

/// `Abstract`: labelled sections plus a copyright line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abstract<'n> {
    /// `AbstractText+`.
    #[serde(borrow)]
    pub sections: Few<Section<'n>, 4>,
    /// `CopyrightInformation?`.
    #[serde(borrow)]
    pub copyright: Option<&'n str>,
}

/// One `AbstractText`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section<'n> {
    /// `Label`, the publisher's heading (`RESULTS`, `Résultats`).
    #[serde(borrow)]
    pub label: Option<&'n str>,
    /// `NlmCategory`, NLM's normalization of the heading.
    pub category: Option<NlmCategory>,
    /// The paragraph, raw markup.
    pub text: &'n str,
}

/// `OtherAbstract`: a publisher's or plain-language abstract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtherAbstract<'n> {
    #[allow(missing_docs)]
    pub kind: OtherAbstractType,
    /// `Language`, ISO 639-2 bibliographic, or what the source wrote when
    /// it is not one.
    #[serde(borrow)]
    pub language: LangClaim<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub sections: Few<Section<'n>, 4>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub copyright: Option<&'n str>,
}

/// A MeSH-keyed term: publication types, descriptors, qualifiers,
/// substances, supplementary concepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshTerm<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub ui: MeshUi<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Interned<'n>,
}

/// An author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: AuthorName<'n>,
    /// `Identifier Source="ORCID"`, in any of its three forms.
    #[serde(borrow)]
    pub orcid: Option<OrcidClaim<'n>>,
    /// `EqualContrib`.
    pub equal_contrib: Option<bool>,
    /// `ValidYN`.
    pub valid: bool,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub affiliations: Few<Affiliation<'n>, 2>,
}

/// A person or a collective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorName<'n> {
    #[allow(missing_docs)]
    Person(#[serde(borrow)] PersonName<'n>),
    /// `CollectiveName`, raw markup, and the `InvestigatorList` it points at.
    Collective {
        #[allow(missing_docs)]
        name: &'n str,
        #[allow(missing_docs)]
        #[serde(borrow)]
        investigators: Option<&'n str>,
    },
}

/// `LastName, ForeName?, Initials?, Suffix?`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonName<'n> {
    #[allow(missing_docs)]
    pub last: &'n str,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub fore: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub initials: Option<&'n str>,
    /// Raw markup.
    #[serde(borrow)]
    pub suffix: Option<&'n str>,
}

/// `AffiliationInfo`: the text and its identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Affiliation<'n> {
    /// Raw markup.
    pub text: &'n str,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub ids: Few<AffiliationId<'n>, 2>,
}

/// An affiliation identifier, as written: ROR URL, GRID id, spaced ISNI,
/// Ringgold number. Unvalidated here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliationId<'n> {
    #[allow(missing_docs)]
    pub source: AffiliationIdSource,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub value: Interned<'n>,
}

/// `InvestigatorList`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigatorList<'n> {
    /// `ID`, the target of a `CollectiveName@Investigators`.
    #[serde(borrow)]
    pub id: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub investigators: Few<Investigator<'n>, 8>,
}

/// An investigator: a person with the same identifiers an author has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Investigator<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: PersonName<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub orcid: Option<OrcidClaim<'n>>,
    #[allow(missing_docs)]
    pub valid: bool,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub affiliations: Few<Affiliation<'n>, 2>,
}

/// A MeSH heading: descriptor plus qualifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshHeading<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub descriptor: MeshTerm<'n>,
    /// `MajorTopicYN` on the descriptor.
    pub major: bool,
    /// `Type="Geographic"`.
    pub geographic: bool,
    /// `AutoHM`: assigned by NLM's automated indexer.
    pub auto: bool,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub qualifiers: Few<Qualifier<'n>, 4>,
}

/// A MeSH qualifier on a heading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qualifier<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub term: MeshTerm<'n>,
    #[allow(missing_docs)]
    pub major: bool,
    #[allow(missing_docs)]
    pub auto: bool,
}

/// A supplementary concept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplMesh<'n> {
    #[allow(missing_docs)]
    pub kind: SupplMeshType,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub term: MeshTerm<'n>,
}

/// A chemical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chemical<'n> {
    /// `0`, a CAS number, an EC number or a UNII, as text.
    #[serde(borrow)]
    pub registry_number: Interned<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub substance: MeshTerm<'n>,
}

/// A keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keyword<'n> {
    /// The owner of the list it came from.
    pub owner: KeywordOwner,
    /// Raw markup; edge whitespace and empties occur.
    pub text: &'n str,
    #[allow(missing_docs)]
    pub major: bool,
}

/// A grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub id: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub acronym: Option<Interned<'n>>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub agency: Interned<'n>,
    /// `Country`; an empty element (92 % of grants) is `None`.
    #[serde(borrow)]
    pub country: Option<Interned<'n>>,
}

/// A data bank with accession numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataBank<'n> {
    /// `ClinicalTrials.gov`, `Dryad`, `GEO`, …
    #[serde(borrow)]
    pub name: Interned<'n>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub accessions: Few<&'n str, 4>,
}

/// One `PubMedPubDate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    #[allow(missing_docs)]
    pub status: PubStatus,
    #[allow(missing_docs)]
    pub date: Date,
    /// Local time with no zone, when given.
    pub time: Option<TimeOfDay>,
}

/// A `CommentsCorrections` entry: how this record relates to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    /// `ErratumFor`, `RetractionOf`, `CommentOn`, …
    pub kind: RefType,
    /// The other record, when PubMed identified it.
    pub pmid: Option<Pmid>,
}

/// One entry of a record's reference list: an edge whose target is
/// identified by whichever of three namespaces the source supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference<'n> {
    /// `ArticleId IdType="pubmed"`, as claimed.
    #[serde(borrow)]
    pub pmid: Option<PmidClaim<'n>>,
    /// `ArticleId IdType="doi"`.
    #[serde(borrow)]
    pub doi: Option<DoiClaim<'n>>,
    /// `ArticleId IdType="pmcid"`, as claimed.
    #[serde(borrow)]
    pub pmc: Option<PmcClaim<'n>>,
    /// The citation text, raw markup.
    pub citation: &'n str,
}

/// A value that did not fit the model.
#[derive(Debug)]
pub struct Error {
    /// The record, once its PMID was readable.
    pub pmid: Option<Pmid>,
    /// The field, as an element path fragment.
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
    Date(DateError),
    MedlineDate(MedlineDateError),
    MeshUi(MeshUiError),
    Issn(IssnError),
    Lang(LangError),
    /// A closed vocabulary that the value is not in.
    Vocabulary(&'static str),
    /// Two fields that must agree do not.
    Inconsistent(&'static str),
    /// Allowed by the DTD, not modelled here yet.
    Unmodelled(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.pmid {
            Some(pmid) => write!(f, "PMID {pmid}: ")?,
            None => f.write_str("PMID unknown: ")?,
        }
        write!(f, "{} = {:?}: ", self.field, self.raw)?;
        fmt::Display::fmt(&self.kind, f)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::Id(e) => write!(f, "{e}"),
            ErrorKind::Date(e) => write!(f, "{e}"),
            ErrorKind::MedlineDate(e) => write!(f, "{e}"),
            ErrorKind::MeshUi(e) => write!(f, "{e}"),
            ErrorKind::Issn(e) => write!(f, "{e}"),
            ErrorKind::Lang(e) => write!(f, "{e}"),
            ErrorKind::Vocabulary(what) => write!(f, "not in the vocabulary of {what}"),
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

/// Error context: the PMID once known.
struct Ctx {
    pmid: Option<Pmid>,
}

impl Ctx {
    fn err(&self, field: &'static str, raw: &str, kind: ErrorKind) -> Error {
        Error {
            pmid: self.pmid,
            field,
            raw: raw.to_owned(),
            kind,
        }
    }

    fn date(&self, field: &'static str, d: &DateText<'_>) -> Result<Date, Error> {
        let year =
            Year::parse(&d.year).map_err(|e| self.err(field, &d.year, ErrorKind::Date(e)))?;
        let month =
            Month::parse(&d.month).map_err(|e| self.err(field, &d.month, ErrorKind::Date(e)))?;
        let day = Day::parse(&d.day).map_err(|e| self.err(field, &d.day, ErrorKind::Date(e)))?;
        Date::new(year, month, day).map_err(|e| {
            self.err(
                field,
                &format!("{}-{}-{}", d.year, d.month, d.day),
                ErrorKind::Date(e),
            )
        })
    }

    fn published(&self, pub_date: &PubDate<'_>) -> Result<PartialDate, Error> {
        const FIELD: &str = "JournalIssue/PubDate";
        let (year, month, day, season) = match pub_date {
            PubDate::Medline(text) => {
                return PartialDate::parse_medline(text)
                    .map_err(|e| self.err(FIELD, text, ErrorKind::MedlineDate(e)));
            }
            PubDate::Date {
                year,
                month,
                day,
                season,
            } => (year, month, day, season),
        };
        let y = Year::parse(year).map_err(|e| self.err(FIELD, year, ErrorKind::Date(e)))?;
        let m = month
            .as_deref()
            .map(|m| Month::parse(m).map_err(|e| self.err(FIELD, m, ErrorKind::Date(e))))
            .transpose()?;
        let d = day
            .as_deref()
            .map(|d| Day::parse(d).map_err(|e| self.err(FIELD, d, ErrorKind::Date(e))))
            .transpose()?;
        // `Season` holds a month range as often as a season on older
        // records — `Sep-Oct`, `Jan-Dec`, `Apr-Jun`, and `Mar/Apr` with a
        // slash, 336 of 15,432 looked up on 2026-09-15 — the forms
        // MedlineDate spells with the year in front, so those go through
        // its grammar with the slash read as the hyphen it stands for.
        if let Some(s) = season.as_deref()
            && m.is_none()
            && d.is_none()
            && Season::from_name(s).is_none()
        {
            return PartialDate::parse_medline(&format!("{year} {}", s.replace('/', "-")))
                .map_err(|e| self.err(FIELD, s, ErrorKind::MedlineDate(e)));
        }
        let s = season
            .as_deref()
            .map(|s| Season::parse(s).map_err(|e| self.err(FIELD, s, ErrorKind::Date(e))))
            .transpose()?;
        Ok(match (m, d, s) {
            (None, None, None) => PartialDate::Year(y),
            (Some(m), None, None) => PartialDate::YearMonth(y, m),
            (Some(m), Some(d), None) => PartialDate::Day(
                Date::new(y, m, d).map_err(|e| self.err(FIELD, year, ErrorKind::Date(e)))?,
            ),
            (None, None, Some(s)) => PartialDate::YearSeason(y, s),
            _ => {
                return Err(self.err(
                    FIELD,
                    year,
                    ErrorKind::Inconsistent("PubDate parts in a combination the DTD forbids"),
                ));
            }
        })
    }

    fn mesh_term<'n>(
        &self,
        field: &'static str,
        keyed: &'n record::Keyed<'_>,
    ) -> Result<MeshTerm<'n>, Error> {
        Ok(MeshTerm {
            ui: MeshUi::parse(&keyed.ui)
                .map_err(|e| self.err(field, &keyed.ui, ErrorKind::MeshUi(e)))?,
            name: Interned::Text(&keyed.value),
        })
    }

    fn lang(&self, field: &'static str, s: &str) -> Result<Lang, Error> {
        Lang::from_bibliographic(s).map_err(|e| self.err(field, s, ErrorKind::Lang(e)))
    }

    fn sections<'n>(texts: &'n [AbstractText<'_>]) -> Few<Section<'n>, 4> {
        texts
            .iter()
            .map(|t| Section {
                label: t.label.as_deref(),
                category: t.nlm_category,
                text: t.content.as_str(),
            })
            .collect()
    }

    fn person<'n>(name: &'n NativePersonName<'_>) -> PersonName<'n> {
        PersonName {
            last: &name.last_name,
            fore: name.fore_name.as_deref(),
            initials: name.initials.as_deref(),
            suffix: name.suffix.map(RawXml::as_str),
        }
    }

    /// The single ORCID an `Identifier*` list may carry.
    fn orcid<'n>(
        &self,
        ids: &'n [record::Identifier<'_>],
    ) -> Result<Option<OrcidClaim<'n>>, Error> {
        let mut orcid = None;
        for id in ids {
            if &*id.source != "ORCID" {
                return Err(self.err(
                    "Identifier@Source",
                    &id.source,
                    ErrorKind::Unmodelled("an author identifier that is not an ORCID"),
                ));
            }
            if orcid
                .replace(Claim::from_parse(
                    &id.value,
                    Orcid::from_url_or_bare(&id.value),
                ))
                .is_some()
            {
                return Err(self.err(
                    "Identifier",
                    &id.value,
                    ErrorKind::Inconsistent("more than one ORCID"),
                ));
            }
        }
        Ok(orcid)
    }

    fn affiliations<'n>(
        &self,
        infos: &'n [record::AffiliationInfo<'_>],
    ) -> Result<Few<Affiliation<'n>, 2>, Error> {
        infos
            .iter()
            .map(|info| {
                let ids = info
                    .identifiers
                    .iter()
                    .map(|id| {
                        let source = AffiliationIdSource::parse(&id.source).ok_or_else(|| {
                            self.err(
                                "AffiliationInfo/Identifier@Source",
                                &id.source,
                                ErrorKind::Vocabulary("affiliation identifier sources"),
                            )
                        })?;
                        Ok(AffiliationId {
                            source,
                            value: Interned::Text(&id.value),
                        })
                    })
                    .collect::<Result<_, Error>>()?;
                Ok(Affiliation {
                    text: info.affiliation.as_str(),
                    ids,
                })
            })
            .collect()
    }

    fn author<'n>(&self, a: &'n record::Author<'_>) -> Result<Author<'n>, Error> {
        let name = match &a.name {
            NativeAuthorName::Person(p) => AuthorName::Person(Self::person(p)),
            NativeAuthorName::Collective(c) => AuthorName::Collective {
                name: c.content.as_str(),
                investigators: c.investigators.as_deref(),
            },
        };
        Ok(Author {
            name,
            orcid: self.orcid(&a.identifiers)?,
            equal_contrib: a.equal_contrib.map(|v| v == YesNo::Yes),
            valid: a.valid == YesNo::Yes,
            affiliations: self.affiliations(&a.affiliations)?,
        })
    }

    fn investigator<'n>(&self, i: &'n record::Investigator<'_>) -> Result<Investigator<'n>, Error> {
        Ok(Investigator {
            name: Self::person(&i.name),
            orcid: self.orcid(&i.identifiers)?,
            valid: i.valid == YesNo::Yes,
            affiliations: self.affiliations(&i.affiliations)?,
        })
    }

    fn history(&self, h: &record::PubMedPubDate<'_>) -> Result<HistoryEntry, Error> {
        const FIELD: &str = "History/PubMedPubDate";
        if let Some(second) = &h.second {
            return Err(self.err(
                FIELD,
                second,
                ErrorKind::Unmodelled("a Second in a history date"),
            ));
        }
        let date = self.date(FIELD, &h.date)?;
        let time = match (h.hour.as_deref(), h.minute.as_deref()) {
            (None, None) => None,
            (Some(hour), Some(minute)) => {
                let parse = |s: &str| -> Option<u8> {
                    if s.is_empty() || s.len() > 2 || !s.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    s.parse().ok()
                };
                let (Some(hh), Some(mm)) = (parse(hour), parse(minute)) else {
                    return Err(self.err(
                        FIELD,
                        &format!("{hour}:{minute}"),
                        ErrorKind::Date(DateError::Time),
                    ));
                };
                Some(TimeOfDay::new(hh, mm).ok_or_else(|| {
                    self.err(
                        FIELD,
                        &format!("{hour}:{minute}"),
                        ErrorKind::Date(DateError::Time),
                    )
                })?)
            }
            (Some(hour), None) => {
                return Err(self.err(
                    FIELD,
                    hour,
                    ErrorKind::Unmodelled("an Hour without a Minute"),
                ));
            }
            (None, Some(minute)) => {
                return Err(self.err(
                    FIELD,
                    minute,
                    ErrorKind::Inconsistent("Minute without Hour"),
                ));
            }
        };
        Ok(HistoryEntry {
            status: h.pub_status,
            date,
            time,
        })
    }
}

/// The record's own `ArticleId`s, at most one per type.
#[derive(Default)]
struct Own<'n> {
    pubmed: Option<&'n str>,
    doi: Option<&'n str>,
    pii: Option<&'n str>,
    pmc: Option<&'n str>,
    mid: Option<&'n str>,
    medline: Option<&'n str>,
    sici: Option<&'n str>,
}

impl<'n> Own<'n> {
    fn take(&mut self, ctx: &Ctx, id: &'n ArticleId<'_>) -> Result<(), Error> {
        const FIELD: &str = "ArticleIdList/ArticleId";
        let slot = match id.id_type {
            IdType::Pubmed => &mut self.pubmed,
            IdType::Doi => &mut self.doi,
            IdType::Pii => &mut self.pii,
            IdType::Pmc => &mut self.pmc,
            IdType::Mid => {
                // A manuscript system's letters, then digits: NIHMS, EMS,
                // UKMS, CAMS and HALMS seen (2026-09-15).
                let letters = id.value.bytes().take_while(u8::is_ascii_uppercase).count();
                let digits = &id.value[letters..];
                if letters == 0 || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(ctx.err(
                        FIELD,
                        &id.value,
                        ErrorKind::Unmodelled("a mid that is not uppercase letters then digits"),
                    ));
                }
                &mut self.mid
            }
            IdType::Medline => &mut self.medline,
            IdType::Sici => &mut self.sici,
            _ => {
                return Err(ctx.err(
                    FIELD,
                    &id.value,
                    ErrorKind::Unmodelled("this IdType on a record's own id list"),
                ));
            }
        };
        if slot.is_some() {
            return Err(ctx.err(
                FIELD,
                &id.value,
                ErrorKind::Inconsistent("more than one ArticleId of one type"),
            ));
        }
        *slot = Some(&id.value);
        Ok(())
    }
}

fn identifiers<'n>(
    ctx: &Ctx,
    article: &'n PubmedArticle<'_>,
    pmid: Pmid,
) -> Result<Identifiers<'n>, Error> {
    let mut own = Own::default();
    if let Some(data) = &article.pubmed_data {
        for id in &data.article_ids {
            own.take(ctx, id)?;
        }
    }
    if let Some(claimed) = own.pubmed
        && Pmid::from_digits(claimed) != Ok(pmid)
    {
        return Err(ctx.err(
            "ArticleIdList/ArticleId[pubmed]",
            claimed,
            ErrorKind::Inconsistent("differs from MedlineCitation/PMID"),
        ));
    }
    let doi = own.doi.map(|raw| Claim::from_parse(raw, Doi::parse(raw)));
    let pmc = own
        .pmc
        .map(|raw| {
            PmcId::from_prefixed(raw).map_err(|e| ctx.err("ArticleId[pmc]", raw, ErrorKind::Id(e)))
        })
        .transpose()?;

    let (mut elocation_doi, mut elocation_pii) = (Few::new(), Few::new());
    for e in &article.medline_citation.article.elocation_ids {
        const FIELD: &str = "ELocationID";
        if e.valid == YesNo::No {
            return Err(ctx.err(FIELD, &e.value, ErrorKind::Unmodelled("ValidYN=\"N\"")));
        }
        match e.eid_type {
            EIdType::Doi => elocation_doi.push(Claim::from_parse(&e.value, Doi::parse(&e.value))),
            EIdType::Pii => elocation_pii.push(&*e.value),
        }
    }
    if let Some(Claim::Valid(a)) = &doi {
        for b in elocation_doi.iter().filter_map(Claim::valid) {
            if a != b {
                return Err(ctx.err(
                    "ELocationID[doi]",
                    b.as_str(),
                    ErrorKind::Inconsistent("differs from the valid ArticleId doi"),
                ));
            }
        }
    }
    Ok(Identifiers {
        doi,
        elocation_doi,
        pii: own.pii,
        mid: own.mid,
        elocation_pii,
        pmc,
        medline: own.medline,
        sici: own.sici,
    })
}

fn dates(ctx: &Ctx, c: &MedlineCitation<'_>) -> Result<Dates, Error> {
    let published = ctx.published(&c.article.journal.journal_issue.pub_date)?;
    let electronic = match c.article.article_dates.as_slice() {
        [] => None,
        [one] => Some(ctx.date("ArticleDate", one)?),
        [_, second, ..] => {
            return Err(ctx.err(
                "ArticleDate",
                &second.year,
                ErrorKind::Unmodelled("more than one ArticleDate"),
            ));
        }
    };
    Ok(Dates {
        published,
        electronic,
        revised: c
            .date_revised
            .as_ref()
            .map(|d| ctx.date("DateRevised", d))
            .transpose()?,
        completed: c
            .date_completed
            .as_ref()
            .map(|d| ctx.date("DateCompleted", d))
            .transpose()?,
    })
}

fn journal<'n>(c: &'n MedlineCitation<'_>) -> Journal<'n> {
    let j = &c.article.journal;
    let info = &c.medline_journal_info;
    let issn = j.issn.as_ref().map(|i| TypedIssn {
        kind: i.issn_type,
        issn: Claim::from_parse(&i.value, Issn::parse(&i.value)),
    });
    let pagination = c.article.pagination.as_ref().map(|p| match p {
        NativePagination::Pages {
            start_page,
            end_page,
            medline_pgn,
        } => Pagination::Pages {
            start: start_page,
            end: end_page.as_deref(),
            medline: medline_pgn.as_deref(),
        },
        NativePagination::MedlinePgn(p) => Pagination::Medline(p),
    });
    Journal {
        title: j.title.as_deref().map(Interned::Text),
        iso_abbreviation: j.iso_abbreviation.as_deref().map(Interned::Text),
        medline_ta: Interned::Text(&info.medline_ta),
        nlm_unique_id: info.nlm_unique_id.as_deref().map(Interned::Text),
        issn,
        issn_linking: info
            .issn_linking
            .as_deref()
            .map(|s| Claim::from_parse(s, Issn::parse(s))),
        country: info.country.as_deref().map(Interned::Text),
        pub_model: c.article.pub_model,
        cited_medium: j.journal_issue.cited_medium,
        volume: j.journal_issue.volume.as_deref(),
        issue: j.journal_issue.issue.as_deref(),
        pagination,
    }
}

/// DTD content the file has never contained: an error on first sight.
fn unmodelled(ctx: &Ctx, article: &PubmedArticle<'_>) -> Result<(), Error> {
    let c = &article.medline_citation;
    let checks: [(&'static str, bool); 2] = [
        ("SpaceFlightMission", !c.space_flight_missions.is_empty()),
        (
            "ObjectList",
            article
                .pubmed_data
                .as_ref()
                .is_some_and(|d| !d.objects.is_empty()),
        ),
    ];
    for (name, present) in checks {
        if present {
            return Err(ctx.err(
                name,
                "",
                ErrorKind::Unmodelled("this element, present for the first time"),
            ));
        }
    }
    Ok(())
}

/// Normalizes one record, emitting its citation and then its references.
///
/// # Errors
///
/// The first value that does not fit the model; see [`ErrorKind`]. The
/// emitter may already have received the citation when a reference fails.
#[allow(clippy::too_many_lines)] // one block per field group; the field set is the point
pub fn normalize<'n, K: Sink<'n, Pubmed>>(
    article: &'n PubmedArticle<'_>,
    emit: &mut K,
) -> Result<(), Error> {
    let c = &article.medline_citation;
    let mut ctx = Ctx { pmid: None };
    let pmid = Pmid::from_digits(&c.pmid.value)
        .map_err(|e| ctx.err("PMID", &c.pmid.value, ErrorKind::Id(e)))?;
    ctx.pmid = Some(pmid);
    let version = model::id::strict_u32(&c.pmid.version)
        .map_err(|e| ctx.err("PMID@Version", &c.pmid.version, ErrorKind::Id(e)))?;
    unmodelled(&ctx, article)?;

    let ids = identifiers(&ctx, article, pmid)?;
    let dates = dates(&ctx, c)?;
    let journal = journal(c);

    let r#abstract = c.article.r#abstract.as_ref().map(|a| Abstract {
        sections: Ctx::sections(&a.texts),
        copyright: a.copyright_information.as_deref(),
    });
    let other_abstracts = c
        .other_abstracts
        .iter()
        .map(|o| {
            Ok(OtherAbstract {
                kind: o.r#type,
                language: Claim::from_parse(&o.language, Lang::from_bibliographic(&o.language)),
                sections: Ctx::sections(&o.texts),
                copyright: o.copyright_information.as_deref(),
            })
        })
        .collect::<Result<_, Error>>()?;

    let languages = c
        .article
        .languages
        .iter()
        .map(|l| ctx.lang("Language", l))
        .collect::<Result<_, _>>()?;
    let publication_types = c
        .article
        .publication_types
        .iter()
        .map(|pt| ctx.mesh_term("PublicationType@UI", pt))
        .collect::<Result<_, _>>()?;

    let authors = c
        .article
        .author_list
        .as_ref()
        .map(|l| {
            l.authors
                .iter()
                .map(|a| ctx.author(a))
                .collect::<Result<_, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let investigator_lists = c
        .investigator_lists
        .iter()
        .map(|l| {
            Ok(InvestigatorList {
                id: l.id.as_deref(),
                investigators: l
                    .investigators
                    .iter()
                    .map(|i| ctx.investigator(i))
                    .collect::<Result<_, _>>()?,
            })
        })
        .collect::<Result<_, Error>>()?;

    let mesh = c
        .mesh_headings
        .iter()
        .map(|h| {
            let d = &h.descriptor;
            Ok(MeshHeading {
                descriptor: MeshTerm {
                    ui: MeshUi::parse(&d.ui)
                        .map_err(|e| ctx.err("DescriptorName@UI", &d.ui, ErrorKind::MeshUi(e)))?,
                    name: Interned::Text(&d.value),
                },
                major: d.major_topic == YesNo::Yes,
                geographic: d.r#type.is_some(),
                auto: d.auto_hm,
                qualifiers: h
                    .qualifiers
                    .iter()
                    .map(|q| {
                        Ok(Qualifier {
                            term: MeshTerm {
                                ui: MeshUi::parse(&q.ui).map_err(|e| {
                                    ctx.err("QualifierName@UI", &q.ui, ErrorKind::MeshUi(e))
                                })?,
                                name: Interned::Text(&q.value),
                            },
                            major: q.major_topic == YesNo::Yes,
                            auto: q.auto_hm,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            })
        })
        .collect::<Result<_, Error>>()?;
    let suppl_mesh = c
        .suppl_mesh
        .iter()
        .map(|s| {
            Ok(SupplMesh {
                kind: s.r#type,
                term: MeshTerm {
                    ui: MeshUi::parse(&s.ui)
                        .map_err(|e| ctx.err("SupplMeshName@UI", &s.ui, ErrorKind::MeshUi(e)))?,
                    name: Interned::Text(&s.value),
                },
            })
        })
        .collect::<Result<_, Error>>()?;
    let chemicals = c
        .chemicals
        .iter()
        .map(|ch| {
            Ok(Chemical {
                registry_number: Interned::Text(&ch.registry_number),
                substance: ctx.mesh_term("NameOfSubstance@UI", &ch.name_of_substance)?,
            })
        })
        .collect::<Result<_, Error>>()?;
    let keywords = c
        .keyword_lists
        .iter()
        .flat_map(|l| {
            l.keywords.iter().map(move |k| Keyword {
                owner: l.owner,
                text: k.content.as_str(),
                major: k.major_topic == YesNo::Yes,
            })
        })
        .collect();
    let grants = c
        .article
        .grant_list
        .iter()
        .flat_map(|l| l.grants.iter())
        .map(|g| Grant {
            id: g.grant_id.as_deref(),
            acronym: g.acronym.as_deref().map(Interned::Text),
            agency: Interned::Text(&g.agency),
            country: g
                .country
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(Interned::Text),
        })
        .collect();
    let data_banks = c
        .article
        .data_bank_list
        .iter()
        .flat_map(|l| l.data_banks.iter())
        .map(|b| DataBank {
            name: Interned::Text(&b.name),
            accessions: b.accession_numbers.iter().map(|a| &**a).collect(),
        })
        .collect();

    let mut relations = Few::new();
    for cc in &c.comments_corrections {
        let pmid = cc
            .pmid
            .as_ref()
            .map(|p| {
                Pmid::from_digits(&p.value)
                    .map_err(|e| ctx.err("CommentsCorrections/PMID", &p.value, ErrorKind::Id(e)))
            })
            .transpose()?;
        relations.push(Relation {
            kind: cc.ref_type,
            pmid,
        });
    }

    let (history, publication_status) = match &article.pubmed_data {
        Some(d) => (
            d.history
                .iter()
                .map(|h| ctx.history(h))
                .collect::<Result<_, _>>()?,
            Some(
                PublicationStatus::parse(&d.publication_status).ok_or_else(|| {
                    ctx.err(
                        "PublicationStatus",
                        &d.publication_status,
                        ErrorKind::Vocabulary("publication statuses"),
                    )
                })?,
            ),
        ),
        None => (Few::new(), None),
    };

    emit.work(&Citation {
        pmid,
        version,
        status: c.status,
        owner: c.owner,
        indexing_method: c.indexing_method.as_deref(),
        ids,
        dates,
        journal,
        title: c.article.article_title.content.as_str(),
        vernacular_title: c.article.vernacular_title.map(RawXml::as_str),
        r#abstract,
        other_abstracts,
        languages,
        publication_types,
        authors,
        investigator_lists,
        mesh,
        suppl_mesh,
        chemicals,
        keywords,
        grants,
        data_banks,
        coi_statement: c.coi_statement.map(RawXml::as_str),
        number_of_references: c
            .number_of_references
            .as_deref()
            .map(|n| {
                // Digits, zero allowed (2 of 15,432 say 0).
                if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(ctx.err(
                        "NumberOfReferences",
                        n,
                        ErrorKind::Unmodelled("a count that is not digits"),
                    ));
                }
                n.parse::<u32>().map_err(|_| {
                    ctx.err(
                        "NumberOfReferences",
                        n,
                        ErrorKind::Unmodelled("a count past u32"),
                    )
                })
            })
            .transpose()?,
        gene_symbols: c.gene_symbols.iter().map(|g| &**g).collect(),
        personal_name_subjects: c.personal_name_subjects.iter().map(Ctx::person).collect(),
        other_ids: c
            .other_ids
            .iter()
            .map(|o| OtherId {
                source: o.source,
                value: &o.value,
            })
            .collect(),
        general_notes: c
            .general_notes
            .iter()
            .map(|n| GeneralNote {
                owner: n.owner,
                text: &n.value,
            })
            .collect(),
        citation_subsets: c.citation_subsets.iter().map(|s| &**s).collect(),
        history,
        publication_status,
        relations,
    });

    if let Some(data) = &article.pubmed_data {
        references(&ctx, pmid, &data.reference_lists, emit)?;
    }
    Ok(())
}

/// Normalizes a `DeleteCitation`, emitting one deletion per PMID.
///
/// # Errors
///
/// A PMID or version that is not one.
pub fn deletions<'n, K: Sink<'n, Pubmed>>(
    deleted: &'n DeleteCitation<'_>,
    emit: &mut K,
) -> Result<(), Error> {
    let ctx = Ctx { pmid: None };
    for p in &deleted.pmids {
        let pmid = Pmid::from_digits(&p.value)
            .map_err(|e| ctx.err("DeleteCitation/PMID", &p.value, ErrorKind::Id(e)))?;
        model::id::strict_u32(&p.version)
            .map_err(|e| ctx.err("DeleteCitation/PMID@Version", &p.version, ErrorKind::Id(e)))?;
        emit.deletion(&pmid);
    }
    Ok(())
}

fn references<'n, K: Sink<'n, Pubmed>>(
    ctx: &Ctx,
    from: Pmid,
    lists: &'n [ReferenceListRecord<'_>],
    emit: &mut K,
) -> Result<(), Error> {
    const FIELD: &str = "Reference/ArticleIdList/ArticleId";
    for list in lists {
        for r in &list.references {
            let (mut pmid, mut doi, mut pmc) = (None, None, None);
            for id in &r.article_ids {
                let taken = match id.id_type {
                    IdType::Pubmed => pmid
                        .replace(Claim::from_parse(&id.value, Pmid::from_digits(&id.value)))
                        .is_some(),
                    IdType::Doi => doi
                        .replace(Claim::from_parse(&id.value, Doi::parse(&id.value)))
                        .is_some(),
                    // The 2026 baseline files write `pmcid` with bare
                    // digits; 295,126 reference ids looked up from older
                    // records (2026-09-15) write `pmc` with the prefix,
                    // as a record's own id does.
                    IdType::Pmcid => pmc
                        .replace(Claim::from_parse(&id.value, PmcId::from_digits(&id.value)))
                        .is_some(),
                    IdType::Pmc => pmc
                        .replace(Claim::from_parse(
                            &id.value,
                            PmcId::from_prefixed(&id.value),
                        ))
                        .is_some(),
                    _ => {
                        return Err(ctx.err(
                            FIELD,
                            &id.value,
                            ErrorKind::Unmodelled("this IdType on a reference"),
                        ));
                    }
                };
                if taken {
                    return Err(ctx.err(
                        FIELD,
                        &id.value,
                        ErrorKind::Inconsistent(
                            "more than one ArticleId of one type on a reference",
                        ),
                    ));
                }
            }
            emit.reference(
                &from,
                &Reference {
                    pmid,
                    doi,
                    pmc,
                    citation: r.citation.as_str(),
                },
            );
        }
        references(ctx, from, &list.sub_lists, emit)?;
    }
    Ok(())
}
