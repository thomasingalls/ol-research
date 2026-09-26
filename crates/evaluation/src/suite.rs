//! A suite on disk (evaluation plan §6): a directory of plain files —
//! `queries.tsv`, `key.qrels`, `filters.tsv` and, when the suite is
//! halved, `halves.tsv` (§7). What wrote it, and from what, is the
//! command line's `about.json` beside them.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};
use std::path::Path;

use crate::interval::{MIN_QUERIES, SplitMix64};
use crate::trec::{self, Key, Run};

/// The queries file.
pub const QUERIES: &str = "queries.tsv";
/// The answer key.
pub const KEY: &str = "key.qrels";
/// What each query's run leaves out.
pub const FILTERS: &str = "filters.tsv";
/// Which half each query is in; absent for a suite kept whole.
pub const HALVES: &str = "halves.tsv";

/// The seed the halves are drawn with. Changing it redraws every suite's
/// halves, which spends every test half (evaluation plan §7).
pub const SPLIT_SEED: u64 = 0x6f6c_2d72_6874_0002;

/// What a query's run leaves out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Works published after this year, in a restricted run: the year the
    /// set's judges saw the corpus up to.
    pub until: Option<u16>,
    /// A work never in its own results: RELISH's seed paper.
    pub exclude: Option<String>,
}

/// A query's half.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    /// Looked at while working.
    Working,
    /// Read at milestones, only in total.
    Test,
}

/// A suite.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Suite {
    /// Query id and text, in file order.
    pub queries: Vec<(String, String)>,
    #[allow(missing_docs)]
    pub key: Key,
    /// By query id; a query without one leaves nothing out.
    pub filters: BTreeMap<String, Filter>,
    /// By query id; empty for a suite kept whole.
    pub halves: BTreeMap<String, Half>,
}

/// A suite's file that is not what it should be.
#[derive(Debug)]
pub enum Error {
    #[allow(missing_docs)]
    Io(String, io::Error),
    #[allow(missing_docs)]
    Line(String, trec::Error),
    /// The files disagree: a query in one and not another.
    Mismatch(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(file, e) => write!(f, "{file}: {e}"),
            Self::Line(file, e) => write!(f, "{file}: {e}"),
            Self::Mismatch(what) => f.write_str(what),
        }
    }
}

impl std::error::Error for Error {}

/// The halves of a suite whose key scores `scored` queries: drawn when
/// each half keeps [`MIN_QUERIES`], so that either can carry an
/// interval; otherwise none, and the suite is used whole. The queries
/// are shuffled from [`SPLIT_SEED`] in the order given and the first
/// half, rounded up, is the working half.
#[must_use]
pub fn halves(scored: &[String]) -> BTreeMap<String, Half> {
    if scored.len() < 2 * MIN_QUERIES {
        return BTreeMap::new();
    }
    let mut order: Vec<&String> = scored.iter().collect();
    let mut rng = SplitMix64::new(SPLIT_SEED);
    // Fisher and Yates.
    for i in (1..order.len()).rev() {
        #[allow(clippy::cast_possible_truncation)]
        let j = rng.below(i as u64 + 1) as usize;
        order.swap(i, j);
    }
    let working = order.len().div_ceil(2);
    order
        .into_iter()
        .enumerate()
        .map(|(i, q)| {
            (
                q.clone(),
                if i < working {
                    Half::Working
                } else {
                    Half::Test
                },
            )
        })
        .collect()
}

/// Every query in the test half: a suite held out whole, read only in
/// total at milestones (evaluation plan §7) — as TREC Precision Medicine
/// 2020 is, while 2019 is worked on (the user's call, 2026-09-26).
#[must_use]
pub fn held_out(scored: &[String]) -> BTreeMap<String, Half> {
    scored.iter().map(|q| (q.clone(), Half::Test)).collect()
}

impl Suite {
    /// Whether the whole suite is its test half.
    #[must_use]
    pub fn is_held_out(&self) -> bool {
        !self.halves.is_empty() && self.halves.values().all(|h| *h == Half::Test)
    }

    /// The queries the key has a relevant work for at any grade, in the
    /// queries file's order: the ones a report can average.
    #[must_use]
    pub fn scored(&self) -> Vec<String> {
        self.queries
            .iter()
            .filter(|(q, _)| {
                self.key
                    .queries
                    .get(q)
                    .is_some_and(|judged| judged.values().any(|g| *g > 0))
            })
            .map(|(q, _)| q.clone())
            .collect()
    }

    /// The key over one half. A suite kept whole is all working half and
    /// has no test half.
    #[must_use]
    pub fn key_over(&self, half: Half) -> Key {
        if self.halves.is_empty() {
            return match half {
                Half::Working => self.key.clone(),
                Half::Test => Key::default(),
            };
        }
        Key {
            queries: self
                .key
                .queries
                .iter()
                .filter(|(q, _)| self.halves.get(*q) == Some(&half))
                .map(|(q, j)| (q.clone(), j.clone()))
                .collect(),
        }
    }

    /// Reads a suite's directory.
    ///
    /// # Errors
    ///
    /// A file missing or malformed, or files that disagree about which
    /// queries there are.
    pub fn read(dir: &Path) -> Result<Self, Error> {
        let read = |name: &str| -> Result<String, Error> {
            std::fs::read_to_string(dir.join(name))
                .map_err(|e| Error::Io(dir.join(name).display().to_string(), e))
        };
        let at = |name: &str| dir.join(name).display().to_string();
        let queries =
            trec::read_queries(&read(QUERIES)?).map_err(|e| Error::Line(at(QUERIES), e))?;
        let key = Key::read(&read(KEY)?).map_err(|e| Error::Line(at(KEY), e))?;
        let filters = read_filters(&read(FILTERS)?).map_err(|e| Error::Line(at(FILTERS), e))?;
        let halves = if dir.join(HALVES).exists() {
            read_halves(&read(HALVES)?).map_err(|e| Error::Line(at(HALVES), e))?
        } else {
            BTreeMap::new()
        };
        let ids: std::collections::BTreeSet<&String> = queries.iter().map(|(q, _)| q).collect();
        for (file, named) in [
            (KEY, key.queries.keys().collect::<Vec<_>>()),
            (FILTERS, filters.keys().collect()),
            (HALVES, halves.keys().collect()),
        ] {
            if let Some(stray) = named.into_iter().find(|q| !ids.contains(q)) {
                return Err(Error::Mismatch(format!(
                    "{}: query {stray} is not in {QUERIES}",
                    at(file)
                )));
            }
        }
        Ok(Self {
            queries,
            key,
            filters,
            halves,
        })
    }

    /// Writes the suite into `dir`, which must not hold one already.
    ///
    /// # Errors
    ///
    /// I/O, or a suite's file already there.
    pub fn write(&self, dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let create = |name: &str| {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.join(name))
                .map(io::BufWriter::new)
        };
        let mut out = create(QUERIES)?;
        for (q, text) in &self.queries {
            writeln!(out, "{q}\t{text}")?;
        }
        out.flush()?;
        let mut out = create(KEY)?;
        self.key.write(&mut out)?;
        out.flush()?;
        let mut out = create(FILTERS)?;
        for (q, f) in &self.filters {
            let until = f.until.map_or_else(|| "-".to_owned(), |y| y.to_string());
            writeln!(out, "{q}\t{until}\t{}", f.exclude.as_deref().unwrap_or("-"))?;
        }
        out.flush()?;
        if !self.halves.is_empty() {
            let mut out = create(HALVES)?;
            for (q, h) in &self.halves {
                let h = match h {
                    Half::Working => "working",
                    Half::Test => "test",
                };
                writeln!(out, "{q}\t{h}")?;
            }
            out.flush()?;
        }
        Ok(())
    }
}

fn line_error(line: usize, what: String) -> trec::Error {
    trec::Error { line, what }
}

fn read_filters(text: &str) -> Result<BTreeMap<String, Filter>, trec::Error> {
    let mut filters = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        let [q, until, exclude] = line.split('\t').collect::<Vec<_>>()[..] else {
            return Err(line_error(n, "not three tab-separated columns".to_owned()));
        };
        let until = match until {
            "-" => None,
            y => Some(
                y.parse::<u16>()
                    .map_err(|_| line_error(n, format!("year {y:?}")))?,
            ),
        };
        let exclude = (exclude != "-").then(|| exclude.to_owned());
        if filters
            .insert(q.to_owned(), Filter { until, exclude })
            .is_some()
        {
            return Err(line_error(n, format!("query {q} again")));
        }
    }
    Ok(filters)
}

fn read_halves(text: &str) -> Result<BTreeMap<String, Half>, trec::Error> {
    let mut halves = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        let half = match line.split_once('\t') {
            Some((q, "working")) => (q, Half::Working),
            Some((q, "test")) => (q, Half::Test),
            _ => return Err(line_error(n, format!("not a query and its half: {line:?}"))),
        };
        if halves.insert(half.0.to_owned(), half.1).is_some() {
            return Err(line_error(n, format!("query {} again", half.0)));
        }
    }
    Ok(halves)
}

/// The run with every work its query's key did not judge taken out, or
/// judged with a negative grade — the condensed list (evaluation plan
/// §3.2), what `trec_eval -J` scores.
#[must_use]
pub fn condensed(run: &Run, key: &Key) -> Run {
    Run {
        queries: run
            .queries
            .iter()
            .map(|(q, works)| {
                let judged = key.queries.get(q);
                let kept = works
                    .iter()
                    .filter(|s| judged.and_then(|j| j.get(&s.work)).is_some_and(|g| *g >= 0))
                    .cloned()
                    .collect();
                (q.clone(), kept)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("q{i}")).collect()
    }

    #[test]
    fn a_small_suite_is_kept_whole() {
        assert!(halves(&ids(59)).is_empty());
        assert_eq!(halves(&ids(60)).len(), 60);
    }

    #[test]
    fn halves_are_even_seeded_and_stable() {
        let got = halves(&ids(101));
        let working = got.values().filter(|h| **h == Half::Working).count();
        assert_eq!(working, 51);
        assert_eq!(got, halves(&ids(101)));
        // Not simply the first half of the file.
        let first: Vec<bool> = (0..51)
            .map(|i| got[&format!("q{i}")] == Half::Working)
            .collect();
        assert!(first.iter().any(|w| !w));
    }

    #[test]
    fn a_suite_writes_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("evaluation-suite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let queries: Vec<(String, String)> = ids(60)
            .into_iter()
            .map(|q| (q.clone(), format!("text of {q}")))
            .collect();
        let mut key = Key::default();
        for (q, _) in &queries {
            key.queries
                .entry(q.clone())
                .or_default()
                .insert("ol:A".to_owned(), 1);
        }
        let mut filters = BTreeMap::new();
        filters.insert(
            "q0".to_owned(),
            Filter {
                until: Some(2018),
                exclude: Some("ol:B".to_owned()),
            },
        );
        filters.insert("q1".to_owned(), Filter::default());
        let mut suite = Suite {
            queries,
            key,
            filters,
            halves: BTreeMap::new(),
        };
        suite.halves = halves(&suite.scored());
        suite.write(&dir).unwrap();
        assert!(suite.write(&dir).is_err(), "a suite is never written over");
        let back = Suite::read(&dir).unwrap();
        assert_eq!(back, suite);
        assert_eq!(back.key_over(Half::Working).queries.len(), 30);
        assert_eq!(back.key_over(Half::Test).queries.len(), 30);
        assert!(!back.is_held_out());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_held_out_suite_is_all_test_half() {
        let mut suite = Suite {
            queries: vec![
                ("1".to_owned(), "a".to_owned()),
                ("2".to_owned(), "b".to_owned()),
            ],
            key: Key::read("1 0 x 1\n2 0 y 1\n").unwrap(),
            ..Suite::default()
        };
        assert!(
            suite.key_over(Half::Test).queries.is_empty(),
            "kept whole: no test half"
        );
        suite.halves = held_out(&suite.scored());
        assert!(suite.is_held_out());
        assert!(suite.key_over(Half::Working).queries.is_empty());
        assert_eq!(suite.key_over(Half::Test).queries.len(), 2);
    }

    #[test]
    fn a_condensed_run_keeps_only_the_judged() {
        let key = Key::read("1 0 a 1\n1 0 b 0\n1 0 c -1\n2 0 d 1\n").unwrap();
        let run =
            Run::read("1 Q0 a 1 4 t\n1 Q0 x 2 3 t\n1 Q0 b 3 2 t\n1 Q0 c 4 1 t\n3 Q0 a 1 1 t\n")
                .unwrap();
        let got = condensed(&run, &key);
        let kept: Vec<&str> = got.queries["1"].iter().map(|s| s.work.as_str()).collect();
        assert_eq!(kept, vec!["a", "b"]);
        assert!(got.queries["3"].is_empty());
    }
}
