//! A citation looked up (docs/citation-lookup-plan.md, §3): what the
//! identifiers in it name, what its words turn up, each work checked
//! against it, and the answer made of the two — the works it may be, and
//! the near misses. Synchronous: [`crate::service`] runs it on a search
//! thread within the search's time limit, since half of it is a search.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Range;
use std::time::Instant;

use citation_match::check::{self, Agreement, Citation, Verdict, Work};
use citation_match::identifiers::{self, Found, Reading};
use merge::{Sources, WorkId};
use serve::{Resolved, Row};
use tonic::Status;

use crate::answers::Answers;
use crate::convert;
use crate::proto::v1 as pb;
use crate::querylog::Looked;
use crate::service::{Stopped, answered, status};
use crate::terms::Terms;

/// The works a citation's words are searched for: Crossref's number for
/// a plain-text citation (plan, D3).
pub(crate) const CANDIDATES: usize = 20;

/// One work the lookup reached, and how: by the names in the citation
/// that name it, by its place among the candidates its words turned up,
/// or both.
struct Reached {
    id: WorkId,
    named_by: Vec<String>,
    rank: Option<usize>,
}

/// Looks `text` up, its words read by the query parser as `asked`, the
/// search stopped at `until`: the answer, and what the log says of it.
///
/// Both paths run every time (D2). A work an identifier names is listed
/// whatever the check says of it, since the citation names it — and the
/// check is shown beside it, so a DOI that belongs to another paper is
/// plain to see. A work only the words turned up is listed when it
/// passes the check (D4), and is a near miss when it does not. When an
/// identifier names one work and the words pass another, the answer says
/// they differ.
///
/// # Errors
///
/// The engine, or its time limit, as [`Stopped::Search`]; a work with no
/// public id, a row that does not decode, or a name the finder read that
/// the names table refuses, as [`Stopped::Answer`].
pub(crate) fn lookup<A: Answers>(
    a: &A,
    terms: &Terms,
    text: &str,
    asked: &query_parser::Query,
    until: Instant,
) -> Result<(pb::LookupCitationResponse, Looked), Stopped> {
    let mut reached: Vec<Reached> = Vec::new();
    let (identifiers, only_names) = named(a, text, &mut reached).map_err(Stopped::Answer)?;

    // What its words turn up, when it has any: a text of names alone is
    // not searched, a bare token being a name and not a word.
    let hits = if only_names {
        Vec::new()
    } else {
        a.candidates(asked, CANDIDATES, until)
            .map_err(Stopped::Search)?
    };
    for (rank, hit) in hits.iter().enumerate() {
        match reached.iter_mut().find(|r| r.id == hit.id) {
            Some(r) => r.rank = r.rank.or(Some(rank)),
            None => reached.push(Reached {
                id: hit.id,
                named_by: Vec::new(),
                rank: Some(rank),
            }),
        }
    }

    // Each checked against it.
    let ids: Vec<WorkId> = reached.iter().map(|r| r.id).collect();
    let rows = a
        .rows(&ids)
        .map_err(|e| Stopped::Answer(Status::internal(e.to_string())))?;
    let citation = Citation::read(text);
    let mut checked: Vec<(Reached, Verdict)> = reached
        .into_iter()
        .map(|r| {
            let work = rows.get(&r.id).map(work_of).unwrap_or_default();
            let verdict = check::check(&citation, &work);
            (r, verdict)
        })
        .collect();
    checked.sort_by(order);
    let (listed, near): (Vec<_>, Vec<_>) = checked
        .into_iter()
        .partition(|(r, v)| !r.named_by.is_empty() || v.passes());

    let differ = listed.iter().any(|(r, _)| !r.named_by.is_empty())
        && listed
            .iter()
            .any(|(r, v)| r.named_by.is_empty() && v.passes());
    let could_not = if !listed.is_empty() {
        ""
    } else if identifiers.is_empty() && asked.words().is_empty() {
        "nothing-to-go-on"
    } else if near.is_empty() {
        "nothing-came-up"
    } else {
        "nothing-agreed"
    };
    let looked = Looked {
        identifiers: identifiers.len(),
        candidates: hits.len(),
        matches: listed.len(),
        near: near.len(),
        nearest: near.first().map(|(_, v)| (v.score, failed(v))),
    };
    let mut sources = Sources::NONE;
    for row in rows.values() {
        for s in convert::held_by(row).iter() {
            sources.insert(s);
        }
    }
    let entries = |list: Vec<(Reached, Verdict)>| {
        list.into_iter()
            .map(|(r, v)| entry(a, &rows, r, &v))
            .collect::<Result<Vec<_>, Status>>()
            .map_err(Stopped::Answer)
    };
    Ok((
        pb::LookupCitationResponse {
            answered: Some(answered(a, terms, sources)),
            identifiers,
            matches: entries(listed)?,
            could_not: could_not.to_owned(),
            near_misses: entries(near)?,
            differ,
            only_names,
        },
        looked,
    ))
}

/// What the citation names: every name it is read as ([`read_names`]),
/// with what it names, each work it names added to `reached` with the
/// names that led there — first come, first listed, one entry a work; and
/// whether the text is those names and nothing more — identifiers, with
/// their labels and links, and not a word besides.
fn named<A: Answers>(
    a: &A,
    text: &str,
    reached: &mut Vec<Reached>,
) -> Result<(Vec<pb::CitedName>, bool), Status> {
    let reads = read_names(a, text)?;
    let written: Vec<Range<usize>> = reads.iter().map(|r| r.written.clone()).collect();
    let only_names = !reads.is_empty()
        && !identifiers::without(text, &written)
            .chars()
            .any(citation_match::text::in_word);
    let mut identifiers = Vec::new();
    for read in reads {
        let (cited, ids) = cited(a, text, read)?;
        for id in ids {
            match reached.iter_mut().find(|r| r.id == id) {
                Some(r) if !r.named_by.contains(&cited.name) => {
                    r.named_by.push(cited.name.clone());
                }
                Some(_) => {}
                None => reached.push(Reached {
                    id,
                    named_by: vec![cited.name.clone()],
                    rank: None,
                }),
            }
        }
        identifiers.push(cited);
    }
    Ok((identifiers, only_names))
}

/// One name a text was read as, where, and what it names.
pub(crate) struct Read {
    name: String,
    span: Range<usize>,
    /// Where it stands with its label or link.
    written: Range<usize>,
    resolved: Option<Resolved>,
}

impl Read {
    /// Whether the name names anything.
    pub(crate) const fn names_something(&self) -> bool {
        self.resolved.is_some()
    }
}

/// Every name `text` is read as, and what each names — for a citation,
/// and for Find by ID when what was typed is no name as it stands:
/// - each identifier the finder finds in it — a DOI in any of its
///   spellings, `PMID: 123`, a PubMed or OpenAlex link — as the first of
///   its readings the names table holds, or its last when none is;
/// - when the finder finds none and the whole text is one token: the
///   token as a name, as written (`reg1016:S…`, `ol:…`), and bare in every
///   namespace whose parser takes it ([`identifiers::bare`]), keeping
///   those that name something (the user, 2026-09-28: no namespace
///   required). Never a bare number inside a longer citation: its volume
///   of 193 would be PMID 193, another paper, and a work a name leads to
///   is always listed.
///
/// # Errors
///
/// A name the finder read that the names table refuses, and the table's
/// own failures.
pub(crate) fn read_names<A: Answers>(a: &A, text: &str) -> Result<Vec<Read>, Status> {
    let mut out = Vec::new();
    for found in identifiers::find(text) {
        let (reading, resolved) = first_held(a, &found)?;
        out.push(Read {
            name: reading.name.clone(),
            span: reading.span.clone(),
            written: found.written(text),
            resolved,
        });
    }
    let token = text.trim();
    if !out.is_empty() || token.is_empty() || token.contains(char::is_whitespace) {
        return Ok(out);
    }
    let start = text.len() - text.trim_start().len();
    let as_written = std::iter::once(token.to_owned()).filter(|t| t.contains(':'));
    for name in as_written.chain(identifiers::bare(token, a.publisher_namespaces())) {
        match a.resolve(&name) {
            Ok(Some(resolved)) => out.push(Read {
                name,
                span: start..start + token.len(),
                written: start..start + token.len(),
                resolved: Some(resolved),
            }),
            // Offered on the chance: a namespace that does not take the
            // token, or one that takes it and holds nothing by it.
            Ok(None) | Err(serve::Error::NotAName(_)) => {}
            Err(e) => return Err(status(&e)),
        }
    }
    Ok(out)
}

/// A name as the protocol says it, and the works it names.
pub(crate) fn cited<A: Answers>(
    a: &A,
    text: &str,
    read: Read,
) -> Result<(pb::CitedName, Vec<WorkId>), Status> {
    let (outcome, ids) = match read.resolved {
        Some(Resolved::One(id)) => ("one", vec![id]),
        Some(Resolved::Shared(ids)) => ("shared", ids),
        Some(Resolved::Split { into, .. }) => ("split", into),
        None => ("none", Vec::new()),
    };
    let cited = pb::CitedName {
        written: text[read.span.clone()].to_owned(),
        start: u32::try_from(read.span.start).unwrap_or(u32::MAX),
        end: u32::try_from(read.span.end).unwrap_or(u32::MAX),
        name: read.name,
        resolved: outcome.to_owned(),
        public_ids: ids
            .iter()
            .map(|id| convert::public(a, *id))
            .collect::<Result<_, _>>()?,
    };
    Ok((cited, ids))
}

/// The fields a verdict found not in the citation, by name.
fn failed(verdict: &Verdict) -> Vec<&'static str> {
    verdict
        .fields()
        .into_iter()
        .filter(|(_, a)| *a == Agreement::NotInCitation)
        .map(|(name, _)| name)
        .collect()
}

/// Best first: a work that passes the check before one that does not; a
/// higher score before a lower; a work the citation names before one only
/// its words turned up; and then the search's own order.
fn order((ra, va): &(Reached, Verdict), (rb, vb): &(Reached, Verdict)) -> Ordering {
    vb.passes()
        .cmp(&va.passes())
        .then(vb.score.total_cmp(&va.score))
        .then(ra.named_by.is_empty().cmp(&rb.named_by.is_empty()))
        .then(
            ra.rank
                .unwrap_or(usize::MAX)
                .cmp(&rb.rank.unwrap_or(usize::MAX)),
        )
}

/// What a served row says, as the check reads it: its title, its first
/// author as printed, and its year. Volume, issue and pages are not on the
/// row yet — they come with the citation fields plan (plan §7) — so until
/// then the check reads three of its five.
fn work_of(row: &Row) -> Work<'_> {
    let text = row.text.as_ref();
    Work {
        title: text.and_then(|t| t.title.as_deref()),
        first_author: text.and_then(|t| t.authors.first()).map(String::as_str),
        year: row.fields.published.as_ref().map(|p| p.value.year.get()),
        ..Work::default()
    }
}

/// One work of the answer, with its summary and its check.
fn entry<A: Answers>(
    a: &A,
    rows: &HashMap<WorkId, Row>,
    reached: Reached,
    verdict: &Verdict,
) -> Result<pb::CitationMatch, Status> {
    let public_id = convert::public(a, reached.id)?;
    Ok(pb::CitationMatch {
        summary: rows
            .get(&reached.id)
            .map(|row| convert::summary(public_id.clone(), row)),
        public_id,
        named_by: reached.named_by,
        by_text: reached.rank.is_some(),
        verdict: Some(pb::Verdict {
            title: word(verdict.title),
            first_author: word(verdict.first_author),
            year: word(verdict.year),
            volume: word(verdict.volume),
            first_page: word(verdict.first_page),
            unaccounted: verdict.unaccounted.clone(),
            score: verdict.score,
            passes: verdict.passes(),
            rule: verdict
                .rule
                .map(|r| r.name().to_owned())
                .unwrap_or_default(),
        }),
    })
}

/// An agreement in the protocol's words.
fn word(agreement: Agreement) -> String {
    match agreement {
        Agreement::Agrees => "agrees",
        Agreement::NotInCitation => "not-in-citation",
        Agreement::WorkDoesNotSay => "work-does-not-say",
    }
    .to_owned()
}

/// The first reading of `found` a name in the table answers to, and what
/// it answers; when none does, the last reading — the one with everything
/// a sentence may have added taken off — and nothing.
fn first_held<'f, A: Answers>(
    a: &A,
    found: &'f Found,
) -> Result<(&'f Reading, Option<Resolved>), Status> {
    for reading in &found.readings {
        match a.resolve(&reading.name) {
            Ok(Some(resolved)) => return Ok((reading, Some(resolved))),
            Ok(None) => {}
            // The finder offers only names the merge's parser takes, so a
            // refusal here is the two disagreeing: this server's fault,
            // not the citation's.
            Err(serve::Error::NotAName(why)) => {
                return Err(Status::internal(format!(
                    "the citation finder read {:?}, which the names table refuses: {why}",
                    reading.name
                )));
            }
            Err(e) => return Err(status(&e)),
        }
    }
    let last = found.readings.last().ok_or_else(|| {
        Status::internal("the citation finder found an identifier with no reading")
    })?;
    Ok((last, None))
}
