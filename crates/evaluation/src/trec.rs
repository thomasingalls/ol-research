//! The files (evaluation plan §6): a suite's queries, an answer key in
//! TREC's qrels format and a run in TREC's run format. Every reader fails
//! on the first line it cannot read, naming the line: a malformed or
//! repeated line is a broken file, never a line to skip.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Write};

/// A line that is not what its file's format says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// One-based.
    pub line: usize,
    #[allow(missing_docs)]
    pub what: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.what)
    }
}

impl std::error::Error for Error {}

/// A line's columns, split as `trec_eval` splits them: at C's `isspace`
/// characters — space, tab, line feed, vertical tab, form feed, carriage
/// return — and nothing else, so a no-break space stays inside a column.
fn columns(line: &str) -> Vec<&str> {
    line.split([' ', '\t', '\n', '\u{b}', '\u{c}', '\r'])
        .filter(|c| !c.is_empty())
        .collect()
}

fn error(line: usize, what: impl Into<String>) -> Error {
    Error {
        line,
        what: what.into(),
    }
}

/// A suite's queries: `id<TAB>text` a line, the text as typed, in file
/// order.
///
/// # Errors
///
/// A line without a tab, an id that is empty or holds white space (the
/// id is a column of the run and the key), or an id seen before.
pub fn read_queries(text: &str) -> Result<Vec<(String, String)>, Error> {
    let mut seen = std::collections::HashSet::new();
    let mut queries = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        let Some((id, query)) = line.split_once('\t') else {
            return Err(error(n, "no tab between the id and the text"));
        };
        check_column(n, "query id", id)?;
        if !seen.insert(id) {
            return Err(error(n, format!("query {id} again")));
        }
        queries.push((id.to_owned(), query.to_owned()));
    }
    Ok(queries)
}

fn check_column(n: usize, what: &str, value: &str) -> Result<(), Error> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return Err(error(
            n,
            format!("{what} {value:?} is empty or holds white space"),
        ));
    }
    Ok(())
}

/// An answer key: by query, by work, the grade a judge gave. A work
/// absent from a query's map was not judged for it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Key {
    #[allow(missing_docs)]
    pub queries: BTreeMap<String, BTreeMap<String, i64>>,
}

impl Key {
    /// Reads TREC's qrels: `query iteration work grade`, white space
    /// between. The iteration column is ignored, as `trec_eval` ignores
    /// it.
    ///
    /// # Errors
    ///
    /// A line without four columns, a grade that is not an integer, or a
    /// work judged twice for one query.
    pub fn read(text: &str) -> Result<Self, Error> {
        let mut queries: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            let n = n + 1;
            let columns = columns(line);
            let [query, _iteration, work, grade] = columns[..] else {
                return Err(error(n, format!("{} columns, not 4", columns.len())));
            };
            let grade: i64 = grade
                .parse()
                .map_err(|_| error(n, format!("grade {grade:?} is not an integer")))?;
            let judged = queries.entry(query.to_owned()).or_default();
            if judged.insert(work.to_owned(), grade).is_some() {
                return Err(error(n, format!("{work} judged twice for query {query}")));
            }
        }
        Ok(Self { queries })
    }

    /// Writes TREC's qrels, queries and works in order.
    ///
    /// # Errors
    ///
    /// Writing.
    pub fn write(&self, out: &mut impl Write) -> io::Result<()> {
        for (query, judged) in &self.queries {
            for (work, grade) in judged {
                writeln!(out, "{query} 0 {work} {grade}")?;
            }
        }
        Ok(())
    }

    /// The positive grades the key uses, lowest first: one for a binary
    /// key, `[1, 2]` for TREC's usual three grades.
    #[must_use]
    pub fn positive_grades(&self) -> Vec<i64> {
        let mut grades: Vec<i64> = self
            .queries
            .values()
            .flat_map(BTreeMap::values)
            .copied()
            .filter(|g| *g > 0)
            .collect();
        grades.sort_unstable();
        grades.dedup();
        grades
    }
}

/// One returned work and its score.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    #[allow(missing_docs)]
    pub work: String,
    #[allow(missing_docs)]
    pub score: f64,
}

/// A run: by query, the works returned, in rank order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Run {
    #[allow(missing_docs)]
    pub queries: BTreeMap<String, Vec<Scored>>,
}

impl Run {
    /// Reads TREC's run format: `query Q0 work rank score tag`, white
    /// space between, and puts each query's works in rank order as
    /// `trec_eval` does ([`order`]) — the rank column is ignored, as it
    /// ignores it.
    ///
    /// # Errors
    ///
    /// A line without six columns, a score that is not a finite number,
    /// or a work returned twice for one query.
    pub fn read(text: &str) -> Result<Self, Error> {
        let mut queries: BTreeMap<String, Vec<Scored>> = BTreeMap::new();
        let mut seen: BTreeMap<String, std::collections::HashSet<String>> = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            let n = n + 1;
            let columns = columns(line);
            let [query, _q0, work, _rank, score, _tag] = columns[..] else {
                return Err(error(n, format!("{} columns, not 6", columns.len())));
            };
            let score: f64 = score
                .parse()
                .ok()
                .filter(|s: &f64| s.is_finite())
                .ok_or_else(|| error(n, format!("score {score:?} is not a finite number")))?;
            if !seen
                .entry(query.to_owned())
                .or_default()
                .insert(work.to_owned())
            {
                return Err(error(n, format!("{work} returned twice for query {query}")));
            }
            queries.entry(query.to_owned()).or_default().push(Scored {
                work: work.to_owned(),
                score,
            });
        }
        for works in queries.values_mut() {
            order(works);
        }
        Ok(Self { queries })
    }

    /// Writes TREC's run format in rank order, ranks from 1, `tag` in the
    /// last column. The score is written as the shortest text that reads
    /// back as the same number, so `trec_eval` orders the file as this
    /// crate does.
    ///
    /// # Errors
    ///
    /// Writing.
    ///
    /// # Panics
    ///
    /// When `tag` is empty or holds white space.
    pub fn write(&self, tag: &str, out: &mut impl Write) -> io::Result<()> {
        assert!(
            !tag.is_empty() && !tag.chars().any(char::is_whitespace),
            "a run's tag is one column"
        );
        for (query, works) in &self.queries {
            for (i, scored) in works.iter().enumerate() {
                writeln!(
                    out,
                    "{query} Q0 {} {} {} {tag}",
                    scored.work,
                    i + 1,
                    scored.score
                )?;
            }
        }
        Ok(())
    }
}

/// Puts a query's works in `trec_eval`'s rank order (`comp_sim_docno`
/// in its `form_res_rels.c`): score descending, and among equal scores
/// the work named later in byte order first — `strcmp` of the second
/// against the first. Scores compare as numbers, so 0 and -0 are equal.
///
/// # Panics
///
/// On a score that is not a number; [`Run::read`] refuses one.
pub fn order(works: &mut [Scored]) {
    works.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .expect("a run's scores are numbers")
            .then_with(|| b.work.as_bytes().cmp(a.work.as_bytes()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_keep_their_text_and_order() {
        let got = read_queries("q2\tIL-6 receptor\nq1\t  spaced\ttabbed \n").unwrap();
        assert_eq!(
            got,
            vec![
                ("q2".to_owned(), "IL-6 receptor".to_owned()),
                ("q1".to_owned(), "  spaced\ttabbed ".to_owned()),
            ]
        );
    }

    #[test]
    fn a_query_file_fails_at_its_first_bad_line() {
        assert_eq!(read_queries("q1\ta\nq2 b\n").unwrap_err().line, 2);
        assert_eq!(read_queries("q1\ta\nq1\tb\n").unwrap_err().line, 2);
        assert_eq!(read_queries("\ta\n").unwrap_err().line, 1);
        assert_eq!(read_queries("q 1\ta\n").unwrap_err().line, 1);
    }

    #[test]
    fn a_key_reads_and_writes_back() {
        let text = "1 0 ol:A 2\n1 0 ol:B 0\n2 0 ol:A 1\n";
        let key = Key::read(text).unwrap();
        assert_eq!(key.queries["1"]["ol:A"], 2);
        assert_eq!(key.positive_grades(), vec![1, 2]);
        let mut out = Vec::new();
        key.write(&mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), text);
    }

    #[test]
    fn a_key_fails_at_its_first_bad_line() {
        assert_eq!(Key::read("1 0 a 1\n1 0 a 0\n").unwrap_err().line, 2);
        assert_eq!(Key::read("1 0 a\n").unwrap_err().line, 1);
        assert_eq!(Key::read("1 0 a x\n").unwrap_err().line, 1);
    }

    #[test]
    fn a_run_is_ordered_by_score_then_work_descending() {
        let text = "1 Q0 a 1 1.5 t\n1 Q0 b 2 2 t\n1 Q0 c 3 1.5 t\n";
        let run = Run::read(text).unwrap();
        let order: Vec<&str> = run.queries["1"].iter().map(|s| s.work.as_str()).collect();
        assert_eq!(order, vec!["b", "c", "a"]);
        let mut out = Vec::new();
        run.write("t", &mut out).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "1 Q0 b 1 2 t\n1 Q0 c 2 1.5 t\n1 Q0 a 3 1.5 t\n"
        );
    }

    #[test]
    fn zero_and_minus_zero_are_one_score() {
        let run = Run::read("1 Q0 a 1 -0 t\n1 Q0 b 2 0 t\n").unwrap();
        let order: Vec<&str> = run.queries["1"].iter().map(|s| s.work.as_str()).collect();
        assert_eq!(order, vec!["b", "a"]);
    }

    #[test]
    fn columns_part_where_c_says_space() {
        // A vertical tab parts columns; a no-break space does not.
        let key = Key::read("1\u{b}0 a\u{a0}b 1\r\n").unwrap();
        assert_eq!(key.queries["1"]["a\u{a0}b"], 1);
    }

    #[test]
    fn a_run_fails_at_its_first_bad_line() {
        assert_eq!(
            Run::read("1 Q0 a 1 1 t\n1 Q0 a 2 0 t\n").unwrap_err().line,
            2
        );
        assert_eq!(Run::read("1 Q0 a 1 NaN t\n").unwrap_err().line, 1);
        assert_eq!(Run::read("1 Q0 a 1 1\n").unwrap_err().line, 1);
    }

    #[test]
    fn a_written_score_reads_back_exactly() {
        let score = f64::from(0.1_f32);
        let run = Run {
            queries: BTreeMap::from([(
                "1".to_owned(),
                vec![Scored {
                    work: "a".to_owned(),
                    score,
                }],
            )]),
        };
        let mut out = Vec::new();
        run.write("t", &mut out).unwrap();
        let back = Run::read(&String::from_utf8(out).unwrap()).unwrap();
        assert_eq!(back.queries["1"][0].score.to_bits(), score.to_bits());
    }
}
