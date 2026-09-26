//! This crate's arithmetic against NIST's `trec_eval` (evaluation plan
//! §5), when `TREC_EVAL` names the binary: generated answer keys and
//! runs — grades below, at and above the level, unjudged works, equal
//! scores, queries with no relevant work, queries only one file has —
//! written as files and scored by both, every per-query value required to
//! match what `trec_eval -q` prints.
//!
//! `trec_eval` prints four decimals, so a value matches when it is within
//! half of the last printed place.
//!
//! It must be `trec_eval` 10.0 (<https://github.com/usnistgov/trec_eval>, tag
//! `v10.0`), built with `make`: 9.0.8 reads scores as `float` and prints
//! nothing for a query the run lacks under `-c`. What is kept out of the
//! generated files is what `trec_eval` itself mishandles or refuses — a
//! query whose every grade is negative (its nDCG reads the previous
//! query's grade levels), scores that are not numbers, a work twice in a
//! query, blank and comment lines — and a query with no relevant work is
//! not compared, since this crate leaves it out on purpose.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use evaluation::measure::{self, DEPTHS, Measure};
use evaluation::trec::{Key, Run, Scored};
use proptest::prelude::*;
use proptest::test_runner::{TestCaseError, TestRunner};

/// Half the last place `trec_eval` prints, and a hair for the rounding.
const PRINTED: f64 = 0.000_05 + 1e-12;

fn binary() -> Option<PathBuf> {
    std::env::var_os("TREC_EVAL").map(PathBuf::from)
}

/// A fresh pair of file names under the test's scratch directory.
fn scratch() -> (PathBuf, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("trec_eval");
    std::fs::create_dir_all(&dir).unwrap();
    let stem = format!("{}-{n}", std::process::id());
    (
        dir.join(format!("{stem}.qrels")),
        dir.join(format!("{stem}.run")),
    )
}

/// Runs `trec_eval` over the two files and reads its per-query lines:
/// by measure name, by query, the value.
fn scored_by_trec_eval(
    bin: &Path,
    key: &Key,
    run: &Run,
    args: &[&str],
) -> BTreeMap<(String, String), f64> {
    let (qrels, results) = scratch();
    let mut text = Vec::new();
    key.write(&mut text).unwrap();
    std::fs::write(&qrels, text).unwrap();
    let mut text = Vec::new();
    run.write("generated", &mut text).unwrap();
    std::fs::write(&results, text).unwrap();
    let output = Command::new(bin)
        .args(args)
        .arg(&qrels)
        .arg(&results)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "trec_eval {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_file(&qrels).unwrap();
    std::fs::remove_file(&results).unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| {
            let [name, query, value] = line.split_whitespace().collect::<Vec<_>>()[..] else {
                panic!("a trec_eval line of other than three columns: {line:?}");
            };
            let value = value.parse::<f64>().ok()?;
            Some(((name.to_owned(), query.to_owned()), value))
        })
        .collect()
}

/// Works from a small alphabet, so that runs and keys meet; scores from
/// a few values, so that equal scores are common.
fn arb_run() -> impl Strategy<Value = Run> {
    prop::collection::btree_map(
        "q[0-5]",
        prop::collection::btree_map(
            "w[a-z][0-2]?",
            prop::sample::select(vec![-0.0, 0.0, 0.5, 1.0, 2.0, 2.5, 7.25]),
            0..70,
        ),
        0..6,
    )
    .prop_map(|queries| {
        let mut run = Run::default();
        for (query, works) in queries {
            let mut scored: Vec<Scored> = works
                .into_iter()
                .map(|(work, score)| Scored { work, score })
                .collect();
            evaluation::trec::order(&mut scored);
            run.queries.insert(query, scored);
        }
        run
    })
}

fn arb_key() -> impl Strategy<Value = Key> {
    prop::collection::btree_map(
        "q[0-5]",
        prop::collection::btree_map("w[a-z][0-2]?", -1_i64..=3, 1..40)
            .prop_filter("a query with a grade of 0 or more", |judged| {
                judged.values().any(|g| *g >= 0)
            }),
        1..6,
    )
    .prop_map(|queries| Key { queries })
}

/// The measures `trec_eval` names, and the arguments that ask for them.
fn named() -> (Vec<Measure>, Vec<String>) {
    let measures: Vec<Measure> = Measure::report()
        .into_iter()
        .filter(|m| m.trec_eval().is_some())
        .collect();
    let depths = DEPTHS.map(|k| k.to_string()).join(",");
    let args = [
        format!("P.{depths}"),
        format!("recall.{depths}"),
        "Rprec".to_owned(),
        "set_P".to_owned(),
        "set_recall".to_owned(),
        "set_F".to_owned(),
        "ndcg_cut.10".to_owned(),
        "recip_rank".to_owned(),
    ]
    .into_iter()
    .flat_map(|m| ["-m".to_owned(), m])
    .collect();
    (measures, args)
}

/// Generated cases per property.
const CASES: u32 = 256;

/// Every measure `trec_eval` has, query by query.
#[test]
fn the_measures_are_trec_evals() {
    let Some(bin) = binary() else {
        eprintln!("TREC_EVAL not set; skipping");
        return;
    };
    let (measures, named_args) = named();
    let mut runner = TestRunner::new(ProptestConfig::with_cases(CASES));
    runner
        .run(&(arb_key(), arb_run(), 1_i64..=3), |(key, run, level)| {
            let level_text = level.to_string();
            let mut args: Vec<&str> = vec!["-c", "-q", "-l", &level_text];
            args.extend(named_args.iter().map(String::as_str));
            let theirs = scored_by_trec_eval(&bin, &key, &run, &args);
            let ours = measure::evaluate(&key, &run, level, &measures);
            for (m, measure) in measures.iter().enumerate() {
                let name = measure.trec_eval().unwrap();
                for (q, query) in ours.queries.iter().enumerate() {
                    let Some(their) = theirs.get(&(name.clone(), query.clone())) else {
                        return Err(TestCaseError::fail(format!(
                            "trec_eval printed no {name} for query {query}"
                        )));
                    };
                    let our = ours.columns[m][q];
                    prop_assert!(
                        (our - their).abs() <= PRINTED,
                        "{name} query {query}: ours {our}, trec_eval {their}"
                    );
                }
            }
            Ok(())
        })
        .unwrap();
}

/// F at a depth, the judged share and the high bound, which `trec_eval`
/// does not name, from the counts it prints for the first *k* alone
/// (`-M k`).
#[test]
fn the_depth_measures_follow_trec_evals_counts() {
    let Some(bin) = binary() else {
        eprintln!("TREC_EVAL not set; skipping");
        return;
    };
    let mut runner = TestRunner::new(ProptestConfig::with_cases(CASES));
    runner
        .run(&(arb_key(), arb_run(), 1_i64..=3), |(key, run, level)| {
            let level_text = level.to_string();
            for k in DEPTHS {
                let measures = [
                    Measure::Precision(k),
                    Measure::Recall(k),
                    Measure::F(k),
                    Measure::Judged(k),
                    Measure::PrecisionIfUnjudgedRelevant(k),
                ];
                let depth = k.to_string();
                let theirs = scored_by_trec_eval(
                    &bin,
                    &key,
                    &run,
                    &[
                        "-c",
                        "-q",
                        "-l",
                        &level_text,
                        "-M",
                        &depth,
                        "-m",
                        "num_ret",
                        "-m",
                        "num_rel",
                        "-m",
                        "num_rel_ret",
                        "-m",
                        "num_nonrel_judged_ret",
                    ],
                );
                let ours = measure::evaluate(&key, &run, level, &measures);
                #[allow(clippy::cast_precision_loss)]
                let k_f = k as f64;
                for (q, query) in ours.queries.iter().enumerate() {
                    let count = |name: &str| {
                        theirs
                            .get(&(name.to_owned(), query.clone()))
                            .copied()
                            .unwrap_or_else(|| {
                                panic!("trec_eval printed no {name} for query {query}")
                            })
                    };
                    let (returned, relevant, found, judged_not) = (
                        count("num_ret"),
                        count("num_rel"),
                        count("num_rel_ret"),
                        count("num_nonrel_judged_ret"),
                    );
                    let p = found / k_f;
                    let r = found / relevant;
                    let f = if p + r == 0.0 {
                        0.0
                    } else {
                        2.0 * p * r / (p + r)
                    };
                    let expected = [
                        p,
                        r,
                        f,
                        (found + judged_not) / k_f,
                        (returned - judged_not) / k_f,
                    ];
                    for (m, want) in expected.iter().enumerate() {
                        let got = ours.columns[m][q];
                        prop_assert!(
                            (got - want).abs() <= 1e-12,
                            "{} query {query}: ours {got}, from trec_eval's counts {want}",
                            measures[m]
                        );
                    }
                }
            }
            Ok(())
        })
        .unwrap();
}
