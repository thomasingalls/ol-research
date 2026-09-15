//! `PubmedArticle` and the `MedlineCitation` subtree.

use super::{Child, Parser, Result, Tag, date_ymd, list, pmid};
use crate::attr::{
    DescriptorType, KeywordOwner, NoteOwner, OtherAbstractType, OtherIdSource, Owner, RefType,
    Status, SupplMeshType, YesNo,
};
use crate::error::ErrorKind;
use crate::record::{
    AffiliationInfo, Chemical, CommentsCorrections, DescriptorName, Few, GeneralNote, Identifier,
    Investigator, InvestigatorList, Keyed, Keyword, KeywordList, MedlineCitation,
    MedlineJournalInfo, MeshHeading, OtherAbstract, OtherId, PersonName, PubmedArticle,
    QualifierName, SupplMeshName,
};
use crate::text::{RawXml, Text};

/// `PubmedArticle (MedlineCitation, PubmedData?)`.
pub(super) fn pubmed_article<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<PubmedArticle<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut citation, mut data) = (None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "MedlineCitation" => set_once!(p, citation, "MedlineCitation", medline_citation(p, t)?),
            "PubmedData" => set_once!(p, data, "PubmedData", super::data::pubmed_data(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = PubmedArticle {
        medline_citation: p.required("MedlineCitation", citation)?,
        pubmed_data: data,
    };
    p.leave();
    Ok(out)
}

// One arm per child element of the content model; splitting would hide the
// model, not shorten it.
#[allow(clippy::too_many_lines)]
fn medline_citation<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<MedlineCitation<'a>> {
    p.enter(&tag);
    let (mut owner, mut status) = (None, None);
    let (mut version_id, mut version_date, mut indexing_method) = (None, None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "Owner" => owner = Some(p.enum_attr("Owner", &value, Owner::parse)?),
            "Status" => status = Some(p.enum_attr("Status", &value, Status::parse)?),
            "VersionID" => version_id = Some(value),
            "VersionDate" => version_date = Some(value),
            "IndexingMethod" => indexing_method = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let status = p.required_attr("Status", status)?;

    let (mut pmid_slot, mut date_completed, mut date_revised) = (None, None, None);
    let (mut article, mut journal_info) = (None, None);
    let (mut chemicals, mut suppl_mesh, mut comments_corrections) = (None, None, None);
    let (mut gene_symbols, mut mesh_headings, mut personal_name_subjects) = (None, None, None);
    let (mut number_of_references, mut coi_statement) = (None, None);
    let mut citation_subsets = Few::new();
    let mut other_ids = Few::new();
    let mut other_abstracts = Few::new();
    let mut keyword_lists = Few::new();
    let mut space_flight_missions = Few::new();
    let mut investigator_lists = Few::new();
    let mut general_notes = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "PMID" => {
                let value = pmid(p, t)?;
                p.set_pmid(&value.value);
                set_once!(p, pmid_slot, "PMID", value);
            }
            "DateCompleted" => set_once!(p, date_completed, "DateCompleted", date_ymd(p, t)?),
            "DateRevised" => set_once!(p, date_revised, "DateRevised", date_ymd(p, t)?),
            "Article" => set_once!(p, article, "Article", super::article::article(p, t)?),
            "MedlineJournalInfo" => {
                set_once!(
                    p,
                    journal_info,
                    "MedlineJournalInfo",
                    medline_journal_info(p, t)?
                );
            }
            "ChemicalList" => {
                set_once!(
                    p,
                    chemicals,
                    "ChemicalList",
                    list(p, t, "Chemical", chemical)?
                );
            }
            "SupplMeshList" => {
                set_once!(
                    p,
                    suppl_mesh,
                    "SupplMeshList",
                    list(p, t, "SupplMeshName", suppl_mesh_name)?
                );
            }
            "CitationSubset" => citation_subsets.push(p.leaf(&t)?),
            "CommentsCorrectionsList" => set_once!(
                p,
                comments_corrections,
                "CommentsCorrectionsList",
                list(p, t, "CommentsCorrections", comments_corrections_elem)?
            ),
            "GeneSymbolList" => set_once!(
                p,
                gene_symbols,
                "GeneSymbolList",
                list(p, t, "GeneSymbol", |p, t| p.leaf(&t))?
            ),
            "MeshHeadingList" => {
                set_once!(
                    p,
                    mesh_headings,
                    "MeshHeadingList",
                    list(p, t, "MeshHeading", mesh_heading)?
                );
            }
            "NumberOfReferences" => {
                set_once!(p, number_of_references, "NumberOfReferences", p.leaf(&t)?);
            }
            "PersonalNameSubjectList" => set_once!(
                p,
                personal_name_subjects,
                "PersonalNameSubjectList",
                list(p, t, "PersonalNameSubject", personal_name_subject)?
            ),
            "OtherID" => other_ids.push(other_id(p, t)?),
            "OtherAbstract" => other_abstracts.push(other_abstract(p, t)?),
            "KeywordList" => keyword_lists.push(keyword_list(p, t)?),
            "CoiStatement" => set_once!(p, coi_statement, "CoiStatement", p.mixed(&t)?),
            "SpaceFlightMission" => space_flight_missions.push(p.leaf(&t)?),
            "InvestigatorList" => investigator_lists.push(investigator_list(p, t)?),
            "GeneralNote" => general_notes.push(general_note(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = MedlineCitation {
        owner: owner.unwrap_or(Owner::Nlm),
        status,
        version_id,
        version_date,
        indexing_method,
        pmid: p.required("PMID", pmid_slot)?,
        date_completed,
        date_revised,
        article: p.required("Article", article)?,
        medline_journal_info: p.required("MedlineJournalInfo", journal_info)?,
        chemicals: chemicals.unwrap_or_default(),
        suppl_mesh: suppl_mesh.unwrap_or_default(),
        citation_subsets,
        comments_corrections: comments_corrections.unwrap_or_default(),
        gene_symbols: gene_symbols.unwrap_or_default(),
        mesh_headings: mesh_headings.unwrap_or_default(),
        number_of_references,
        personal_name_subjects: personal_name_subjects.unwrap_or_default(),
        other_ids,
        other_abstracts,
        keyword_lists,
        coi_statement,
        space_flight_missions,
        investigator_lists,
        general_notes,
    };
    p.leave();
    Ok(out)
}

/// `MedlineJournalInfo (Country?, MedlineTA, NlmUniqueID?, ISSNLinking?)`.
fn medline_journal_info<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<MedlineJournalInfo<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut country, mut medline_ta, mut nlm_unique_id, mut issn_linking) =
        (None, None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Country" => set_once!(p, country, "Country", p.leaf(&t)?),
            "MedlineTA" => set_once!(p, medline_ta, "MedlineTA", p.leaf(&t)?),
            "NlmUniqueID" => set_once!(p, nlm_unique_id, "NlmUniqueID", p.leaf(&t)?),
            "ISSNLinking" => set_once!(p, issn_linking, "ISSNLinking", p.leaf(&t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = MedlineJournalInfo {
        country,
        medline_ta: p.required("MedlineTA", medline_ta)?,
        nlm_unique_id,
        issn_linking,
    };
    p.leave();
    Ok(out)
}

/// A `#PCDATA` element with `UI CDATA #REQUIRED` and nothing else.
pub(super) fn keyed<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Keyed<'a>> {
    p.enter(&tag);
    let mut ui = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "UI" => ui = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let ui = p.required_attr("UI", ui)?;
    let value = p.text(&tag)?;
    p.leave();
    Ok(Keyed { ui, value })
}

/// `Chemical (RegistryNumber, NameOfSubstance)`.
fn chemical<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Chemical<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut registry_number, mut name_of_substance) = (None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "RegistryNumber" => set_once!(p, registry_number, "RegistryNumber", p.leaf(&t)?),
            "NameOfSubstance" => set_once!(p, name_of_substance, "NameOfSubstance", keyed(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = Chemical {
        registry_number: p.required("RegistryNumber", registry_number)?,
        name_of_substance: p.required("NameOfSubstance", name_of_substance)?,
    };
    p.leave();
    Ok(out)
}

/// `SupplMeshName (#PCDATA)` with `Type` and `UI`, both required.
fn suppl_mesh_name<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<SupplMeshName<'a>> {
    p.enter(&tag);
    let (mut r#type, mut ui) = (None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "Type" => r#type = Some(p.enum_attr("Type", &value, SupplMeshType::parse)?),
            "UI" => ui = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = SupplMeshName {
        r#type: p.required_attr("Type", r#type)?,
        ui: p.required_attr("UI", ui)?,
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `CommentsCorrections (RefSource, PMID?, Note?)` with `RefType` required.
fn comments_corrections_elem<'a>(
    p: &mut Parser<'a>,
    tag: Tag<'a>,
) -> Result<CommentsCorrections<'a>> {
    p.enter(&tag);
    let mut ref_type = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "RefType" => ref_type = Some(p.enum_attr("RefType", &value, RefType::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let ref_type = p.required_attr("RefType", ref_type)?;
    let (mut ref_source, mut pmid_slot, mut note) = (None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "RefSource" => set_once!(p, ref_source, "RefSource", p.leaf(&t)?),
            "PMID" => set_once!(p, pmid_slot, "PMID", pmid(p, t)?),
            "Note" => set_once!(p, note, "Note", p.leaf(&t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = CommentsCorrections {
        ref_type,
        ref_source: p.required("RefSource", ref_source)?,
        pmid: pmid_slot,
        note,
    };
    p.leave();
    Ok(out)
}

/// `MeshHeading (DescriptorName, QualifierName*)`.
fn mesh_heading<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<MeshHeading<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let mut descriptor = None;
    let mut qualifiers = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "DescriptorName" => set_once!(p, descriptor, "DescriptorName", descriptor_name(p, t)?),
            "QualifierName" => qualifiers.push(qualifier_name(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = MeshHeading {
        descriptor: p.required("DescriptorName", descriptor)?,
        qualifiers,
    };
    p.leave();
    Ok(out)
}

/// `AutoHM (Y) #IMPLIED`: present means `Y`; anything else is invalid.
fn auto_hm(p: &Parser<'_>, value: &str) -> Result<bool> {
    if value == "Y" {
        Ok(true)
    } else {
        Err(p.error(ErrorKind::InvalidAttributeValue {
            name: "AutoHM",
            value: value.to_owned(),
        }))
    }
}

/// `DescriptorName (#PCDATA)` with `MajorTopicYN "N"`, `AutoHM`, `Type`, `UI` required.
fn descriptor_name<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<DescriptorName<'a>> {
    p.enter(&tag);
    let (mut major_topic, mut auto, mut r#type, mut ui) = (None, false, None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "MajorTopicYN" => {
                major_topic = Some(p.enum_attr("MajorTopicYN", &value, YesNo::parse)?);
            }
            "AutoHM" => auto = auto_hm(p, &value)?,
            "Type" => r#type = Some(p.enum_attr("Type", &value, DescriptorType::parse)?),
            "UI" => ui = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = DescriptorName {
        major_topic: major_topic.unwrap_or(YesNo::No),
        auto_hm: auto,
        r#type,
        ui: p.required_attr("UI", ui)?,
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `QualifierName (#PCDATA)` with `AutoHM`, `MajorTopicYN "N"`, `UI` required.
fn qualifier_name<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<QualifierName<'a>> {
    p.enter(&tag);
    let (mut major_topic, mut auto, mut ui) = (None, false, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "MajorTopicYN" => {
                major_topic = Some(p.enum_attr("MajorTopicYN", &value, YesNo::parse)?);
            }
            "AutoHM" => auto = auto_hm(p, &value)?,
            "UI" => ui = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = QualifierName {
        major_topic: major_topic.unwrap_or(YesNo::No),
        auto_hm: auto,
        ui: p.required_attr("UI", ui)?,
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// The four name children shared by `Author`, `Investigator` and
/// `PersonalNameSubject`.
#[derive(Default)]
pub(super) struct NameSlots<'a> {
    last_name: Option<Text<'a>>,
    fore_name: Option<Text<'a>>,
    initials: Option<Text<'a>>,
    suffix: Option<RawXml<'a>>,
}

impl<'a> NameSlots<'a> {
    /// Consumes `tag` if it is a name part; hands it back otherwise.
    pub(super) fn take(&mut self, p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Option<Tag<'a>>> {
        match tag.name() {
            "LastName" => set_once!(p, self.last_name, "LastName", p.leaf(&tag)?),
            "ForeName" => set_once!(p, self.fore_name, "ForeName", p.leaf(&tag)?),
            "Initials" => set_once!(p, self.initials, "Initials", p.leaf(&tag)?),
            "Suffix" => set_once!(p, self.suffix, "Suffix", p.mixed(&tag)?),
            _ => return Ok(Some(tag)),
        }
        Ok(None)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.last_name.is_none()
            && self.fore_name.is_none()
            && self.initials.is_none()
            && self.suffix.is_none()
    }

    pub(super) fn finish(self, p: &Parser<'a>) -> Result<PersonName<'a>> {
        Ok(PersonName {
            last_name: p.required("LastName", self.last_name)?,
            fore_name: self.fore_name,
            initials: self.initials,
            suffix: self.suffix,
        })
    }
}

/// `PersonalNameSubject (LastName, ForeName?, Initials?, Suffix?)`.
fn personal_name_subject<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<PersonName<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let mut name = NameSlots::default();
    while let Child::Elem(t) = p.next_child(&tag)? {
        if let Some(t) = name.take(p, t)? {
            return Err(p.unexpected(&t));
        }
    }
    let out = name.finish(p)?;
    p.leave();
    Ok(out)
}

/// `Identifier (#PCDATA)` with `Source CDATA #REQUIRED`.
pub(super) fn identifier<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Identifier<'a>> {
    p.enter(&tag);
    let mut source = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "Source" => source = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = Identifier {
        source: p.required_attr("Source", source)?,
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `AffiliationInfo (Affiliation, Identifier*)`.
pub(super) fn affiliation_info<'a>(
    p: &mut Parser<'a>,
    tag: Tag<'a>,
) -> Result<AffiliationInfo<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let mut affiliation = None;
    let mut identifiers = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Affiliation" => set_once!(p, affiliation, "Affiliation", p.mixed(&t)?),
            "Identifier" => identifiers.push(identifier(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = AffiliationInfo {
        affiliation: p.required("Affiliation", affiliation)?,
        identifiers,
    };
    p.leave();
    Ok(out)
}

/// `OtherID (#PCDATA)` with `Source` required.
fn other_id<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<OtherId<'a>> {
    p.enter(&tag);
    let mut source = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "Source" => source = Some(p.enum_attr("Source", &value, OtherIdSource::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = OtherId {
        source: p.required_attr("Source", source)?,
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `OtherAbstract (AbstractText+, CopyrightInformation?)` with `Type`
/// required and `Language "eng"`.
fn other_abstract<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<OtherAbstract<'a>> {
    p.enter(&tag);
    let (mut r#type, mut language) = (None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "Type" => r#type = Some(p.enum_attr("Type", &value, OtherAbstractType::parse)?),
            "Language" => language = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let r#type = p.required_attr("Type", r#type)?;
    let (texts, copyright_information) = super::article::abstract_children(p, &tag)?;
    let out = OtherAbstract {
        r#type,
        language: language.unwrap_or(Text::Borrowed("eng")),
        texts,
        copyright_information,
    };
    p.leave();
    Ok(out)
}

/// `KeywordList (Keyword+)` with `Owner "NLM"`.
fn keyword_list<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<KeywordList<'a>> {
    p.enter(&tag);
    let mut owner = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "Owner" => owner = Some(p.enum_attr("Owner", &value, KeywordOwner::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let mut keywords = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Keyword" => keywords.push(keyword(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = KeywordList {
        owner: owner.unwrap_or(KeywordOwner::Nlm),
        keywords: p.at_least_one("Keyword", keywords)?,
    };
    p.leave();
    Ok(out)
}

/// `Keyword (%text; | mml:math)*` with `MajorTopicYN "N"`.
fn keyword<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Keyword<'a>> {
    p.enter(&tag);
    let mut major_topic = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "MajorTopicYN" => {
                major_topic = Some(p.enum_attr("MajorTopicYN", &value, YesNo::parse)?);
            }
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = Keyword {
        major_topic: major_topic.unwrap_or(YesNo::No),
        content: p.markup(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `InvestigatorList (Investigator+)` with `ID #IMPLIED`.
fn investigator_list<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<InvestigatorList<'a>> {
    p.enter(&tag);
    let mut id = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "ID" => id = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let mut investigators = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Investigator" => investigators.push(investigator(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = InvestigatorList {
        id,
        investigators: p.at_least_one("Investigator", investigators)?,
    };
    p.leave();
    Ok(out)
}

/// `Investigator (LastName, ForeName?, Initials?, Suffix?, Identifier*,
/// AffiliationInfo*)` with `ValidYN "Y"`.
fn investigator<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Investigator<'a>> {
    p.enter(&tag);
    let mut valid = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "ValidYN" => valid = Some(p.enum_attr("ValidYN", &value, YesNo::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let mut name = NameSlots::default();
    let mut identifiers = Few::new();
    let mut affiliations = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        let Some(t) = name.take(p, t)? else { continue };
        match t.name() {
            "Identifier" => identifiers.push(identifier(p, t)?),
            "AffiliationInfo" => affiliations.push(affiliation_info(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = Investigator {
        valid: valid.unwrap_or(YesNo::Yes),
        name: name.finish(p)?,
        identifiers,
        affiliations,
    };
    p.leave();
    Ok(out)
}

/// `GeneralNote (#PCDATA)` with `Owner "NLM"`.
fn general_note<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<GeneralNote<'a>> {
    p.enter(&tag);
    let mut owner = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "Owner" => owner = Some(p.enum_attr("Owner", &value, NoteOwner::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = GeneralNote {
        owner: owner.unwrap_or(NoteOwner::Nlm),
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}
