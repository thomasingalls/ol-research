//! The report (evaluation plan §4): per level, every measure's mean and
//! 95% interval; for two runs, the difference and its interval, and the
//! acceptance rule's verdict.

use std::io::{self, Write};

use crate::measure::{self, Evaluated, Measure};
use crate::suite::{Suite, condensed};
use crate::trec::{Key, Run};

/// What the acceptance rule says of a new run against a base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// No measure is worse beyond its interval.
    Passes,
    /// Some measure is worse beyond its interval.
    Refused,
    /// Some level has too few queries for intervals, and nothing is
    /// worse where there are enough: the rule cannot speak.
    TooFewQueries,
}

/// An interval as the report prints it.
fn interval_text(interval: Option<crate::interval::Interval>, signed: bool) -> String {
    match interval {
        Some(i) if signed => format!("{:+.4} to {:+.4}", i.low, i.high),
        Some(i) => format!("{:.4} to {:.4}", i.low, i.high),
        None => format!("none: fewer than {} queries", crate::interval::MIN_QUERIES),
    }
}

/// The levels a key is reported at: lenient (any positive grade) and
/// strict (the top grade), or one level for a key with one positive
/// grade.
#[must_use]
pub fn levels(key: &Key) -> Vec<(&'static str, i64)> {
    match key.positive_grades()[..] {
        [] => Vec::new(),
        [only] => vec![("relevant", only)],
        [lowest, .., highest] => vec![("lenient", lowest), ("strict", highest)],
    }
}

fn heading(out: &mut impl Write, name: &str, e: &Evaluated) -> io::Result<()> {
    writeln!(
        out,
        "{name} (grade {} or above): {} queries averaged; {} left out with no relevant work; \
         {} the run returned nothing for; {} in the run and not the key",
        e.level,
        e.queries.len(),
        e.without_relevant.len(),
        e.not_in_run.len(),
        e.not_in_key.len()
    )
}

/// Writes the report of one run.
///
/// # Errors
///
/// Writing.
pub fn score(key: &Key, run: &Run, out: &mut impl Write) -> io::Result<()> {
    let measures = Measure::report();
    let levels = levels(key);
    if levels.is_empty() {
        return writeln!(out, "the key judges no work relevant");
    }
    for (name, level) in levels {
        let e = measure::evaluate(key, run, level, &measures);
        heading(out, name, &e)?;
        let Some(averaged) = e.averaged() else {
            writeln!(out, "  nothing to average")?;
            continue;
        };
        writeln!(out, "  {:<28} {:>7}   95% interval", "measure", "mean")?;
        for a in averaged {
            writeln!(
                out,
                "  {:<28} {:>7.4}   {}",
                a.measure.to_string(),
                a.mean,
                interval_text(a.interval, false)
            )?;
        }
    }
    writeln!(
        out,
        "recall is of the relevant works the key knows; a 95% interval is of the \
         mean over {} resamples of the queries",
        crate::interval::RESAMPLES
    )
}

/// Writes the comparison of two runs, and returns the acceptance
/// rule's verdict: no measure worse beyond its interval at any level.
///
/// # Errors
///
/// Writing.
pub fn compare(key: &Key, base: &Run, new: &Run, out: &mut impl Write) -> io::Result<Verdict> {
    let measures = Measure::report();
    let levels = levels(key);
    if levels.is_empty() {
        writeln!(out, "the key judges no work relevant")?;
        return Ok(Verdict::TooFewQueries);
    }
    let mut worse = Vec::new();
    let mut too_few = false;
    for (name, level) in levels {
        let b = measure::evaluate(key, base, level, &measures);
        let n = measure::evaluate(key, new, level, &measures);
        heading(out, name, &b)?;
        let Some(compared) = measure::compare(&b, &n) else {
            writeln!(out, "  nothing to average")?;
            too_few = true;
            continue;
        };
        too_few |= b.queries.len() < crate::interval::MIN_QUERIES;
        writeln!(
            out,
            "  {:<28} {:>7} {:>7} {:>11}   95% interval of the difference",
            "measure", "base", "new", "difference"
        )?;
        for c in compared {
            let mark = if c.worse() {
                worse.push(format!("{name} {}", c.measure));
                "  worse"
            } else if c.better() {
                "  better"
            } else {
                ""
            };
            writeln!(
                out,
                "  {:<28} {:>7.4} {:>7.4} {:>+11.4}   {}{mark}",
                c.measure.to_string(),
                c.base,
                c.new,
                c.difference,
                interval_text(c.interval, true)
            )?;
        }
    }
    let verdict = if !worse.is_empty() {
        writeln!(
            out,
            "verdict: refused; worse beyond its interval: {}",
            worse.join(", ")
        )?;
        Verdict::Refused
    } else if too_few {
        writeln!(
            out,
            "verdict: none; a level has fewer than {} queries, too few for intervals",
            crate::interval::MIN_QUERIES
        )?;
        Verdict::TooFewQueries
    } else {
        writeln!(
            out,
            "verdict: passes; no measure is worse beyond its interval"
        )?;
        Verdict::Passes
    };
    Ok(verdict)
}

/// Which of a suite's queries a report reads: the working half, or the
/// whole of a suite too small to halve (evaluation plan §7).
fn part(suite: &Suite, out: &mut impl Write) -> io::Result<()> {
    let scored = suite.scored().len();
    if suite.halves.is_empty() {
        writeln!(
            out,
            "the whole suite, {scored} queries with a relevant work: too few to halve"
        )
    } else {
        let working = suite
            .scored()
            .iter()
            .filter(|q| suite.halves.get(*q) == Some(&crate::suite::Half::Working))
            .count();
        writeln!(
            out,
            "the working half: {working} of {scored} queries with a relevant work; the test half is not read"
        )
    }
}

/// Writes a suite's report of one run: the part of the suite read, then
/// every result as returned (open), then with the results nobody judged
/// taken out (condensed; evaluation plan §3.2).
///
/// # Errors
///
/// Writing.
pub fn score_suite(suite: &Suite, run: &Run, out: &mut impl Write) -> io::Result<()> {
    let key = suite.working_key();
    part(suite, out)?;
    writeln!(out)?;
    writeln!(out, "open: every result as the search returned it")?;
    score(&key, run, out)?;
    writeln!(out)?;
    writeln!(out, "condensed: the results nobody judged taken out")?;
    score(&key, &condensed(run, &key), out)
}

/// Writes a suite's comparison of two runs, open and condensed, and
/// returns the verdict over both: refused if either refuses.
///
/// # Errors
///
/// Writing.
pub fn compare_suite(
    suite: &Suite,
    base: &Run,
    new: &Run,
    out: &mut impl Write,
) -> io::Result<Verdict> {
    let key = suite.working_key();
    part(suite, out)?;
    writeln!(out)?;
    writeln!(out, "open: every result as the search returned it")?;
    let open = compare(&key, base, new, out)?;
    writeln!(out)?;
    writeln!(out, "condensed: the results nobody judged taken out")?;
    let tight = compare(&key, &condensed(base, &key), &condensed(new, &key), out)?;
    let verdict = match (open, tight) {
        (Verdict::Refused, _) | (_, Verdict::Refused) => Verdict::Refused,
        (Verdict::TooFewQueries, _) | (_, Verdict::TooFewQueries) => Verdict::TooFewQueries,
        (Verdict::Passes, Verdict::Passes) => Verdict::Passes,
    };
    let said = match verdict {
        Verdict::Passes => "passes",
        Verdict::Refused => "refused",
        Verdict::TooFewQueries => "none",
    };
    writeln!(out)?;
    writeln!(out, "verdict over both: {said}")?;
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    #[test]
    fn a_key_is_reported_at_its_levels() {
        let graded = Key::read("1 0 a 2\n1 0 b 1\n1 0 c 0\n").unwrap();
        assert_eq!(levels(&graded), vec![("lenient", 1), ("strict", 2)]);
        let binary = Key::read("1 0 a 1\n1 0 c 0\n").unwrap();
        assert_eq!(levels(&binary), vec![("relevant", 1)]);
        let none = Key::read("1 0 c 0\n").unwrap();
        assert!(levels(&none).is_empty());
    }

    #[test]
    fn a_run_compared_with_itself_passes() {
        let mut key = String::new();
        let mut run = String::new();
        for q in 0..30 {
            writeln!(key, "{q} 0 a 2\n{q} 0 b 1").unwrap();
            writeln!(run, "{q} Q0 b 1 2 t\n{q} Q0 c 2 1 t").unwrap();
        }
        let (key, run) = (Key::read(&key).unwrap(), Run::read(&run).unwrap());
        let mut out = Vec::new();
        assert_eq!(
            compare(&key, &run, &run, &mut out).unwrap(),
            Verdict::Passes
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("verdict: passes"), "{text}");
    }

    #[test]
    fn a_small_suite_gets_means_and_no_verdict() {
        let key = Key::read("1 0 a 2\n1 0 b 1\n2 0 c 1\n").unwrap();
        let run = Run::read("1 Q0 b 1 2 t\n1 Q0 a 2 1 t\n2 Q0 d 1 1 t\n").unwrap();
        let mut out = Vec::new();
        assert_eq!(
            compare(&key, &run, &run, &mut out).unwrap(),
            Verdict::TooFewQueries
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("none: fewer than 30 queries"), "{text}");
        assert!(text.contains("verdict: none"), "{text}");
        let mut out = Vec::new();
        score(&key, &run, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        // The mean is printed; its interval is not.
        assert!(
            text.lines().any(|line| {
                let columns: Vec<&str> = line.split_whitespace().collect();
                columns.starts_with(&["P@10", "0.1000", "none:"])
            }),
            "{text}"
        );
    }

    #[test]
    fn a_run_that_loses_everywhere_is_refused() {
        // Forty queries, each with one relevant work: the base finds it
        // first every time, the new run never.
        let mut key = String::new();
        let mut base = String::new();
        let mut new = String::new();
        for q in 0..40 {
            writeln!(key, "{q} 0 good{q} 1").unwrap();
            writeln!(base, "{q} Q0 good{q} 1 1 t").unwrap();
            writeln!(new, "{q} Q0 other{q} 1 1 t").unwrap();
        }
        let key = Key::read(&key).unwrap();
        let mut out = Vec::new();
        let verdict = compare(
            &key,
            &Run::read(&base).unwrap(),
            &Run::read(&new).unwrap(),
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(verdict, Verdict::Refused, "{text}");
        assert!(text.contains("verdict: refused"), "{text}");
        assert!(text.contains("relevant P@10"), "{text}");
    }
}
