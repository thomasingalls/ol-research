//! The boundary: the core's types to the wire's, `WorkId`s becoming
//! public ids on the way.

use std::collections::HashMap;

use merge::identity::Standing;
use merge::view::OwnedVenue;
use merge::{SourceId, Sources, WorkId};
use model::date::Bound;
use serve::Row;
use serve::row::{Fields, Named};
use tonic::Status;

use crate::answers::Answers;
use crate::proto::v1 as pb;
use crate::terms::Terms;

/// A work's public id, or the status that says it has none.
///
/// # Errors
///
/// No public id minted for the work.
pub fn public<A: Answers>(a: &A, id: WorkId) -> Result<String, Status> {
    a.public_of(id).ok_or_else(|| {
        Status::failed_precondition(
            "a work with no public id: mint them with merge --public-key and publish an epoch",
        )
    })
}

fn name(n: &Named) -> pb::Name {
    let text = n.name.to_string();
    let (namespace, value) = text.split_once(':').unwrap_or(("", text.as_str()));
    pb::Name {
        namespace: namespace.to_owned(),
        value: value.to_owned(),
        standing: n.standing.to_string(),
        by: n.by.iter().map(|s| s.name().to_owned()).collect(),
        native: n.native,
    }
}

fn date(b: Bound) -> pb::Date {
    pb::Date {
        year: u32::from(b.year.get()),
        month: b.month.map_or(0, |m| u32::from(m as u8)),
        day: b.day.map_or(0, |d| u32::from(d.get())),
    }
}

fn venue_text(v: &OwnedVenue) -> String {
    let mut parts = Vec::new();
    if let Some(s) = v.openalex {
        parts.push(format!("openalex {s}"));
    }
    if !v.issns.is_empty() {
        parts.push(format!(
            "issn {}",
            v.issns
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if let Some(nlm) = &v.nlm {
        parts.push(format!("nlm {nlm}"));
    }
    parts.join("; ")
}

/// The placement's parts where another source differs (merge version 17):
/// the journal, each level, the pages, the issue's date.
fn placement_disputed(
    p: &merge::placement::OwnedPromoted,
    push: &mut impl FnMut(&str, SourceId, Vec<(SourceId, String)>),
) {
    if let Some(x) = &p.journal {
        push(
            "journal",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, venue_text(&o.value)))
                .collect(),
        );
    }
    for x in &p.levels {
        push(
            x.value.kind.as_str(),
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.label.as_deref().unwrap_or("").to_owned()))
                .collect(),
        );
    }
    if let Some(x) = &p.within {
        push(
            "pages",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.to_string()))
                .collect(),
        );
    }
    if let Some(x) = &p.date {
        push(
            "issue date",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.year.get().to_string()))
                .collect(),
        );
    }
}

fn disputed(f: &Fields) -> Vec<pb::Disputed> {
    let mut out = Vec::new();
    let mut push = |field: &str, from: SourceId, others: Vec<(SourceId, String)>| {
        if !others.is_empty() {
            out.push(pb::Disputed {
                field: field.to_owned(),
                promoted_from: from.name().to_owned(),
                others: others
                    .into_iter()
                    .map(|(s, v)| format!("{}: {v}", s.name()))
                    .collect(),
            });
        }
    };
    if let Some(x) = &f.title {
        push(
            "title",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, merge::view::strip_markup(&o.value)))
                .collect(),
        );
    }
    if let Some(x) = &f.authors {
        push(
            "authors",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.to_string()))
                .collect(),
        );
    }
    if let Some(p) = &f.placement {
        placement_disputed(p, &mut push);
    }
    if let Some(x) = &f.online {
        push(
            "online",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.year.get().to_string()))
                .collect(),
        );
    }
    if let Some(x) = &f.published {
        push(
            "published",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.year.get().to_string()))
                .collect(),
        );
    }
    if let Some(x) = &f.language {
        push(
            "language",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.to_string()))
                .collect(),
        );
    }
    if let Some(x) = &f.form {
        push(
            "form",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.to_string()))
                .collect(),
        );
    }
    if let Some(x) = &f.genre {
        push(
            "genre",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, o.value.to_string()))
                .collect(),
        );
    }
    if let Some(x) = &f.access {
        push(
            "access",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, if o.value { "open" } else { "closed" }.to_owned()))
                .collect(),
        );
    }
    out
}

/// The source a row's abstract came from: the first in the merge plan's
/// precedence that has one.
#[must_use]
pub fn abstract_source(f: &Fields) -> Option<SourceId> {
    [SourceId::Pubmed, SourceId::OpenAlex, SourceId::Crossref]
        .into_iter()
        .find(|s| f.abstract_in.contains(*s))
}

/// The sources holding a record of a row's work.
#[must_use]
pub fn held_by(row: &Row) -> Sources {
    row.records.iter().map(|r| r.source).collect()
}

/// A row as a wire work.
///
/// # Errors
///
/// No public id for the work.
pub fn work<A: Answers>(a: &A, row: &Row, terms: &Terms) -> Result<pb::Work, Status> {
    let public_id = match &row.public {
        Some(p) => p.clone(),
        None => public(a, row.id)?,
    };
    let f = &row.fields;
    let text = row.text.as_ref();
    let abstract_text = match (
        text.and_then(|t| t.abstract_text.as_ref()),
        abstract_source(f),
    ) {
        (Some(a), Some(source)) if terms.abstract_allowed(source) => a.clone(),
        _ => String::new(),
    };
    let sources = |s: Sources| s.iter().map(|x| x.name().to_owned()).collect::<Vec<_>>();
    Ok(pb::Work {
        public_id,
        stub: row.stub,
        names: row.names.iter().map(name).collect(),
        records: row
            .records
            .iter()
            .map(|r| pb::Record {
                source: r.source.name().to_owned(),
                native: r.native.to_string(),
                promoted: r.promoted,
                revised: r.revised().map(|d| d.to_string()).unwrap_or_default(),
            })
            .collect(),
        held_by: sources(held_by(row)),
        title: text.and_then(|t| t.title.clone()).unwrap_or_default(),
        authors: text.map(|t| t.authors.clone()).unwrap_or_default(),
        venue: text.and_then(|t| t.venue.clone()).unwrap_or_default(),
        published: f.published.as_ref().map(|p| date(p.value)),
        language: f
            .language
            .as_ref()
            .map(|l| l.value.to_string())
            .unwrap_or_default(),
        form: f
            .form
            .as_ref()
            .map(|x| x.value.to_string())
            .unwrap_or_default(),
        genre: f
            .genre
            .as_ref()
            .map(|x| x.value.to_string())
            .unwrap_or_default(),
        retracted_by: sources(f.status.retracted),
        corrected_by: sources(f.status.corrected),
        concern_by: sources(f.status.concern),
        open_access: f.access.as_ref().map(|x| x.value),
        r#abstract: abstract_text,
        disputed: disputed(f),
        references: a
            .page(row.id, crate::answers::Direction::References, 0, 0)
            .total,
        citers: a
            .page(row.id, crate::answers::Direction::Citers, 0, 0)
            .total,
    })
}

/// The rows a hydrated answer needs, in one call (serving plan §11,
/// item 31): the ids of the whole page at once, deduplicated and walked
/// in the index's order, rather than a lookup per row. Empty where the
/// request did not ask to hydrate, and then every summary below is
/// absent.
///
/// # Errors
///
/// A row that does not decode.
fn hydrated<A: Answers>(
    a: &A,
    ids: impl Iterator<Item = WorkId>,
    hydrate: bool,
) -> Result<HashMap<WorkId, Row>, Status> {
    if !hydrate {
        return Ok(HashMap::new());
    }
    let ids: Vec<WorkId> = ids.collect();
    a.rows(&ids).map_err(|e| Status::internal(e.to_string()))
}

/// One work as a list shows it: **the one summary** an arc, a neighbour
/// and a path's step all carry, so that the three cannot drift and a
/// client renders any of them with one function.
///
/// A stub is the case it is shaped for. It has no title and no text at
/// all — nothing we hold is a record of it — and its promoted names are
/// what there is to show, so they travel here beside the flag that says
/// to show them. They are most of what a citation list holds: 98.55% of
/// reference targets on the epoch of 2026-09-19 are stubs, so a hydrated
/// list that carried titles alone would be mostly blank rows.
///
/// The fields are the ones [`work`] promotes, read the same way, so a
/// summary never disagrees with the work it summarises.
fn summary(public_id: String, row: &Row) -> pb::Summary {
    let text = row.text.as_ref();
    let status = &row.fields.status;
    let sources = |s: Sources| s.iter().map(|x| x.name().to_owned()).collect::<Vec<_>>();
    pb::Summary {
        public_id,
        stub: row.stub,
        title: text.and_then(|t| t.title.clone()).unwrap_or_default(),
        names: row
            .names
            .iter()
            .filter(|n| n.standing == Standing::Promoted)
            .map(name)
            .collect(),
        authors: text.map(|t| t.authors.clone()).unwrap_or_default(),
        venue: text.and_then(|t| t.venue.clone()).unwrap_or_default(),
        published: row.fields.published.as_ref().map(|p| date(p.value)),
        retracted_by: sources(status.retracted),
        corrected_by: sources(status.corrected),
        concern_by: sources(status.concern),
    }
}

/// A work as a wire arc: its public id, and its summary when the request
/// asked to hydrate and the work has a row. An id with no row — retired,
/// or past the epoch — carries no summary, since a summary saying
/// nothing would be indistinguishable from a work we know nothing about.
fn arc(public_id: String, id: WorkId, rows: &HashMap<WorkId, Row>, by: Sources) -> pb::Arc {
    pb::Arc {
        by: by.iter().map(|s| s.name().to_owned()).collect(),
        summary: rows.get(&id).map(|row| summary(public_id.clone(), row)),
        public_id,
    }
}

/// Arcs as wire arcs, hydrated when asked.
///
/// # Errors
///
/// No public id for a work, a row that does not decode.
pub fn arcs<A: Answers>(a: &A, arcs: &[graph::Arc], hydrate: bool) -> Result<Vec<pb::Arc>, Status> {
    let rows = hydrated(a, arcs.iter().map(|x| x.work), hydrate)?;
    arcs.iter()
        .map(|x| Ok(arc(public(a, x.work)?, x.work, &rows, x.by)))
        .collect()
}

/// The neighbours of an expansion, hydrated when asked. The raw overlap
/// and the degree it is out of travel beside the score, since "7 of its
/// 9 citers also cite this" is what a reader can weigh and a cosine is
/// not.
///
/// # Errors
///
/// No public id for a work, or a row that does not decode.
pub fn neighbours<A: Answers>(
    a: &A,
    found: &[serve::Neighbour],
    hydrate: bool,
) -> Result<Vec<pb::Neighbour>, Status> {
    let rows = hydrated(a, found.iter().map(|n| n.work), hydrate)?;
    found
        .iter()
        .map(|n| {
            let public_id = public(a, n.work)?;
            Ok(pb::Neighbour {
                touches: n.touches,
                degree: n.degree,
                score: n.score,
                summary: rows.get(&n.work).map(|row| summary(public_id.clone(), row)),
                public_id,
            })
        })
        .collect()
}

/// Search hits as wire hits, hydrated when asked. The title is the
/// index's own and travels either way; the summary is the one every
/// hydrated list carries, so a result renders as a row of any list does.
///
/// # Errors
///
/// No public id for a work, or a row that does not decode.
pub fn hits<A: Answers>(
    a: &A,
    hits: &[search::Hit],
    hydrate: bool,
) -> Result<Vec<pb::Hit>, Status> {
    let rows = hydrated(a, hits.iter().map(|h| h.id), hydrate)?;
    hits.iter()
        .map(|h| {
            let public_id = public(a, h.id)?;
            Ok(pb::Hit {
                score: h.score,
                title: h.title.clone().unwrap_or_default(),
                summary: rows.get(&h.id).map(|row| summary(public_id.clone(), row)),
                public_id,
            })
        })
        .collect()
}

/// One path as wire works: the first reached by no arc, every later one
/// carrying the asserters of the arc that reached it. The same [`pb::Arc`]
/// a list sends, so a client reads a path the way it reads a page.
///
/// # Errors
///
/// No public id for a work on the path, or a row that does not decode.
pub fn steps<A: Answers>(
    a: &A,
    steps: &[graph::Step],
    hydrate: bool,
) -> Result<Vec<pb::Arc>, Status> {
    let rows = hydrated(a, steps.iter().map(|s| s.work), hydrate)?;
    steps
        .iter()
        .map(|step| Ok(arc(public(a, step.work)?, step.work, &rows, step.by)))
        .collect()
}
