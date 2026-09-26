//! The bulk row read (serving plan §11, items 13 and 31): that it
//! answers exactly what a row at a time answers — the delta's overrides
//! included, since a batch-touched row is read from the works delta and
//! not the base — and, over a real store when `OL_RHT_STORE` names one,
//! what it saves.
//!
//! The correctness half is here rather than over a live epoch because
//! the cases that matter are the ones a published base alone cannot
//! show: an id a batch restated, an id a batch retired, an id nobody
//! minted, and a page naming the same work twice.

use std::collections::HashMap;

use merge::view::{Text, View};
use merge::{SourceId, WorkId};

use crate::core::Serving;
use crate::fixture::{self, cites, record};
use crate::incremental::{self, Batch, Incoming};
use crate::row::Row;

/// One PubMed record, named and titled, citing each name in
/// `references`.
fn work(pmid: u32, title: &str, references: &[&str]) -> Incoming {
    let pmid = format!("pmid:{pmid}");
    let mut r = record(
        SourceId::Pubmed,
        &[pmid.as_str()],
        title,
        View::default(),
        Text::default(),
    );
    for name in references {
        cites(&mut r, &[name]);
    }
    r
}

/// Six held works and the stubs they cite.
fn corpus() -> Vec<Incoming> {
    (1..=6u32)
        .map(|n| {
            work(
                n,
                &format!("Work {n}"),
                &[&format!("doi:10.1000/{n}"), "doi:10.1000/shared"],
            )
        })
        .collect()
}

/// Every id the epoch could hold, and two past it.
fn every_id(serving: &Serving) -> Vec<WorkId> {
    (1..=serving.minted() + 2).filter_map(WorkId::new).collect()
}

/// [`Serving::work`] for each id, which is what the bulk read must
/// agree with exactly.
fn one_at_a_time(serving: &Serving, ids: &[WorkId]) -> HashMap<WorkId, Row> {
    ids.iter()
        .filter_map(|id| serving.work(*id).unwrap().map(|row| (*id, row)))
        .collect()
}

#[test]
fn a_bulk_read_answers_what_a_row_at_a_time_answers() {
    let store = fixture::store("rows-bulk");
    assert_eq!(fixture::full_merge(&store, &corpus(), &[]), 1);
    let serving = Serving::open(&store).unwrap();
    let ids = every_id(&serving);
    assert!(ids.len() > 6, "{} ids", ids.len());
    assert_eq!(serving.rows(&ids).unwrap(), one_at_a_time(&serving, &ids));

    // The order the caller asks in is not the order the index is walked
    // in, and a page may name one work twice — a work cited by two of
    // its own citers, say. Neither changes the answer.
    let mut shuffled: Vec<WorkId> = ids.iter().rev().copied().collect();
    shuffled.extend_from_slice(&ids);
    assert_eq!(
        serving.rows(&shuffled).unwrap(),
        serving.rows(&ids).unwrap()
    );
    assert!(serving.rows(&[]).unwrap().is_empty());

    // An id nobody minted is absent, not an error, and absence is the
    // only thing that says so.
    let past = WorkId::new(serving.minted() + 1).unwrap();
    assert!(!serving.rows(&[past]).unwrap().contains_key(&past));
}

#[test]
fn a_bulk_read_applies_the_deltas_overrides() {
    let store = fixture::store("rows-delta");
    assert_eq!(fixture::full_merge(&store, &corpus(), &[]), 1);
    let serving = Serving::open(&store).unwrap();
    let first = serving.resolve("pmid:1").unwrap();
    let Some(crate::Resolved::One(first)) = first else {
        panic!("pmid:1 is one work")
    };
    assert!(!serving.in_delta(first), "nothing has touched it yet");

    // A batch restating one work under a new title and adding another:
    // the first is a row the works delta now holds, the second a row
    // past the base entirely.
    let mut restated = work(1, "Work 1, revised", &["doi:10.1000/1"]);
    restated.view.set = "batch".into();
    let mut added = work(7, "Work 7", &["doi:10.1000/shared"]);
    added.view.set = "batch".into();
    let mut delivered = corpus();
    delivered.extend([restated.clone(), added.clone()]);
    let applied = incremental::apply_with(
        &store,
        &serving,
        Batch {
            records: vec![restated, added],
            cited_only: Vec::new(),
            deletions: Vec::new(),
            store_deletions: Vec::new(),
            inputs: vec!["batch-1".into()],
        },
        Some(&fixture::key()),
        &fixture::Layer(delivered),
    )
    .unwrap();
    assert_eq!(applied.batch, 2);
    assert!(serving.in_delta(first), "the batch restated it");

    let ids = every_id(&serving);
    let bulk = serving.rows(&ids).unwrap();
    assert_eq!(bulk, one_at_a_time(&serving, &ids));
    // Not merely equal to each other: the delta's title, and not the
    // base's, which is what an override being applied means.
    let title = bulk[&first]
        .text
        .as_ref()
        .and_then(|t| t.title.as_deref())
        .unwrap_or_default();
    assert_eq!(title, "Work 1, revised");
    let seventh = match serving.resolve("pmid:7").unwrap() {
        Some(crate::Resolved::One(id)) => id,
        other => panic!("{other:?}"),
    };
    assert!(bulk.contains_key(&seventh), "the row the batch added");
}

/// The live epoch, when `OL_RHT_STORE` names a store with one: what the
/// bulk read saves against a lookup per id, in the two shapes the
/// transport hydrates — a page of a list, and an expansion. Skipped
/// otherwise, and it reads only, so it is safe against the store the
/// server is serving from.
// Block counts and byte counts printed as ratios and megabytes: a
// measurement's arithmetic, not an answer's.
#[allow(clippy::cast_precision_loss)]
#[test]
fn the_live_epoch_measures_the_bulk_read() {
    let Ok(root) = std::env::var("OL_RHT_STORE") else {
        eprintln!("OL_RHT_STORE not set; skipping");
        return;
    };
    let store = ingest::Store::new(root);
    let serving = Serving::open(&store).unwrap();
    let hub = match serving.resolve("doi:10.1136/bmj.n71").unwrap() {
        Some(crate::Resolved::One(id)) => id,
        other => {
            eprintln!("the measurement's work is {other:?}; skipping");
            return;
        }
    };
    // The two shapes a request hydrates: one page of a list at the
    // transport's cap, and one expansion at its default budget.
    let page: Vec<WorkId> = serving.citers(hub).take(100).map(|a| a.work).collect();
    let expanded: Vec<WorkId> = serving
        .related(hub, crate::By::CitedAlongside, 20)
        .neighbours
        .iter()
        .map(|n| n.work)
        .collect();
    eprintln!(
        "epoch {}, batch {}: {} works in the index",
        serving.epoch().epoch,
        serving.watermark().batch,
        serving.works().manifest().works
    );
    let big: Vec<WorkId> = serving
        .related(hub, crate::By::CitedAlongside, 100)
        .neighbours
        .iter()
        .map(|n| n.work)
        .collect();
    let hits: Vec<WorkId> = serving
        .search(&query_parser::parse("systematic review").unwrap(), 100)
        .unwrap()
        .hits
        .iter()
        .map(|h| h.id)
        .collect();
    for (what, ids) in [
        ("a page of citers", &page),
        ("an expansion", &expanded),
        ("a 100-work expansion", &big),
        ("a search page", &hits),
    ] {
        // Both passes over warm pages, so the mmap is not what is being
        // measured; the one-block cache is why the pass order is run
        // twice each way.
        let _ = serving.rows(ids).unwrap();
        let _ = one_at_a_time(&serving, ids);
        let counted = || {
            (
                serving.works().decompressed(),
                serving.works().decompressed_bytes(),
            )
        };
        let before = counted();
        let started = std::time::Instant::now();
        let singly = one_at_a_time(&serving, ids);
        let singly_ms = started.elapsed().as_secs_f64() * 1000.0;
        let (singly_blocks, singly_bytes) = (counted().0 - before.0, counted().1 - before.1);
        let before = counted();
        let started = std::time::Instant::now();
        let bulk = serving.rows(ids).unwrap();
        let bulk_ms = started.elapsed().as_secs_f64() * 1000.0;
        let (bulk_blocks, bulk_bytes) = (counted().0 - before.0, counted().1 - before.1);
        assert_eq!(bulk, singly);
        let mib = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
        let distinct: std::collections::HashSet<WorkId> = ids.iter().copied().collect();
        eprintln!(
            "{what}: {} ids ({} distinct), {} rows\n  \
             one at a time: {singly_blocks} blocks, {:.1} MiB, {singly_ms:.2} ms\n  \
             in one call:   {bulk_blocks} blocks, {:.1} MiB, {bulk_ms:.2} ms  \
             ({:.1}x fewer blocks, {:.1}x faster)",
            ids.len(),
            distinct.len(),
            bulk.len(),
            mib(singly_bytes),
            mib(bulk_bytes),
            singly_blocks as f64 / bulk_blocks.max(1) as f64,
            singly_ms / bulk_ms,
        );
    }
}
