//! Whether two runs are one answer (tokenizer plan T1.4): the check that
//! a change which should change nothing changed nothing.
//!
//! Two builds of one index do not answer alike to the last digit. A
//! work's score is its terms' scores added in an order the build's
//! segments decide, and works tied on a score come in the order they lie
//! in the index; so a score can differ in its last digits, and among works
//! tied on a score another can stand at a rank, or be the one the depth
//! lets in. Those are counted apart, and everything else is a real
//! difference: a work only one run returned that is not tied at its end, a
//! score apart by more than rounding, a rank held by works whose scores
//! are apart.

use std::collections::BTreeMap;

use crate::trec::{Run, Scored};

/// How far apart two scores can be and be one score added in another
/// order: this share of the larger of them, or of one.
pub const TOLERANCE: f64 = 1e-5;

/// Real differences described, at most; the rest are counted.
pub const DESCRIBED: usize = 20;

/// What two runs differ by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Differences {
    /// The first run's lines.
    pub lines: usize,
    /// Works both returned whose scores differ, within [`TOLERANCE`].
    pub rounded: usize,
    /// Ranks holding another of works tied on a score.
    pub tied: usize,
    /// Works one run returned and the other did not, tied with the last
    /// its run returned: another of the tied works made the depth.
    pub at_the_end: usize,
    /// Real differences.
    pub real: usize,
    /// The first [`DESCRIBED`] of them.
    pub described: Vec<String>,
}

impl Differences {
    /// No real difference.
    #[must_use]
    pub const fn same(&self) -> bool {
        self.real == 0
    }

    fn real(&mut self, what: String) {
        self.real += 1;
        if self.described.len() < DESCRIBED {
            self.described.push(what);
        }
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOLERANCE * a.abs().max(b.abs()).max(1.0)
}

/// What `a` and `b` differ by.
#[must_use]
pub fn compare(a: &Run, b: &Run) -> Differences {
    let mut d = Differences {
        lines: a.queries.values().map(Vec::len).sum(),
        ..Differences::default()
    };
    for query in a.queries.keys().filter(|q| !b.queries.contains_key(*q)) {
        d.real(format!("query {query}: in the first run only"));
    }
    for query in b.queries.keys().filter(|q| !a.queries.contains_key(*q)) {
        d.real(format!("query {query}: in the second run only"));
    }
    for (query, ra) in &a.queries {
        let Some(rb) = b.queries.get(query) else {
            continue;
        };
        if ra.len() != rb.len() {
            d.real(format!(
                "query {query}: {} works against {}",
                ra.len(),
                rb.len()
            ));
            continue;
        }
        one_query(query, ra, rb, &mut d);
    }
    d
}

fn one_query(query: &str, ra: &[Scored], rb: &[Scored], d: &mut Differences) {
    let scores = |run: &[Scored]| -> BTreeMap<String, f64> {
        run.iter().map(|s| (s.work.clone(), s.score)).collect()
    };
    let (sa, sb) = (scores(ra), scores(rb));
    for (work, x) in &sa {
        let Some(y) = sb.get(work) else { continue };
        if x.to_bits() == y.to_bits() {
            continue;
        }
        if close(*x, *y) {
            d.rounded += 1;
        } else {
            d.real(format!("query {query}: {work} scored {x} against {y}"));
        }
    }
    for (mine, run, other) in [(&sa, ra, &sb), (&sb, rb, &sa)] {
        let Some(last) = run.last() else { continue };
        for (work, score) in mine.iter().filter(|(w, _)| !other.contains_key(*w)) {
            if close(*score, last.score) {
                d.at_the_end += 1;
            } else {
                d.real(format!(
                    "query {query}: {work} ({score}) in one run only, and not tied with its last"
                ));
            }
        }
    }
    for (rank, (x, y)) in ra.iter().zip(rb).enumerate() {
        if x.work == y.work {
            continue;
        }
        if close(x.score, y.score) {
            d.tied += 1;
        } else {
            d.real(format!(
                "query {query}, rank {}: {} ({}) against {} ({})",
                rank + 1,
                x.work,
                x.score,
                y.work,
                y.score
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::trec::order;

    fn run(works: &[(&str, f64)]) -> Run {
        let mut works: Vec<Scored> = works
            .iter()
            .map(|(work, score)| Scored {
                work: (*work).to_owned(),
                score: *score,
            })
            .collect();
        order(&mut works);
        Run {
            queries: [("q".to_owned(), works)].into(),
        }
    }

    #[test]
    fn what_a_builds_segments_decide_is_not_a_difference() {
        let a = run(&[
            ("w1", 22.084_041_595),
            ("w2", 5.5),
            ("w3", 5.5),
            ("w4", 1.0),
        ]);
        // A score's last digits, two tied works the other way round, and
        // another of the works tied at the end.
        let b = run(&[
            ("w1", 22.084_039_688),
            ("w2", 5.500_000_1),
            ("w3", 5.5),
            ("w5", 1.0),
        ]);
        let d = compare(&a, &b);
        assert!(d.same(), "{d:?}");
        // Two scores; the two tied ranks and the last; a work each way.
        assert_eq!((d.rounded, d.tied, d.at_the_end), (2, 3, 2));
    }

    #[test]
    fn anything_else_is() {
        let a = run(&[("w1", 9.0), ("w2", 5.0), ("w3", 1.0)]);
        // A score apart by more than rounding.
        assert_eq!(
            compare(&a, &run(&[("w1", 9.0), ("w2", 5.1), ("w3", 1.0)])).real,
            1
        );
        // A work only one returned, not at the end: once for each run's
        // stray, once for the rank.
        let d = compare(&a, &run(&[("w1", 9.0), ("w9", 5.0), ("w3", 1.0)]));
        assert!(!d.same(), "{d:?}");
        assert!(d.described[0].contains("in one run only"), "{d:?}");
        // Another length, another query.
        assert_eq!(compare(&a, &run(&[("w1", 9.0)])).real, 1);
        let mut other = a.clone();
        other.queries.insert("r".into(), Vec::new());
        assert_eq!(compare(&a, &other).real, 1);
        assert_eq!(compare(&other, &a).real, 1);
    }

    proptest! {
        /// A run is itself; and with every score moved by less than
        /// rounding, and its works put in rank order again, it still is.
        #[test]
        fn a_run_moved_by_rounding_is_the_same(
            scores in prop::collection::vec((0.0f64..50.0, -1.0f64..1.0), 0..40),
        ) {
            let named: Vec<(String, f64, f64)> = scores
                .iter()
                .enumerate()
                .map(|(i, (score, nudge))| (format!("w{i}"), *score, *nudge))
                .collect();
            let a = run(&named.iter().map(|(w, s, _)| (w.as_str(), *s)).collect::<Vec<_>>());
            let d = compare(&a, &a);
            prop_assert_eq!((d.rounded, d.tied, d.at_the_end, d.real), (0, 0, 0, 0));
            let b = run(&named
                .iter()
                .map(|(w, s, nudge)| (w.as_str(), s + nudge * 0.2 * TOLERANCE))
                .collect::<Vec<_>>());
            prop_assert!(compare(&a, &b).same(), "{:?}", compare(&a, &b));
        }

        /// A work's score moved by far more than rounding is a real
        /// difference, wherever it stands.
        #[test]
        fn a_score_moved_is_a_difference(
            scores in prop::collection::vec(1.0f64..50.0, 1..40),
            pick in any::<prop::sample::Index>(),
        ) {
            let named: Vec<(String, f64)> =
                scores.iter().enumerate().map(|(i, s)| (format!("w{i}"), *s)).collect();
            let a = run(&named.iter().map(|(w, s)| (w.as_str(), *s)).collect::<Vec<_>>());
            let moved = pick.index(named.len());
            let b = run(&named
                .iter()
                .enumerate()
                .map(|(i, (w, s))| (w.as_str(), if i == moved { s * 1.01 } else { *s }))
                .collect::<Vec<_>>());
            prop_assert!(!compare(&a, &b).same());
        }
    }
}
