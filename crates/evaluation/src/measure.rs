//! The measures (evaluation plan §3.3), per query, as `trec_eval`
//! computes them (§5): precision, recall and F at a depth, R-precision,
//! the same three over everything returned, and as a side column nDCG
//! and reciprocal rank; beside precision, the share of the first *k*
//! anyone judged and precision had every unjudged work been relevant.
//!
//! A work is relevant at a level when a judge gave it that grade or
//! higher (`trec_eval -l`); a work the key does not name for a query is
//! unjudged, and so is one it gives a negative grade — TREC's "in the
//! pool, not judged", which `trec_eval` counts as neither relevant nor
//! judged not relevant.
//!
//! The conventions are `trec_eval` 10.0's, read from its source
//! (evaluation plan §5), with one difference on purpose: a query with no
//! relevant work at a level is left out of that level's averages, where
//! `trec_eval` averages it in at 0.

use std::collections::BTreeMap;
use std::fmt;

use crate::interval::{self, Interval};
use crate::trec::{Key, Run, Scored};

/// The depths a run is measured at (decided 2026-09-26): what is read
/// first, the web client's page, what a reranker could still reach.
pub const DEPTHS: [usize; 3] = [10, 50, 1000];

/// One measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Measure {
    /// Relevant works in the first *k*, over *k* (`P_k`).
    Precision(usize),
    /// Relevant works in the first *k*, over the relevant works the key
    /// knows (`recall_k`).
    Recall(usize),
    /// The harmonic mean of the two, 0 when both are.
    F(usize),
    /// Precision in the first *k* had every unjudged work been relevant:
    /// the high bound beside [`Measure::Precision`]'s low one.
    PrecisionIfUnjudgedRelevant(usize),
    /// Works in the first *k* the key judged either way, over *k*: the
    /// relevant and those judged not relevant (`num_rel_ret` and
    /// `num_nonrel_judged_ret` at `-M k`).
    Judged(usize),
    /// Precision in the first *R*, *R* the relevant works the key knows
    /// (`Rprec`).
    RPrecision,
    /// Relevant works returned over works returned (`set_P`).
    SetPrecision,
    /// Relevant works returned over relevant works known (`set_recall`).
    SetRecall,
    /// Their harmonic mean (`set_F`).
    SetF,
    /// Normalized discounted cumulative gain in the first *k*, the grade
    /// as the gain (`ndcg_cut_k`). Graded, so the same at every level, as
    /// `trec_eval` ignores `-l` for it.
    Ndcg(usize),
    /// One over the rank of the first relevant work, 0 with none
    /// (`recip_rank`).
    ReciprocalRank,
}

impl Measure {
    /// The measures every report prints, in its order: the headline at
    /// each depth, then R-precision and the set measures, then the side
    /// column.
    #[must_use]
    pub fn report() -> Vec<Self> {
        let mut all = Vec::new();
        for k in DEPTHS {
            all.extend([
                Self::Precision(k),
                Self::PrecisionIfUnjudgedRelevant(k),
                Self::Judged(k),
                Self::Recall(k),
                Self::F(k),
            ]);
        }
        all.extend([
            Self::RPrecision,
            Self::SetPrecision,
            Self::SetRecall,
            Self::SetF,
            Self::Ndcg(10),
            Self::ReciprocalRank,
        ]);
        all
    }

    /// The name `trec_eval` prints for it, where it has one.
    #[must_use]
    pub fn trec_eval(self) -> Option<String> {
        match self {
            Self::Precision(k) => Some(format!("P_{k}")),
            Self::Recall(k) => Some(format!("recall_{k}")),
            Self::RPrecision => Some("Rprec".to_owned()),
            Self::SetPrecision => Some("set_P".to_owned()),
            Self::SetRecall => Some("set_recall".to_owned()),
            Self::SetF => Some("set_F".to_owned()),
            Self::Ndcg(k) => Some(format!("ndcg_cut_{k}")),
            Self::ReciprocalRank => Some("recip_rank".to_owned()),
            Self::F(_) | Self::PrecisionIfUnjudgedRelevant(_) | Self::Judged(_) => None,
        }
    }

    /// Whether more is better, so that a drop beyond its interval is a
    /// change for the worse. The judged share is neither.
    #[must_use]
    pub const fn higher_is_better(self) -> bool {
        !matches!(self, Self::Judged(_))
    }
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Precision(k) => write!(f, "P@{k}"),
            Self::Recall(k) => write!(f, "recall@{k}"),
            Self::F(k) => write!(f, "F@{k}"),
            Self::PrecisionIfUnjudgedRelevant(k) => write!(f, "P@{k} if unjudged relevant"),
            Self::Judged(k) => write!(f, "judged@{k}"),
            Self::RPrecision => write!(f, "R-precision"),
            Self::SetPrecision => write!(f, "set P"),
            Self::SetRecall => write!(f, "set recall"),
            Self::SetF => write!(f, "set F"),
            Self::Ndcg(k) => write!(f, "nDCG@{k}"),
            Self::ReciprocalRank => write!(f, "reciprocal rank"),
        }
    }
}

/// `n / d` as a share; `d` is never 0 where this is called.
#[allow(clippy::cast_precision_loss)]
fn share(n: usize, d: usize) -> f64 {
    n as f64 / d as f64
}

/// The harmonic mean of `p` and `r`, 0 when both are 0.
fn harmonic(p: f64, r: f64) -> f64 {
    if p + r == 0.0 {
        0.0
    } else {
        2.0 * p * r / (p + r)
    }
}

/// One query's value of each of `measures`: `ranked` in rank order,
/// `grades` the key's judgments for the query, relevant at `level` or
/// above. `None` when the key knows no relevant work for the query at
/// that level: recall has nothing to divide by, and the query is left
/// out of that level's averages (evaluation plan §10, item 2).
#[must_use]
pub fn values(
    measures: &[Measure],
    ranked: &[Scored],
    grades: &BTreeMap<String, i64>,
    level: i64,
) -> Option<Vec<f64>> {
    let relevant = grades.values().filter(|g| **g >= level).count();
    if relevant == 0 {
        return None;
    }
    // The grade of a judged work; a negative grade is no judgment.
    let grade_of = |s: &Scored| grades.get(&s.work).copied().filter(|g| *g >= 0);
    let is_relevant = |s: &Scored| grade_of(s).is_some_and(|g| g >= level);
    let relevant_in = |k: usize| ranked.iter().take(k).filter(|s| is_relevant(s)).count();
    let returned = ranked.len();
    let relevant_returned = relevant_in(returned);
    Some(
        measures
            .iter()
            .map(|m| match *m {
                Measure::Precision(k) => share(relevant_in(k), k),
                Measure::Recall(k) => share(relevant_in(k), relevant),
                Measure::F(k) => {
                    harmonic(share(relevant_in(k), k), share(relevant_in(k), relevant))
                }
                Measure::PrecisionIfUnjudgedRelevant(k) => share(
                    ranked
                        .iter()
                        .take(k)
                        .filter(|s| grade_of(s).is_none_or(|g| g >= level))
                        .count(),
                    k,
                ),
                Measure::Judged(k) => share(
                    ranked
                        .iter()
                        .take(k)
                        .filter(|s| grade_of(s).is_some())
                        .count(),
                    k,
                ),
                Measure::RPrecision => share(relevant_in(relevant), relevant),
                Measure::SetPrecision => {
                    if returned == 0 {
                        0.0
                    } else {
                        share(relevant_returned, returned)
                    }
                }
                Measure::SetRecall => share(relevant_returned, relevant),
                Measure::SetF => {
                    let p = if returned == 0 {
                        0.0
                    } else {
                        share(relevant_returned, returned)
                    };
                    harmonic(p, share(relevant_returned, relevant))
                }
                Measure::Ndcg(k) => ndcg(ranked, grades, k),
                Measure::ReciprocalRank => ranked
                    .iter()
                    .position(is_relevant)
                    .map_or(0.0, |i| 1.0 / share(i + 1, 1)),
            })
            .collect(),
    )
}

/// The discount at a zero-based position.
fn discount(position: usize) -> f64 {
    share(position + 2, 1).log2()
}

/// nDCG in the first `k`: the grades as gains (a grade of 0 or below
/// gains nothing), each discounted by the log of its rank plus one, over
/// the same sum for the key's own best ordering.
fn ndcg(ranked: &[Scored], grades: &BTreeMap<String, i64>, k: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let gain = |g: i64| if g > 0 { g as f64 } else { 0.0 };
    let dcg: f64 = ranked
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, s)| grades.get(&s.work).map_or(0.0, |g| gain(*g)) / discount(i))
        .sum();
    let mut best: Vec<i64> = grades.values().copied().filter(|g| *g > 0).collect();
    best.sort_unstable_by(|a, b| b.cmp(a));
    let ideal: f64 = best
        .iter()
        .take(k)
        .enumerate()
        .map(|(i, g)| gain(*g) / discount(i))
        .sum();
    if ideal == 0.0 { 0.0 } else { dcg / ideal }
}

/// A run measured against a key at one level: the queries averaged, and
/// each measure's per-query values in the queries' order.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluated {
    #[allow(missing_docs)]
    pub level: i64,
    #[allow(missing_docs)]
    pub measures: Vec<Measure>,
    /// The key's queries with a relevant work at the level, in order.
    pub queries: Vec<String>,
    /// `columns[m][q]`: measure `m` for query `q`.
    pub columns: Vec<Vec<f64>>,
    /// The key's queries with no relevant work at the level, left out.
    pub without_relevant: Vec<String>,
    /// Of `queries`, those the run returned nothing for: every measure 0.
    pub not_in_run: Vec<String>,
    /// Queries the run answered that the key does not have.
    pub not_in_key: Vec<String>,
}

/// Measures `run` against `key` at `level`. Every query of the key counts,
/// the ones the run has nothing for at 0 (`trec_eval -c`).
#[must_use]
pub fn evaluate(key: &Key, run: &Run, level: i64, measures: &[Measure]) -> Evaluated {
    let mut evaluated = Evaluated {
        level,
        measures: measures.to_vec(),
        queries: Vec::new(),
        columns: vec![Vec::new(); measures.len()],
        without_relevant: Vec::new(),
        not_in_run: Vec::new(),
        not_in_key: run
            .queries
            .keys()
            .filter(|q| !key.queries.contains_key(*q))
            .cloned()
            .collect(),
    };
    for (query, grades) in &key.queries {
        let ranked = run.queries.get(query).map_or(&[][..], Vec::as_slice);
        let Some(values) = values(measures, ranked, grades, level) else {
            evaluated.without_relevant.push(query.clone());
            continue;
        };
        if ranked.is_empty() {
            evaluated.not_in_run.push(query.clone());
        }
        evaluated.queries.push(query.clone());
        for (column, value) in evaluated.columns.iter_mut().zip(values) {
            column.push(value);
        }
    }
    evaluated
}

/// A measure's mean over the queries and its interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Averaged {
    #[allow(missing_docs)]
    pub measure: Measure,
    #[allow(missing_docs)]
    pub mean: f64,
    /// `None` below [`interval::MIN_QUERIES`].
    pub interval: Option<Interval>,
}

/// Each column's interval, or none for every column below the floor.
fn intervals_over(columns: &[Vec<f64>], queries: usize) -> Vec<Option<Interval>> {
    if queries < interval::MIN_QUERIES {
        return vec![None; columns.len()];
    }
    interval::intervals(columns, &vec![0; queries])
        .into_iter()
        .map(Some)
        .collect()
}

impl Evaluated {
    /// Each measure's mean and 95% interval, every measure resampled
    /// with the same draws. `None` with no query to average.
    #[must_use]
    pub fn averaged(&self) -> Option<Vec<Averaged>> {
        if self.queries.is_empty() {
            return None;
        }
        let intervals = intervals_over(&self.columns, self.queries.len());
        Some(
            self.measures
                .iter()
                .zip(&self.columns)
                .zip(intervals)
                .map(|((measure, column), interval)| Averaged {
                    measure: *measure,
                    mean: mean(column),
                    interval,
                })
                .collect(),
        )
    }
}

/// The mean of `values`, summed in order.
#[must_use]
pub fn mean(values: &[f64]) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let n = values.len() as f64;
    values.iter().sum::<f64>() / n
}

/// Two runs over the same key and level: each measure's means, and the
/// mean of the per-query differences (new less base) with its interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Compared {
    #[allow(missing_docs)]
    pub measure: Measure,
    #[allow(missing_docs)]
    pub base: f64,
    #[allow(missing_docs)]
    pub new: f64,
    #[allow(missing_docs)]
    pub difference: f64,
    /// `None` below [`interval::MIN_QUERIES`].
    pub interval: Option<Interval>,
}

impl Compared {
    /// Worse beyond its interval: the whole interval of the difference
    /// below zero, for a measure where more is better — what the
    /// acceptance rule refuses (evaluation plan §4). Never without an
    /// interval.
    #[must_use]
    pub fn worse(&self) -> bool {
        self.measure.higher_is_better() && self.interval.is_some_and(|i| i.high < 0.0)
    }

    /// Better beyond its interval.
    #[must_use]
    pub fn better(&self) -> bool {
        self.measure.higher_is_better() && self.interval.is_some_and(|i| i.low > 0.0)
    }
}

/// Compares two evaluations of one key at one level, query by query.
/// `None` with no query to average.
///
/// # Panics
///
/// When the two were not measured over the same queries and measures,
/// which one key and one level guarantee.
#[must_use]
pub fn compare(base: &Evaluated, new: &Evaluated) -> Option<Vec<Compared>> {
    assert_eq!(
        base.queries, new.queries,
        "two evaluations of one key and level"
    );
    assert_eq!(
        base.measures, new.measures,
        "two evaluations of one key and level"
    );
    if base.queries.is_empty() {
        return None;
    }
    let differences: Vec<Vec<f64>> = base
        .columns
        .iter()
        .zip(&new.columns)
        .map(|(b, n)| n.iter().zip(b).map(|(n, b)| n - b).collect())
        .collect();
    let intervals = intervals_over(&differences, base.queries.len());
    Some(
        base.measures
            .iter()
            .enumerate()
            .zip(intervals)
            .map(|((m, measure), interval)| Compared {
                measure: *measure,
                base: mean(&base.columns[m]),
                new: mean(&new.columns[m]),
                difference: mean(&differences[m]),
                interval,
            })
            .collect(),
    )
}

/// The runs every key can be checked with (evaluation plan §5): its
/// ideal ranking — every judged work, best grade first — and that
/// ranking reversed. Scores fall with rank, so `trec_eval` keeps the
/// order.
#[must_use]
pub fn sanity_runs(key: &Key) -> (Run, Run) {
    let mut ideal = Run::default();
    let mut reversed = Run::default();
    for (query, grades) in &key.queries {
        let mut judged: Vec<(&String, i64)> = grades.iter().map(|(w, g)| (w, *g)).collect();
        // Best grade first; among equal grades, work order, so the file
        // is the same every time.
        judged.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let n = judged.len();
        let scored = |order: &mut dyn Iterator<Item = &(&String, i64)>| -> Vec<Scored> {
            order
                .enumerate()
                .map(|(i, (w, _))| Scored {
                    work: (*w).clone(),
                    score: share(n - i, 1),
                })
                .collect()
        };
        ideal
            .queries
            .insert(query.clone(), scored(&mut judged.iter()));
        reversed
            .queries
            .insert(query.clone(), scored(&mut judged.iter().rev()));
    }
    (ideal, reversed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn ranked(works: &[&str]) -> Vec<Scored> {
        works
            .iter()
            .enumerate()
            .map(|(i, w)| Scored {
                work: (*w).to_owned(),
                score: share(works.len() - i, 1),
            })
            .collect()
    }

    fn grades(pairs: &[(&str, i64)]) -> BTreeMap<String, i64> {
        pairs.iter().map(|(w, g)| ((*w).to_owned(), *g)).collect()
    }

    fn one(measure: Measure, run: &[&str], key: &[(&str, i64)], level: i64) -> f64 {
        values(&[measure], &ranked(run), &grades(key), level).unwrap()[0]
    }

    // A worked query, by hand: ten returned, `a` `c` `f` relevant among
    // them (grades 2, 1, 2), `b` judged not relevant, the rest unjudged,
    // `z` relevant and never returned.
    const RUN: [&str; 10] = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
    const KEY: [(&str, i64); 5] = [("a", 2), ("b", 0), ("c", 1), ("f", 2), ("z", 1)];

    #[test]
    fn the_worked_query_at_depth_ten() {
        assert!((one(Measure::Precision(10), &RUN, &KEY, 1) - 0.3).abs() < 1e-15);
        assert!((one(Measure::Recall(10), &RUN, &KEY, 1) - 0.75).abs() < 1e-15);
        // 2 × 0.3 × 0.75 / 1.05
        assert!((one(Measure::F(10), &RUN, &KEY, 1) - 0.45 / 1.05).abs() < 1e-15);
        // Nine not judged not relevant: `a`, `c`, `f` and six unjudged.
        assert!((one(Measure::PrecisionIfUnjudgedRelevant(10), &RUN, &KEY, 1) - 0.9).abs() < 1e-15);
        assert!((one(Measure::Judged(10), &RUN, &KEY, 1) - 0.4).abs() < 1e-15);
    }

    #[test]
    fn the_worked_query_strict() {
        // At grade 2 only `a` and `f` are relevant.
        assert!((one(Measure::Precision(10), &RUN, &KEY, 2) - 0.2).abs() < 1e-15);
        assert!((one(Measure::Recall(10), &RUN, &KEY, 2) - 1.0).abs() < 1e-15);
        // R = 2: of `a` `b`, one relevant.
        assert!((one(Measure::RPrecision, &RUN, &KEY, 2) - 0.5).abs() < 1e-15);
        // `c` is judged and below the level, so not a high bound's either.
        assert!((one(Measure::PrecisionIfUnjudgedRelevant(10), &RUN, &KEY, 2) - 0.8).abs() < 1e-15);
    }

    #[test]
    fn the_worked_query_whole() {
        // R = 4: of `a` `b` `c` `d`, two relevant.
        assert!((one(Measure::RPrecision, &RUN, &KEY, 1) - 0.5).abs() < 1e-15);
        assert!((one(Measure::SetPrecision, &RUN, &KEY, 1) - 0.3).abs() < 1e-15);
        assert!((one(Measure::SetRecall, &RUN, &KEY, 1) - 0.75).abs() < 1e-15);
        assert!((one(Measure::ReciprocalRank, &RUN, &KEY, 1) - 1.0).abs() < 1e-15);
        assert!((one(Measure::ReciprocalRank, &["b", "c"], &KEY, 1) - 0.5).abs() < 1e-15);
        assert!(one(Measure::ReciprocalRank, &["b"], &KEY, 1).abs() < 1e-15);
    }

    #[test]
    fn a_negative_grade_is_no_judgment() {
        let key = [("a", 1), ("b", -1), ("c", 0)];
        let run = ["a", "b", "c", "d"];
        // `a` and `c` judged; `b` and `d` not.
        assert!((one(Measure::Judged(10), &run, &key, 1) - 0.2).abs() < 1e-15);
        // `a`, `b` and `d` could be relevant; `c` cannot.
        assert!((one(Measure::PrecisionIfUnjudgedRelevant(10), &run, &key, 1) - 0.3).abs() < 1e-15);
    }

    #[test]
    fn precision_divides_by_the_depth_not_by_what_came_back() {
        assert!((one(Measure::Precision(10), &["a"], &KEY, 1) - 0.1).abs() < 1e-15);
        assert!((one(Measure::SetPrecision, &["a"], &KEY, 1) - 1.0).abs() < 1e-15);
        assert!(one(Measure::SetPrecision, &[], &KEY, 1).abs() < 1e-15);
    }

    #[test]
    fn a_query_without_a_relevant_work_is_left_out() {
        assert_eq!(
            values(
                &[Measure::Precision(10)],
                &ranked(&["a"]),
                &grades(&[("a", 0)]),
                1
            ),
            None
        );
        let key = Key::read("1 0 a 0\n2 0 b 1\n").unwrap();
        let run = Run::read("2 Q0 b 1 1 t\n3 Q0 c 1 1 t\n").unwrap();
        let got = evaluate(&key, &run, 1, &[Measure::Precision(10)]);
        assert_eq!(got.queries, vec!["2"]);
        assert_eq!(got.without_relevant, vec!["1"]);
        assert_eq!(got.not_in_key, vec!["3"]);
    }

    #[test]
    fn a_query_the_run_missed_counts_as_nothing_found() {
        let key = Key::read("1 0 a 1\n2 0 b 1\n").unwrap();
        let run = Run::read("1 Q0 a 1 1 t\n").unwrap();
        let got = evaluate(&key, &run, 1, &[Measure::Precision(10)]);
        assert_eq!(got.queries, vec!["1", "2"]);
        assert_eq!(got.not_in_run, vec!["2"]);
        assert_eq!(got.columns, vec![vec![0.1, 0.0]]);
    }

    /// The third sanity run (evaluation plan §5): twenty queries, each
    /// with five relevant works hidden among 5,000, shuffled from a fixed
    /// seed, score near nothing — while the ideal run over the same key
    /// scores 1.
    #[test]
    fn a_random_ranking_scores_near_nothing() {
        let mut rng = interval::SplitMix64::new(interval::SEED);
        let mut key = Key::default();
        let mut random = Run::default();
        for q in 0..20 {
            let query = format!("q{q}");
            let mut works: Vec<String> = (0..5_000).map(|w| format!("w{w}")).collect();
            key.queries.insert(
                query.clone(),
                works.iter().take(5).map(|w| (w.clone(), 1)).collect(),
            );
            // Fisher and Yates.
            for i in (1..works.len()).rev() {
                let j = usize::try_from(rng.below(i as u64 + 1)).unwrap();
                works.swap(i, j);
            }
            random.queries.insert(
                query,
                ranked(&works.iter().map(String::as_str).collect::<Vec<_>>()),
            );
        }
        let measures = [
            Measure::Precision(10),
            Measure::Ndcg(10),
            Measure::RPrecision,
        ];
        let got = evaluate(&key, &random, 1, &measures);
        for column in &got.columns {
            assert!(mean(column) < 0.01, "{}", mean(column));
        }
        let (ideal, _) = sanity_runs(&key);
        let best = evaluate(&key, &ideal, 1, &measures);
        assert!((mean(&best.columns[1]) - 1.0).abs() < 1e-12);
        assert!((mean(&best.columns[2]) - 1.0).abs() < 1e-12);
    }

    fn arb_key() -> impl Strategy<Value = Key> {
        // Up to six queries over twenty works, grades -1 to 2.
        prop::collection::btree_map(
            "q[0-5]",
            prop::collection::btree_map("w[a-t]", -1_i64..=2, 1..12),
            1..6,
        )
        .prop_map(|queries| Key { queries })
    }

    proptest! {
        /// The ideal run is perfect wherever perfection is defined, and
        /// its reverse never beats it.
        #[test]
        fn the_ideal_run_is_perfect(key in arb_key(), level in 1_i64..=2) {
            let (ideal, reversed) = sanity_runs(&key);
            let measures = [Measure::RPrecision, Measure::Ndcg(10), Measure::ReciprocalRank, Measure::SetRecall];
            let best = evaluate(&key, &ideal, level, &measures);
            let worst = evaluate(&key, &reversed, level, &measures);
            for column in &best.columns[..3] {
                for v in column {
                    prop_assert!((v - 1.0).abs() < 1e-12, "{v}");
                }
            }
            for (b, w) in best.columns.iter().zip(&worst.columns) {
                for (b, w) in b.iter().zip(w) {
                    prop_assert!(w <= &(b + 1e-12));
                }
            }
        }

        /// Every value is a share, and the unjudged bounds are ordered.
        #[test]
        fn values_are_shares_and_bounds_are_ordered(
            key in arb_key(),
            order in prop::collection::vec("w[a-z]", 0..30),
            level in 1_i64..=2,
        ) {
            let mut seen = std::collections::HashSet::new();
            let works: Vec<&str> = order.iter().map(String::as_str).filter(|w| seen.insert(*w)).collect();
            let measures = Measure::report();
            for grades in key.queries.values() {
                let Some(got) = values(&measures, &ranked(&works), grades, level) else {
                    continue;
                };
                for v in &got {
                    prop_assert!((0.0..=1.0 + 1e-12).contains(v), "{v}");
                }
                for k in DEPTHS {
                    let at = |m: Measure| got[measures.iter().position(|x| *x == m).unwrap()];
                    prop_assert!(at(Measure::Precision(k)) <= at(Measure::PrecisionIfUnjudgedRelevant(k)));
                    prop_assert!(at(Measure::Precision(k)) <= at(Measure::Judged(k)));
                }
            }
        }
    }
}
