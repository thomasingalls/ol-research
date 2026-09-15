//! The `PubmedData` subtree and `DeleteCitation`.

use super::{Child, DateSlots, Parser, Result, Tag, list, pmid};
use crate::attr::{IdType, PubStatus};
use crate::error::ErrorKind;
use crate::record::{
    ArticleId, DeleteCitation, Few, Object, Param, PubMedPubDate, PubmedData, Reference,
    ReferenceList,
};

/// `PubmedData (History?, PublicationStatus, ArticleIdList, ObjectList?, ReferenceList*)`.
pub(super) fn pubmed_data<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<PubmedData<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut history, mut publication_status, mut article_ids, mut objects) =
        (None, None, None, None);
    let mut reference_lists = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "History" => set_once!(
                p,
                history,
                "History",
                list(p, t, "PubMedPubDate", pubmed_pub_date)?
            ),
            "PublicationStatus" => {
                set_once!(p, publication_status, "PublicationStatus", p.leaf(&t)?);
            }
            "ArticleIdList" => set_once!(p, article_ids, "ArticleIdList", article_id_list(p, t)?),
            "ObjectList" => set_once!(p, objects, "ObjectList", list(p, t, "Object", object)?),
            "ReferenceList" => reference_lists.push(reference_list(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = PubmedData {
        history: history.unwrap_or_default(),
        publication_status: p.required("PublicationStatus", publication_status)?,
        article_ids: p.required("ArticleIdList", article_ids)?,
        objects: objects.unwrap_or_default(),
        reference_lists,
    };
    p.leave();
    Ok(out)
}

/// `PubMedPubDate (Year, Month, Day, (Hour, (Minute, Second?)?)?)` with `PubStatus`.
fn pubmed_pub_date<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<PubMedPubDate<'a>> {
    p.enter(&tag);
    let mut pub_status = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "PubStatus" => pub_status = Some(p.enum_attr("PubStatus", &value, PubStatus::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let pub_status = p.required_attr("PubStatus", pub_status)?;
    let mut slots = DateSlots::default();
    let (mut hour, mut minute, mut second) = (None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        let Some(t) = slots.take(p, t)? else { continue };
        match t.name() {
            "Hour" => set_once!(p, hour, "Hour", p.leaf(&t)?),
            "Minute" => set_once!(p, minute, "Minute", p.leaf(&t)?),
            "Second" => set_once!(p, second, "Second", p.leaf(&t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    if minute.is_some() && hour.is_none() {
        return Err(p.content_model("Minute without Hour"));
    }
    if second.is_some() && minute.is_none() {
        return Err(p.content_model("Second without Minute"));
    }
    let out = PubMedPubDate {
        pub_status,
        date: slots.finish(p)?,
        hour,
        minute,
        second,
    };
    p.leave();
    Ok(out)
}

/// `ArticleIdList (ArticleId+)`.
pub(super) fn article_id_list<'a>(
    p: &mut Parser<'a>,
    tag: Tag<'a>,
) -> Result<Few<ArticleId<'a>, 4>> {
    list(p, tag, "ArticleId", article_id)
}

/// `ArticleId (#PCDATA)` with `IdType "pubmed"`.
fn article_id<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<ArticleId<'a>> {
    p.enter(&tag);
    let mut id_type = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "IdType" => id_type = Some(p.enum_attr("IdType", &value, IdType::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = ArticleId {
        id_type: id_type.unwrap_or(IdType::Pubmed),
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `Object (Param*)` with `Type CDATA #REQUIRED`.
fn object<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Object<'a>> {
    p.enter(&tag);
    let mut r#type = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "Type" => r#type = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let r#type = p.required_attr("Type", r#type)?;
    let mut params = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Param" => params.push(param(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    p.leave();
    Ok(Object { r#type, params })
}

/// `Param (%text;)*` with `Name CDATA #REQUIRED`.
fn param<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Param<'a>> {
    p.enter(&tag);
    let mut name = None;
    p.each_attr(&tag, |key, value| {
        match key {
            "Name" => name = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = Param {
        name: p.required_attr("Name", name)?,
        content: p.markup(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `ReferenceList (Title?, Reference*, ReferenceList*)`.
fn reference_list<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<ReferenceList<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let mut title = None;
    let mut references = Few::new();
    let mut sub_lists = Vec::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Title" => set_once!(p, title, "Title", p.leaf(&t)?),
            "Reference" => references.push(reference(p, t)?),
            "ReferenceList" => sub_lists.push(reference_list(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    p.leave();
    Ok(ReferenceList {
        title,
        references,
        sub_lists,
    })
}

/// `Reference (Citation, ArticleIdList?)`.
fn reference<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Reference<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut citation, mut article_ids) = (None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Citation" => set_once!(p, citation, "Citation", p.mixed(&t)?),
            "ArticleIdList" => set_once!(p, article_ids, "ArticleIdList", article_id_list(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = Reference {
        citation: p.required("Citation", citation)?,
        article_ids: article_ids.unwrap_or_default(),
    };
    p.leave();
    Ok(out)
}

/// `DeleteCitation (PMID+)`.
pub(super) fn delete_citation<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<DeleteCitation<'a>> {
    let pmids = list(p, tag, "PMID", pmid)?;
    if pmids.is_empty() {
        return Err(p.error(ErrorKind::MissingElement("PMID")));
    }
    Ok(DeleteCitation { pmids })
}
