//! Native work → normalized records.
//!
//! Value-level strictness (plan §6) over the fields the evidence has
//! justified so far (plan §4a). Every DOI, ORCID, ISSN and ISBN in the
//! sample passed its checksum, every timestamp agreed with itself three
//! ways, and every `URL` was the DOI's — so those are strict types and
//! enforced invariants, and a record that breaks one stops the run with
//! its DOI, the field and the raw text. Reference DOIs are the exception,
//! as a [`Claim`]: one in five hundred carries a Unicode hyphen.
//!
//! Records are pushed to a [`Sink`] by reference as they are built.

use serde::{Deserialize, Serialize};
use std::fmt;

use ingest::Sink;
use model::{
    Claim, Date, DateError, DateTime, Doi, DoiError, Isbn, IsbnError, Issn, IssnError, Lang,
    LangError, Orcid, OrcidError, ParseError, PartialDate, Registrant,
};

use crate::Crossref;
use crate::date::{DateParts, PartialDate as PartialDateParts};
use crate::record::{Contributor, Serial, Work as Native};
use crate::vocab::{
    AssertedBy, ContentVersion, IdKind, RelationAssertedBy, RelationIdKind, Sequence, SerialKind,
    UpdateType, WorkType,
};

/// A DOI as a source asserted it.
pub type DoiClaim<'n> = Claim<'n, Doi<'n>, DoiError>;

/// One Crossref work, normalized: the fields with evidence behind them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Work<'n> {
    /// The work's DOI, lowercased.
    #[serde(borrow)]
    pub doi: Doi<'n>,
    /// `prefix`: the DOI's current owner, which differs from the DOI's own
    /// registrant on 7 % of works.
    pub owner: Registrant,
    /// `member`.
    pub member: u32,
    /// `type`.
    pub kind: WorkType,
    /// DOI registration.
    pub created: DateTime,
    /// Latest metadata deposit.
    pub deposited: DateTime,
    /// Latest indexing.
    pub indexed: DateTime,
    /// `issued`, `None` for `[[null]]`. `published` is checked equal to it
    /// and not kept.
    pub issued: Option<PartialDate>,
    /// `published-print`.
    pub published_print: Option<PartialDate>,
    /// `published-online`.
    pub published_online: Option<PartialDate>,
    /// `title[0]`, raw markup included.
    #[serde(borrow)]
    pub title: Option<&'n str>,
    /// `title[1..]`: translations, in the sample.
    #[serde(borrow)]
    pub alternate_titles: Vec<&'n str>,
    /// `subtitle[0]`; empty strings occur.
    #[serde(borrow)]
    pub subtitle: Option<&'n str>,
    /// A JATS (usually) XML fragment.
    pub r#abstract: Option<&'n str>,
    /// `container-title[0]`.
    #[serde(borrow)]
    pub container_title: Option<&'n str>,
    /// From `issn-type`, checked against `ISSN`.
    pub issns: Vec<TypedIssn>,
    /// From `isbn-type`, checked against `ISBN`.
    pub isbns: Vec<TypedIsbn>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issue: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub page: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub article_number: Option<&'n str>,
    #[allow(missing_docs)]
    pub publisher: &'n str,
    /// `language`, ISO 639-1.
    pub language: Option<Lang>,
    /// `author[]`.
    #[serde(borrow)]
    pub authors: Vec<Author<'n>>,
    /// `references-count`, checked against the reference array.
    pub references_count: Option<u32>,
    /// `is-referenced-by-count`.
    pub cited_by_count: Option<u32>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub licenses: Vec<License<'n>>,
    /// Updates this work applies to others: it is the notice.
    #[serde(borrow)]
    pub updates_to: Vec<Update<'n>>,
    /// Updates applied to this work: it is the retracted, corrected, … one.
    #[serde(borrow)]
    pub updated_by: Vec<Update<'n>>,
    /// `relation`, flattened.
    #[serde(borrow)]
    pub relations: Vec<Relation<'n>>,
    /// `alternative-id`, often the DOI itself or a PII.
    #[serde(borrow)]
    pub alternative_ids: Vec<&'n str>,
}

/// An author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Author<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: AuthorName<'n>,
    #[allow(missing_docs)]
    pub orcid: Option<Orcid>,
    /// `authenticated-orcid`.
    pub authenticated_orcid: Option<bool>,
    #[allow(missing_docs)]
    pub sequence: Sequence,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub affiliations: Vec<Affiliation<'n>>,
}

/// A person (`family`, maybe `given`) or an organisation (`name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorName<'n> {
    #[allow(missing_docs)]
    Person {
        #[allow(missing_docs)]
        #[serde(borrow)]
        given: Option<&'n str>,
        #[allow(missing_docs)]
        family: &'n str,
    },
    #[allow(missing_docs)]
    Org(&'n str),
}

/// An affiliation: a name, a ROR id, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Affiliation<'n> {
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub name: Option<&'n str>,
    /// `https://ror.org/…`, unvalidated here.
    #[serde(borrow)]
    pub ror: Option<&'n str>,
}

/// An ISSN with its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedIssn {
    #[allow(missing_docs)]
    pub kind: SerialKind,
    #[allow(missing_docs)]
    pub issn: Issn,
}

/// An ISBN with its kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedIsbn {
    #[allow(missing_docs)]
    pub kind: SerialKind,
    #[allow(missing_docs)]
    pub isbn: Isbn,
}

/// A license.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct License<'n> {
    #[allow(missing_docs)]
    pub url: &'n str,
    #[allow(missing_docs)]
    pub version: ContentVersion,
    #[allow(missing_docs)]
    pub start: DateTime,
    /// `delay-in-days`.
    pub delay_days: i64,
}

/// A Crossmark update, either direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Update<'n> {
    /// The other work.
    #[serde(borrow)]
    pub doi: Doi<'n>,
    #[allow(missing_docs)]
    pub kind: UpdateType,
    #[allow(missing_docs)]
    pub at: DateTime,
}

/// One relation to another object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation<'n> {
    /// `has-preprint`, `is-preprint-of`, …
    pub kind: &'n str,
    #[allow(missing_docs)]
    pub id: &'n str,
    #[allow(missing_docs)]
    pub id_kind: RelationIdKind,
    #[allow(missing_docs)]
    pub asserted_by: RelationAssertedBy,
}

/// One entry of a work's reference list: an edge whose target is a DOI
/// (76 % of the sample), matching signals, or both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference<'n> {
    #[allow(missing_docs)]
    pub key: &'n str,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub doi: Option<DoiClaim<'n>>,
    /// Present exactly when `doi` is.
    pub asserted_by: Option<AssertedBy>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub unstructured: Option<&'n str>,
    /// Raw: `2019`, but also `nd` and `0`.
    #[serde(borrow)]
    pub year: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub article_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub journal_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub series_title: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub author: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub volume: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub issue: Option<&'n str>,
    #[allow(missing_docs)]
    #[serde(borrow)]
    pub first_page: Option<&'n str>,
}

/// A value that did not fit the model.
#[derive(Debug)]
pub struct Error {
    /// The work, once its DOI was readable.
    pub doi: Option<String>,
    /// The field.
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
    Doi(DoiError),
    Orcid(OrcidError),
    Issn(IssnError),
    Isbn(IsbnError),
    Date(DateError),
    Lang(LangError),
    Id(ParseError),
    /// Two fields that must agree do not.
    Inconsistent(&'static str),
    /// Present in the data, not modelled here yet.
    Unmodelled(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.doi {
            Some(doi) => write!(f, "DOI {doi}: ")?,
            None => f.write_str("DOI unknown: ")?,
        }
        write!(f, "{} = {:?}: ", self.field, self.raw)?;
        match &self.kind {
            ErrorKind::Doi(e) => write!(f, "{e}"),
            ErrorKind::Orcid(e) => write!(f, "{e}"),
            ErrorKind::Issn(e) => write!(f, "{e}"),
            ErrorKind::Isbn(e) => write!(f, "{e}"),
            ErrorKind::Date(e) => write!(f, "{e}"),
            ErrorKind::Lang(e) => write!(f, "{e}"),
            ErrorKind::Id(e) => write!(f, "{e}"),
            ErrorKind::Inconsistent(what) => write!(f, "inconsistent: {what}"),
            ErrorKind::Unmodelled(what) => write!(f, "not modelled yet: {what}"),
        }
    }
}

impl std::error::Error for Error {}

struct Ctx {
    doi: Option<String>,
}

impl Ctx {
    fn err(&self, field: &'static str, raw: &str, kind: ErrorKind) -> Error {
        Error {
            doi: self.doi.clone(),
            field,
            raw: raw.to_owned(),
            kind,
        }
    }

    /// A full timestamp, its three representations checked against each
    /// other.
    fn stamp(
        &self,
        field: &'static str,
        parts: DateParts,
        date_time: &str,
        timestamp: i64,
    ) -> Result<DateTime, Error> {
        let at = DateTime::parse_utc(date_time)
            .map_err(|e| self.err(field, date_time, ErrorKind::Date(e)))?;
        let (Some(y), Some(m), Some(d)) = (parts.year, parts.month, parts.day) else {
            return Err(self.err(
                field,
                date_time,
                ErrorKind::Inconsistent("date-parts is not a full date"),
            ));
        };
        let date = Date::from_numbers(y, m, d)
            .map_err(|e| self.err(field, &format!("{y}-{m}-{d}"), ErrorKind::Date(e)))?;
        if date != at.date() {
            return Err(self.err(
                field,
                date_time,
                ErrorKind::Inconsistent("date-parts disagrees with date-time"),
            ));
        }
        if timestamp.div_euclid(1000) != at.epoch_seconds() {
            return Err(self.err(
                field,
                &timestamp.to_string(),
                ErrorKind::Inconsistent("timestamp disagrees with date-time"),
            ));
        }
        Ok(at)
    }

    /// A partial date; `None` when absent or `[[null]]`.
    fn partial(
        &self,
        field: &'static str,
        p: Option<&PartialDateParts>,
    ) -> Result<Option<PartialDate>, Error> {
        let Some(p) = p else { return Ok(None) };
        let d = p.date_parts;
        let Some(year) = d.year else { return Ok(None) };
        PartialDate::from_numbers(year, d.month, d.day)
            .map(Some)
            .map_err(|e| self.err(field, &format!("{d:?}"), ErrorKind::Date(e)))
    }

    /// `issn-type` / `isbn-type` entries, checked to be the same set as the
    /// plain `ISSN` / `ISBN` list.
    fn serials<T>(
        &self,
        field: &'static str,
        typed: Option<&[Serial<'_>]>,
        plain: Option<&[crate::record::Text<'_>]>,
        parse: impl Fn(&str) -> Result<T, ErrorKind>,
    ) -> Result<Vec<(SerialKind, T)>, Error> {
        let typed = typed.unwrap_or_default();
        let plain = plain.unwrap_or_default();
        if typed.len() != plain.len() || !typed.iter().all(|t| plain.contains(&t.value)) {
            return Err(self.err(
                field,
                "",
                ErrorKind::Inconsistent("typed list differs from plain list"),
            ));
        }
        typed
            .iter()
            .map(|t| {
                parse(&t.value)
                    .map(|v| (t.r#type, v))
                    .map_err(|e| self.err(field, &t.value, e))
            })
            .collect()
    }

    fn author<'n>(&self, c: &'n Contributor<'_>) -> Result<Author<'n>, Error> {
        let name = match (c.family.as_deref(), c.name.as_deref()) {
            (Some(family), None) => AuthorName::Person {
                given: c.given.as_deref(),
                family,
            },
            (None, Some(name)) => {
                if c.given.is_some() {
                    return Err(self.err(
                        "author",
                        name,
                        ErrorKind::Inconsistent("given with name"),
                    ));
                }
                AuthorName::Org(name)
            }
            (Some(family), Some(_)) => {
                return Err(self.err(
                    "author",
                    family,
                    ErrorKind::Inconsistent("both family and name"),
                ));
            }
            (None, None) => {
                return Err(self.err(
                    "author",
                    "",
                    ErrorKind::Inconsistent("neither family nor name"),
                ));
            }
        };
        let orcid = c
            .orcid
            .as_deref()
            .map(|o| {
                Orcid::from_https_url(o)
                    .map_err(|e| self.err("author.ORCID", o, ErrorKind::Orcid(e)))
            })
            .transpose()?;
        let mut affiliations = Vec::with_capacity(c.affiliation.len());
        for a in &c.affiliation {
            let mut ror = None;
            for id in a.id.as_deref().unwrap_or_default() {
                if id.id_type != IdKind::Ror {
                    return Err(self.err(
                        "affiliation.id",
                        &id.id,
                        ErrorKind::Unmodelled("affiliation id that is not a ROR"),
                    ));
                }
                if ror.replace(&*id.id).is_some() {
                    return Err(self.err(
                        "affiliation.id",
                        &id.id,
                        ErrorKind::Unmodelled("more than one affiliation id"),
                    ));
                }
            }
            affiliations.push(Affiliation {
                name: a.name.as_deref(),
                ror,
            });
        }
        Ok(Author {
            name,
            orcid,
            authenticated_orcid: c.authenticated_orcid,
            sequence: c.sequence,
            affiliations,
        })
    }

    fn update<'n>(
        &self,
        field: &'static str,
        u: &'n crate::record::Update<'_>,
    ) -> Result<Update<'n>, Error> {
        Ok(Update {
            doi: Doi::parse(&u.doi).map_err(|e| self.err(field, &u.doi, ErrorKind::Doi(e)))?,
            kind: u.r#type,
            at: self.stamp(
                field,
                u.updated.date_parts,
                &u.updated.date_time,
                u.updated.timestamp,
            )?,
        })
    }
}

fn first<'n>(list: Option<&'n Vec<crate::record::Text<'_>>>) -> Option<&'n str> {
    list.and_then(|l| l.first()).map(|t| &**t)
}

/// Normalizes one work, emitting it and then its references.
///
/// # Errors
///
/// The first value that does not fit the model; see [`ErrorKind`]. The
/// emitter may already have received the work when a reference fails.
#[allow(clippy::too_many_lines)] // one block per field group; the field set is the point
pub fn normalize<'n, K: Sink<'n, Crossref>>(w: &'n Native<'_>, emit: &mut K) -> Result<(), Error> {
    let mut ctx = Ctx { doi: None };
    let doi = Doi::parse(&w.doi).map_err(|e| ctx.err("DOI", &w.doi, ErrorKind::Doi(e)))?;
    ctx.doi = Some(doi.as_str().to_owned());
    if w.url.strip_prefix(Doi::URL_PREFIX) != Some(&*w.doi) {
        return Err(ctx.err(
            "URL",
            &w.url,
            ErrorKind::Inconsistent("URL is not https://doi.org/ + DOI"),
        ));
    }
    let owner = Registrant::parse(&w.prefix)
        .map_err(|e| ctx.err("prefix", &w.prefix, ErrorKind::Doi(e)))?;
    let member = model::id::strict_u32(&w.member)
        .map_err(|e| ctx.err("member", &w.member, ErrorKind::Id(e)))?;

    let created = ctx.stamp(
        "created",
        w.created.date_parts,
        &w.created.date_time,
        w.created.timestamp,
    )?;
    let deposited = ctx.stamp(
        "deposited",
        w.deposited.date_parts,
        &w.deposited.date_time,
        w.deposited.timestamp,
    )?;
    let indexed = ctx.stamp(
        "indexed",
        w.indexed.date_parts,
        &w.indexed.date_time,
        w.indexed.timestamp,
    )?;
    let issued = ctx.partial("issued", Some(&w.issued))?;
    if let Some(published) = &w.published
        && published.date_parts != w.issued.date_parts
    {
        return Err(ctx.err(
            "published",
            &format!("{:?}", published.date_parts),
            ErrorKind::Inconsistent("published differs from issued"),
        ));
    }
    let published_print = ctx.partial("published-print", w.published_print.as_ref())?;
    let published_online = ctx.partial("published-online", w.published_online.as_ref())?;

    if let (Some(a), Some(b)) = (w.reference_count, w.references_count)
        && a != b
    {
        return Err(ctx.err(
            "reference-count",
            &a.to_string(),
            ErrorKind::Inconsistent("reference-count differs from references-count"),
        ));
    }
    let references = w.reference.as_deref().unwrap_or_default();
    if let Some(n) = w.references_count
        && usize::try_from(n).ok() != Some(references.len())
        && w.reference.is_some()
    {
        return Err(ctx.err(
            "references-count",
            &n.to_string(),
            ErrorKind::Inconsistent("references-count differs from the reference array"),
        ));
    }

    let journal_serials = ctx
        .serials(
            "issn-type",
            w.issn_type.as_deref(),
            w.issn.as_deref(),
            |s| Issn::parse(s).map_err(ErrorKind::Issn),
        )?
        .into_iter()
        .map(|(kind, issn)| TypedIssn { kind, issn })
        .collect();
    let book_serials = ctx
        .serials(
            "isbn-type",
            w.isbn_type.as_deref(),
            w.isbn.as_deref(),
            |s| Isbn::parse(s).map_err(ErrorKind::Isbn),
        )?
        .into_iter()
        .map(|(kind, isbn)| TypedIsbn { kind, isbn })
        .collect();
    let language = w
        .language
        .as_deref()
        .map(|l| Lang::from_alpha_2(l).map_err(|e| ctx.err("language", l, ErrorKind::Lang(e))))
        .transpose()?;
    let authors = w
        .author
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|c| ctx.author(c))
        .collect::<Result<Vec<_>, _>>()?;
    let licenses = w
        .license
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|l| {
            Ok(License {
                url: &l.url,
                version: l.content_version,
                start: ctx.stamp(
                    "license.start",
                    l.start.date_parts,
                    &l.start.date_time,
                    l.start.timestamp,
                )?,
                delay_days: l.delay_in_days,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let updates_to = w
        .update_to
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|u| ctx.update("update-to", u))
        .collect::<Result<Vec<_>, _>>()?;
    let updated_by = w
        .updated_by
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|u| ctx.update("updated-by", u))
        .collect::<Result<Vec<_>, _>>()?;
    let relations = w
        .relation
        .iter()
        .flat_map(|map| map.iter())
        .flat_map(|(kind, targets)| {
            targets.iter().map(move |t| Relation {
                kind,
                id: &t.id,
                id_kind: t.id_type,
                asserted_by: t.asserted_by,
            })
        })
        .collect();

    emit.work(&Work {
        owner,
        member,
        kind: w.r#type,
        created,
        deposited,
        indexed,
        issued,
        published_print,
        published_online,
        title: first(w.title.as_ref()),
        alternate_titles: w
            .title
            .iter()
            .flat_map(|t| t.iter().skip(1))
            .map(|t| &**t)
            .collect(),
        subtitle: first(w.subtitle.as_ref()),
        r#abstract: w.r#abstract.as_deref(),
        container_title: first(w.container_title.as_ref()),
        issns: journal_serials,
        isbns: book_serials,
        volume: w.volume.as_deref(),
        issue: w.issue.as_deref(),
        page: w.page.as_deref(),
        article_number: w.article_number.as_deref(),
        publisher: &w.publisher,
        language,
        authors,
        references_count: w.references_count,
        cited_by_count: w.is_referenced_by_count,
        licenses,
        updates_to,
        updated_by,
        relations,
        alternative_ids: w
            .alternative_id
            .iter()
            .flat_map(|a| a.iter())
            .map(|t| &**t)
            .collect(),
        doi: doi.clone(),
    });

    for r in references {
        if r.doi.is_some() != r.doi_asserted_by.is_some() {
            return Err(ctx.err(
                "reference",
                &r.key,
                ErrorKind::Inconsistent("DOI and doi-asserted-by must appear together"),
            ));
        }
        emit.reference(
            &doi,
            &Reference {
                key: &r.key,
                doi: r
                    .doi
                    .as_deref()
                    .map(|d| Claim::from_parse(d, Doi::parse(d))),
                asserted_by: r.doi_asserted_by,
                unstructured: r.unstructured.as_deref(),
                year: r.year.as_deref(),
                article_title: r.article_title.as_deref(),
                journal_title: r.journal_title.as_deref(),
                volume_title: r.volume_title.as_deref(),
                series_title: r.series_title.as_deref(),
                author: r.author.as_deref(),
                volume: r.volume.as_deref(),
                issue: r.issue.as_deref(),
                first_page: r.first_page.as_deref(),
            },
        );
    }
    Ok(())
}
