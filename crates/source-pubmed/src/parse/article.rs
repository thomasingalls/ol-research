//! The `Article` subtree.

use super::citation::{NameSlots, affiliation_info, identifier, keyed};
use super::{Child, DateSlots, Parser, Result, Tag, list};
use crate::attr::{AuthorListType, CitedMedium, EIdType, IssnType, NlmCategory, PubModel, YesNo};
use crate::error::ErrorKind;
use crate::record::{
    Abstract, AbstractText, Article, ArticleTitle, Author, AuthorList, AuthorName, CollectiveName,
    DataBank, DataBankList, Date, ELocationId, Few, Grant, GrantList, Issn, Journal, JournalIssue,
    Pagination, PubDate,
};
use crate::text::Text;

/// `Article (Journal, ArticleTitle, ((Pagination, ELocationID*) | ELocationID+),
/// Abstract?, AuthorList?, Language+, DataBankList?, GrantList?,
/// PublicationTypeList, VernacularTitle?, ArticleDate*)` with `PubModel`.
#[allow(clippy::too_many_lines)]
pub(super) fn article<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Article<'a>> {
    p.enter(&tag);
    let mut pub_model = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "PubModel" => pub_model = Some(p.enum_attr("PubModel", &value, PubModel::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let pub_model = p.required_attr("PubModel", pub_model)?;

    let (mut journal, mut article_title, mut pagination, mut r#abstract) = (None, None, None, None);
    let (mut author_list, mut data_bank_list, mut grant_list) = (None, None, None);
    let (mut publication_types, mut vernacular_title) = (None, None);
    let mut elocation_ids = Few::new();
    let mut languages = Few::new();
    let mut article_dates = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Journal" => set_once!(p, journal, "Journal", journal_elem(p, t)?),
            "ArticleTitle" => {
                set_once!(p, article_title, "ArticleTitle", article_title_elem(p, t)?);
            }
            "Pagination" => set_once!(p, pagination, "Pagination", pagination_elem(p, t)?),
            "ELocationID" => elocation_ids.push(elocation_id(p, t)?),
            "Abstract" => set_once!(p, r#abstract, "Abstract", abstract_elem(p, t)?),
            "AuthorList" => set_once!(p, author_list, "AuthorList", author_list_elem(p, t)?),
            "Language" => languages.push(p.leaf(&t)?),
            "DataBankList" => set_once!(
                p,
                data_bank_list,
                "DataBankList",
                data_bank_list_elem(p, t)?
            ),
            "GrantList" => set_once!(p, grant_list, "GrantList", grant_list_elem(p, t)?),
            "PublicationTypeList" => set_once!(
                p,
                publication_types,
                "PublicationTypeList",
                list(p, t, "PublicationType", keyed)?
            ),
            "VernacularTitle" => set_once!(p, vernacular_title, "VernacularTitle", p.mixed(&t)?),
            "ArticleDate" => article_dates.push(article_date(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    if pagination.is_none() && elocation_ids.is_empty() {
        return Err(p.content_model("Article needs Pagination or at least one ELocationID"));
    }
    let out = Article {
        pub_model,
        journal: p.required("Journal", journal)?,
        article_title: p.required("ArticleTitle", article_title)?,
        pagination,
        elocation_ids,
        r#abstract,
        author_list,
        languages: p.at_least_one("Language", languages)?,
        data_bank_list,
        grant_list,
        publication_types: p.required("PublicationTypeList", publication_types)?,
        vernacular_title,
        article_dates,
    };
    p.leave();
    Ok(out)
}

/// `ArticleTitle (%text; | mml:math)*` with `book`, `part`, `sec`.
fn article_title_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<ArticleTitle<'a>> {
    p.enter(&tag);
    let (mut book, mut part, mut sec) = (None, None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "book" => book = Some(value),
            "part" => part = Some(value),
            "sec" => sec = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = ArticleTitle {
        book,
        part,
        sec,
        content: p.markup(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `Journal (ISSN?, JournalIssue, Title?, ISOAbbreviation?)`.
fn journal_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Journal<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut issn, mut journal_issue, mut title, mut iso_abbreviation) = (None, None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "ISSN" => set_once!(p, issn, "ISSN", issn_elem(p, t)?),
            "JournalIssue" => {
                set_once!(p, journal_issue, "JournalIssue", journal_issue_elem(p, t)?);
            }
            "Title" => set_once!(p, title, "Title", p.leaf(&t)?),
            "ISOAbbreviation" => set_once!(p, iso_abbreviation, "ISOAbbreviation", p.leaf(&t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = Journal {
        issn,
        journal_issue: p.required("JournalIssue", journal_issue)?,
        title,
        iso_abbreviation,
    };
    p.leave();
    Ok(out)
}

/// `ISSN (#PCDATA)` with `IssnType` required.
fn issn_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Issn<'a>> {
    p.enter(&tag);
    let mut issn_type = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "IssnType" => issn_type = Some(p.enum_attr("IssnType", &value, IssnType::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = Issn {
        issn_type: p.required_attr("IssnType", issn_type)?,
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `JournalIssue (Volume?, Issue?, PubDate)` with `CitedMedium` required.
fn journal_issue_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<JournalIssue<'a>> {
    p.enter(&tag);
    let mut cited_medium = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "CitedMedium" => {
                cited_medium = Some(p.enum_attr("CitedMedium", &value, CitedMedium::parse)?);
            }
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let cited_medium = p.required_attr("CitedMedium", cited_medium)?;
    let (mut volume, mut issue, mut pub_date) = (None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Volume" => set_once!(p, volume, "Volume", p.leaf(&t)?),
            "Issue" => set_once!(p, issue, "Issue", p.leaf(&t)?),
            "PubDate" => set_once!(p, pub_date, "PubDate", pub_date_elem(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = JournalIssue {
        cited_medium,
        volume,
        issue,
        pub_date: p.required("PubDate", pub_date)?,
    };
    p.leave();
    Ok(out)
}

/// `PubDate ((Year, ((Month, Day?) | Season)?) | MedlineDate)`.
fn pub_date_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<PubDate<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut year, mut month, mut day, mut season, mut medline_date) =
        (None, None, None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Year" => set_once!(p, year, "Year", p.leaf(&t)?),
            "Month" => set_once!(p, month, "Month", p.leaf(&t)?),
            "Day" => set_once!(p, day, "Day", p.leaf(&t)?),
            "Season" => set_once!(p, season, "Season", p.leaf(&t)?),
            "MedlineDate" => set_once!(p, medline_date, "MedlineDate", p.leaf(&t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = match (medline_date, year) {
        (Some(medline), None) => {
            if month.is_some() || day.is_some() || season.is_some() {
                return Err(p.content_model("MedlineDate excludes Year, Month, Day and Season"));
            }
            PubDate::Medline(medline)
        }
        (Some(_), Some(_)) => return Err(p.content_model("PubDate has both MedlineDate and Year")),
        (None, None) => return Err(p.error(ErrorKind::MissingElement("Year"))),
        (None, Some(year)) => {
            if day.is_some() && month.is_none() {
                return Err(p.content_model("Day without Month"));
            }
            if season.is_some() && (month.is_some() || day.is_some()) {
                return Err(p.content_model("Season excludes Month and Day"));
            }
            PubDate::Date {
                year,
                month,
                day,
                season,
            }
        }
    };
    p.leave();
    Ok(out)
}

/// `Pagination ((StartPage, EndPage?, MedlinePgn?) | MedlinePgn)`.
fn pagination_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Pagination<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut start_page, mut end_page, mut medline_pgn) = (None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "StartPage" => set_once!(p, start_page, "StartPage", p.leaf(&t)?),
            "EndPage" => set_once!(p, end_page, "EndPage", p.leaf(&t)?),
            "MedlinePgn" => set_once!(p, medline_pgn, "MedlinePgn", p.leaf(&t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = match (start_page, end_page, medline_pgn) {
        (Some(start_page), end_page, medline_pgn) => Pagination::Pages {
            start_page,
            end_page,
            medline_pgn,
        },
        (None, None, Some(pgn)) => Pagination::MedlinePgn(pgn),
        (None, Some(_), _) => return Err(p.content_model("EndPage without StartPage")),
        (None, None, None) => return Err(p.error(ErrorKind::MissingElement("MedlinePgn"))),
    };
    p.leave();
    Ok(out)
}

/// `ELocationID (#PCDATA)` with `EIdType` required, `ValidYN "Y"`.
fn elocation_id<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<ELocationId<'a>> {
    p.enter(&tag);
    let (mut eid_type, mut valid) = (None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "EIdType" => eid_type = Some(p.enum_attr("EIdType", &value, EIdType::parse)?),
            "ValidYN" => valid = Some(p.enum_attr("ValidYN", &value, YesNo::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = ELocationId {
        eid_type: p.required_attr("EIdType", eid_type)?,
        valid: valid.unwrap_or(YesNo::Yes),
        value: p.text(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `Abstract (AbstractText+, CopyrightInformation?)`.
fn abstract_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Abstract<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (texts, copyright_information) = abstract_children(p, &tag)?;
    p.leave();
    Ok(Abstract {
        texts,
        copyright_information,
    })
}

/// The children shared by `Abstract` and `OtherAbstract`:
/// `AbstractText+, CopyrightInformation?`.
pub(super) fn abstract_children<'a>(
    p: &mut Parser<'a>,
    tag: &Tag<'a>,
) -> Result<(Few<AbstractText<'a>, 4>, Option<Text<'a>>)> {
    let mut texts = Few::new();
    let mut copyright_information = None;
    while let Child::Elem(t) = p.next_child(tag)? {
        match t.name() {
            "AbstractText" => texts.push(abstract_text(p, t)?),
            "CopyrightInformation" => {
                set_once!(
                    p,
                    copyright_information,
                    "CopyrightInformation",
                    p.leaf(&t)?
                );
            }
            _ => return Err(p.unexpected(&t)),
        }
    }
    Ok((
        p.at_least_one("AbstractText", texts)?,
        copyright_information,
    ))
}

/// `AbstractText (%text; | mml:math | DispFormula)*` with `Label`, `NlmCategory`.
fn abstract_text<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<AbstractText<'a>> {
    p.enter(&tag);
    let (mut label, mut nlm_category) = (None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "Label" => label = Some(value),
            "NlmCategory" => {
                nlm_category = Some(p.enum_attr("NlmCategory", &value, NlmCategory::parse)?);
            }
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = AbstractText {
        label,
        nlm_category,
        content: p.markup(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `AuthorList (Author+)` with `CompleteYN "Y"`, `Type`.
fn author_list_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<AuthorList<'a>> {
    p.enter(&tag);
    let (mut complete, mut r#type) = (None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "CompleteYN" => complete = Some(p.enum_attr("CompleteYN", &value, YesNo::parse)?),
            "Type" => r#type = Some(p.enum_attr("Type", &value, AuthorListType::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let mut authors = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Author" => authors.push(author(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = AuthorList {
        complete: complete.unwrap_or(YesNo::Yes),
        r#type,
        authors: p.at_least_one("Author", authors)?,
    };
    p.leave();
    Ok(out)
}

/// `Author (((LastName, ForeName?, Initials?, Suffix?) | CollectiveName),
/// Identifier*, AffiliationInfo*)` with `ValidYN "Y"`, `EqualContrib`.
fn author<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Author<'a>> {
    p.enter(&tag);
    let (mut valid, mut equal_contrib) = (None, None);
    p.each_attr(&tag, |name, value| {
        match name {
            "ValidYN" => valid = Some(p.enum_attr("ValidYN", &value, YesNo::parse)?),
            "EqualContrib" => {
                equal_contrib = Some(p.enum_attr("EqualContrib", &value, YesNo::parse)?);
            }
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let mut person = NameSlots::default();
    let mut collective = None;
    let mut identifiers = Few::new();
    let mut affiliations = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        let Some(t) = person.take(p, t)? else {
            continue;
        };
        match t.name() {
            "CollectiveName" => set_once!(p, collective, "CollectiveName", collective_name(p, t)?),
            "Identifier" => identifiers.push(identifier(p, t)?),
            "AffiliationInfo" => affiliations.push(affiliation_info(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let name = match collective {
        Some(collective) => {
            if !person.is_empty() {
                return Err(p.content_model(
                    "CollectiveName excludes LastName, ForeName, Initials, Suffix",
                ));
            }
            AuthorName::Collective(collective)
        }
        None => AuthorName::Person(person.finish(p)?),
    };
    let out = Author {
        valid: valid.unwrap_or(YesNo::Yes),
        equal_contrib,
        name,
        identifiers,
        affiliations,
    };
    p.leave();
    Ok(out)
}

/// `CollectiveName (%text;)*` with `Investigators IDREF`.
fn collective_name<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<CollectiveName<'a>> {
    p.enter(&tag);
    let mut investigators = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "Investigators" => investigators = Some(value),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let out = CollectiveName {
        investigators,
        content: p.markup(&tag)?,
    };
    p.leave();
    Ok(out)
}

/// `DataBankList (DataBank+)` with `CompleteYN "Y"`.
fn data_bank_list_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<DataBankList<'a>> {
    p.enter(&tag);
    let mut complete = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "CompleteYN" => complete = Some(p.enum_attr("CompleteYN", &value, YesNo::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let mut data_banks = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "DataBank" => data_banks.push(data_bank(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = DataBankList {
        complete: complete.unwrap_or(YesNo::Yes),
        data_banks: p.at_least_one("DataBank", data_banks)?,
    };
    p.leave();
    Ok(out)
}

/// `DataBank (DataBankName, AccessionNumberList?)`.
fn data_bank<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<DataBank<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut name, mut accession_numbers) = (None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "DataBankName" => set_once!(p, name, "DataBankName", p.leaf(&t)?),
            "AccessionNumberList" => set_once!(
                p,
                accession_numbers,
                "AccessionNumberList",
                list(p, t, "AccessionNumber", |p, t| p.leaf(&t))?
            ),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = DataBank {
        name: p.required("DataBankName", name)?,
        accession_numbers: accession_numbers.unwrap_or_default(),
    };
    p.leave();
    Ok(out)
}

/// `GrantList (Grant+)` with `CompleteYN "Y"`.
fn grant_list_elem<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<GrantList<'a>> {
    p.enter(&tag);
    let mut complete = None;
    p.each_attr(&tag, |name, value| {
        match name {
            "CompleteYN" => complete = Some(p.enum_attr("CompleteYN", &value, YesNo::parse)?),
            other => return Err(p.unexpected_attr(other)),
        }
        Ok(())
    })?;
    let mut grants = Few::new();
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "Grant" => grants.push(grant(p, t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = GrantList {
        complete: complete.unwrap_or(YesNo::Yes),
        grants: p.at_least_one("Grant", grants)?,
    };
    p.leave();
    Ok(out)
}

/// `Grant (GrantID?, Acronym?, Agency, Country?)`.
fn grant<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Grant<'a>> {
    p.enter(&tag);
    p.no_attributes(&tag)?;
    let (mut grant_id, mut acronym, mut agency, mut country) = (None, None, None, None);
    while let Child::Elem(t) = p.next_child(&tag)? {
        match t.name() {
            "GrantID" => set_once!(p, grant_id, "GrantID", p.leaf(&t)?),
            "Acronym" => set_once!(p, acronym, "Acronym", p.leaf(&t)?),
            "Agency" => set_once!(p, agency, "Agency", p.leaf(&t)?),
            "Country" => set_once!(p, country, "Country", p.leaf(&t)?),
            _ => return Err(p.unexpected(&t)),
        }
    }
    let out = Grant {
        grant_id,
        acronym,
        agency: p.required("Agency", agency)?,
        country,
    };
    p.leave();
    Ok(out)
}

/// `ArticleDate (Year, Month, Day)` with `DateType CDATA #FIXED "Electronic"`.
fn article_date<'a>(p: &mut Parser<'a>, tag: Tag<'a>) -> Result<Date<'a>> {
    p.enter(&tag);
    p.each_attr(&tag, |name, value| match name {
        "DateType" if &*value == "Electronic" => Ok(()),
        "DateType" => Err(p.error(ErrorKind::InvalidAttributeValue {
            name: "DateType",
            value: value.into_owned(),
        })),
        other => Err(p.unexpected_attr(other)),
    })?;
    let mut slots = DateSlots::default();
    while let Child::Elem(t) = p.next_child(&tag)? {
        if let Some(t) = slots.take(p, t)? {
            return Err(p.unexpected(&t));
        }
    }
    let date = slots.finish(p)?;
    p.leave();
    Ok(date)
}
