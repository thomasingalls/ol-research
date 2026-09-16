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
use crate::incremental::{self, Batch, Error, Incoming};

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
        records: vec![a3, b2, d, ef, t, u, nu],
        deletions: vec![
            (Pubmed, OwnedName::parse("pmid:7").unwrap()),
            (Pubmed, OwnedName::parse("pmid:8").unwrap()),
        ],
        inputs: vec!["batch-1".into()],
    }
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
        t_doi_row.names[0].cited_by.to_vec(),
        t_doi_citers,
        "M before the batch, and V's record in it"
    );
    let v2_row = serving.work(v2).unwrap().unwrap();
    let mut v2_citers = vec![(m, SourceId::Crossref), (u_stub, SourceId::Pubmed)];
    v2_citers.sort_unstable();
    assert_eq!(
        v2_row.names[0].cited_by.to_vec(),
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
        named.cited_by.to_vec()
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
        &[(SourceId::Pubmed, "pmid:7"), (SourceId::Pubmed, "pmid:8")],
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
    use SourceId::{OpenAlex, Pubmed};
    let store = fixture::store("refuse");
    fixture::full_merge(&store, &base(), &[]);
    let serving = Serving::open(&store).unwrap();
    let journal_before = std::fs::read_to_string(store.journal()).unwrap();
    let registry_before = std::fs::read_to_string(store.registry()).unwrap();
    // A cited work losing its last record.
    let deleted = Batch {
        deletions: vec![(Pubmed, OwnedName::parse("pmid:10").unwrap())],
        ..Batch::default()
    };
    let err = incremental::apply(&store, &serving, deleted, None).unwrap_err();
    assert!(matches!(err, Error::FullMerge(_)), "{err}");
    assert!(err.to_string().contains("still cited"), "{err}");
    // A name moving between works: OpenAlex's record of K re-delivered
    // with another DOI, so K's names part between two works — a split.
    let mut split = Batch::default();
    split.records.push(record(
        OpenAlex,
        &["openalex:W11", "doi:10.1000/k2"],
        "Kappa",
        view(true, 1, None),
        text(Some("Kappa abstract."), &["Kay O"], None),
    ));
    let err = incremental::apply(&store, &serving, split, None).unwrap_err();
    assert!(err.to_string().contains("split"), "{err}");
    // A text only the normalized layer has: L loses the PubMed record
    // whose abstract it carried, and OpenAlex's was never in the row.
    let text_gone = Batch {
        deletions: vec![(Pubmed, OwnedName::parse("pmid:13").unwrap())],
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
        &[(SourceId::Pubmed, "pmid:7"), (SourceId::Pubmed, "pmid:8")],
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
