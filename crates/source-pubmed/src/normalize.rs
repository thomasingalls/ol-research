//! Native record → normalized records.
//!
//! This is where value-level strictness lives (plan §6): identifiers,
//! dates and vocabularies become `model` types, and a value that does not
//! fit stops the run with the PMID, the field and the raw text. The field
//! set is what the evidence has justified so far (plan §4a) and grows one
//! field at a time; anything not here is still in the raw layer.
//!
//! Records are pushed to an [`Emit`] by reference as they are built — no
//! per-record collections beyond the record itself (plan §5).

use std::fmt;

use model::{
    Claim, Date, DateError, Day, Doi, DoiError, Lang, LangError, MedlineDateError, MeshUi,
    MeshUiError, Month, ParseError, PartialDate, PmcId, Pmid, Season, Year,
};

use crate::attr::{EIdType, IdType, RefType, Status, YesNo};
use crate::record::{
    ArticleId, Date as DateText, MedlineCitation, PubDate, PubmedArticle,
    ReferenceList as ReferenceListRecord,
};

/// A DOI as a source asserted it.
pub type DoiClaim<'n> = Claim<'n, Doi<'n>, DoiError>;

/// One PubMed record, normalized: the fields with evidence behind them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation<'n> {
    /// The record's identifier.
    pub pmid: Pmid,
    /// `PMID@Version`.
    pub version: u32,
    /// Indexing status: `Publisher`, `In-Process`, `MEDLINE`, …
    pub status: Status,
    /// `ArticleId IdType="doi"` — the record's own DOI as it asserts it.
    pub doi: Option<DoiClaim<'n>>,
    /// `ELocationID EIdType="doi"` — the same identifier from the article
    /// block. Agrees with `doi` whenever both are valid; when they differ,
    /// one is malformed (nine e-location strings and one literal `DOI` in
    /// the first file).
    pub elocation_doi: Option<DoiClaim<'n>>,
    /// `ArticleId IdType="pii"`, an opaque publisher item identifier.
    pub pii: Option<&'n str>,
    /// `ArticleId IdType="pmc"`.
    pub pmc: Option<PmcId>,
    /// `JournalIssue/PubDate`, at the precision the record gave.
    pub published: PartialDate,
    /// `ArticleDate`, the electronic publication date. At most one is
    /// modelled; a second is an error until one is observed.
    pub electronic: Option<Date>,
    /// `DateRevised`.
    pub revised: Option<Date>,
    /// `DateCompleted`.
    pub completed: Option<Date>,
    /// `Language+`, ISO 639-2 bibliographic codes.
    pub languages: Vec<Lang>,
    /// `PublicationType+`, keyed by MeSH UI.
    pub publication_types: Vec<PublicationType<'n>>,
    /// `CommentsCorrections*`: errata, retractions, comments, by kind.
    pub relations: Vec<Relation>,
}

/// A `PublicationType`: MeSH descriptor UI plus the term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicationType<'n> {
    /// The descriptor UI, e.g. `D016428`.
    pub ui: MeshUi<'n>,
    /// The term, e.g. `Journal Article`.
    pub name: &'n str,
}

/// A `CommentsCorrections` entry: how this record relates to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relation {
    /// `ErratumFor`, `RetractionOf`, `CommentOn`, …
    pub kind: RefType,
    /// The other record, when PubMed identified it.
    pub pmid: Option<Pmid>,
}

/// One entry of a record's reference list: an edge whose target is
/// identified by whichever of three namespaces the source supplied — in
/// the first file 74 % supplied none, and the rest any combination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference<'n> {
    /// The citing record.
    pub from: Pmid,
    /// `ArticleId IdType="pubmed"`.
    pub pmid: Option<Pmid>,
    /// `ArticleId IdType="doi"`.
    pub doi: Option<DoiClaim<'n>>,
    /// `ArticleId IdType="pmcid"`.
    pub pmc: Option<PmcId>,
    /// The citation text, raw markup included.
    pub citation: &'n str,
}

/// Where normalized records go. Called once per record, by reference.
pub trait Emit<'n> {
    /// One citation.
    fn citation(&mut self, citation: &Citation<'n>);
    /// One reference of the most recently emitted citation.
    fn reference(&mut self, reference: &Reference<'n>);
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
pub enum ErrorKind {
    /// An identifier.
    Id(ParseError),
    /// A date part.
    Date(DateError),
    /// A `MedlineDate`.
    MedlineDate(MedlineDateError),
    /// A MeSH UI.
    MeshUi(MeshUiError),
    /// A language code.
    Lang(LangError),
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
        match &self.kind {
            ErrorKind::Id(e) => write!(f, "{e}"),
            ErrorKind::Date(e) => write!(f, "{e}"),
            ErrorKind::MedlineDate(e) => write!(f, "{e}"),
            ErrorKind::MeshUi(e) => write!(f, "{e}"),
            ErrorKind::Lang(e) => write!(f, "{e}"),
            ErrorKind::Inconsistent(what) => write!(f, "inconsistent: {what}"),
            ErrorKind::Unmodelled(what) => write!(f, "not modelled yet: {what}"),
        }
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
}

/// The record's own identifiers, from its `ArticleIdList` and `ELocationID`s.
#[derive(Default)]
struct Own<'n> {
    pubmed: Option<&'n str>,
    doi: Option<&'n str>,
    pii: Option<&'n str>,
    pmc: Option<&'n str>,
}

impl<'n> Own<'n> {
    fn take(&mut self, ctx: &Ctx, id: &'n ArticleId<'_>) -> Result<(), Error> {
        const FIELD: &str = "ArticleIdList/ArticleId";
        let slot = match id.id_type {
            IdType::Pubmed => &mut self.pubmed,
            IdType::Doi => &mut self.doi,
            IdType::Pii => &mut self.pii,
            IdType::Pmc => &mut self.pmc,
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

/// The identifier fields of a citation.
struct Identifiers<'n> {
    doi: Option<DoiClaim<'n>>,
    elocation_doi: Option<DoiClaim<'n>>,
    pii: Option<&'n str>,
    pmc: Option<PmcId>,
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

    let mut elocation_doi = None;
    for e in &article.medline_citation.article.elocation_ids {
        const FIELD: &str = "ELocationID[doi]";
        if e.eid_type != EIdType::Doi {
            continue;
        }
        if e.valid == YesNo::No {
            return Err(ctx.err(FIELD, &e.value, ErrorKind::Unmodelled("ValidYN=\"N\"")));
        }
        if elocation_doi.is_some() {
            return Err(ctx.err(
                FIELD,
                &e.value,
                ErrorKind::Inconsistent("more than one ELocationID doi"),
            ));
        }
        elocation_doi = Some(Claim::from_parse(&e.value, Doi::parse(&e.value)));
    }
    if let (Some(Claim::Valid(a)), Some(Claim::Valid(b))) = (&doi, &elocation_doi)
        && a != b
    {
        return Err(ctx.err(
            "ELocationID[doi]",
            b.as_str(),
            ErrorKind::Inconsistent("differs from the valid ArticleId doi"),
        ));
    }
    Ok(Identifiers {
        doi,
        elocation_doi,
        pii: own.pii,
        pmc,
    })
}

/// The date fields of a citation.
struct Dates {
    published: PartialDate,
    electronic: Option<Date>,
    revised: Option<Date>,
    completed: Option<Date>,
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

/// Normalizes one record, emitting its citation and then its references.
///
/// # Errors
///
/// The first value that does not fit the model; see [`ErrorKind`]. The
/// emitter may already have received the citation when a reference fails.
pub fn normalize<'n, E: Emit<'n>>(
    article: &'n PubmedArticle<'_>,
    emit: &mut E,
) -> Result<(), Error> {
    let c = &article.medline_citation;
    let mut ctx = Ctx { pmid: None };
    let pmid = Pmid::from_digits(&c.pmid.value)
        .map_err(|e| ctx.err("PMID", &c.pmid.value, ErrorKind::Id(e)))?;
    ctx.pmid = Some(pmid);
    let version = model::id::strict_u32(&c.pmid.version)
        .map_err(|e| ctx.err("PMID@Version", &c.pmid.version, ErrorKind::Id(e)))?;

    let ids = identifiers(&ctx, article, pmid)?;
    let dates = dates(&ctx, c)?;

    let mut languages = Vec::with_capacity(c.article.languages.len());
    for l in &c.article.languages {
        languages.push(
            Lang::from_bibliographic(l).map_err(|e| ctx.err("Language", l, ErrorKind::Lang(e)))?,
        );
    }

    let mut publication_types = Vec::with_capacity(c.article.publication_types.len());
    for pt in &c.article.publication_types {
        let ui = MeshUi::parse(&pt.ui)
            .map_err(|e| ctx.err("PublicationType@UI", &pt.ui, ErrorKind::MeshUi(e)))?;
        publication_types.push(PublicationType {
            ui,
            name: &pt.value,
        });
    }

    let mut relations = Vec::with_capacity(c.comments_corrections.len());
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

    emit.citation(&Citation {
        pmid,
        version,
        status: c.status,
        doi: ids.doi,
        elocation_doi: ids.elocation_doi,
        pii: ids.pii,
        pmc: ids.pmc,
        published: dates.published,
        electronic: dates.electronic,
        revised: dates.revised,
        completed: dates.completed,
        languages,
        publication_types,
        relations,
    });

    if let Some(data) = &article.pubmed_data {
        references(&ctx, pmid, &data.reference_lists, emit)?;
    }
    Ok(())
}

fn references<'n, E: Emit<'n>>(
    ctx: &Ctx,
    from: Pmid,
    lists: &'n [ReferenceListRecord<'_>],
    emit: &mut E,
) -> Result<(), Error> {
    const FIELD: &str = "Reference/ArticleIdList/ArticleId";
    for list in lists {
        for r in &list.references {
            let (mut pmid, mut doi, mut pmc) = (None, None, None);
            for id in &r.article_ids {
                let taken = match id.id_type {
                    IdType::Pubmed => pmid
                        .replace(
                            Pmid::from_digits(&id.value)
                                .map_err(|e| ctx.err(FIELD, &id.value, ErrorKind::Id(e)))?,
                        )
                        .is_some(),
                    IdType::Doi => doi
                        .replace(Claim::from_parse(&id.value, Doi::parse(&id.value)))
                        .is_some(),
                    IdType::Pmcid => pmc
                        .replace(
                            PmcId::from_digits(&id.value)
                                .map_err(|e| ctx.err(FIELD, &id.value, ErrorKind::Id(e)))?,
                        )
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
            emit.reference(&Reference {
                from,
                pmid,
                doi,
                pmc,
                citation: r.citation.as_str(),
            });
        }
        references(ctx, from, &list.sub_lists, emit)?;
    }
    Ok(())
}
