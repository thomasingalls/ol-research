//! The traversal primitives on a store built in memory (serving plan
//! §11 item 7, graph plan §7): an expansion's direction and its degree
//! normalisation, and a bounded directed path — over a real epoch, and
//! again with a batch folded over it, since both read the delta.
//!
//! The shape the fixture is built for is the corpus's: a hub cited by
//! everything, a work cited by the seeds alone, and a raw overlap that
//! ties them. On the epoch of 2026-09-19 the tie is not a corner case —
//! 911,071 co-citation candidates were touched by exactly one seed and
//! 39,951 by two — so what the ranking does with a tie is most of what
//! it does.

use merge::view::{Text, View};
use merge::{SourceId, WorkId};

use crate::core::{Serving, Walk};
use crate::fixture::{self, cites, record};
use crate::incremental::{self, Batch, Incoming};

/// One PubMed record, named and titled, with one reference per name in
/// `references` — a `cites` call is one reference, however many names it
/// carries, and these each name one work.
fn work(pmid: u32, doi: Option<&str>, title: &str, references: &[&str]) -> Incoming {
    let pmid = format!("pmid:{pmid}");
    let mut names = vec![pmid.as_str()];
    names.extend(doi);
    let mut r = record(
        SourceId::Pubmed,
        &names,
        title,
        View::default(),
        Text::default(),
    );
    for name in references {
        cites(&mut r, &[name]);
    }
    r
}

/// Three seeds, each citing both T and the hub H; T citing a stub D; and
/// six more works citing H alone, so that H's citer list is three times
/// the seeds' and the raw overlap says nothing.
fn corpus() -> Vec<Incoming> {
    let mut records = vec![
        work(1, None, "Seed one", &["doi:10.1000/t", "doi:10.1000/h"]),
        work(2, None, "Seed two", &["doi:10.1000/t", "doi:10.1000/h"]),
        work(3, None, "Seed three", &["doi:10.1000/t", "doi:10.1000/h"]),
        work(10, Some("doi:10.1000/t"), "Tee", &["doi:10.1000/d"]),
        work(11, Some("doi:10.1000/h"), "The hub", &[]),
    ];
    for n in 20..26u32 {
        records.push(work(n, None, "A citer of the hub", &["doi:10.1000/h"]));
    }
    records
}

fn open(name: &str) -> (ingest::Store, Serving) {
    let store = fixture::store(name);
    assert_eq!(fixture::full_merge(&store, &corpus(), &[]), 1);
    let serving = Serving::open(&store).unwrap();
    (store, serving)
}

fn id(serving: &Serving, text: &str) -> WorkId {
    match serving.resolve(text) {
        Ok(Some(crate::core::Resolved::One(id))) => id,
        other => panic!("{text}: {other:?}"),
    }
}

/// `(work, touches, degree)` of an expansion, in the order it ranked them.
fn ranked(found: &crate::core::Expansion) -> Vec<(u32, u32, u64)> {
    found
        .neighbours
        .iter()
        .map(|n| (n.work.get(), n.touches, n.degree))
        .collect()
}

#[test]
fn an_expansion_normalises_by_degree_before_it_truncates() {
    let (_store, serving) = open("expand-cosine");
    let seeds = [
        id(&serving, "pmid:1"),
        id(&serving, "pmid:2"),
        id(&serving, "pmid:3"),
    ];
    let (t, h) = (id(&serving, "doi:10.1000/t"), id(&serving, "doi:10.1000/h"));
    assert_eq!((serving.in_degree(t), serving.in_degree(h)), (3, 9));
    // Both reached by all three seeds: the raw overlap cannot tell them
    // apart, and the degree can.
    let found = serving.expand(&seeds, Walk::References, 10, None);
    assert_eq!(ranked(&found), [(t.get(), 3, 3), (h.get(), 3, 9)]);
    let top = &found.neighbours;
    assert!((top[0].score - 1.0).abs() < 1e-9, "{}", top[0].score);
    assert!((top[1].score - 3.0 / 27f64.sqrt()).abs() < 1e-9);
    // Every work the hop reached, against the two the budget showed.
    assert_eq!(
        (found.total, found.seeds_used, found.seeds_total),
        (2, 3, 3)
    );
    // And the truncate takes the score's order, not the overlap's: a
    // budget of one keeps the work the seeds share and not the hub.
    assert_eq!(
        serving.expand(&seeds, Walk::References, 1, None).neighbours[0].work,
        t,
        "the hub would win on raw overlap, which ties"
    );
}

#[test]
fn the_direction_is_the_callers_and_the_degree_is_its_mirror() {
    let (_store, serving) = open("expand-direction");
    let t = id(&serving, "doi:10.1000/t");
    let d = id(&serving, "doi:10.1000/d");
    let seeds = [
        id(&serving, "pmid:1"),
        id(&serving, "pmid:2"),
        id(&serving, "pmid:3"),
    ];
    assert!(serving.is_stub(d), "nothing holds doi:10.1000/d");
    // Walking what T cites reaches the stub, and the degree it is out of
    // is the stub's citers — the orientation not walked.
    let cited = serving.expand(&[t], Walk::References, 10, None);
    assert_eq!(ranked(&cited), [(d.get(), 1, 1)]);
    // Walking what cites T reaches the three seeds, each out of its own
    // reference count.
    let citing = serving.expand(&[t], Walk::Citers, 10, None);
    assert_eq!(
        ranked(&citing),
        seeds.map(|s| (s.get(), 1, 2)).to_vec(),
        "each seed cites T and the hub"
    );
    // Both is the two together, the stub ahead on its smaller degree.
    let both = serving.expand(&[t], Walk::Both, 10, None);
    assert_eq!(both.neighbours[0].work, d);
    assert_eq!(both.neighbours.len(), 4);
    assert!(both.neighbours[1..].iter().all(|n| n.degree == 2));
    // A seed named twice is one seed, so the divisor is one and not two.
    assert_eq!(
        serving.expand(&[t, t], Walk::References, 10, None),
        serving.expand(&[t], Walk::References, 10, None)
    );
}

#[test]
fn a_path_is_directed_bounded_and_answers_when_there_is_none() {
    let (_store, serving) = open("path");
    let (s1, s2) = (id(&serving, "pmid:1"), id(&serving, "pmid:2"));
    let t = id(&serving, "doi:10.1000/t");
    let d = id(&serving, "doi:10.1000/d");
    let bounds = graph::Bounds::DEFAULT;
    // S1 -> T -> D, the one shortest path, with the asserters on it.
    let graph::Found::Paths { hops, paths, .. } = serving.path(s1, d, bounds) else {
        panic!("no path")
    };
    assert_eq!(hops, 2);
    assert_eq!(
        paths[0].iter().map(|s| s.work).collect::<Vec<_>>(),
        [s1, t, d]
    );
    assert!(paths[0][0].by.is_empty());
    assert!(
        paths[0][1..]
            .iter()
            .all(|s| s.by.contains(SourceId::Pubmed))
    );
    // The other direction is the other question: the stub cites nothing.
    let graph::Found::None { depth, reached } = serving.path(d, s1, bounds) else {
        panic!("a stub cites nothing")
    };
    assert_eq!((depth, reached.forward), (6, 1));
    // Two works that only share what they cite have no path either way.
    assert!(matches!(
        serving.path(s1, s2, bounds),
        graph::Found::None { .. }
    ));
    // A bound too short is no path, never a wrong one.
    assert!(matches!(
        serving.path(s1, d, graph::Bounds { depth: 1, ..bounds }),
        graph::Found::None { depth: 1, .. }
    ));
    // A spent budget is its own outcome, and says what it did prove.
    let graph::Found::Stopped { proved, visits, .. } = serving.path(
        s1,
        d,
        graph::Bounds {
            visits: 1,
            ..bounds
        },
    ) else {
        panic!("one work expanded cannot reach two hops")
    };
    assert_eq!((proved, visits), (1, 1));
}

#[test]
fn both_read_the_delta_the_batch_left() {
    let (store, serving) = open("traversal-delta");
    let t = id(&serving, "doi:10.1000/t");
    let d = id(&serving, "doi:10.1000/d");
    assert_eq!(serving.in_degree(t), 3);
    let mut later = work(4, None, "A fourth seed", &["doi:10.1000/t"]);
    later.view.set = "batch".into();
    let applied = incremental::apply(
        &store,
        &serving,
        Batch {
            records: vec![later],
            cited_only: Vec::new(),
            deletions: Vec::new(),
            store_deletions: Vec::new(),
            inputs: vec!["batch-1".into()],
        },
        Some(&fixture::key()),
    )
    .unwrap();
    assert_eq!((applied.batch, applied.records), (2, 1));
    assert_eq!(serving.watermark().batch, 2);
    let fourth = id(&serving, "pmid:4");
    // The degree the score divides by counts the batch's arc.
    assert_eq!(serving.in_degree(t), 4);
    let citing = serving.expand(&[t], Walk::Citers, 10, None);
    assert!(citing.neighbours.iter().any(|n| n.work == fourth));
    // The new work's only reference is T, so its degree there is one.
    assert_eq!(serving.out_degree(fourth), 1);
    // And a path runs over the arc the batch added.
    let graph::Found::Paths { hops, paths, .. } = serving.path(fourth, d, graph::Bounds::DEFAULT)
    else {
        panic!("no path over the batch's arc")
    };
    assert_eq!(hops, 2);
    assert_eq!(
        paths[0].iter().map(|s| s.work).collect::<Vec<_>>(),
        [fourth, t, d]
    );
}
