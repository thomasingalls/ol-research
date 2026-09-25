//! Two open cores against each other (serving plan §5): an epoch plus
//! the deltas folded over it, and the epoch a full merge built after
//! them, must answer every question identically. What differs is
//! returned as text, one line a difference, so a test or a check can
//! print it; nothing is a difference.

use std::collections::BTreeMap;

use fst::Streamer;
use merge::build::OwnedUnresolved;
use merge::delta::UnresolvedLine;
use merge::record::Unresolved;
use merge::{SourceId, WorkId};

use crate::core::Serving;
use crate::error::Error;

/// Queries the lexical comparison runs, since the engine's scores depend
/// on its segments and only the hits can be compared.
pub const QUERIES: [&str; 6] = [
    "aspirin",
    "randomized trial",
    "systematic review",
    "protein",
    "cancer",
    "learning",
];

/// Every difference between `a` and `b`: rows by id, names by key, the
/// lists of every work, the lexical hits of the fixed queries. At most
/// `limit` lines.
///
/// # Errors
///
/// A row that does not decode, or the engine.
#[allow(clippy::too_many_lines)]
pub fn differences(a: &Serving, b: &Serving, limit: usize) -> Result<Vec<String>, Error> {
    let mut out: Vec<String> = Vec::new();
    let mut note = |what: String| {
        if out.len() < limit {
            out.push(what);
        }
    };
    let minted = a.minted().max(b.minted());
    if a.minted() != b.minted() {
        note(format!("ids minted: {} against {}", a.minted(), b.minted()));
    }
    for n in 1..=minted {
        let Some(id) = WorkId::new(n) else {
            continue;
        };
        let (ra, rb) = (a.work(id)?, b.work(id)?);
        match (&ra, &rb) {
            (None, None) => {}
            (Some(_), None) | (None, Some(_)) => note(format!(
                "{id}: a row on one side only ({})",
                if ra.is_some() { "a" } else { "b" }
            )),
            (Some(x), Some(y)) => {
                if x.names != y.names {
                    note(format!(
                        "{id}: names differ\n  a: {:?}\n  b: {:?}",
                        x.names, y.names
                    ));
                }
                if x.records != y.records {
                    note(format!(
                        "{id}: records differ\n  a: {:?}\n  b: {:?}",
                        x.records, y.records
                    ));
                }
                if x.rejected != y.rejected {
                    note(format!("{id}: rejected assertions differ"));
                }
                if x.fields != y.fields {
                    note(format!(
                        "{id}: fields differ\n  a: {:?}\n  b: {:?}",
                        x.fields, y.fields
                    ));
                }
                if x.flags != y.flags {
                    note(format!("{id}: flags differ"));
                }
                if x.stub != y.stub {
                    note(format!("{id}: a stub on one side only"));
                }
                if x.public != y.public {
                    note(format!(
                        "{id}: public id {:?} against {:?}",
                        x.public, y.public
                    ));
                }
                if x.text != y.text {
                    note(format!(
                        "{id}: text differs\n  a: {:?}\n  b: {:?}",
                        x.text, y.text
                    ));
                }
            }
        }
        if a.live(id) != b.live(id) {
            note(format!(
                "{id}: live as {:?} against {:?}",
                a.live(id),
                b.live(id)
            ));
        }
        // What a retired id with no successor answers with (serving plan
        // §11, item 24): the tail's parts against the base's.
        if a.split_into(id) != b.split_into(id) {
            note(format!(
                "{id}: split into {:?} against {:?}",
                a.split_into(id),
                b.split_into(id)
            ));
        }
        let refs = |s: &Serving| -> Vec<(WorkId, String)> {
            s.references(id)
                .map(|x| (x.work, x.by.to_string()))
                .collect()
        };
        let cites = |s: &Serving| -> Vec<(WorkId, String)> {
            s.citers(id).map(|x| (x.work, x.by.to_string())).collect()
        };
        if refs(a) != refs(b) {
            note(format!(
                "{id}: references differ\n  a: {:?}\n  b: {:?}",
                refs(a),
                refs(b)
            ));
        }
        if cites(a) != cites(b) {
            note(format!(
                "{id}: citers differ\n  a: {:?}\n  b: {:?}",
                cites(a),
                cites(b)
            ));
        }
        if a.public_of(id) != b.public_of(id) {
            note(format!("{id}: public id differs in the lookup"));
        }
        if a.is_stub(id) != b.is_stub(id) {
            note(format!("{id}: stubness differs in the lookup"));
        }
    }
    // Every key either side's base knows resolves the same on both.
    for (side, s, other) in [("a", a, b), ("b", b, a)] {
        let mut stream = s.names().stream();
        while let Some((key, _)) = stream.next() {
            let key = String::from_utf8_lossy(key).into_owned();
            let (x, y) = (s.resolve_key(&key), other.resolve_key(&key));
            if x != y {
                note(format!("{key} (from {side}): {x:?} against {y:?}"));
            }
        }
    }
    // Who cited each name, beside the work each reference landed on:
    // the evidence a later batch settles references again from (serving
    // plan §11, items 26, 28 and 35), which a batch can leave stale while
    // every arc is right.
    let mut cited: std::collections::BTreeSet<String> = a.cited_as_names().into_iter().collect();
    cited.extend(b.cited_as_names());
    for key in cited {
        let (x, y) = (a.cited_as_key(&key), b.cited_as_key(&key));
        if x != y {
            note(format!(
                "{key}: cited-as rows differ\n  a: {x:?}\n  b: {y:?}"
            ));
        }
    }
    for query in QUERIES {
        let (ha, hb) = (a.search(query, 100)?, b.search(query, 100)?);
        if ha.total != hb.total {
            note(format!(
                "search {query:?}: {} hits against {}",
                ha.total, hb.total
            ));
        }
        let ids = |h: &search::Hits| {
            let mut ids: Vec<WorkId> = h.hits.iter().map(|x| x.id).collect();
            ids.sort_unstable();
            ids
        };
        if ha.total <= 100 && ids(&ha) != ids(&hb) {
            note(format!("search {query:?}: different works hit"));
        }
        if ha.facets != hb.facets {
            note(format!("search {query:?}: facet counts differ"));
        }
    }
    if a.search_index().documents() != b.search_index().documents() {
        note(format!(
            "the lexical index holds {} documents against {}",
            a.search_index().documents(),
            b.search_index().documents()
        ));
    }
    Ok(out)
}

/// Every unresolved row of the store as it stands, by citing work and
/// source: the merged sets' rows with the delta log's lines laid over
/// them. The serving layer answers none of these, so a comparison takes
/// this before a full merge replaces the merged directory and again after
/// (serving plan §11, item 35, the user, 2026-09-25). Each pair's rows are
/// sorted, since the order a record's copies were read in is no part of
/// what they say.
///
/// # Errors
///
/// I/O, or a set or a line that does not read.
pub fn unresolved(
    store: &ingest::Store,
) -> Result<BTreeMap<(WorkId, SourceId), Vec<OwnedUnresolved>>, Error> {
    let merged = merge::layout::dir(store);
    let mut rows: BTreeMap<(WorkId, SourceId), Vec<OwnedUnresolved>> = BTreeMap::new();
    for path in
        merge::sets::find(&merged, ingest::Kind::Unresolved).map_err(|e| Error::io(&merged, e))?
    {
        let reader = merge::layout::open(&path).map_err(|e| Error::io(&path, e))?;
        let table = reader.table();
        reader.with_table(|| -> Result<(), Error> {
            for row in reader.records::<Unresolved<'_>>() {
                let row = row.map_err(|e| Error::io(&path, e))?;
                rows.entry((row.from, row.by))
                    .or_default()
                    .push(OwnedUnresolved::of(&row, table));
            }
            Ok(())
        })?;
    }
    let log = merge::delta::dir(&merged).join(merge::delta::UNRESOLVED);
    let bytes = std::fs::metadata(&log).map_or(0, |m| m.len());
    let lines: Vec<UnresolvedLine> =
        merge::delta::read(&log, 0, bytes, None).map_err(|e| Error::io(&log, e))?;
    for line in lines {
        match line {
            UnresolvedLine::Put {
                from,
                by,
                rows: put,
                ..
            } => {
                if put.is_empty() {
                    rows.remove(&(from, by));
                } else {
                    rows.insert((from, by), put);
                }
            }
            UnresolvedLine::Move { from, into, .. } => {
                let moved: Vec<(WorkId, SourceId)> =
                    rows.keys().filter(|(f, _)| *f == from).copied().collect();
                for key in moved {
                    let mut list = rows.remove(&key).unwrap_or_default();
                    for row in &mut list {
                        row.from = into;
                    }
                    rows.entry((into, key.1)).or_default().extend(list);
                }
            }
        }
    }
    for list in rows.values_mut() {
        list.sort_by_cached_key(|r| format!("{r:?}"));
    }
    Ok(rows)
}

/// What differs between two stores' unresolved rows, as
/// [`unresolved`] takes them: at most `limit` lines.
#[must_use]
pub fn unresolved_differences(
    a: &BTreeMap<(WorkId, SourceId), Vec<OwnedUnresolved>>,
    b: &BTreeMap<(WorkId, SourceId), Vec<OwnedUnresolved>>,
    limit: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    let keys: std::collections::BTreeSet<&(WorkId, SourceId)> = a.keys().chain(b.keys()).collect();
    for key in keys {
        let (x, y) = (a.get(key), b.get(key));
        if x != y && out.len() < limit {
            out.push(format!(
                "{} by {}: unresolved rows differ\n  a: {x:?}\n  b: {y:?}",
                key.0, key.1
            ));
        }
    }
    out
}
