//! The report (evaluation plan §4): per level, every measure's mean and
//! 95% interval; for two runs, the difference and its interval, and the
//! acceptance rule's verdict.

use std::io::{self, Write};

use crate::measure::{self, Evaluated, Measure};
use crate::trec::{Key, Run};

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
                "  {:<28} {:>7.4}   {:.4} to {:.4}",
                a.measure.to_string(),
                a.mean,
                a.interval.low,
                a.interval.high
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

/// Writes the comparison of two runs, and returns whether the new one
/// passes the acceptance rule: no measure worse beyond its interval at
/// any level.
///
/// # Errors
///
/// Writing.
pub fn compare(key: &Key, base: &Run, new: &Run, out: &mut impl Write) -> io::Result<bool> {
    let measures = Measure::report();
    let levels = levels(key);
    if levels.is_empty() {
        writeln!(out, "the key judges no work relevant")?;
        return Ok(true);
    }
    let mut worse = Vec::new();
    for (name, level) in levels {
        let b = measure::evaluate(key, base, level, &measures);
        let n = measure::evaluate(key, new, level, &measures);
        heading(out, name, &b)?;
        let Some(compared) = measure::compare(&b, &n) else {
            writeln!(out, "  nothing to average")?;
            continue;
        };
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
                "  {:<28} {:>7.4} {:>7.4} {:>+11.4}   {:+.4} to {:+.4}{mark}",
                c.measure.to_string(),
                c.base,
                c.new,
                c.difference,
                c.interval.low,
                c.interval.high
            )?;
        }
    }
    if worse.is_empty() {
        writeln!(
            out,
            "verdict: passes; no measure is worse beyond its interval"
        )?;
    } else {
        writeln!(
            out,
            "verdict: refused; worse beyond its interval: {}",
            worse.join(", ")
        )?;
    }
    Ok(worse.is_empty())
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
        let key = Key::read("1 0 a 2\n1 0 b 1\n2 0 c 1\n").unwrap();
        let run = Run::read("1 Q0 b 1 2 t\n1 Q0 a 2 1 t\n2 Q0 d 1 1 t\n").unwrap();
        let mut out = Vec::new();
        assert!(compare(&key, &run, &run, &mut out).unwrap());
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("verdict: passes"), "{text}");
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
        let passed = compare(
            &key,
            &Run::read(&base).unwrap(),
            &Run::read(&new).unwrap(),
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(!passed, "{text}");
        assert!(text.contains("verdict: refused"), "{text}");
        assert!(text.contains("relevant P@10"), "{text}");
    }
}
