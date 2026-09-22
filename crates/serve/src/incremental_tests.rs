//! The delta path on a store built in memory (serving plan §5, §11 item
//! 8): new records, a re-deposit that replaces a reference list, a
//! record bridging two works, existing stubs cited again, a deleted
//! record with and without a work left behind; the cases the rules
//! leave to a full merge; and the test of §5 — the core over the epoch
//! plus the batch answers exactly as the core over the epoch a full
//! merge builds after it, and that merge appends nothing.

use merge::journal;
use merge::view::{Text, Venue, View};
use merge::{OwnedName, SourceId, WorkId};
use model::Issn;

use crate::compare;
use crate::core::{Resolved, Serving};
use crate::fixture::{self, cites, record};
use crate::incremental::{self, Batch, Deletion, Incoming};

/// The set every batch record and deletion of these tests is delivered in:
/// after the base's, so its copies are the newer (merge plan §8).
const BATCH: &str = "batch";

/// A record as a batch delivers it.
fn delivered(mut r: Incoming) -> Incoming {
    r.view.set = BATCH.into();
    r
}

/// A deletion a batch carries.
fn withdrawal(source: SourceId, name: &str) -> Deletion {
    Deletion {
        source,
        native: OwnedName::parse(name).unwrap(),
        set: BATCH.into(),
    }
}

fn view(has_abstract: bool, authors: u32, issn: Option<&str>) -> View<'static> {
    View {
        has_abstract,
        authors,
        venue: Venue {
            issns: issn.map(|i| Issn::parse(i).unwrap()).into_iter().collect(),
            ..Venue::default()
        },
        ..View::default()
    }
}

fn text(abstract_text: Option<&str>, authors: &[&str], venue: Option<&str>) -> Text {
    Text {
        abstract_text: abstract_text.map(str::to_owned),
        authors: authors.iter().map(|a| (*a).to_owned()).collect(),
        venue: venue.map(str::to_owned),
    }
}

/// The base: works A (PubMed + Crossref), B, C, E and F (one work in
/// truth, two here), G citing F, H (two records), I alone, J alone and
/// cited, K (OpenAlex + Crossref); C cites A and two stubs.
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn base() -> Vec<Incoming> {
    use SourceId::{Crossref, OpenAlex, Pubmed};
    let mut a1 = record(
        Pubmed,
        &["pmid:1", "doi:10.1000/a"],
        "Alpha",
        view(true, 3, Some("0960-1481")),
        text(
            Some("Alpha abstract."),
            &["Ada", "Bob", "Cy"],
            Some("J Alpha"),
        ),
    );
    cites(&mut a1, &["doi:10.1000/b"]);
    let a2 = record(
        Crossref,
        &["doi:10.1000/a"],
        "Alpha",
        view(false, 2, Some("0960-1481")),
        text(None, &["Ada L", "Bob M"], Some("Journal of Alpha")),
    );
    let b = record(
        Crossref,
        &["doi:10.1000/b"],
        "Beta",
        view(false, 1, None),
        text(None, &["Bea"], None),
    );
    let mut c = record(
        Pubmed,
        &["pmid:3"],
        "Gamma",
        view(true, 1, None),
        text(Some("Gamma abstract."), &["Cee"], None),
    );
    cites(&mut c, &["doi:10.1000/a"]);
    cites(&mut c, &["doi:10.1000/stub"]);
    cites(&mut c, &["pmid:9"]);
    cites(&mut c, &["pmid:10"]);
    cites(&mut c, &["pmid:20", "doi:10.1000/t"]);
    cites(&mut c, &["pmid:21", "pmc:PMC21"]);
    cites(&mut c, &["pmid:40"]);
    let mut m = record(
        Crossref,
        &["doi:10.1000/m"],
        "Mu",
        view(false, 1, None),
        text(None, &["Mu"], None),
    );
    cites(&mut m, &["doi:10.1000/t"]);
    cites(&mut m, &["doi:10.1000/v2"]);
    cites(
        &mut c,
        &["pmid:30", "pmc:PMC30", "doi:10.1000/v2", "doi:10.1000/v3"],
    );
    let v = record(
        Crossref,
        &["doi:10.1000/v"],
        "Nu",
        view(false, 1, None),
        text(None, &["Nu"], None),
    );
    let e = record(
        Crossref,
        &["doi:10.1000/e"],
        "Epsilon",
        view(false, 2, None),
        text(None, &["Ed", "Em"], Some("J Eps")),
    );
    let f = record(
        Pubmed,
        &["pmid:6"],
        "Epsilon",
        view(true, 2, None),
        text(Some("Epsilon abstract."), &["Ed X", "Em Y"], None),
    );
    let mut g = record(
        Pubmed,
        &["pmid:12"],
        "Gee",
        view(false, 1, None),
        text(None, &["Gee"], None),
    );
    cites(&mut g, &["pmid:6"]);
    let h1 = record(
        Pubmed,
        &["pmid:7", "doi:10.1000/h"],
        "Eta",
        view(true, 2, None),
        text(Some("Eta abstract."), &["Hal", "Hen"], None),
    );
    let h2 = record(
        Crossref,
        &["doi:10.1000/h"],
        "Eta",
        view(false, 2, None),
        text(None, &["Hal C", "Hen C"], Some("J Eta")),
    );
    let i = record(
        Pubmed,
        &["pmid:8"],
        "Iota",
        view(false, 1, None),
        text(None, &["Io"], None),
    );
    let j = record(
        Pubmed,
        &["pmid:10"],
        "Jay",
        view(false, 1, None),
        text(None, &["Jay"], None),
    );
    let k1 = record(
        Crossref,
        &["doi:10.1000/k"],
        "Kappa",
        view(false, 1, None),
        text(None, &["Kay"], None),
    );
    let k2 = record(
        OpenAlex,
        &["openalex:W11", "doi:10.1000/k"],
        "Kappa",
        view(true, 1, None),
        text(Some("Kappa abstract."), &["Kay O"], Some("Kappa Journal")),
    );
    let l1 = record(
        Pubmed,
        &["pmid:13"],
        "Lambda",
        view(true, 1, None),
        text(Some("Lambda abstract."), &["Lam"], None),
    );
    let l2 = record(
        OpenAlex,
        &["openalex:W13", "pmid:13"],
        "Lambda",
        view(true, 1, None),
        text(Some("Lambda abstract (OA)."), &["Lam O"], Some("J Lambda")),
    );
    vec![a1, a2, b, c, e, f, g, h1, h2, i, j, k1, k2, l1, l2, m, v]
}

/// The batch: OpenAlex's record of A, B re-deposited with references,
/// a new work D citing the stubs by a new alias, OpenAlex's record
/// bridging E and F, H's PubMed record deleted, I deleted.
fn batch() -> Batch {
    use SourceId::{Crossref, OpenAlex, Pubmed};
    let mut a3 = record(
        OpenAlex,
        &["openalex:W1", "doi:10.1000/a", "pmid:1"],
        "Alpha",
        view(true, 3, Some("0960-1481")),
        text(
            Some("Alpha abstract (OpenAlex)."),
            &["A", "B", "C"],
            Some("J Alpha (OA)"),
        ),
    );
    cites(&mut a3, &["doi:10.1000/b"]);
    cites(&mut a3, &["openalex:W99"]);
    let mut b2 = record(
        Crossref,
        &["doi:10.1000/b"],
        "Beta",
        view(false, 1, None),
        text(None, &["Bea"], None),
    );
    cites(&mut b2, &["pmid:1"]);
    cites(&mut b2, &["doi:10.1000/new"]);
    let mut d = record(
        Pubmed,
        &["pmid:4"],
        "Delta",
        view(true, 1, None),
        text(Some("Delta abstract."), &["Dee"], None),
    );
    cites(&mut d, &["pmid:9", "doi:10.1000/nine"]);
    cites(&mut d, &["doi:10.1000/stub"]);
    let mut ef = record(
        OpenAlex,
        &["openalex:W6", "doi:10.1000/e", "pmid:6"],
        "Epsilon",
        view(true, 2, None),
        text(
            Some("Epsilon abstract (OA)."),
            &["Ed O", "Em O"],
            Some("J Eps (OA)"),
        ),
    );
    // Stubs named beside a held work, the registry's order choosing the
    // stub: the arc lands on the stub and the full merge counts the
    // citing work among the stub name's citers (serving plan §11, item
    // 21) — S2, which D also cites alone, and the pmid:40 stub, which
    // nothing in the batch cites alone.
    cites(&mut ef, &["pmid:9", "doi:10.1000/b"]);
    // Two stubs a record takes over by their PMID alone: T's DOI is cited
    // alone by M and becomes a stub of its own; U's PMC id is cited only
    // beside its PMID and is dropped.
    let mut t = record(
        Pubmed,
        &["pmid:20"],
        "Tau",
        view(false, 1, None),
        text(None, &["Tau"], None),
    );
    cites(&mut t, &["pmid:40", "doi:10.1000/m"]);
    let mut u = record(
        Pubmed,
        &["pmid:21"],
        "Upsilon",
        view(false, 1, None),
        text(None, &["Ups"], None),
    );
    // The stub the merged stub's leftover DOI becomes, named beside B: two
    // DOIs rank alike and the first named wins, so the arc lands on the
    // new stub, which nothing in the batch cites alone.
    cites(&mut u, &["doi:10.1000/v2", "doi:10.1000/b"]);
    // A stub merged into an older work by the record that claims both:
    // the stub's DOI, which M cites alone, becomes a stub of its own.
    let mut nu = record(
        Pubmed,
        &["pmid:30", "doi:10.1000/v", "pmc:PMC30"],
        "Nu",
        view(true, 1, None),
        text(Some("Nu abstract."), &["Nu P"], None),
    );
    // The stub T's leftover DOI becomes, cited alone in the same batch.
    cites(&mut nu, &["doi:10.1000/t"]);
    Batch {
        records: [a3, b2, d, ef, t, u, nu]
            .into_iter()
            .map(delivered)
            .collect(),
        cited_only: Vec::new(),
        deletions: vec![withdrawal(Pubmed, "pmid:7"), withdrawal(Pubmed, "pmid:8")],
        store_deletions: Vec::new(),
        inputs: vec!["batch-1".into()],
    }
}

/// A stub name's citers as records, the text mark left aside.
fn pairs(named: &crate::row::Named) -> Vec<(WorkId, SourceId)> {
    named.cited_by.iter().map(|c| c.pair()).collect()
}

fn resolve(s: &Serving, name: &str) -> Option<WorkId> {
    match s.resolve(name).unwrap() {
        Some(Resolved::One(id)) => Some(id),
        _ => None,
    }
}

fn refs(s: &Serving, id: WorkId) -> Vec<(u32, String)> {
    s.references(id)
        .map(|a| (a.work.get(), a.by.to_string()))
        .collect()
}

fn citers(s: &Serving, id: WorkId) -> Vec<(u32, String)> {
    s.citers(id)
        .map(|a| (a.work.get(), a.by.to_string()))
        .collect()
}

#[test]
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn a_batch_folds_and_equals_the_full_merge_after_it() {
    let store = fixture::store("batch");
    let base_records = base();
    assert_eq!(fixture::full_merge(&store, &base_records, &[]), 1);
    let serving = Serving::open(&store).unwrap();
    assert_eq!(serving.watermark().batch, 1);
    let a = resolve(&serving, "pmid:1").unwrap();
    let b = resolve(&serving, "doi:10.1000/b").unwrap();
    let e = resolve(&serving, "doi:10.1000/e").unwrap();
    let f = resolve(&serving, "pmid:6").unwrap();
    let g = resolve(&serving, "pmid:12").unwrap();
    let h = resolve(&serving, "pmid:7").unwrap();
    let i = resolve(&serving, "pmid:8").unwrap();
    let s1 = resolve(&serving, "doi:10.1000/stub").unwrap();
    let s2 = resolve(&serving, "pmid:9").unwrap();
    assert!(serving.is_stub(s1) && serving.is_stub(s2));
    assert!(e < f, "the DOI-named work was minted first");
    assert_eq!(refs(&serving, b), vec![]);
    assert_eq!(citers(&serving, f), vec![(g.get(), "pubmed".to_owned())]);
    let minted_before = serving.minted();

    let t_stub = resolve(&serving, "pmid:20").unwrap();
    let m = resolve(&serving, "doi:10.1000/m").unwrap();
    let u_stub = resolve(&serving, "pmid:21").unwrap();
    assert_eq!(resolve(&serving, "doi:10.1000/t"), Some(t_stub));
    assert_eq!(resolve(&serving, "pmc:PMC21"), Some(u_stub));
    let v = resolve(&serving, "doi:10.1000/v").unwrap();
    let v_stub = resolve(&serving, "pmid:30").unwrap();
    assert!(v < v_stub && serving.is_stub(v_stub));
    assert_eq!(resolve(&serving, "doi:10.1000/v3"), Some(v_stub));
    let c = resolve(&serving, "pmid:3").unwrap();
    let applied = incremental::apply(&store, &serving, batch(), Some(&fixture::key())).unwrap();
    assert_eq!(applied.batch, 2);
    assert_eq!((applied.records, applied.deleted), (7, 2));
    assert_eq!(applied.merged, 2, "F into E, the pmid:30 stub into V");
    assert_eq!(applied.dropped, 1, "I");
    // The pmid:30 stub merged into V; its DOI went to a stub M cites.
    assert_eq!(resolve(&serving, "pmid:30"), Some(v));
    assert_eq!(resolve(&serving, "pmc:PMC30"), Some(v));
    assert_eq!(serving.live(v_stub), Some(v));
    let v2 = resolve(&serving, "doi:10.1000/v2").unwrap();
    assert!(v2 != v && serving.is_stub(v2));
    assert_eq!(
        resolve(&serving, "doi:10.1000/v3"),
        None,
        "cited only beside kept names: dropped"
    );
    assert!(refs(&serving, m).iter().any(|(w, _)| *w == v2.get()));
    assert!(refs(&serving, m).iter().all(|(w, _)| *w != v.get()));
    assert!(refs(&serving, c).iter().any(|(w, _)| *w == v.get()));
    assert!(refs(&serving, c).iter().all(|(w, _)| *w != v2.get()));
    assert_eq!(
        applied.stubs_minted, 4,
        "W99, doi:new, T's leftover DOI, the merged stub's leftover DOI"
    );
    assert_eq!(applied.minted, 5, "D and the four stubs");
    assert_eq!(
        applied.names_dropped, 4,
        "U's PMC id, the merged stub's v3, H's deleted PMID, I's PMID"
    );
    assert_eq!(applied.public_minted, 5);
    assert!(applied.lexical.put >= 7, "{:?}", applied.lexical);
    assert_eq!(serving.watermark().batch, 2, "folded as it went");
    assert_eq!(serving.minted(), minted_before + 5);
    // T took its stub's id and PMID; the DOI went to a stub M cites.
    assert_eq!(resolve(&serving, "pmid:20"), Some(t_stub));
    assert!(!serving.is_stub(t_stub));
    let t_doi = resolve(&serving, "doi:10.1000/t").unwrap();
    assert!(t_doi != t_stub && serving.is_stub(t_doi));
    assert!(refs(&serving, m).contains(&(t_doi.get(), "crossref".to_owned())));
    assert!(refs(&serving, c).iter().any(|(w, _)| *w == t_stub.get()));
    assert!(refs(&serving, c).iter().all(|(w, _)| *w != t_doi.get()));
    let t_doi_row = serving.work(t_doi).unwrap().unwrap();
    let mut t_doi_citers = vec![(m, SourceId::Crossref), (v, SourceId::Pubmed)];
    t_doi_citers.sort_unstable();
    assert_eq!(
        pairs(&t_doi_row.names[0]),
        t_doi_citers,
        "M before the batch, and V's record in it"
    );
    let v2_row = serving.work(v2).unwrap().unwrap();
    let mut v2_citers = vec![(m, SourceId::Crossref), (u_stub, SourceId::Pubmed)];
    v2_citers.sort_unstable();
    assert_eq!(
        pairs(&v2_row.names[0]),
        v2_citers,
        "M before the batch, and U's reference beside B"
    );
    // U took its stub's id; the PMC id nobody cited alone is gone.
    assert!(!serving.is_stub(u_stub));
    assert_eq!(resolve(&serving, "pmc:PMC21"), None);

    // A gained OpenAlex's record and name; its text follows precedence.
    assert_eq!(resolve(&serving, "openalex:W1"), Some(a));
    let row = serving.work(a).unwrap().unwrap();
    assert_eq!(row.records.len(), 3);
    let t = row.text.as_ref().unwrap();
    assert_eq!(
        t.abstract_text.as_deref(),
        Some("Alpha abstract."),
        "PubMed first"
    );
    assert_eq!(
        t.authors,
        vec!["Ada L", "Bob M"],
        "Crossref first, from the old row"
    );
    assert_eq!(
        t.venue.as_deref(),
        Some("J Alpha (OA)"),
        "OpenAlex first, from the batch"
    );
    let w99 = resolve(&serving, "openalex:W99").unwrap();
    assert!(serving.is_stub(w99));
    assert_eq!(
        refs(&serving, a),
        vec![
            (b.get(), "pubmed+openalex".to_owned()),
            (w99.get(), "openalex".to_owned())
        ]
    );
    // B's re-deposit: it cites A and a new stub now.
    let new = resolve(&serving, "doi:10.1000/new").unwrap();
    assert_eq!(
        refs(&serving, b),
        vec![
            (a.get(), "crossref".to_owned()),
            (new.get(), "crossref".to_owned())
        ]
    );
    // D is new, and its reference gave S2 an alias; E named S2 beside B.
    let d = resolve(&serving, "pmid:4").unwrap();
    assert!(d.get() > minted_before);
    assert_eq!(resolve(&serving, "doi:10.1000/nine"), Some(s2));
    let s2_row = serving.work(s2).unwrap().unwrap();
    assert!(s2_row.stub);
    assert_eq!(s2_row.names.len(), 2);
    let mut s2_citers = vec![
        (c.get(), "pubmed".to_owned()),
        (d.get(), "pubmed".to_owned()),
        (e.get(), "openalex".to_owned()),
    ];
    s2_citers.sort();
    assert_eq!(citers(&serving, s2), s2_citers);
    // A stub name's citers are the works whose arcs landed on the stub
    // through it, a reference that named a held work beside it included.
    let cited_by = |id: WorkId, name: &str| -> Vec<(WorkId, SourceId)> {
        let name = OwnedName::parse(name).unwrap();
        let row = serving.work(id).unwrap().unwrap();
        let named = row.names.iter().find(|n| n.name == name).unwrap();
        pairs(named)
    };
    let mut pmid9 = vec![
        (c, SourceId::Pubmed),
        (d, SourceId::Pubmed),
        (e, SourceId::OpenAlex),
    ];
    pmid9.sort_unstable();
    assert_eq!(cited_by(s2, "pmid:9"), pmid9);
    assert_eq!(
        cited_by(s2, "doi:10.1000/nine"),
        vec![(d, SourceId::Pubmed)]
    );
    let s40 = resolve(&serving, "pmid:40").unwrap();
    let mut pmid40 = vec![(c, SourceId::Pubmed), (t_stub, SourceId::Pubmed)];
    pmid40.sort_unstable();
    assert_eq!(cited_by(s40, "pmid:40"), pmid40, "T named it beside M");
    assert_eq!(citers(&serving, s1).len(), 2);
    // F merged into E: the retired id answers as E, G cites E.
    assert_eq!(resolve(&serving, "pmid:6"), Some(e));
    assert_eq!(serving.live(f), Some(e));
    assert!(serving.work(f).unwrap().is_none());
    assert_eq!(refs(&serving, f), refs(&serving, e));
    assert_eq!(citers(&serving, e), vec![(g.get(), "pubmed".to_owned())]);
    assert_eq!(refs(&serving, g), vec![(e.get(), "pubmed".to_owned())]);
    let e_row = serving.work(e).unwrap().unwrap();
    assert_eq!(e_row.records.len(), 3);
    assert_eq!(
        e_row.text.as_ref().unwrap().abstract_text.as_deref(),
        Some("Epsilon abstract."),
        "PubMed's abstract, from F's old row"
    );
    // H lost its PubMed record; I is gone.
    let h_row = serving.work(h).unwrap().unwrap();
    assert_eq!(h_row.records.len(), 1);
    assert!(h_row.text.as_ref().unwrap().abstract_text.is_none());
    assert_eq!(resolve(&serving, "pmid:7"), None);
    assert!(serving.work(i).unwrap().is_none());
    assert_eq!(resolve(&serving, "pmid:8"), None);
    // The lexical index absorbed the batch.
    let hits = serving.search("delta", 10).unwrap();
    assert_eq!(hits.total, 1);
    assert_eq!(serving.search("iota", 10).unwrap().total, 0);
    assert_eq!(serving.public_of(d).map(|p| p.len()), Some(17));

    // A full merge over everything appends nothing and agrees.
    let registry_lines = journal::last(&store.journal()).unwrap().unwrap().registry;
    let mut all = base_records;
    all.extend(batch().records);
    let epoch = fixture::full_merge(
        &store,
        &all,
        &[
            (SourceId::Pubmed, "pmid:7", BATCH),
            (SourceId::Pubmed, "pmid:8", BATCH),
        ],
    );
    assert_eq!(epoch, 2);
    let after = journal::last(&store.journal()).unwrap().unwrap();
    assert_eq!(after.batch, 3);
    assert_eq!(after.registry, registry_lines, "zero churn");
    assert_eq!(
        (after.minted, after.merged, after.split, after.dropped),
        (0, 0, 0, 0)
    );
    let rebuilt = Serving::open(&store).unwrap();
    assert_eq!(rebuilt.watermark().batch, 3);
    let differences = compare::differences(&serving, &rebuilt, 20).unwrap();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    // And a second open of the old epoch would refuse to fold past the
    // full merge: the batch line is a compaction.
    std::fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn the_cases_a_full_merge_owns_stop_the_batch_before_it_writes() {
    use SourceId::Pubmed;
    let store = fixture::store("refuse");
    fixture::full_merge(&store, &base(), &[]);
    let serving = Serving::open(&store).unwrap();
    let journal_before = std::fs::read_to_string(store.journal()).unwrap();
    let registry_before = std::fs::read_to_string(store.registry()).unwrap();
    // A cited work losing its last record no longer stops the batch: it
    // becomes its citers' stub, tested beside the rule in
    // a_newer_copy_speaks_and_withdrawn_citations_leave_no_trace.
    // A record that drops a name no longer parts its work — the name
    // stays with the record (§8, 2026-09-18) — so the split a
    // re-delivery used to cause is tested where it belongs, beside the
    // rule, in a_newer_copy_speaks_and_withdrawn_citations_leave_no_trace.
    // A text only the normalized layer has: L loses the PubMed record
    // whose abstract it carried, and OpenAlex's was never in the row.
    let text_gone = Batch {
        deletions: vec![withdrawal(Pubmed, "pmid:13")],
        ..Batch::default()
    };
    let err = incremental::apply(&store, &serving, text_gone, None).unwrap_err();
    assert!(err.to_string().contains("normalized layer"), "{err}");
    // Nothing was written.
    assert_eq!(
        std::fs::read_to_string(store.journal()).unwrap(),
        journal_before
    );
    assert_eq!(
        std::fs::read_to_string(store.registry()).unwrap(),
        registry_before
    );
    assert_eq!(serving.watermark().batch, 1);
    // A split whose answer is an order over names the row does not hold.
    // Alpha2's reference names held work C and a stub, and §6 lands it on
    // the stub because its own text writes that DOI out; when a record
    // arrives for the stub's other name, the DOI is dropped — no
    // reference gave it without a held name beside it (merge plan §11,
    // item 14) — and §6 falls back to an order over C, which the row
    // never held. The batch refuses rather than guess (§11, item 28).
    let store = fixture::store("refuse-split");
    let mut both = record(
        Pubmed,
        &["pmid:90"],
        "Alpha2",
        view(false, 1, None),
        text(None, &["Alpha2"], None),
    );
    fixture::cites_as(
        &mut both,
        &["pmid:1", "doi:10.1000/p91"],
        "Bee A. 1999. https://doi.org/10.1000/p91",
    );
    let mut pair = record(
        Pubmed,
        &["pmid:92"],
        "Beta2",
        view(false, 1, None),
        text(None, &["Beta2"], None),
    );
    cites(&mut pair, &["pmid:91", "doi:10.1000/p91"]);
    fixture::full_merge(
        &store,
        &base_and(vec![delivered(both), delivered(pair)]),
        &[],
    );
    let serving = Serving::open(&store).unwrap();
    let arriving = delivered(record(
        Pubmed,
        &["pmid:91"],
        "Gamma2",
        view(false, 1, None),
        text(None, &["Gamma2"], None),
    ));
    let err = incremental::apply(
        &store,
        &serving,
        Batch {
            records: vec![arriving],
            ..Batch::default()
        },
        None,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("every name it gave there dropped"),
        "{err}"
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    let store = fixture::store("refuse");
    fixture::full_merge(&store, &base(), &[]);
    let serving = Serving::open(&store).unwrap();
    // A batch after a full merge the epoch does not have is refused.
    fixture::full_merge(&store, &base(), &[]);
    let err = incremental::apply(&store, &serving, Batch::default(), None).unwrap_err();
    assert!(err.to_string().contains("publish an epoch"), "{err}");
    std::fs::remove_dir_all(store.root()).unwrap();
}

#[test]
fn a_reader_opened_later_folds_the_same_and_stops_at_a_compaction() {
    let store = fixture::store("later");
    fixture::full_merge(&store, &base(), &[]);
    let serving = Serving::open(&store).unwrap();
    incremental::apply(&store, &serving, batch(), Some(&fixture::key())).unwrap();
    let later = Serving::open(&store).unwrap();
    assert_eq!(later.watermark().batch, 2);
    let differences = compare::differences(&serving, &later, 20).unwrap();
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    // A second batch: nothing new, but the clock advances and folds.
    let empty = incremental::apply(&store, &serving, Batch::default(), None).unwrap();
    assert_eq!(empty.batch, 3);
    assert_eq!(later.refresh().unwrap().folded, 1);
    assert_eq!(later.watermark().batch, 3);
    // A compaction: the old reader stops at the full line.
    let mut all = base();
    all.extend(batch().records);
    fixture::full_merge(
        &store,
        &all,
        &[
            (SourceId::Pubmed, "pmid:7", BATCH),
            (SourceId::Pubmed, "pmid:8", BATCH),
        ],
    );
    let advanced = later.refresh().unwrap();
    assert_eq!(advanced.folded, 0);
    assert!(matches!(
        advanced.stopped,
        Some(crate::fold::Stopped::Compaction(4))
    ));
    assert_eq!(later.watermark().batch, 3);
    std::fs::remove_dir_all(store.root()).unwrap();
}

/// Applies `batches` in order to a store merged in full from `base`, then
/// runs a full merge over every record and deletion and checks that it
/// appends nothing to the registry and answers exactly as the folded
/// batches do. Returns the store and the serving core for further checks;
/// the caller removes the store.
fn batches_equal_the_full_merge(
    name: &str,
    base: Vec<Incoming>,
    store_deletions: &[Deletion],
    batches: Vec<Batch>,
) -> (ingest::Store, Serving) {
    let store = fixture::store(name);
    let mut all = base;
    let mut deletions: Vec<(SourceId, String, String)> = store_deletions
        .iter()
        .map(|d| (d.source, d.native.to_string(), d.set.to_string()))
        .collect();
    {
        let first: Vec<(SourceId, &str, &str)> = deletions
            .iter()
            .map(|(s, n, set)| (*s, n.as_str(), set.as_str()))
            .collect();
        fixture::full_merge(&store, &all, &first);
    }
    let serving = Serving::open(&store).unwrap();
    for mut batch in batches {
        batch.store_deletions = store_deletions.to_vec();
        all.extend(batch.records.iter().cloned());
        deletions.extend(
            batch
                .deletions
                .iter()
                .map(|d| (d.source, d.native.to_string(), d.set.to_string())),
        );
        incremental::apply(&store, &serving, batch, Some(&fixture::key())).unwrap();
    }
    let before = journal::last(&store.journal()).unwrap().unwrap();
    let deletions: Vec<(SourceId, &str, &str)> = deletions
        .iter()
        .map(|(s, n, set)| (*s, n.as_str(), set.as_str()))
        .collect();
    fixture::full_merge(&store, &all, &deletions);
    let after = journal::last(&store.journal()).unwrap().unwrap();
    assert_eq!(after.registry, before.registry, "{name}: zero churn");
    assert_eq!(
        (after.minted, after.merged, after.split, after.dropped),
        (0, 0, 0, 0),
        "{name}"
    );
    let rebuilt = Serving::open(&store).unwrap();
    let differences = compare::differences(&serving, &rebuilt, 20).unwrap();
    assert!(
        differences.is_empty(),
        "{name}:\n{}",
        differences.join("\n")
    );
    (store, serving)
}

/// The base with more records.
fn base_and(extra: Vec<Incoming>) -> Vec<Incoming> {
    let mut all = base();
    all.extend(extra);
    all
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_split_gives_the_id_to_the_part_that_holds_the_records() {
    use SourceId::Pubmed;
    // Merge plan §11, item 3, decided 2026-09-19 over the corpus's
    // fourteen splits, every one of them one record-holding part and one
    // bare stub: the part that holds the records keeps the id, and the
    // part that is names only is minted as a stub.
    let store = fixture::store("split-keeps-the-id");
    let held = record(
        Pubmed,
        &["pmid:100", "doi:10.1000/q"],
        "Qoppa",
        view(false, 1, None),
        text(None, &["Qo"], None),
    );
    let mut reader = record(
        Pubmed,
        &["pmid:101"],
        "Qoppa reader",
        view(false, 1, None),
        text(None, &["Qr"], None),
    );
    cites(&mut reader, &["doi:10.1000/q"]);
    fixture::full_merge(&store, &base_and(vec![held, reader.clone()]), &[]);
    let before = {
        let serving = Serving::open(&store).unwrap();
        let id = resolve(&serving, "pmid:100").unwrap();
        assert_eq!(resolve(&serving, "doi:10.1000/q"), Some(id), "one work");
        id
    };
    // The record is delivered again without the DOI, and nothing else
    // holds it: the work keeps its id and the DOI, which one reader
    // cites alone, becomes a stub of its own.
    let narrowed = record(
        Pubmed,
        &["pmid:100"],
        "Qoppa",
        view(false, 1, None),
        text(None, &["Qo"], None),
    );
    fixture::full_merge(&store, &base_and(vec![narrowed, reader]), &[]);
    let line = journal::last(&store.journal()).unwrap().unwrap();
    assert_eq!(
        (line.minted, line.merged, line.split, line.dropped),
        (1, 0, 0, 0),
        "the stub alone is minted, and nothing is retired"
    );
    let serving = Serving::open(&store).unwrap();
    assert_eq!(resolve(&serving, "pmid:100"), Some(before), "kept its id");
    assert_eq!(serving.live(before), Some(before));
    let stub = resolve(&serving, "doi:10.1000/q").unwrap();
    assert_ne!(stub, before);
    assert!(serving.is_stub(stub));
    assert!(
        serving.split_into(before).is_empty(),
        "no split was written"
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_split_no_part_continues_is_retired_and_its_citers_resolve_again() {
    use SourceId::Pubmed;
    // The fallback of merge plan §11, item 3, which the corpus's
    // fourteen splits do not show and `pubmed26n1336` does: a stub whose
    // names two arriving records divide between them. Neither part is
    // the old work — both hold records — so the id is retired with no
    // successor and both parts are minted. The names no part took go to
    // a stub of their own where a reference cites them alone, and every
    // reference that landed on the stub is settled again by §6.
    let mut both = record(
        Pubmed,
        &["pmid:201"],
        "Sigma reader",
        view(false, 1, None),
        text(None, &["Sr"], None),
    );
    cites(&mut both, &["pmid:200", "pmc:PMC200", "doi:10.1000/s200"]);
    let mut alone = record(
        Pubmed,
        &["pmid:202"],
        "Sigma reader two",
        view(false, 1, None),
        text(None, &["Sr2"], None),
    );
    cites(&mut alone, &["doi:10.1000/s200"]);
    let one = delivered(record(
        Pubmed,
        &["pmid:200", "doi:10.1000/a200"],
        "Sigma",
        view(false, 1, None),
        text(None, &["Si"], None),
    ));
    let two = delivered(record(
        Pubmed,
        &["pmid:203", "pmc:PMC200", "doi:10.1000/b200"],
        "Tau two",
        view(false, 1, None),
        text(None, &["Ta"], None),
    ));
    let base = base_and(vec![delivered(both.clone()), delivered(alone.clone())]);
    // What the names are before the batch: one stub, the three of them
    // cited together.
    let before = fixture::store("split-before");
    fixture::full_merge(&before, &base, &[]);
    let stub = {
        let serving = Serving::open(&before).unwrap();
        let id = resolve(&serving, "pmid:200").unwrap();
        assert!(serving.is_stub(id));
        assert_eq!(resolve(&serving, "pmc:PMC200"), Some(id), "one stub");
        assert_eq!(resolve(&serving, "doi:10.1000/s200"), Some(id));
        id
    };
    std::fs::remove_dir_all(before.root()).unwrap();
    let (store, serving) = batches_equal_the_full_merge(
        "split-retires",
        base,
        &[],
        vec![Batch {
            records: vec![one, two],
            ..Batch::default()
        }],
    );
    assert_eq!(serving.live(stub), None, "retired with no successor");
    let w1 = resolve(&serving, "pmid:200").unwrap();
    let w2 = resolve(&serving, "pmc:PMC200").unwrap();
    let left = resolve(&serving, "doi:10.1000/s200").unwrap();
    assert!(!serving.is_stub(w1) && !serving.is_stub(w2) && w1 != w2);
    assert!(serving.is_stub(left));
    let mut into = serving.split_into(stub);
    into.sort_unstable();
    let mut parts = vec![w1, w2];
    parts.sort_unstable();
    assert_eq!(into, parts, "the retired id answers with its parts");
    // Serving plan §11, item 24: the public id that was out in the world
    // answers as what was once so.
    let public = serving.public_of(stub).unwrap();
    assert!(
        matches!(serving.resolve(&public).unwrap(), Some(Resolved::Split { id, into }) if id == stub && into == serving.split_into(stub))
    );
    // §6 over the names the reference gave: the PMID ranks before the
    // PMC id and the DOI, so the arc is the PMID's part's.
    let x = resolve(&serving, "pmid:201").unwrap();
    let y = resolve(&serving, "pmid:202").unwrap();
    let arcs = refs(&serving, x);
    assert!(
        arcs.iter().any(|(w, _)| *w == w1.get()),
        "the lowest namespace decides: {arcs:?}"
    );
    assert!(
        arcs.iter().all(|(w, _)| *w != w2.get() && *w != stub.get()),
        "and nothing is left pointing at the other part or the retired id: {arcs:?}"
    );
    assert_eq!(
        citers(&serving, left),
        vec![(y.get(), "pubmed".to_owned())],
        "the leftover name keeps the reader that cites it alone"
    );
    // The base answers as the tail does, which is the new names file.
    let rebuilt = Serving::open(&store).unwrap();
    assert_eq!(rebuilt.split_into(stub), serving.split_into(stub));
    assert_eq!(rebuilt.live(stub), None);
    drop(serving);
    drop(rebuilt);
    std::fs::remove_dir_all(store.root()).unwrap();
}

#[test]
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn a_newer_copy_speaks_and_withdrawn_citations_leave_no_trace() {
    use SourceId::{Crossref, Pubmed};
    // A record delivered again with a shorter reference list: B cited E and
    // a stub, and its new copy cites A alone. All three arcs stand, in
    // both merges: a record's citations are the union over its copies
    // (merge plan §8, 2026-09-18), because PubMed swaps a publisher's
    // reference list for PMC's shorter one when an article lands there.
    let mut with_b_citing = base();
    let b = with_b_citing
        .iter_mut()
        .find(|r| r.native == OwnedName::parse("doi:10.1000/b").unwrap())
        .unwrap();
    cites(b, &["doi:10.1000/e"]);
    cites(b, &["pmid:9"]);
    let mut b2 = delivered(record(
        Crossref,
        &["doi:10.1000/b"],
        "Beta",
        view(false, 1, None),
        text(None, &["Bea"], None),
    ));
    cites(&mut b2, &["pmid:1"]);
    let (store, serving) = batches_equal_the_full_merge(
        "copy-list",
        with_b_citing,
        &[],
        vec![Batch {
            records: vec![b2],
            ..Batch::default()
        }],
    );
    let b = resolve(&serving, "doi:10.1000/b").unwrap();
    let a = resolve(&serving, "pmid:1").unwrap();
    let e = resolve(&serving, "doi:10.1000/e").unwrap();
    let s2 = resolve(&serving, "pmid:9").unwrap();
    let mut from_b: Vec<u32> = refs(&serving, b).into_iter().map(|(w, _)| w).collect();
    from_b.sort_unstable();
    let mut expected = vec![a.get(), e.get(), s2.get()];
    expected.sort_unstable();
    assert_eq!(from_b, expected);
    assert_eq!(citers(&serving, e), vec![(b.get(), "crossref".to_owned())]);
    let stub = serving.work(s2).unwrap().unwrap();
    assert!(stub.names[0].cited_by.iter().any(|c| c.from == b));
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    // A later copy that adds a DOI: the name joins the work, with no id of
    // its own and no split.
    let p = record(
        Pubmed,
        &["pmid:70"],
        "Pi",
        view(false, 1, None),
        text(None, &["Pi"], None),
    );
    let p2 = delivered(record(
        Pubmed,
        &["pmid:70", "doi:10.1000/p70"],
        "Pi",
        view(false, 1, None),
        text(None, &["Pi"], None),
    ));
    let (store, serving) = batches_equal_the_full_merge(
        "copy-names",
        base_and(vec![p]),
        &[],
        vec![Batch {
            records: vec![p2],
            ..Batch::default()
        }],
    );
    assert_eq!(
        resolve(&serving, "doi:10.1000/p70"),
        resolve(&serving, "pmid:70")
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    // A later copy that drops a DOI: the name stays, because absence is
    // not an assertion (§8, 2026-09-18) — K keeps the DOI its OpenAlex
    // record gave before and gains the new one, where the work's names
    // used to part between two works and stop the batch.
    let k3 = delivered(record(
        SourceId::OpenAlex,
        &["openalex:W11", "doi:10.1000/k2"],
        "Kappa",
        view(true, 1, None),
        text(Some("Kappa abstract."), &["Kay O"], Some("Kappa Journal")),
    ));
    let (store, serving) = batches_equal_the_full_merge(
        "copy-kept-name",
        base(),
        &[],
        vec![Batch {
            records: vec![k3],
            ..Batch::default()
        }],
    );
    let k = resolve(&serving, "doi:10.1000/k").unwrap();
    assert_eq!(resolve(&serving, "doi:10.1000/k2"), Some(k));
    assert_eq!(resolve(&serving, "openalex:W11"), Some(k));
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    // A cited work losing its last record: it keeps its id and becomes
    // the stub its citers' references name, which the cited-as evidence
    // says name by name (serving plan §11, item 28, 2026-09-18). J is
    // cited by C through pmid:10, its only name.
    let (store, serving) = batches_equal_the_full_merge(
        "last-record",
        base(),
        &[],
        vec![Batch {
            deletions: vec![withdrawal(Pubmed, "pmid:10")],
            ..Batch::default()
        }],
    );
    let j = resolve(&serving, "pmid:10").unwrap();
    let c = resolve(&serving, "pmid:3").unwrap();
    let row = serving.work(j).unwrap().unwrap();
    assert!(row.stub, "{row:?}");
    assert_eq!(citers(&serving, j), vec![(c.get(), "pubmed".to_owned())]);
    assert!(pairs(&row.names[0]).contains(&(c, Pubmed)));
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    // A record arriving for some of a stub's names, where the citer used
    // a name the record kept *and* one it left, and its own citation text
    // writes the left one out: §6 asks the text before the order, so the
    // arc follows the DOI to the leftover stub even though the citer
    // still names the record's work and the order puts a PMID first
    // (serving plan §11, item 28, 2026-09-19).
    let mut reader = record(
        Pubmed,
        &["pmid:80"],
        "Chi",
        view(false, 1, None),
        text(None, &["Chi"], None),
    );
    fixture::cites_as(
        &mut reader,
        &["pmid:81", "doi:10.1000/p81"],
        "Psi O. 1999. https://doi.org/10.1000/P81",
    );
    let held = delivered(record(
        Pubmed,
        &["pmid:81"],
        "Psi",
        view(false, 1, None),
        text(None, &["Psi"], None),
    ));
    // And a second reader that gives the DOI alone, so the DOI is a name
    // of its own and not merely an assertion about Psi's work (merge plan
    // §11, item 14) — without it the DOI is dropped and the arc stays.
    let mut alone = record(
        Pubmed,
        &["pmid:82"],
        "Omega",
        view(false, 1, None),
        text(None, &["Omega"], None),
    );
    cites(&mut alone, &["doi:10.1000/p81"]);
    let (store, serving) = batches_equal_the_full_merge(
        "split-by-text",
        base_and(vec![delivered(reader), delivered(alone)]),
        &[],
        vec![Batch {
            records: vec![held],
            ..Batch::default()
        }],
    );
    let chi = resolve(&serving, "pmid:80").unwrap();
    let psi = resolve(&serving, "pmid:81").unwrap();
    let left = resolve(&serving, "doi:10.1000/p81").unwrap();
    assert!(!serving.is_stub(psi), "the record's own work");
    assert!(serving.is_stub(left) && left != psi, "the DOI was left");
    let arcs = refs(&serving, chi);
    assert!(
        arcs.iter().any(|(w, _)| *w == left.get()),
        "the text writes the DOI out, so the arc is the DOI's: {arcs:?}"
    );
    assert!(
        arcs.iter().all(|(w, _)| *w != psi.get()),
        "and is not the kept PMID's, whose order would otherwise win"
    );
    let row = serving.work(left).unwrap().unwrap();
    assert!(
        row.names[0].cited_by.iter().any(|c| c.in_text),
        "the stub it became keeps the mark, so it can be split again"
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    // A withdrawn record's citation of a stub another record still cites:
    // the stub keeps only the citer that remains.
    let mut xp = record(
        Pubmed,
        &["pmid:60", "doi:10.1000/p60"],
        "Pi",
        view(false, 1, None),
        text(None, &["Pi"], None),
    );
    cites(&mut xp, &["pmid:61"]);
    let xc = record(
        Crossref,
        &["doi:10.1000/p60"],
        "Pi",
        view(false, 1, None),
        text(None, &["Pi C"], None),
    );
    let mut r = record(
        Pubmed,
        &["pmid:62"],
        "Rho",
        view(false, 1, None),
        text(None, &["Rho"], None),
    );
    cites(&mut r, &["pmid:61"]);
    let (store, serving) = batches_equal_the_full_merge(
        "withdrawn-citer",
        base_and(vec![xp, xc, r]),
        &[],
        vec![Batch {
            deletions: vec![withdrawal(Pubmed, "pmid:60")],
            ..Batch::default()
        }],
    );
    let stub = serving
        .work(resolve(&serving, "pmid:61").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(
        pairs(&stub.names[0]),
        vec![(resolve(&serving, "pmid:62").unwrap(), Pubmed)]
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    // The same, and then a record takes the stub over by its PMID: the
    // withdrawn citer does not keep the stub's DOI alive, so no stub is
    // minted for it and no arc drawn from the withdrawn record's work.
    let mut c1 = record(
        Crossref,
        &["doi:10.1000/c81"],
        "Cee",
        view(false, 1, None),
        text(None, &["Cee"], None),
    );
    cites(&mut c1, &["pmid:81", "doi:10.1000/s81"]);
    let mut xp = record(
        Pubmed,
        &["pmid:82", "doi:10.1000/x82"],
        "Ex",
        view(false, 1, None),
        text(None, &["Ex"], None),
    );
    cites(&mut xp, &["doi:10.1000/s81"]);
    let xc = record(
        Crossref,
        &["doi:10.1000/x82"],
        "Ex",
        view(false, 1, None),
        text(None, &["Ex C"], None),
    );
    let mut takes_over = record(
        Pubmed,
        &["pmid:81"],
        "Ar",
        view(false, 1, None),
        text(None, &["Ar"], None),
    );
    takes_over.view.set = "batch2".into();
    let (store, serving) = batches_equal_the_full_merge(
        "withdrawn-then-taken-over",
        base_and(vec![c1, xp, xc]),
        &[],
        vec![
            Batch {
                deletions: vec![withdrawal(Pubmed, "pmid:82")],
                ..Batch::default()
            },
            Batch {
                records: vec![takes_over],
                ..Batch::default()
            },
        ],
    );
    let x = resolve(&serving, "doi:10.1000/x82").unwrap();
    assert!(
        refs(&serving, x).is_empty(),
        "no arc from the withdrawn record"
    );
    assert_eq!(resolve(&serving, "doi:10.1000/s81"), None);
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();
    // A batch of an older set than a deletions set the store holds: the
    // store's deletion withdraws the batch's copy too, as the full merge's
    // rule says, so the copy does not take the record's stub over.
    let p = record(
        Pubmed,
        &["pmid:90"],
        "Old",
        view(false, 1, None),
        text(None, &["Old"], None),
    );
    let mut cites_p = record(
        Crossref,
        &["doi:10.1000/cites90"],
        "Citing",
        view(false, 1, None),
        text(None, &["Cit"], None),
    );
    cites(&mut cites_p, &["pmid:90"]);
    let later_deletion = Deletion {
        source: Pubmed,
        native: OwnedName::parse("pmid:90").unwrap(),
        set: "zdeleted".into(),
    };
    let older_copy = delivered(record(
        Pubmed,
        &["pmid:90"],
        "Old",
        view(false, 1, None),
        text(None, &["Old"], None),
    ));
    let (store, serving) = batches_equal_the_full_merge(
        "withdrawn-by-the-store",
        base_and(vec![p, cites_p]),
        &[later_deletion],
        vec![Batch {
            records: vec![older_copy],
            ..Batch::default()
        }],
    );
    let stub = resolve(&serving, "pmid:90").unwrap();
    assert!(serving.is_stub(stub), "still only cited");
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_records_several_references_to_a_row_divide_one_at_a_time() {
    use SourceId::{Crossref, Pubmed};
    // Serving plan §11, item 25, traced 2026-09-19: the churn `1339`
    // left. A record that cites one work twice is one citer in a stub's
    // row and one arc in the graph, and the batch used to read its
    // references' names together — so a reference that gave only names
    // no part took looked like one that had also given a name a part
    // took, and made no stub name. Its leftovers then reached their
    // stubs by other citers alone, one each, where the full merge makes
    // the one stub that reference's own names are. The cited-as row
    // says which namespace landed each reference, which is as near as
    // the evidence comes to telling them apart.
    //
    // W is one work of a PubMed record with a wrong link and Crossref's
    // record of the DOI. The PubMed record carries no text, so
    // withdrawing it moves no promoted field.
    let wrong = record(
        Pubmed,
        &["pmid:500", "doi:10.1000/w5", "pmc:PMC500"],
        "Wrong link",
        view(false, 0, None),
        text(None, &[], None),
    );
    let right = record(
        Crossref,
        &["doi:10.1000/w5"],
        "Wrong link",
        view(false, 1, None),
        text(None, &["W"], None),
    );
    let mut by_pmid = record(
        Pubmed,
        &["pmid:510"],
        "Cites the PMID",
        view(false, 1, None),
        text(None, &["P"], None),
    );
    cites(&mut by_pmid, &["pmid:500"]);
    let mut by_pmc = record(
        Pubmed,
        &["pmid:511"],
        "Cites the PMC id",
        view(false, 1, None),
        text(None, &["Q"], None),
    );
    cites(&mut by_pmc, &["pmc:PMC500"]);
    // The record that cites W twice: once by the DOI the Crossref record
    // keeps, once by the two names the withdrawal leaves behind.
    let mut twice = record(
        Pubmed,
        &["pmid:512"],
        "Cites it twice",
        view(false, 1, None),
        text(None, &["R"], None),
    );
    cites(&mut twice, &["doi:10.1000/w5"]);
    cites(&mut twice, &["pmid:500", "pmc:PMC500"]);
    let (store, serving) = batches_equal_the_full_merge(
        "two-references",
        base_and(vec![wrong, right, by_pmid, by_pmc, twice]),
        &[],
        vec![Batch {
            deletions: vec![withdrawal(Pubmed, "pmid:500")],
            ..Batch::default()
        }],
    );
    let w = resolve(&serving, "doi:10.1000/w5").unwrap();
    let stub = resolve(&serving, "pmid:500").unwrap();
    assert!(serving.is_stub(stub));
    assert_eq!(
        resolve(&serving, "pmc:PMC500"),
        Some(stub),
        "one stub, not one for each name: the reference that gave both is one"
    );
    let r = resolve(&serving, "pmid:512").unwrap();
    let mut arcs: Vec<u32> = refs(&serving, r).into_iter().map(|(x, _)| x).collect();
    arcs.sort_unstable();
    let mut both = vec![w.get(), stub.get()];
    both.sort_unstable();
    assert_eq!(
        arcs, both,
        "its other reference keeps its arc on the work it still names"
    );
    let row = serving.work(stub).unwrap().unwrap();
    for name in &row.names {
        assert!(
            pairs(name).contains(&(r, Pubmed)),
            "{}: {:?}",
            name.name,
            pairs(name)
        );
    }
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_reference_that_named_another_held_work_makes_no_stub_name() {
    use SourceId::{Crossref, Pubmed};
    // Serving plan §11, item 26's blind spot, closed 2026-09-19. Merge
    // plan §11, item 14: a reference that names a held work makes no
    // name of its own, its other names being assertions about that work.
    // The full merge asks that of every reference; the batch could only
    // ask it of the names the row it is dividing holds, and assumed no
    // held work stood anywhere else — so a name the full merge drops, or
    // leaves to another citer, became a stub of its own. The cited-as
    // row now carries the mark, and the batch reads it.
    //
    // The shape: W is a PubMed record's wrong link and Crossref's record
    // of the DOI; V is a held work of its own; one record cites W's PMC
    // id beside V's DOI, and another cites the PMC id alone.
    let wrong = record(
        Pubmed,
        &["pmid:700", "doi:10.1000/w7", "pmc:PMC700"],
        "Wrong link",
        view(false, 0, None),
        text(None, &[], None),
    );
    let right = record(
        Crossref,
        &["doi:10.1000/w7"],
        "Wrong link",
        view(false, 1, None),
        text(None, &["W"], None),
    );
    let other = record(
        Crossref,
        &["doi:10.1000/v7"],
        "Another held work",
        view(false, 1, None),
        text(None, &["V"], None),
    );
    let mut beside = record(
        Pubmed,
        &["pmid:712"],
        "Names both",
        view(false, 1, None),
        text(None, &["B"], None),
    );
    cites(&mut beside, &["pmc:PMC700", "doi:10.1000/v7"]);
    let mut alone = record(
        Pubmed,
        &["pmid:713"],
        "Names it alone",
        view(false, 1, None),
        text(None, &["A"], None),
    );
    cites(&mut alone, &["pmc:PMC700"]);
    let (store, serving) = batches_equal_the_full_merge(
        "beside-a-held-work",
        base_and(vec![wrong, right, other, beside, alone]),
        &[],
        vec![Batch {
            deletions: vec![withdrawal(Pubmed, "pmid:700")],
            ..Batch::default()
        }],
    );
    let stub = resolve(&serving, "pmc:PMC700").unwrap();
    assert!(serving.is_stub(stub));
    let b = resolve(&serving, "pmid:712").unwrap();
    let a = resolve(&serving, "pmid:713").unwrap();
    let row = serving.work(stub).unwrap().unwrap();
    let marks: Vec<(WorkId, bool)> = row.names[0]
        .cited_by
        .iter()
        .map(|c| (c.from, c.held_beside))
        .collect();
    assert!(
        marks.contains(&(b, true)),
        "the reference that named another held work is marked: {marks:?}"
    );
    assert!(
        marks.contains(&(a, false)),
        "and the one that named nothing else is not: {marks:?}"
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn the_divisions_a_row_cannot_settle_stop_the_batch() {
    use SourceId::{Crossref, Pubmed};
    // Two shapes the row's own evidence cannot answer, both found
    // 2026-09-19 with the marks of items 25 and 26. A refusal is the
    // answer §5 gives: the normalized layer has the reference lists.
    //
    // One: a reference that named a held work besides this row, and
    // whose only name here item 14 therefore drops. Where its arc goes
    // is that other work's, and the row cannot name it — the same wall
    // `split_arc` already meets from the other side.
    let store = fixture::store("refuse-beside-held");
    let wrong = record(
        Pubmed,
        &["pmid:600", "doi:10.1000/w6", "pmc:PMC600"],
        "Wrong link",
        view(false, 0, None),
        text(None, &[], None),
    );
    let right = record(
        Crossref,
        &["doi:10.1000/w6"],
        "Wrong link",
        view(false, 1, None),
        text(None, &["W"], None),
    );
    let other = record(
        Crossref,
        &["doi:10.1000/v6"],
        "Another held work",
        view(false, 1, None),
        text(None, &["V"], None),
    );
    let mut beside = record(
        Pubmed,
        &["pmid:612"],
        "Names both",
        view(false, 1, None),
        text(None, &["B"], None),
    );
    cites(&mut beside, &["pmc:PMC600", "doi:10.1000/v6"]);
    fixture::full_merge(&store, &base_and(vec![wrong, right, other, beside]), &[]);
    let serving = Serving::open(&store).unwrap();
    let registry_before = std::fs::read_to_string(store.registry()).unwrap();
    let err = incremental::apply(
        &store,
        &serving,
        Batch {
            deletions: vec![withdrawal(Pubmed, "pmid:600")],
            ..Batch::default()
        },
        None,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("every name it gave there dropped"),
        "{err}"
    );
    assert_eq!(
        std::fs::read_to_string(store.registry()).unwrap(),
        registry_before
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();

    // Two: a record that cited the row twice through the same namespace,
    // the division parting the names its two references gave. The
    // landing tells references apart only where they landed by different
    // namespaces; here they did not, and which gave which is only in
    // their own lists.
    let store = fixture::store("refuse-two-references");
    let wrong = record(
        Pubmed,
        &["pmid:800", "doi:10.1000/w8", "pmc:PMC800"],
        "Wrong link",
        view(false, 0, None),
        text(None, &[], None),
    );
    let right = record(
        Crossref,
        &["doi:10.1000/w8"],
        "Wrong link",
        view(false, 1, None),
        text(None, &["W"], None),
    );
    let mut twice = record(
        Pubmed,
        &["pmid:812"],
        "Cites it twice by the PMID",
        view(false, 1, None),
        text(None, &["R"], None),
    );
    cites(&mut twice, &["pmid:800", "doi:10.1000/w8"]);
    cites(&mut twice, &["pmid:800", "pmc:PMC800"]);
    fixture::full_merge(&store, &base_and(vec![wrong, right, twice]), &[]);
    let serving = Serving::open(&store).unwrap();
    let registry_before = std::fs::read_to_string(store.registry()).unwrap();
    let err = incremental::apply(
        &store,
        &serving,
        Batch {
            deletions: vec![withdrawal(Pubmed, "pmid:800")],
            ..Batch::default()
        },
        None,
    )
    .unwrap_err();
    assert!(err.to_string().contains("more than once through"), "{err}");
    assert_eq!(
        std::fs::read_to_string(store.registry()).unwrap(),
        registry_before
    );
    drop(serving);
    std::fs::remove_dir_all(store.root()).unwrap();
}
