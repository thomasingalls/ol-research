//! The boundary: the core's types to the wire's, `WorkId`s becoming
//! public ids on the way.

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
    if let Some(x) = &f.venue {
        push(
            "venue",
            x.from,
            x.others
                .iter()
                .map(|o| (o.source, venue_text(&o.value)))
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

/// A row's promoted names and title, for a hydrated arc or neighbour.
fn promoted(row: &Row) -> (String, Vec<pb::Name>) {
    let title = row
        .text
        .as_ref()
        .and_then(|t| t.title.clone())
        .unwrap_or_default();
    let names = row
        .names
        .iter()
        .filter(|n| n.standing == Standing::Promoted)
        .map(name)
        .collect();
    (title, names)
}

/// Arcs as wire arcs, hydrated with each work's title and promoted names
/// when asked.
///
/// # Errors
///
/// No public id for a work, or a row that does not decode.
pub fn arcs<A: Answers>(a: &A, arcs: &[graph::Arc], hydrate: bool) -> Result<Vec<pb::Arc>, Status> {
    arcs.iter()
        .map(|arc| {
            let (title, names) = if hydrate {
                match a.work(arc.work) {
                    Ok(Some(row)) => promoted(&row),
                    Ok(None) => (String::new(), Vec::new()),
                    Err(e) => return Err(Status::internal(e.to_string())),
                }
            } else {
                (String::new(), Vec::new())
            };
            Ok(pb::Arc {
                public_id: public(a, arc.work)?,
                by: arc.by.iter().map(|s| s.name().to_owned()).collect(),
                title,
                names,
            })
        })
        .collect()
}

/// A neighbour of an expansion, hydrated with its title when asked.
///
/// # Errors
///
/// No public id for the work.
pub fn neighbour<A: Answers>(
    a: &A,
    id: WorkId,
    touches: u32,
    hydrate: bool,
) -> Result<pb::Neighbour, Status> {
    let title = if hydrate {
        a.work(id)
            .ok()
            .flatten()
            .and_then(|r| r.text.and_then(|t| t.title))
            .unwrap_or_default()
    } else {
        String::new()
    };
    Ok(pb::Neighbour {
        public_id: public(a, id)?,
        touches,
        title,
    })
}
