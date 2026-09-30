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
    #[serde(borrow)]
    pub dates: Dates<'n>,
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
    /// `Language+`, ISO 639-2 bibliographic codes; a code
    /// [`attr::NOT_A_LANGUAGE`](crate::attr::NOT_A_LANGUAGE) has read is not
    /// one of them and is not here.
    pub languages: Few<Lang, 1>,
    /// `PublicationType*`: the DTD says `+`, and the list is sometimes
    /// empty, so a record can have none (model version 4).
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
    #[serde(borrow)]
    pub history: Few<HistoryEntry<'n>, 8>,
    /// `PubmedData/PublicationStatus`.
    pub publication_status: Option<PublicationStatus>,
    /// `CommentsCorrections*`: errata, retractions, comments, by kind.
    #[serde(borrow)]
    pub relations: Few<Relation<'n>, 4>,
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

/// An `ArticleId` of a kind that is no name of ours — a publisher's item
/// id and the rest of the DTD's list — kept as the source wrote it, for
/// the matching stage (merge plan §6). Never identity: an article number
/// such as `e112` belongs to one article per journal, not one in the
/// world (2026-09-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LooseId<'n> {
    /// The `IdType` attribute.
    pub kind: IdType,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub value: &'n str,
}

/// An `ELocationID` the record marks `ValidYN="N"`: an electronic
/// location NLM says is the wrong one. When a publisher sends corrected
/// e-location data, "the revised DOI will be tagged `ValidYN=Y` and the
/// original DOI will be retained with the `ValidYN` value N" (NLM,
/// MEDLINE/PubMed XML element descriptions) — so the flag is not a
/// syntactic complaint about the value, which is usually a perfectly
/// well-formed DOI, but NLM's assertion that this identifier does not
/// name this article.
///
/// It is kept, and it is never a name (2026-09-23). Kept, because a wrong
/// DOI that was published is a wrong DOI that gets cited: a reference
/// carrying it can be matched back to this record only if the record
/// still remembers it, and it is the one thing an erratum leaves behind.
/// Never a name, because a name joins works, and joining on an identifier
/// its own source says belongs elsewhere would fuse this record with
/// whatever the corrected DOI actually names. Kept here rather than in
/// `elocation_doi`/`elocation_pii`, where every entry is a name, and
/// rather than as a `Claim::Malformed`, which says "did not parse" — a
/// different and weaker thing than "NLM says it is wrong".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidElocation<'n> {
    /// The `EIdType` attribute: which slot the record wrote it in.
    pub kind: EIdType,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub value: &'n str,
}

/// The record's identifiers beyond its PMID.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Identifiers<'n> {
    /// `ArticleId IdType="doi"` — the record's own DOI as it asserts it,
    /// and `None` when the list asserts two that disagree ([`concur`]).
    #[serde(borrow)]
    pub doi: Option<DoiClaim<'n>>,
    /// `ELocationID EIdType="doi"` — the same identifier from the article
    /// block, once or, on 5 of 15,432 looked-up records, twice (a case
    /// variant, or a value that is not a DOI beside one that is). A valid
    /// one need not agree with `doi`: on 4 of the 126,740 records of the
    /// 2026 baseline and its first update files the two differ, and at
    /// Crossref either slot may hold the registered DOI, or both may
    /// (2026-09-17). Both are kept and both are names.
    #[serde(borrow)]
    pub elocation_doi: Few<DoiClaim<'n>, 1>,
    /// `ArticleId IdType="pii"`, the publisher's own item identifier, once
    /// or more: PMID 32839689 gives Elsevier's item id and the article
    /// number.
    #[serde(borrow)]
    pub pii: Few<&'n str, 1>,
    /// `mid`: a manuscript id on a record with an author manuscript —
    /// NIH's `NIHMS…` or Europe PMC's `EMS…` (both first seen in
    /// pubmed26n1333, PMIDs 41576088 and 41583386).
    pub mid: Option<&'n str>,
    /// `ELocationID EIdType="pii"`, which need not equal `pii`; twice on
    /// 87 of 15,432 looked-up records.
    #[serde(borrow)]
    pub elocation_pii: Few<&'n str, 1>,
    /// `ArticleId IdType="pmc"`, or `pmcid` on the record's own list,
    /// which names the same thing (2026-09-23), as claimed: 1 of 126,740
    /// records holds a publisher's own id in the slot instead (PMID
    /// 34850701, `wst_2021_260`, which that publisher's other records give
    /// as their `pii`). A malformed one is kept as written and is never a
    /// name.
    #[serde(borrow)]
    pub pmc: Option<PmcClaim<'n>>,
    /// `ArticleId IdType="medline"`: an old MEDLINE unique identifier (28
    /// of 15,432).
    pub medline: Option<&'n str>,
    /// `ArticleId IdType="sici"`: a Serial Item and Contribution
    /// Identifier (11 of 15,432).
    pub sici: Option<&'n str>,
    /// Every other `ArticleId` the record's own list gives, as written:
    /// the DTD's `pmcpid`, `pmpid`, `pmcbook` and `bookaccession`, which
    /// name something — a processing id of an old PMC load, a Bookshelf
    /// document — that this model has no name for. Kept for the matching
    /// stage and never a name, the rule a reference's other ids already
    /// follow (model version 2, 2026-09-18).
    #[serde(borrow)]
    pub loose: Few<LooseId<'n>, 1>,
    /// Every `ELocationID` the record marks `ValidYN="N"`: an identifier
    /// NLM says is wrong, kept and never a name.
    #[serde(borrow)]
    pub invalid_elocation: Few<InvalidElocation<'n>, 1>,
}

/// The record's dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dates<'n> {
    /// `JournalIssue/PubDate`, at the precision the record gave.
    #[serde(borrow)]
    pub published: Published<'n>,
    /// `ArticleDate`, the electronic publication date; at most one is
    /// modelled.
    pub electronic: Option<Date>,
    /// `DateRevised`.
    pub revised: Option<Date>,
    /// `DateCompleted`.
    pub completed: Option<Date>,
}

/// `JournalIssue/PubDate`: the date, or, for a `MedlineDate` this build
/// does not read whole, what of it is certain beside the text as written
/// and why the rest was not read.
///
/// `MedlineDate` is defined by NLM as the date that fits no pattern
/// (`el-MedlineDate.html`: "does not fit the available date patterns") and
/// no list of its forms is published, so the grammar in `model::date` is
/// ours, learned from the corpus, and will meet forms it has not seen: the
/// survey of the whole 2026 baseline on model version 6 still found 9,248
/// it could not read — `1988 EASTER`, `1990 Apr IL`, `1990 Nov-15` — and
/// 245 day ranges whose end comes before their start. Before model version
/// 7 each stopped the run, and in a daily update file would have stopped
/// PubMed's freshness path until someone taught the grammar. Now a
/// `MedlineDate` that begins with a year keeps that year, or for such a
/// day range its first day ([`PartialDate::medline_certain`]), and the
/// record normalizes; one that does not begin with a year still stops the
/// run (the user's decision, 2026-09-24). The stats report counts these by
/// what they say, so the grammar can still be taught from it. Version 8
/// taught it the backwards day ranges, which are a notation (`1995 Mar
/// 22-8` is the 22nd to the 28th), so those are read now and only what
/// that reading cannot make run forwards is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Published<'n> {
    /// Read whole.
    Read(PartialDate),
    /// Not read whole: what of it is certain, and the text as written.
    Kept {
        /// The year the text begins with, or a backwards day range's
        /// first day.
        certain: PartialDate,
        /// `MedlineDate` as written.
        raw: &'n str,
        /// Why the grammar did not read it whole.
        error: MedlineDateError,
    },
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
    /// `Identifier*`, one per organization registry that named this
    /// affiliation; an identifier whose source
    /// [`attr::NOT_AN_ORGANIZATION_ID`](crate::attr::NOT_AN_ORGANIZATION_ID)
    /// has read names no organization and is not here.
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
///
/// Its date and its time are kept as the record wrote them when they name
/// no day or no time that exists, and the record normalizes (model version
/// 6, the user's decision of 2026-09-24). The survey of the whole 2026
/// baseline on model version 5 found 200 records with a history day that
/// never was — the 31st of a thirty-day month, 29 February of a year that
/// had none (`2010-02-29`, `2011-09-31`) — and 48 with `null` where an hour
/// or a minute goes; each of them used to stop the run. Twelve of the 200,
/// fetched from `efetch` the same day, carry the bad day on a date the
/// publisher supplied (`received` 7, `accepted` 4, `revised` 1), never on
/// one NLM stamps itself (`entrez`, `pubmed`, `medline`). Nothing outside
/// this crate reads a history date, so a whole record — title, abstract,
/// authors, MeSH, references — was being lost to a timestamp no consumer
/// has; and nothing is rounded or guessed in its place, because the day
/// a publisher meant by `2011-09-31` is not ours to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry<'n> {
    #[allow(missing_docs)]
    pub status: PubStatus,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub date: HistoryDate<'n>,
    /// Local time with no zone, when given.
    #[serde(borrow)]
    pub time: Option<HistoryTime<'n>>,
}

/// A history date: the day, or the `Year`, `Month` and `Day` as written
/// and why they are not one.
///
/// The shape of [`Claim`], with the raw text in the three parts the record
/// wrote it in: a `Claim` borrows one string, and joining the parts into
/// `2011-09-31` would keep a string the record never contained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryDate<'n> {
    /// It is a day.
    Valid(Date),
    /// It is not; here is what the record wrote, and why.
    Malformed {
        #[allow(missing_docs)]
        year: &'n str,
        #[allow(missing_docs)]
        month: &'n str,
        #[allow(missing_docs)]
        day: &'n str,
        #[allow(missing_docs)]
        error: DateError,
    },
}

/// A history time: an hour and perhaps a minute, or the `Hour` and
/// `Minute` as written and why they are not one — [`DateError::TimeDigits`]
/// for `null` in either (`null:null`, `5:null`), [`DateError::Time`] for
/// digits out of range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryTime<'n> {
    /// It is a time.
    Valid(TimeOfDay),
    /// It is not; here is what the record wrote, and why.
    Malformed {
        #[allow(missing_docs)]
        hour: &'n str,
        /// `Minute`, when the record wrote one.
        minute: Option<&'n str>,
        #[allow(missing_docs)]
        error: DateError,
    },
}

impl HistoryDate<'_> {
    /// The day, if the record named one.
    #[must_use]
    pub const fn valid(&self) -> Option<Date> {
        match self {
            Self::Valid(date) => Some(*date),
            Self::Malformed { .. } => None,
        }
    }
}

impl HistoryTime<'_> {
    /// The time, if the record named one.
    #[must_use]
    pub const fn valid(&self) -> Option<TimeOfDay> {
        match self {
            Self::Valid(time) => Some(*time),
            Self::Malformed { .. } => None,
        }
    }
}

/// The day, or the parts as the record wrote them: `2011-09-31`.
impl fmt::Display for HistoryDate<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid(date) => date.fmt(f),
            Self::Malformed {
                year, month, day, ..
            } => write!(f, "{year}-{month}-{day}"),
        }
    }
}

/// The time, or the parts as the record wrote them: `null:null`.
impl fmt::Display for HistoryTime<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid(time) => time.fmt(f),
            Self::Malformed {
                hour,
                minute: Some(minute),
                ..
            } => write!(f, "{hour}:{minute}"),
            Self::Malformed { hour, .. } => f.write_str(hour),
        }
    }
}

/// A `CommentsCorrections` entry: how this record relates to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation<'n> {
    /// `ErratumFor`, `RetractionOf`, `CommentOn`, …
    pub kind: RefType,
    /// The other record, when PubMed identified it.
    pub pmid: Option<Pmid>,
    /// The other record's DOI, where PubMed gives no PMID and the
    /// `RefSource` is a DOI and nothing else (model version 12): the
    /// erratum 41585761 names the 102 abstracts it corrects so, none of
    /// them in PubMed. A `RefSource` that is a citation in words is not
    /// read for one.
    #[serde(borrow)]
    pub doi: Option<Doi<'n>>,
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
    /// Every other `ArticleId` the reference gives, as written: 1 of the
    /// 2,811,661 references in the 2026 baseline and its first update
    /// files carries one (PMID 32960545 cites a Nutrients article by its
    /// article number, `pii e112`). Kept for the matching stage, never a
    /// name — which is also why a kind the reference gives twice, in two
    /// values that contradict each other, is kept here rather than in the
    /// slot above it ([`kept`], 2026-09-23).
    #[serde(borrow)]
    pub loose: Few<LooseId<'n>, 1>,
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
    /// What the record's reading read as other than written, by field and
    /// how, for the sink once the record is read.
    corrected: std::cell::RefCell<Vec<(&'static str, &'static str)>>,
}

impl Ctx {
    /// A DOI, read ([`Doi::read`]), with what the reading changed noted
    /// for the sink under `field` ([`Sink::corrected`]).
    fn doi<'n>(&self, field: &'static str, raw: &'n str) -> Result<Doi<'n>, DoiError> {
        let (doi, readings) = Doi::read(raw)?;
        self.corrected
            .borrow_mut()
            .extend(readings.made().map(|how| (field, how)));
        Ok(doi)
    }

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

    fn published<'n>(&self, pub_date: &'n PubDate<'_>) -> Result<Published<'n>, Error> {
        const FIELD: &str = "JournalIssue/PubDate";
        let (year, month, day, season) = match pub_date {
            PubDate::Medline(text) => {
                return match PartialDate::parse_medline(text) {
                    Ok(date) => Ok(Published::Read(date)),
                    Err(error) => match PartialDate::medline_certain(text, error) {
                        Some(certain) => Ok(Published::Kept {
                            certain,
                            raw: text,
                            error,
                        }),
                        None => Err(self.err(FIELD, text, ErrorKind::MedlineDate(error))),
                    },
                };
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
                .map(Published::Read)
                .map_err(|e| self.err(FIELD, s, ErrorKind::MedlineDate(e)));
        }
        let s = season
            .as_deref()
            .map(|s| Season::parse(s).map_err(|e| self.err(FIELD, s, ErrorKind::Date(e))))
            .transpose()?;
        Ok(Published::Read(match (m, d, s) {
            (None, None, None) => PartialDate::Year(y),
            (Some(m), None, None) => PartialDate::YearMonth(y, m),
            // The whole date in the report, as `date` writes one, not only
            // the year: the day that does not exist is the finding.
            (Some(m), Some(d), None) => PartialDate::Day(Date::new(y, m, d).map_err(|e| {
                let raw = format!(
                    "{year}-{}-{}",
                    month.as_deref().unwrap_or_default(),
                    day.as_deref().unwrap_or_default()
                );
                self.err(FIELD, &raw, ErrorKind::Date(e))
            })?),
            (None, None, Some(s)) => PartialDate::YearSeason(y, s),
            _ => {
                return Err(self.err(
                    FIELD,
                    year,
                    ErrorKind::Inconsistent("PubDate parts in a combination the DTD forbids"),
                ));
            }
        }))
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

    /// The one ORCID an `Identifier*` list names, if it names one.
    ///
    /// `Author` is `(…, Identifier*, AffiliationInfo*)`: the DTD lets one
    /// author carry the tag more than once, and 59 records of the 2026
    /// baseline, ~37M, do (survey, 2026-09-23). Of the 22 examples the
    /// survey named, re-fetched from `efetch` on 2026-09-23, 18 repeat one
    /// ORCID two or three times — PMID 28854062's author says
    /// `0000-0002-2415-5977` three times, PMID 28145786's says
    /// `http://orcid.org/0000-0002-5621-5655` twice — which is one
    /// assertion made twice, and is kept. Comparison is on the parsed
    /// value, not the text, because the same ORCID is written bare in one
    /// place and as a URL in another.
    ///
    /// The other four carry two different ORCIDs on one author, and in
    /// three of them the odd one is a co-author's, repeated: PMID
    /// 28287839's third author holds her own and then the second author's,
    /// and PMID 28968151's fifth author the same way. Position does not
    /// settle it — on PMID 35138888 it is the *first* of the two that
    /// another author also carries — so an author whose identifiers
    /// disagree gets none. An ORCID names a person; naming the wrong one
    /// is worse than naming none, and a record that does not say is not a
    /// record that says "no ORCID" (merge §5, absence is not an
    /// assertion). The author, the name and the affiliations are kept, so
    /// the record and its references survive what used to stop the run.
    fn orcid<'n>(
        &self,
        ids: &'n [record::Identifier<'_>],
    ) -> Result<Option<OrcidClaim<'n>>, Error> {
        let mut said: Few<OrcidClaim<'n>, 1> = Few::new();
        for id in ids {
            if &*id.source != "ORCID" {
                return Err(self.err(
                    "Identifier@Source",
                    &id.source,
                    ErrorKind::Unmodelled("an author identifier that is not an ORCID"),
                ));
            }
            said.push(Claim::from_parse(
                &id.value,
                Orcid::from_url_or_bare(&id.value),
            ));
        }
        let Some((first, rest)) = said.split_first() else {
            return Ok(None);
        };
        Ok(rest.iter().all(|c| c == first).then_some(*first))
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
                    // A source `attr::NOT_AN_ORGANIZATION_ID` has read is a
                    // wart we know the shape of — a person's ORCID, a
                    // doubled Ringgold, a publisher's private key — and the
                    // identifier goes no further; the affiliation text it
                    // hangs off is kept regardless. Any other source that is
                    // not one of the four registries still stops the run.
                    .filter(|id| crate::attr::not_an_organization_id(&id.source).is_none())
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

    /// One `PubMedPubDate`. Its values never stop the run — a date or a
    /// time that is none is kept as written ([`HistoryEntry`]) — but a
    /// shape the DTD does not allow still does.
    fn history<'n>(&self, h: &'n record::PubMedPubDate<'_>) -> Result<HistoryEntry<'n>, Error> {
        const FIELD: &str = "History/PubMedPubDate";
        if let Some(second) = &h.second {
            return Err(self.err(
                FIELD,
                second,
                ErrorKind::Unmodelled("a Second in a history date"),
            ));
        }
        let date = history_date(&h.date);
        let time = match (h.hour.as_deref(), h.minute.as_deref()) {
            (None, None) => None,
            // `PubMedPubDate (Year, Month, Day, (Hour, (Minute,
            // Second?)?)?)`: the minute is optional inside the time, so an
            // `Hour` with nothing after it is a record saying what it
            // knows, not a record that has lost its minute. It is carried
            // at the precision given (`TimeOfDay::at_hour`) rather than
            // rounded to `:00`, which would put a minute in the normalized
            // layer that PubMed never asserted and that no later reader
            // could tell from a stated zero — the same rule as a `PubDate`
            // that stops at the year (2026-09-23, from the survey of the
            // whole 2026 baseline).
            (Some(hour), minute) => Some(history_time(hour, minute)),
            // The DTD cannot express this and the parser's content model
            // rejects it first; kept here so the value layer says so too.
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

/// A history date's `Year`, `Month` and `Day` as the day they name, or as
/// written with the first reason they name none.
fn history_date<'n>(d: &'n DateText<'_>) -> HistoryDate<'n> {
    let parsed = Year::parse(&d.year).and_then(|year| {
        let month = Month::parse(&d.month)?;
        let day = Day::parse(&d.day)?;
        Date::new(year, month, day)
    });
    match parsed {
        Ok(date) => HistoryDate::Valid(date),
        Err(error) => HistoryDate::Malformed {
            year: &d.year,
            month: &d.month,
            day: &d.day,
            error,
        },
    }
}

/// An `Hour` and the `Minute` beside it when the record wrote one, each one
/// or two digits (history dates write both `9` and `09`), as the time they
/// name or as written with why they name none: `null` in either is
/// [`DateError::TimeDigits`], digits past `23:59` [`DateError::Time`].
fn history_time<'n>(hour: &'n str, minute: Option<&'n str>) -> HistoryTime<'n> {
    let digits = |s: &str| -> Option<u8> {
        if s.is_empty() || s.len() > 2 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    };
    let time = match (digits(hour), minute.map(digits)) {
        (Some(hh), None) => TimeOfDay::at_hour(hh).ok_or(DateError::Time),
        (Some(hh), Some(Some(mm))) => TimeOfDay::new(hh, mm).ok_or(DateError::Time),
        _ => Err(DateError::TimeDigits),
    };
    match time {
        Ok(time) => HistoryTime::Valid(time),
        Err(error) => HistoryTime::Malformed {
            hour,
            minute,
            error,
        },
    }
}

/// What two assertions of one identifier come to: the value both of them
/// make, or nothing when they contradict each other.
///
/// `ArticleIdList` is declared `(ArticleId+)` and `Author` carries
/// `Identifier*`: neither the DTD nor PubMed's serializer promises one id
/// per type, and this file assumed both until the survey of the whole 2026
/// baseline, ~37M records, found 16 references, one record's own list and
/// 59 authors' identifier lists repeating a kind (2026-09-23). Each of
/// those records used to stop the run.
///
/// The survey named 15 of the 16 records with a repeated reference id; 10
/// of them still carry it and were re-fetched from `efetch` on 2026-09-23
/// (NLM has since tidied the other five). They are of three shapes:
///
/// * the same value twice — `10.1212/WNL.0000000000010155.` on both ids of
///   one reference of PMID 32796281, and on 2 of the other 9 records. One
///   assertion, made twice; there is nothing to choose.
/// * the value, and the value with the citation string's full stop still
///   attached — `10.1002/jmv.26224.` beside `10.1002/jmv.26224` on PMID
///   32925547, and on 5 of the other 9: the commonest of the three.
///   `doi.org`'s handle API resolves the bare form and 404s the dotted one
///   on every such pair checked, so the dot is the sentence's, not the
///   identifier's, and the bare form is what both assertions have in
///   common.
/// * two values that disagree — `10.1002/14651858.CD100105.` beside
///   `10.1002/14651858.CD000313.pub5` on one reference of PMID 32956132,
///   `…00639/full.` beside `10.3389/fphys.2017.00639` on PMID 32366796,
///   and `10.3205/2014-603` beside `10.17179/excli2014-603` on PMID
///   26417354's own list, the one record in ~37M whose own list repeats a
///   kind. Exactly one of each pair is registered (the others 404), but
///   nothing in the record says which, and position does not say either:
///   the registered DOI is the second id on PMID 32956132 and the first on
///   PMID 32694353. The list has not said, so this says nothing.
///
/// A longer value that merely starts with a shorter one is *not* treated
/// as the same identifier: `10.1002/14651858.CD000313` and `…CD000313.pub5`
/// are two revisions of one Cochrane review, and PMC's id converter gives
/// `10.12688/f1000research.40670.1` … `.4` for four versions of one
/// `F1000Research` article (2026-09-23). Only trailing sentence punctuation
/// is discarded. Comparison ignores ASCII case, which is right for a DOI
/// by definition and changes nothing for the digits and uppercase codes
/// the other kinds hold.
fn concur<'n>(held: &'n str, again: &'n str) -> Option<&'n str> {
    if held.eq_ignore_ascii_case(again) {
        return Some(held);
    }
    let (a, b) = (held.trim_end_matches('.'), again.trim_end_matches('.'));
    if a.eq_ignore_ascii_case(b) {
        // The one the punctuation is not on.
        return Some(if held.len() <= again.len() {
            held
        } else {
            again
        });
    }
    None
}

/// The record's own `ArticleId`s: one per type, from however many times
/// the list gives it ([`concur`]) — except the publisher's item id and the
/// kinds that are no name of ours, which a record may give more than once
/// and which are kept in full.
#[derive(Default)]
struct Own<'n> {
    pubmed: Option<&'n str>,
    doi: Option<&'n str>,
    pii: Few<&'n str, 1>,
    pmc: Option<&'n str>,
    pmcid: Option<&'n str>,
    mid: Option<&'n str>,
    medline: Option<&'n str>,
    sici: Option<&'n str>,
    loose: Few<LooseId<'n>, 1>,
}

impl<'n> Own<'n> {
    fn take(&mut self, ctx: &Ctx, id: &'n ArticleId<'_>) -> Result<(), Error> {
        const FIELD: &str = "ArticleIdList/ArticleId";
        let slot = match id.id_type {
            IdType::Pubmed => &mut self.pubmed,
            IdType::Doi => &mut self.doi,
            IdType::Pii => {
                self.pii.push(&id.value);
                return Ok(());
            }
            IdType::Pmc => &mut self.pmc,
            // `pmc` and `pmcid` are two tokens for one thing, the
            // record's PMC id: a citation's own list writes `pmc` in
            // every record looked at so far, a reference's list writes
            // either (see `references`, where both fill the one PMC
            // slot). A record's own list that writes `pmcid` is claiming
            // its PMC id, so the two meet in the one slot in
            // `identifiers`, and a record that writes both has claimed it
            // twice — which the one-per-type rule refuses, as it would two
            // `pmc`. They are held apart this far only because the two
            // tokens spell their values differently (2026-09-23).
            IdType::Pmcid => &mut self.pmcid,
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
            // The rest of the DTD's list, kept as written and never a
            // name (2026-09-23). `pmcpid` and `pmpid` are processing ids
            // from PMC's and PubMed's own loading, which identify a
            // deposit rather than a work; `pmcbook` and `bookaccession`
            // name a Bookshelf document, a namespace nothing here
            // resolves — and a citation that also carries one is not made
            // the same work as the book by our saying so. All four are
            // the `LooseId` decision of 2026-09-18, which a reference's
            // other ids already follow, applied to the record's own list:
            // keep the fact, refuse the join, and promote a kind later on
            // evidence rather than drop it now for want of a slot.
            //
            // Which of the four the survey of the whole 2026 baseline saw
            // here is not in its signature — that names the field, not the
            // token — and none of them turned up in the corpora on hand
            // (126,740 records of the last baseline files and the first
            // updates, 15,432 looked up by PMID, 1,900 sampled across
            // 1975-2024 through `efetch`), so the DTD is taken at its word
            // for all four at once. The match is exhaustive: a token added
            // to `IdType` will not compile until someone decides where it
            // goes.
            IdType::Pmcpid | IdType::Pmpid | IdType::Pmcbook | IdType::BookAccession => {
                self.loose.push(LooseId {
                    kind: id.id_type,
                    value: &id.value,
                });
                return Ok(());
            }
        };
        match *slot {
            None => *slot = Some(&id.value),
            // The list has named this kind before. Where the two
            // assertions agree the record has said one thing twice
            // ([`concur`]); where they disagree it has not said, and the
            // slot stays empty rather than picking one. PMID 26417354, the
            // only record in the 2026 baseline whose own list repeats a
            // kind, loses an unregistered DOI that way and keeps the
            // registered one it also gives as its `ELocationID`. Keeping
            // both would mean a list of DOIs in `Identifiers`, as
            // `elocation_doi` already is — a model change, and the user's
            // call (2026-09-23).
            Some(held) => match concur(held, &id.value) {
                Some(one) => *slot = Some(one),
                // Except for the record's own PMID: every other slot holds
                // something the record says *about* itself, and silence is
                // an answer there, but a record that names itself twice,
                // differently, is one this pipeline cannot key. Nothing in
                // the 2026 baseline does.
                None if id.id_type == IdType::Pubmed => {
                    return Err(ctx.err(
                        FIELD,
                        &id.value,
                        ErrorKind::Inconsistent("two ArticleId[pubmed] naming different records"),
                    ));
                }
                None => *slot = None,
            },
        }
        Ok(())
    }
}

/// `ArticleId IdType="pmc"`: `PMC` and digits, or `PMC`, digits, a dot and
/// a version — `PMC8459621.2` on PMID 34621506 (pubmed26n1339),
/// `PMC9277196.3` on PMID 35903216 (2026 baseline).
///
/// That suffix is PMC's own version counter, not PubMed's. The PMC id
/// converter (2026-09-23) lists `PMC9277196.1` … `PMC9277196.4` for the
/// four versions of one `F1000Research` article, each with its own DOI
/// (`10.12688/f1000research.40670.1` … `.4`), while `PMID@Version` counts
/// revisions of the *citation*: NLM makes a new one for a metadata change
/// too. The two counters run over different objects and agree only by
/// coincidence. This file required them to agree (2026-09-18) on the guess
/// that the version was `PMID@Version`'s to hold; 12 records of the 2026
/// baseline, ~37M, refute it — PMID 35903216 is at version 4 with the
/// article's third version in PMC — and each of them used to stop the run.
///
/// The suffix is read only to check that it is a version number, and then
/// dropped, as before: `PmcId` names the article, nothing here models a
/// version of one, and a record whose counters do happen to agree — PMID
/// 34621506 in pubmed26n1339 — normalizes to what it always did. A
/// suffix that is not a number is
/// still an error — no record has shown one, and a `pmc` slot this file
/// cannot read is not a slot to guess at. A value that is not a PMC id at
/// all is a malformed claim, kept as written (2026-09-18).
fn pmc_of<'n>(ctx: &Ctx, raw: &'n str) -> Result<PmcClaim<'n>, Error> {
    const FIELD: &str = "ArticleId[pmc]";
    let (id, suffix) = match raw.split_once('.') {
        Some((id, suffix)) => (id, Some(suffix)),
        None => (raw, None),
    };
    let pmc = match PmcId::from_prefixed(id) {
        Ok(pmc) => pmc,
        Err(error) => return Ok(Claim::Malformed { raw, error }),
    };
    if let Some(suffix) = suffix {
        model::id::strict_u32(suffix).map_err(|e| ctx.err(FIELD, raw, ErrorKind::Id(e)))?;
    }
    Ok(Claim::Valid(pmc))
}

/// `ArticleId IdType="pmcid"` on a record's own list: the same PMC id
/// under the token a reference's list uses. That token's value is written
/// both ways in the corpus — bare digits in the 2026 baseline files,
/// `PMC` and digits in the references of records looked up from older
/// loads (see [`references`]) — and no record's own list in the corpora on
/// hand writes the token at all, so both spellings are read and nothing
/// else is. A value in neither is a malformed claim, kept as written and
/// never a name, as a `pmc` slot that does not parse already is
/// (2026-09-23).
fn pmcid_of(raw: &str) -> PmcClaim<'_> {
    let parsed = if raw.starts_with(PmcId::PREFIX) {
        PmcId::from_prefixed(raw)
    } else {
        PmcId::from_digits(raw)
    };
    Claim::from_parse(raw, parsed)
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
    let doi = own
        .doi
        .map(|raw| Claim::from_parse(raw, ctx.doi("ArticleId(doi)", raw)));
    // One PMC slot for the two tokens that fill it. `pmc_of` no longer takes
    // the citation's version to check a suffix against: PMC counts versions
    // of the article and `PMID@Version` counts versions of the record about
    // it, so the two never had to agree and the twelve records that stopped
    // the run over it were right and the rule was wrong (2026-09-23).
    //
    // A record giving both is not claiming its PMC id twice over, whatever
    // it looked like: 106 records of the 2026 baseline write `pmc` and
    // `pmcid` together (survey v4, 2026-09-23), and until `pmcid` was read
    // at all they failed further up as an id kind with no slot. They are
    // two spellings of one claim and meet the same rule as any other pair
    // of assertions — where they agree the record has said one thing twice
    // ([`concur`]); where they disagree it has not said, and the slot is
    // empty. Only the record's own PMID is worth refusing over.
    let pmc = match (own.pmc, own.pmcid) {
        (Some(raw), None) => Some(pmc_of(ctx, raw)?),
        (None, Some(raw)) => Some(pmcid_of(raw)),
        (None, None) => None,
        (Some(one), Some(other)) => {
            let (a, b) = (pmc_of(ctx, one)?, pmcid_of(other));
            match (&a, &b) {
                // The same id, however each was spelled.
                (Claim::Valid(x), Claim::Valid(y)) if x == y => Some(a),
                // Both malformed and textually the same thing: one claim.
                _ if concur(one, other).is_some() => Some(a),
                _ => None,
            }
        }
    };

    let (mut elocation_doi, mut elocation_pii) = (Few::new(), Few::new());
    let mut invalid_elocation = Few::new();
    for e in &article.medline_citation.article.elocation_ids {
        // `ValidYN="N"` is NLM saying this electronic location is the
        // wrong one — the identifier a publisher sent before it sent a
        // correction, kept beside the corrected one rather than deleted.
        // It goes to its own slot, where it is kept and is no name of this
        // work (see `InvalidElocation`, 2026-09-23).
        if e.valid == YesNo::No {
            invalid_elocation.push(InvalidElocation {
                kind: e.eid_type,
                value: &e.value,
            });
            continue;
        }
        match e.eid_type {
            EIdType::Doi => elocation_doi.push(Claim::from_parse(
                &e.value,
                ctx.doi("ELocationID(doi)", &e.value),
            )),
            EIdType::Pii => elocation_pii.push(&*e.value),
        }
    }
    // Two valid DOIs on one record are kept, both of them: the slots
    // disagree on 4 of 126,740 records, and Crossref says the right one is
    // sometimes the `ArticleId`, sometimes the `ELocationID`, and
    // sometimes both — an early-online DOI beside the issue's (2026-09-18,
    // merge plan §3.3, "two DOIs, one work").
    Ok(Identifiers {
        doi,
        elocation_doi,
        pii: own.pii,
        mid: own.mid,
        elocation_pii,
        pmc,
        medline: own.medline,
        sici: own.sici,
        loose: own.loose,
        invalid_elocation,
    })
}

fn dates<'n>(ctx: &Ctx, c: &'n MedlineCitation<'_>) -> Result<Dates<'n>, Error> {
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
///
/// `SpaceFlightMission` was on this list until 2026-09-23, when the oldest
/// baseline file (pubmed26n0001) first showed it: 20 elements over 4
/// records, NASA's indexing of space-biology studies. Its values are
/// `Skylab 2 Project`, `Skylab 3 Project`, `Skylab 4 Project`, `Soyuz 22
/// Project`, `OFO-1(A) Project`, `Flight Experiment`, `manned`, `unmanned`,
/// `short duration` and `long duration` — free text about the flight, not
/// about the work. Nothing here identifies a work, cites one or is searched,
/// so the field is parsed and dropped rather than modelled, which would have
/// cost a model version for ten phrases in the corpus's first file. The raw
/// layer keeps it if it is ever wanted (the user's call, 2026-09-23).
fn unmodelled(ctx: &Ctx, article: &PubmedArticle<'_>) -> Result<(), Error> {
    let checks: [(&'static str, bool); 1] = [(
        "ObjectList",
        article
            .pubmed_data
            .as_ref()
            .is_some_and(|d| !d.objects.is_empty()),
    )];
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
    let mut ctx = Ctx {
        pmid: None,
        corrected: std::cell::RefCell::new(Vec::new()),
    };
    let pmid = Pmid::from_digits(&c.pmid.value)
        .map_err(|e| ctx.err("PMID", &c.pmid.value, ErrorKind::Id(e)))?;
    ctx.pmid = Some(pmid);
    let version = model::id::strict_u32(&c.pmid.version)
        .map_err(|e| ctx.err("PMID@Version", &c.pmid.version, ErrorKind::Id(e)))?;
    let revised = c
        .date_revised
        .as_ref()
        .map(|d| ctx.date("DateRevised", d))
        .transpose()?;
    if let Some(reason) = crate::exclusions::reason(pmid, version, revised) {
        emit.excluded(&pmid, reason);
        return Ok(());
    }
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

    // A code `attr::NOT_A_LANGUAGE` has read is not a language the record is
    // in — one stray `crg` in the whole 2026 baseline — and is dropped, so a
    // record keeps whatever it does say. Any other code outside ISO 639-2
    // still stops the run.
    let languages = c
        .article
        .languages
        .iter()
        .filter(|l| crate::attr::not_a_language(l).is_none())
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
        // No PMID: a `RefSource` that reads whole as a DOI names the
        // record (model version 12), each reading counted as any DOI's.
        let doi = match (&pmid, &cc.ref_source) {
            (None, std::borrow::Cow::Borrowed(source)) => {
                ctx.doi("CommentsCorrections/RefSource", source).ok()
            }
            _ => None,
        };
        relations.push(Relation {
            kind: cc.ref_type,
            pmid,
            doi,
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

    for (field, how) in ctx.corrected.take() {
        emit.corrected(&pmid, field, how);
    }
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
        references(pmid, &data.reference_lists, emit);
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
    let ctx = Ctx {
        pmid: None,
        corrected: std::cell::RefCell::new(Vec::new()),
    };
    for p in &deleted.pmids {
        let pmid = Pmid::from_digits(&p.value)
            .map_err(|e| ctx.err("DeleteCitation/PMID", &p.value, ErrorKind::Id(e)))?;
        model::id::strict_u32(&p.version)
            .map_err(|e| ctx.err("DeleteCitation/PMID@Version", &p.version, ErrorKind::Id(e)))?;
        emit.deletion(&pmid);
    }
    Ok(())
}

/// One `ArticleId` of a reference: its kind, the value as written, and
/// what that value parsed to.
struct Said<'n, T, E> {
    kind: IdType,
    raw: &'n str,
    claim: Claim<'n, T, E>,
}

/// What a reference says in one of its id slots, once — or nothing, when
/// its assertions contradict each other, in which case every one of them
/// goes to `loose`, which the matching stage keeps and which is never a
/// name.
///
/// Both halves of that matter. A reference names one work: the merge
/// unions every name a reference carries into a single identity (merge
/// plan §6), so emitting two DOIs that disagree would make one work out of
/// two — a false identity, and a worse error than a citation edge that
/// goes unmatched. But the values are still evidence about what was cited,
/// and `loose` is where a reference's ids that name nothing already live
/// (2026-09-18), so nothing is thrown away either.
///
/// Where the assertions do agree — the same value twice, or the value
/// beside itself with the citation's full stop attached ([`concur`]) — the
/// reference keeps its id and its edge. Of the 16 repeated ids in the 10
/// records re-fetched on 2026-09-23, 14 agree that way; the two that do
/// not (PMID 32956132 and PMID 32366796) lose a DOI name and keep their
/// citation text, which is what the matching stage has to work with in any
/// case for the 861,418 of 3,532,199 references in the files on hand that
/// carry no id at all.
/// The claim is moved out rather than copied: a lowercased DOI owns its
/// text, and cloning it would allocate a second copy of one reference id in
/// seven across the corpus.
fn kept<'n, T: PartialEq, E: PartialEq>(
    said: Few<Said<'n, T, E>, 1>,
    loose: &mut Few<LooseId<'n>, 1>,
) -> Option<Claim<'n, T, E>> {
    let mut one = 0;
    for (i, other) in said.iter().enumerate().skip(1) {
        // Parsed values, not text: a `pmcid` written as bare digits and a
        // `pmc` written with the prefix are the same id said twice.
        if other.claim == said[one].claim {
            continue;
        }
        match concur(said[one].raw, other.raw) {
            Some(agreed) if agreed == other.raw => one = i,
            Some(_) => {}
            None => {
                loose.extend(said.iter().map(|s| LooseId {
                    kind: s.kind,
                    value: s.raw,
                }));
                return None;
            }
        }
    }
    said.into_iter().nth(one).map(|s| s.claim)
}

fn references<'n, K: Sink<'n, Pubmed>>(
    from: Pmid,
    lists: &'n [ReferenceListRecord<'_>],
    emit: &mut K,
) {
    for list in lists {
        for r in &list.references {
            let mut pmid: Few<Said<'n, Pmid, ParseError>, 1> = Few::new();
            let mut doi: Few<Said<'n, Doi<'n>, DoiError>, 1> = Few::new();
            let mut pmc: Few<Said<'n, PmcId, ParseError>, 1> = Few::new();
            let mut loose = Few::new();
            for id in &r.article_ids {
                let (kind, raw) = (id.id_type, &*id.value);
                match kind {
                    IdType::Pubmed => pmid.push(Said {
                        kind,
                        raw,
                        claim: Claim::from_parse(raw, Pmid::from_digits(raw)),
                    }),
                    IdType::Doi => doi.push(Said {
                        kind,
                        raw,
                        claim: Claim::from_parse(
                            raw,
                            Doi::read(raw).map(|(doi, readings)| {
                                for how in readings.made() {
                                    emit.corrected(&from, "ReferenceList/ArticleId(doi)", how);
                                }
                                doi
                            }),
                        ),
                    }),
                    // The 2026 baseline files write `pmcid` with bare
                    // digits; 295,126 reference ids looked up from older
                    // records (2026-09-15) write `pmc` with the prefix,
                    // as a record's own id does.
                    IdType::Pmcid => pmc.push(Said {
                        kind,
                        raw,
                        claim: Claim::from_parse(raw, PmcId::from_digits(raw)),
                    }),
                    IdType::Pmc => pmc.push(Said {
                        kind,
                        raw,
                        claim: Claim::from_parse(raw, PmcId::from_prefixed(raw)),
                    }),
                    // Any other kind is kept as written and used by
                    // nothing here (2026-09-18).
                    kind => loose.push(LooseId { kind, value: raw }),
                }
            }
            emit.reference(
                &from,
                &Reference {
                    pmid: kept(pmid, &mut loose),
                    doi: kept(doi, &mut loose),
                    pmc: kept(pmc, &mut loose),
                    loose,
                    citation: r.citation.as_str(),
                },
            );
        }
        references(from, &list.sub_lists, emit);
    }
}
