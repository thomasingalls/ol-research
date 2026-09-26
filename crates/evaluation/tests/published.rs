//! The published sets' own files read whole, when `OL_RHT_EVALUATION`
//! names the directory the fetch Job fills (`raw/evaluation/` of the
//! store, or a copy): each delivery is a fixed artifact, so its counts
//! are explicit cases here, taken from the files on 2026-09-26.

use std::path::PathBuf;

use evaluation::sets::{first_per_seed, read_pm_topics, read_relish_gz};
use evaluation::trec::Key;

fn root() -> Option<PathBuf> {
    std::env::var_os("OL_RHT_EVALUATION").map(PathBuf::from)
}

#[test]
fn trec_pm_2019_reads_whole() {
    let Some(root) = root() else {
        eprintln!("OL_RHT_EVALUATION not set; skipping");
        return;
    };
    let dir = root.join("trec-pm/2019");
    let topics =
        read_pm_topics(&std::fs::read_to_string(dir.join("topics2019.xml")).unwrap()).unwrap();
    assert_eq!(topics.year, 2019);
    assert_eq!(topics.topics.len(), 40);
    assert!(
        topics
            .topics
            .iter()
            .all(|t| t.treatment.is_none() && t.demographic.is_some())
    );
    let key =
        Key::read(&std::fs::read_to_string(dir.join("qrels-treceval-abstracts.2019.txt")).unwrap())
            .unwrap();
    assert_eq!(key.queries.len(), 40);
    let judged: usize = key
        .queries
        .values()
        .map(std::collections::BTreeMap::len)
        .sum();
    assert_eq!(judged, 18_316);
    let strict = key
        .queries
        .values()
        .filter(|j| j.values().any(|g| *g >= 2))
        .count();
    // Topics 5 and 6 have nothing definitely relevant.
    assert_eq!(strict, 38);
}

#[test]
fn trec_pm_2020_reads_whole() {
    let Some(root) = root() else {
        eprintln!("OL_RHT_EVALUATION not set; skipping");
        return;
    };
    let dir = root.join("trec-pm/2020");
    let topics =
        read_pm_topics(&std::fs::read_to_string(dir.join("topics2020.xml")).unwrap()).unwrap();
    assert_eq!(topics.year, 2020);
    assert_eq!(topics.topics.len(), 40);
    assert!(topics.topics.iter().all(|t| t.treatment.is_some()));
    let key = Key::read(
        &std::fs::read_to_string(dir.join("qrels-reduced-phase1-treceval-2020.txt")).unwrap(),
    )
    .unwrap();
    // Nine topics dropped for two or fewer relevant (NIST's page).
    assert_eq!(key.queries.len(), 31);
    let judged: usize = key
        .queries
        .values()
        .map(std::collections::BTreeMap::len)
        .sum();
    assert_eq!(judged, 17_366);
}

#[test]
fn relish_v1_reads_whole() {
    let Some(root) = root() else {
        eprintln!("OL_RHT_EVALUATION not set; skipping");
        return;
    };
    let records =
        read_relish_gz(&std::fs::read(root.join("relish/v1/RELISH_v1.json.gz")).unwrap()).unwrap();
    assert_eq!(records.len(), 3_278);
    assert!(records.iter().all(|r| r.judgments().len() == 60));
    let (kept, dropped) = first_per_seed(records);
    assert_eq!(kept.len(), 3_194);
    assert_eq!(dropped.len(), 84);
}
