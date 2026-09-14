//! Native record types, mirroring `pubmed_250101.dtd` field for field for
//! everything reachable from `PubmedArticle` and `DeleteCitation`. Each
//! type's documentation quotes the content model it implements.
//!
//! Conventions: a `#PCDATA` leaf is a [`Text`]; `(%text; …)*` mixed content
//! is a [`RawXml`]; an optional list wrapper whose content is `X+` (such as
//! `ChemicalList?`) is a plain `Vec<X>`, since the DTD makes "present but
//! empty" impossible; a wrapper that carries attributes keeps its own type.
//! Attribute defaults from the DTD are applied when the attribute is absent.

use crate::attr::{
    AuthorListType, CitedMedium, DescriptorType, EIdType, IdType, IssnType, KeywordOwner,
    NlmCategory, NoteOwner, OtherAbstractType, OtherIdSource, Owner, PubModel, PubStatus, RefType,
    Status, SupplMeshType, YesNo,
};
use crate::text::{RawXml, Text};

/// One member of `PubmedArticleSet ((PubmedArticle | PubmedBookArticle)+, DeleteCitation?)`.
/// `PubmedBookArticle` is not modelled yet; encountering one is an error.
#[derive(Debug, Clone, PartialEq, Eq)]
// One record is alive at a time and never stored in bulk, so the size gap
// between the variants costs nothing; boxing would cost an allocation per
// record.
#[allow(clippy::large_enum_variant)]
pub enum Record<'a> {
    /// A citation.
    Article(PubmedArticle<'a>),
    /// PMIDs deleted by an update file.
    DeleteCitation(DeleteCitation<'a>),
}

/// `DeleteCitation (PMID+)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteCitation<'a> {
    /// At least one.
    pub pmids: Vec<Pmid<'a>>,
}

/// `PubmedArticle (MedlineCitation, PubmedData?)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubmedArticle<'a> {
    /// The citation.
    pub medline_citation: MedlineCitation<'a>,
    /// Identifiers, history and references.
    pub pubmed_data: Option<PubmedData<'a>>,
}

/// `PMID (#PCDATA)` with `Version CDATA #REQUIRED`. Text here; the
/// normalizer makes it a `model::Pmid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pmid<'a> {
    /// The `Version` attribute.
    pub version: Text<'a>,
    /// The digits.
    pub value: Text<'a>,
}

/// `MedlineCitation (PMID, DateCompleted?, DateRevised?, Article,
/// MedlineJournalInfo, ChemicalList?, SupplMeshList?, CitationSubset*,
/// CommentsCorrectionsList?, GeneSymbolList?, MeshHeadingList?,
/// NumberOfReferences?, PersonalNameSubjectList?, OtherID*, OtherAbstract*,
/// KeywordList*, CoiStatement?, SpaceFlightMission*, InvestigatorList*,
/// GeneralNote*)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedlineCitation<'a> {
    /// `Owner (...) "NLM"`.
    pub owner: Owner,
    /// `Status (...) #REQUIRED`.
    pub status: Status,
    /// `VersionID CDATA #IMPLIED`.
    pub version_id: Option<Text<'a>>,
    /// `VersionDate CDATA #IMPLIED`.
    pub version_date: Option<Text<'a>>,
    /// `IndexingMethod CDATA #IMPLIED`.
    pub indexing_method: Option<Text<'a>>,
    /// `PMID`.
    pub pmid: Pmid<'a>,
    /// `DateCompleted?`.
    pub date_completed: Option<Date<'a>>,
    /// `DateRevised?`.
    pub date_revised: Option<Date<'a>>,
    /// `Article`.
    pub article: Article<'a>,
    /// `MedlineJournalInfo`.
    pub medline_journal_info: MedlineJournalInfo<'a>,
    /// `ChemicalList? (Chemical+)`.
    pub chemicals: Vec<Chemical<'a>>,
    /// `SupplMeshList? (SupplMeshName+)`.
    pub suppl_mesh: Vec<SupplMeshName<'a>>,
    /// `CitationSubset*`.
    pub citation_subsets: Vec<Text<'a>>,
    /// `CommentsCorrectionsList? (CommentsCorrections+)`.
    pub comments_corrections: Vec<CommentsCorrections<'a>>,
    /// `GeneSymbolList? (GeneSymbol+)`.
    pub gene_symbols: Vec<Text<'a>>,
    /// `MeshHeadingList? (MeshHeading+)`.
    pub mesh_headings: Vec<MeshHeading<'a>>,
    /// `NumberOfReferences?`.
    pub number_of_references: Option<Text<'a>>,
    /// `PersonalNameSubjectList? (PersonalNameSubject+)`.
    pub personal_name_subjects: Vec<PersonName<'a>>,
    /// `OtherID*`.
    pub other_ids: Vec<OtherId<'a>>,
    /// `OtherAbstract*`.
    pub other_abstracts: Vec<OtherAbstract<'a>>,
    /// `KeywordList*`.
    pub keyword_lists: Vec<KeywordList<'a>>,
    /// `CoiStatement? (%text;)*`.
    pub coi_statement: Option<RawXml<'a>>,
    /// `SpaceFlightMission*`.
    pub space_flight_missions: Vec<Text<'a>>,
    /// `InvestigatorList*`.
    pub investigator_lists: Vec<InvestigatorList<'a>>,
    /// `GeneralNote*`.
    pub general_notes: Vec<GeneralNote<'a>>,
}

/// `(Year, Month, Day)` — `DateCompleted`, `DateRevised`, `ArticleDate`
/// (whose `DateType CDATA #FIXED "Electronic"` is checked, not stored) and
/// the date part of `PubMedPubDate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Date<'a> {
    /// `Year`.
    pub year: Text<'a>,
    /// `Month`.
    pub month: Text<'a>,
    /// `Day`.
    pub day: Text<'a>,
}

/// `Article (Journal, ArticleTitle, ((Pagination, ELocationID*) | ELocationID+),
/// Abstract?, AuthorList?, Language+, DataBankList?, GrantList?,
/// PublicationTypeList, VernacularTitle?, ArticleDate*)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article<'a> {
    /// `PubModel (...) #REQUIRED`.
    pub pub_model: PubModel,
    /// `Journal`.
    pub journal: Journal<'a>,
    /// `ArticleTitle`.
    pub article_title: ArticleTitle<'a>,
    /// `Pagination?` — when absent, at least one `ELocationID` is required.
    pub pagination: Option<Pagination<'a>>,
    /// `ELocationID*`.
    pub elocation_ids: Vec<ELocationId<'a>>,
    /// `Abstract?`.
    pub r#abstract: Option<Abstract<'a>>,
    /// `AuthorList?`.
    pub author_list: Option<AuthorList<'a>>,
    /// `Language+`.
    pub languages: Vec<Text<'a>>,
    /// `DataBankList?`.
    pub data_bank_list: Option<DataBankList<'a>>,
    /// `GrantList?`.
    pub grant_list: Option<GrantList<'a>>,
    /// `PublicationTypeList (PublicationType+)`.
    pub publication_types: Vec<Keyed<'a>>,
    /// `VernacularTitle? (%text; | mml:math)*`.
    pub vernacular_title: Option<RawXml<'a>>,
    /// `ArticleDate*`.
    pub article_dates: Vec<Date<'a>>,
}

/// `ArticleTitle (%text; | mml:math)*` with `%booklinkatts;`:
/// `book`, `part`, `sec`, each `CDATA #IMPLIED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleTitle<'a> {
    /// `book`.
    pub book: Option<Text<'a>>,
    /// `part`.
    pub part: Option<Text<'a>>,
    /// `sec`.
    pub sec: Option<Text<'a>>,
    /// The title markup.
    pub content: RawXml<'a>,
}

/// `Journal (ISSN?, JournalIssue, Title?, ISOAbbreviation?)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journal<'a> {
    /// `ISSN?`.
    pub issn: Option<Issn<'a>>,
    /// `JournalIssue`.
    pub journal_issue: JournalIssue<'a>,
    /// `Title?`.
    pub title: Option<Text<'a>>,
    /// `ISOAbbreviation?`.
    pub iso_abbreviation: Option<Text<'a>>,
}

/// `ISSN (#PCDATA)` with `IssnType (Electronic | Print) #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issn<'a> {
    /// `IssnType`.
    pub issn_type: IssnType,
    /// The ISSN, unvalidated here.
    pub value: Text<'a>,
}

/// `JournalIssue (Volume?, Issue?, PubDate)` with
/// `CitedMedium (Internet | Print) #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalIssue<'a> {
    /// `CitedMedium`.
    pub cited_medium: CitedMedium,
    /// `Volume?`.
    pub volume: Option<Text<'a>>,
    /// `Issue?`.
    pub issue: Option<Text<'a>>,
    /// `PubDate`.
    pub pub_date: PubDate<'a>,
}

/// `PubDate ((Year, ((Month, Day?) | Season)?) | MedlineDate)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubDate<'a> {
    /// The structured branch. `day` requires `month`; `season` excludes both.
    Date {
        /// `Year`.
        year: Text<'a>,
        /// `Month?` — a name (`Jan`) or a number (`01`) in the wild.
        month: Option<Text<'a>>,
        /// `Day?`.
        day: Option<Text<'a>>,
        /// `Season?`.
        season: Option<Text<'a>>,
    },
    /// `MedlineDate` free text, e.g. `1998 Dec-1999 Jan`.
    Medline(Text<'a>),
}

/// `Pagination ((StartPage, EndPage?, MedlinePgn?) | MedlinePgn)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pagination<'a> {
    /// The structured branch.
    Pages {
        /// `StartPage`.
        start_page: Text<'a>,
        /// `EndPage?`.
        end_page: Option<Text<'a>>,
        /// `MedlinePgn?`.
        medline_pgn: Option<Text<'a>>,
    },
    /// `MedlinePgn` alone, e.g. `1-8` or `e2025072830`.
    MedlinePgn(Text<'a>),
}

/// `ELocationID (#PCDATA)` with `EIdType (doi | pii) #REQUIRED`,
/// `ValidYN (Y | N) "Y"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ELocationId<'a> {
    /// `EIdType`.
    pub eid_type: EIdType,
    /// `ValidYN`.
    pub valid: YesNo,
    /// The identifier, unvalidated here.
    pub value: Text<'a>,
}

/// `Abstract (AbstractText+, CopyrightInformation?)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abstract<'a> {
    /// `AbstractText+`.
    pub texts: Vec<AbstractText<'a>>,
    /// `CopyrightInformation?`.
    pub copyright_information: Option<Text<'a>>,
}

/// `AbstractText (%text; | mml:math | DispFormula)*` with
/// `Label CDATA #IMPLIED`, `NlmCategory (...) #IMPLIED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractText<'a> {
    /// `Label`, free text (`RESULTS`, `Résultats`, …).
    pub label: Option<Text<'a>>,
    /// `NlmCategory`.
    pub nlm_category: Option<NlmCategory>,
    /// The paragraph markup.
    pub content: RawXml<'a>,
}

/// `AuthorList (Author+)` with `CompleteYN (Y | N) "Y"`,
/// `Type (authors | editors) #IMPLIED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorList<'a> {
    /// `CompleteYN`.
    pub complete: YesNo,
    /// `Type`.
    pub r#type: Option<AuthorListType>,
    /// `Author+`.
    pub authors: Vec<Author<'a>>,
}

/// `Author (((LastName, ForeName?, Initials?, Suffix?) | CollectiveName),
/// Identifier*, AffiliationInfo*)` with `ValidYN (Y | N) "Y"`,
/// `EqualContrib (Y | N) #IMPLIED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author<'a> {
    /// `ValidYN`.
    pub valid: YesNo,
    /// `EqualContrib`.
    pub equal_contrib: Option<YesNo>,
    /// The name branch.
    pub name: AuthorName<'a>,
    /// `Identifier*` — `Source` is free text: `ORCID` observed.
    pub identifiers: Vec<Identifier<'a>>,
    /// `AffiliationInfo*`.
    pub affiliations: Vec<AffiliationInfo<'a>>,
}

/// The choice inside `Author`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorName<'a> {
    /// `LastName, ForeName?, Initials?, Suffix?`.
    Person(PersonName<'a>),
    /// `CollectiveName`.
    Collective(CollectiveName<'a>),
}

/// `LastName, ForeName?, Initials?, Suffix?` — shared by `Author`,
/// `Investigator` and `PersonalNameSubject`. `Suffix` is `(%text;)*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonName<'a> {
    /// `LastName`.
    pub last_name: Text<'a>,
    /// `ForeName?`.
    pub fore_name: Option<Text<'a>>,
    /// `Initials?`.
    pub initials: Option<Text<'a>>,
    /// `Suffix?`.
    pub suffix: Option<RawXml<'a>>,
}

/// `CollectiveName (%text;)*` with `Investigators IDREF #IMPLIED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectiveName<'a> {
    /// `Investigators`, referencing an `InvestigatorList ID`.
    pub investigators: Option<Text<'a>>,
    /// The name markup.
    pub content: RawXml<'a>,
}

/// `Identifier (#PCDATA)` with `Source CDATA #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier<'a> {
    /// `Source` — `ORCID`, `ROR`, `GRID`, `ISNI`, `RINGGOLD` observed.
    pub source: Text<'a>,
    /// The identifier as written: bare or URL-form, unvalidated here.
    pub value: Text<'a>,
}

/// `AffiliationInfo (Affiliation, Identifier*)`; `Affiliation (%text;)*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffiliationInfo<'a> {
    /// `Affiliation`.
    pub affiliation: RawXml<'a>,
    /// `Identifier*`.
    pub identifiers: Vec<Identifier<'a>>,
}

/// `DataBankList (DataBank+)` with `CompleteYN (Y | N) "Y"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataBankList<'a> {
    /// `CompleteYN`.
    pub complete: YesNo,
    /// `DataBank+`.
    pub data_banks: Vec<DataBank<'a>>,
}

/// `DataBank (DataBankName, AccessionNumberList?)`;
/// `AccessionNumberList (AccessionNumber+)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataBank<'a> {
    /// `DataBankName`.
    pub name: Text<'a>,
    /// `AccessionNumber+`, or empty when the list is absent.
    pub accession_numbers: Vec<Text<'a>>,
}

/// `GrantList (Grant+)` with `CompleteYN (Y | N) "Y"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantList<'a> {
    /// `CompleteYN`.
    pub complete: YesNo,
    /// `Grant+`.
    pub grants: Vec<Grant<'a>>,
}

/// `Grant (GrantID?, Acronym?, Agency, Country?)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant<'a> {
    /// `GrantID?`.
    pub grant_id: Option<Text<'a>>,
    /// `Acronym?`.
    pub acronym: Option<Text<'a>>,
    /// `Agency`.
    pub agency: Text<'a>,
    /// `Country?` — present but empty in most records observed.
    pub country: Option<Text<'a>>,
}

/// A `#PCDATA` element with `UI CDATA #REQUIRED`: `PublicationType`,
/// `NameOfSubstance`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyed<'a> {
    /// `UI`, a MeSH-style identifier such as `D016428`.
    pub ui: Text<'a>,
    /// The term.
    pub value: Text<'a>,
}

/// `MedlineJournalInfo (Country?, MedlineTA, NlmUniqueID?, ISSNLinking?)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedlineJournalInfo<'a> {
    /// `Country?`.
    pub country: Option<Text<'a>>,
    /// `MedlineTA`.
    pub medline_ta: Text<'a>,
    /// `NlmUniqueID?`.
    pub nlm_unique_id: Option<Text<'a>>,
    /// `ISSNLinking?`.
    pub issn_linking: Option<Text<'a>>,
}

/// `Chemical (RegistryNumber, NameOfSubstance)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chemical<'a> {
    /// `RegistryNumber` — `0`, a CAS number, an EC number or a UNII.
    pub registry_number: Text<'a>,
    /// `NameOfSubstance` with its `UI`.
    pub name_of_substance: Keyed<'a>,
}

/// `SupplMeshName (#PCDATA)` with `Type (...) #REQUIRED`, `UI CDATA #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplMeshName<'a> {
    /// `Type`.
    pub r#type: SupplMeshType,
    /// `UI`.
    pub ui: Text<'a>,
    /// The term.
    pub value: Text<'a>,
}

/// `CommentsCorrections (RefSource, PMID?, Note?)` with `RefType (...) #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentsCorrections<'a> {
    /// `RefType` — `RetractionOf`, `ErratumFor`, … This is where PubMed
    /// records retractions, on the affected record.
    pub ref_type: RefType,
    /// `RefSource`, a free-text citation.
    pub ref_source: Text<'a>,
    /// `PMID?` of the related record.
    pub pmid: Option<Pmid<'a>>,
    /// `Note?`.
    pub note: Option<Text<'a>>,
}

/// `MeshHeading (DescriptorName, QualifierName*)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshHeading<'a> {
    /// `DescriptorName`.
    pub descriptor: DescriptorName<'a>,
    /// `QualifierName*`.
    pub qualifiers: Vec<QualifierName<'a>>,
}

/// `DescriptorName (#PCDATA)` with `MajorTopicYN (Y | N) "N"`,
/// `AutoHM (Y) #IMPLIED`, `Type (Geographic) #IMPLIED`, `UI CDATA #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorName<'a> {
    /// `MajorTopicYN`.
    pub major_topic: YesNo,
    /// `AutoHM` present (its only value is `Y`).
    pub auto_hm: bool,
    /// `Type`.
    pub r#type: Option<DescriptorType>,
    /// `UI`.
    pub ui: Text<'a>,
    /// The descriptor.
    pub value: Text<'a>,
}

/// `QualifierName (#PCDATA)` with `AutoHM (Y) #IMPLIED`,
/// `MajorTopicYN (Y | N) "N"`, `UI CDATA #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifierName<'a> {
    /// `MajorTopicYN`.
    pub major_topic: YesNo,
    /// `AutoHM` present.
    pub auto_hm: bool,
    /// `UI`.
    pub ui: Text<'a>,
    /// The qualifier.
    pub value: Text<'a>,
}

/// `OtherID (#PCDATA)` with `Source (...) #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherId<'a> {
    /// `Source`.
    pub source: OtherIdSource,
    /// The identifier.
    pub value: Text<'a>,
}

/// `OtherAbstract (AbstractText+, CopyrightInformation?)` with
/// `Type (...) #REQUIRED`, `Language CDATA "eng"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherAbstract<'a> {
    /// `Type`.
    pub r#type: OtherAbstractType,
    /// `Language`, defaulted to `eng`.
    pub language: Text<'a>,
    /// `AbstractText+`.
    pub texts: Vec<AbstractText<'a>>,
    /// `CopyrightInformation?`.
    pub copyright_information: Option<Text<'a>>,
}

/// `KeywordList (Keyword+)` with `Owner (...) "NLM"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordList<'a> {
    /// `Owner`.
    pub owner: KeywordOwner,
    /// `Keyword+`.
    pub keywords: Vec<Keyword<'a>>,
}

/// `Keyword (%text; | mml:math)*` with `MajorTopicYN (Y | N) "N"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword<'a> {
    /// `MajorTopicYN`.
    pub major_topic: YesNo,
    /// The keyword markup.
    pub content: RawXml<'a>,
}

/// `InvestigatorList (Investigator+)` with `ID ID #IMPLIED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigatorList<'a> {
    /// `ID`, the target of `CollectiveName Investigators`.
    pub id: Option<Text<'a>>,
    /// `Investigator+`.
    pub investigators: Vec<Investigator<'a>>,
}

/// `Investigator (LastName, ForeName?, Initials?, Suffix?, Identifier*,
/// AffiliationInfo*)` with `ValidYN (Y | N) "Y"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Investigator<'a> {
    /// `ValidYN`.
    pub valid: YesNo,
    /// The name.
    pub name: PersonName<'a>,
    /// `Identifier*`.
    pub identifiers: Vec<Identifier<'a>>,
    /// `AffiliationInfo*`.
    pub affiliations: Vec<AffiliationInfo<'a>>,
}

/// `GeneralNote (#PCDATA)` with `Owner (...) "NLM"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralNote<'a> {
    /// `Owner`.
    pub owner: NoteOwner,
    /// The note.
    pub value: Text<'a>,
}

/// `PubmedData (History?, PublicationStatus, ArticleIdList, ObjectList?,
/// ReferenceList*)`; `History (PubMedPubDate+)`; `ArticleIdList (ArticleId+)`;
/// `ObjectList (Object+)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubmedData<'a> {
    /// `PubMedPubDate+`, or empty when `History` is absent.
    pub history: Vec<PubMedPubDate<'a>>,
    /// `PublicationStatus` — `ppublish`, `epublish`, `aheadofprint` observed.
    pub publication_status: Text<'a>,
    /// `ArticleId+`.
    pub article_ids: Vec<ArticleId<'a>>,
    /// `Object+`, or empty when `ObjectList` is absent.
    pub objects: Vec<Object<'a>>,
    /// `ReferenceList*`.
    pub reference_lists: Vec<ReferenceList<'a>>,
}

/// `PubMedPubDate (Year, Month, Day, (Hour, (Minute, Second?)?)?)` with
/// `PubStatus (...) #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubMedPubDate<'a> {
    /// `PubStatus`.
    pub pub_status: PubStatus,
    /// `Year, Month, Day`.
    pub date: Date<'a>,
    /// `Hour?`.
    pub hour: Option<Text<'a>>,
    /// `Minute?`, only with `hour`.
    pub minute: Option<Text<'a>>,
    /// `Second?`, only with `minute`.
    pub second: Option<Text<'a>>,
}

/// `ArticleId (#PCDATA)` with `IdType (...) "pubmed"`.
///
/// Two serializations of one PMC identifier live here: the record's own is
/// `IdType="pmc"` with a `PMC` prefix; a reference's is `IdType="pmcid"`
/// with bare digits. Both stay text at this layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleId<'a> {
    /// `IdType`.
    pub id_type: IdType,
    /// The identifier as written.
    pub value: Text<'a>,
}

/// `Object (Param*)` with `Type CDATA #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object<'a> {
    /// `Type`.
    pub r#type: Text<'a>,
    /// `Param*`.
    pub params: Vec<Param<'a>>,
}

/// `Param (%text;)*` with `Name CDATA #REQUIRED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param<'a> {
    /// `Name`.
    pub name: Text<'a>,
    /// The value markup.
    pub content: RawXml<'a>,
}

/// `ReferenceList (Title?, Reference*, ReferenceList*)` — lists nest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceList<'a> {
    /// `Title?` — `References`, `REFERENCES`, `Literatur`, … observed.
    pub title: Option<Text<'a>>,
    /// `Reference*`.
    pub references: Vec<Reference<'a>>,
    /// `ReferenceList*`.
    pub sub_lists: Vec<ReferenceList<'a>>,
}

/// `Reference (Citation, ArticleIdList?)`; `Citation (%text; | mml:math)*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference<'a> {
    /// The citation markup.
    pub citation: RawXml<'a>,
    /// `ArticleId+`, or empty when the list is absent — `doi`, `pubmed`
    /// and `pmcid` observed.
    pub article_ids: Vec<ArticleId<'a>>,
}
