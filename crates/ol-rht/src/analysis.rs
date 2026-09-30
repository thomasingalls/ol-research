//! T0 of the tokenizer plan (`docs/tokenizer-plan.md` §5): what analysis
//! does to the corpus, and what it did to the baseline's misses, measured
//! before anything is changed. Both commands read the store and write
//! nothing; [`search::analysis`] holds the analyzer as the index uses it
//! and each candidate change of form as a function over terms.
//!
//! - `survey analysis`, the census: every held work's title and abstract
//!   against the invariants of the plan's §2 — text that yields no terms
//!   (I1); runs the length cap drops, by script and length (I5, D3); runs
//!   of the scripts written without spaces (D2); how often the spellings
//!   I3 would bring together occur — accented Latin letters,
//!   compatibility forms, Greek letters, hyphen joins, letters beside
//!   digits (D4–D6); short all-capital words (I4, D7). Over the whole
//!   corpus, by the merged record's language, and by the title's script,
//!   since a work with no language still has a script.
//! - `evaluate misses`, the miss analysis: over a suite's working half and
//!   a baseline run, each relevant work the run placed past 10 or missed,
//!   and whether each change would make more of the query's words meet
//!   the work's text. A ceiling on what the change can gain; beside it the
//!   same count for the judged non-relevant works of the run's top 10,
//!   since a change that makes words meet there too costs precision.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Instant;

use ingest::Store;
use merge::WorkId;
use model::Lang;
use search::analysis::{self, Analyzers, CAP, Script};

use super::Options;

/// Examples kept of each class.
const EXAMPLES: usize = 3;
/// Characters of an example shown.
const SNIPPET: usize = 48;
/// Works handed to a census thread at a time.
const BATCH: usize = 4096;
/// Rows between progress lines on stderr.
const PROGRESS: u64 = 10_000_000;
/// Words shown of those no change reaches.
const SHOWN: usize = 25;

/// Byte lengths of the runs the cap drops: each bucket's upper bound,
/// exclusive, and its label.
const BUCKETS: [(usize, &str); 5] = [
    (64, "40–63"),
    (128, "64–127"),
    (256, "128–255"),
    (512, "256–511"),
    (usize::MAX, "512+"),
];

fn share(n: u64, of: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    if of == 0 {
        0.0
    } else {
        100.0 * n as f64 / of as f64
    }
}

/// An example as a report prints it: the work, and the text cut short.
fn example(public: Option<&str>, id: WorkId, text: &str) -> String {
    let cut: String = text.chars().take(SNIPPET).collect();
    let more = if cut.len() < text.len() { "…" } else { "" };
    match public {
        Some(public) => format!("{public} `{cut}{more}`"),
        None => format!("work {id} `{cut}{more}`"),
    }
}

/// A class's occurrences, the works it occurs in, and examples — the
/// first works by id, one example each.
#[derive(Debug, Default)]
struct Class {
    n: u64,
    works: u64,
    examples: Vec<(WorkId, String)>,
}

impl Class {
    fn add(&mut self, id: WorkId, public: Option<&str>, text: &str) {
        self.n += 1;
        if self.examples.len() < EXAMPLES && self.examples.last().is_none_or(|(at, _)| *at != id) {
            self.examples.push((id, example(public, id, text)));
        }
    }

    fn absorb(&mut self, other: Self) {
        self.n += other.n;
        self.works += other.works;
        self.examples.extend(other.examples);
        self.examples.sort_by_key(|(id, _)| *id);
        self.examples.truncate(EXAMPLES);
    }

    fn print(&self, out: &mut String, what: &str) {
        let _ = writeln!(out, "  {what}: {} in {} works", self.n, self.works);
        for (_, e) in &self.examples {
            let _ = writeln!(out, "      {e}");
        }
    }
}

/// What the census marks a run with.
#[derive(Debug, Clone, Copy)]
enum Mark {
    Capped,
    Unspaced,
    Accented,
    Compatibility,
    GreekAlone,
    GreekJoined,
    /// Mu, or the micro sign, and no other Greek letter, joined to other
    /// letters or digits: `µg`, `µm` — a unit's prefix more often than
    /// the letter, so D5's name (`mug`) would be wrong for most of them.
    MuJoined,
    GreekWord,
    Hyphen,
    Parts,
    Caps,
}

const MARKS: usize = 11;

/// The columns of the tables by language and by script: works with at
/// least one run so marked.
const COLUMNS: [(&str, &[Mark]); 7] = [
    ("capped", &[Mark::Capped]),
    ("unspaced", &[Mark::Unspaced]),
    ("accented", &[Mark::Accented]),
    ("compat", &[Mark::Compatibility]),
    ("greek", &[Mark::GreekAlone, Mark::GreekJoined]),
    ("joins", &[Mark::Hyphen, Mark::Parts]),
    ("caps", &[Mark::Caps]),
];

/// One language's or one script's works.
#[derive(Debug, Default, Clone)]
struct Tally {
    works: u64,
    /// No title or abstract with a letter or digit in it.
    wordless: u64,
    titled: u64,
    title_none: u64,
    abstracted: u64,
    abstract_none: u64,
    /// Every title and abstract it has yields no terms.
    all_none: u64,
    columns: [u64; COLUMNS.len()],
}

impl Tally {
    fn absorb(&mut self, other: &Self) {
        self.works += other.works;
        self.wordless += other.wordless;
        self.titled += other.titled;
        self.title_none += other.title_none;
        self.abstracted += other.abstracted;
        self.abstract_none += other.abstract_none;
        self.all_none += other.all_none;
        for (a, b) in self.columns.iter_mut().zip(other.columns) {
            *a += b;
        }
    }
}

/// The runs of one script written without spaces.
#[derive(Debug, Default, Clone, Copy)]
struct Unspaced {
    runs: u64,
    /// Under the cap: the whole run one term.
    kept: u64,
    chars: u64,
}

/// One work as the census reads it.
#[derive(Debug)]
struct Item {
    id: WorkId,
    public: Option<String>,
    lang: Option<Lang>,
    title: Option<String>,
    abstract_text: Option<String>,
}

#[derive(Debug, Default)]
struct Census {
    all: Tally,
    by_lang: HashMap<Option<Lang>, Tally>,
    by_script: HashMap<Option<Script>, Tally>,
    marks: [Class; MARKS],
    title_none: Class,
    capped: BTreeMap<Script, [u64; BUCKETS.len()]>,
    /// Of the scripts written with spaces: D3's long terms.
    capped_examples: [Class; BUCKETS.len()],
    unspaced: BTreeMap<Script, Unspaced>,
}

/// A text with a letter or digit in it.
fn worded(text: Option<&str>) -> Option<&str> {
    text.filter(|t| t.chars().any(char::is_alphanumeric))
}

/// The script most of a text's letters are in; `None` without letters.
fn dominant(text: &str) -> Option<Script> {
    let mut letters = [0u64; Script::ALL.len()];
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        letters[analysis::script(c) as usize] += 1;
    }
    let (at, n) = letters
        .iter()
        .enumerate()
        .rev()
        .max_by_key(|(_, n)| **n)
        .unwrap_or((0, &0));
    (*n > 0).then_some(Script::ALL[at])
}

impl Census {
    fn work(&mut self, w: &Item) {
        let mut marked = [false; MARKS];
        let mut one = Tally {
            works: 1,
            ..Tally::default()
        };
        let (title, abstract_text) = (
            worded(w.title.as_deref()),
            worded(w.abstract_text.as_deref()),
        );
        let mut none = [false; 2];
        for (i, text) in [title, abstract_text].into_iter().enumerate() {
            let Some(text) = text else { continue };
            let runs = analysis::runs(text);
            none[i] = !runs.iter().any(|(_, r)| r.len() < CAP);
            for (_, run) in &runs {
                self.run(w, run, &mut marked);
            }
            for span in analysis::hyphenated(text, &runs) {
                marked[Mark::Hyphen as usize] = true;
                self.marks[Mark::Hyphen as usize].add(w.id, w.public.as_deref(), span);
            }
        }
        one.wordless = u64::from(title.is_none() && abstract_text.is_none());
        one.titled = u64::from(title.is_some());
        one.title_none = u64::from(none[0]);
        one.abstracted = u64::from(abstract_text.is_some());
        one.abstract_none = u64::from(none[1]);
        one.all_none = u64::from(
            (title.is_some() || abstract_text.is_some())
                && (title.is_none() || none[0])
                && (abstract_text.is_none() || none[1]),
        );
        if let (Some(t), true) = (title, none[0]) {
            self.title_none.add(w.id, w.public.as_deref(), t);
        }
        for (class, was) in self.marks.iter_mut().zip(marked) {
            class.works += u64::from(was);
        }
        for (column, (_, marks)) in one.columns.iter_mut().zip(COLUMNS) {
            *column = u64::from(marks.iter().any(|m| marked[*m as usize]));
        }
        self.all.absorb(&one);
        self.by_lang.entry(w.lang).or_default().absorb(&one);
        self.by_script
            .entry(title.and_then(dominant))
            .or_default()
            .absorb(&one);
    }

    fn run(&mut self, w: &Item, run: &str, marked: &mut [bool; MARKS]) {
        let (mut greek, mut mu, mut other) = (0u32, 0u32, 0u32);
        let (mut accented, mut compatibility) = (false, false);
        for c in run.chars() {
            if analysis::greek_name(c).is_some() {
                greek += 1;
                mu += u32::from(matches!(c, 'µ' | 'μ'));
            } else {
                other += 1;
            }
            accented |= analysis::is_accented_latin(c);
            compatibility |= analysis::is_compatibility(c);
        }
        let script = dominant(run).unwrap_or(Script::Common);
        let mut mark = |m: Mark, census: &mut Self| {
            marked[m as usize] = true;
            census.marks[m as usize].add(w.id, w.public.as_deref(), run);
        };
        if run.len() >= CAP {
            mark(Mark::Capped, self);
            let bucket = BUCKETS
                .iter()
                .position(|(below, _)| run.len() < *below)
                .unwrap_or(BUCKETS.len() - 1);
            self.capped.entry(script).or_default()[bucket] += 1;
            if !script.unspaced() {
                self.capped_examples[bucket].add(w.id, w.public.as_deref(), run);
            }
        }
        if script.unspaced() {
            mark(Mark::Unspaced, self);
            let u = self.unspaced.entry(script).or_default();
            u.runs += 1;
            u.kept += u64::from(run.len() < CAP);
            u.chars += run.chars().count() as u64;
        }
        if accented {
            mark(Mark::Accented, self);
        }
        if compatibility {
            mark(Mark::Compatibility, self);
        }
        match (greek, other) {
            (0, _) => {}
            (1, 0) => mark(Mark::GreekAlone, self),
            (_, 0) => mark(Mark::GreekWord, self),
            _ if mu == greek => mark(Mark::MuJoined, self),
            _ => mark(Mark::GreekJoined, self),
        }
        if analysis::is_split(run) {
            mark(Mark::Parts, self);
        }
        if (2..=5).contains(&run.len()) && run.bytes().all(|b| b.is_ascii_uppercase()) {
            mark(Mark::Caps, self);
        }
    }

    fn absorb(&mut self, other: Self) {
        self.all.absorb(&other.all);
        for (k, t) in other.by_lang {
            self.by_lang.entry(k).or_default().absorb(&t);
        }
        for (k, t) in other.by_script {
            self.by_script.entry(k).or_default().absorb(&t);
        }
        for (a, b) in self.marks.iter_mut().zip(other.marks) {
            a.absorb(b);
        }
        self.title_none.absorb(other.title_none);
        for (k, v) in other.capped {
            let row = self.capped.entry(k).or_default();
            for (a, b) in row.iter_mut().zip(v) {
                *a += b;
            }
        }
        for (a, b) in self.capped_examples.iter_mut().zip(other.capped_examples) {
            a.absorb(b);
        }
        for (k, v) in other.unspaced {
            let u = self.unspaced.entry(k).or_default();
            u.runs += v.runs;
            u.kept += v.kept;
            u.chars += v.chars;
        }
    }
}

/// What the reader saw beside the works it handed on.
#[derive(Debug, Default)]
struct Read {
    rows: u64,
    stubs: u64,
    without_text: u64,
}

/// `survey analysis [--store <root>] [--workers N]`.
pub(crate) fn census(opts: &Options) -> ExitCode {
    if !opts.paths.is_empty()
        || opts.out.is_some()
        || opts.limit.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
        || opts.survey
    {
        return super::usage();
    }
    print(census_of(&opts.store(), opts.workers()), "survey analysis")
}

fn print(report: Result<String, String>, what: &str) -> ExitCode {
    let report = match report {
        Ok(report) => report,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut out = io::stdout().lock();
    match out.write_all(report.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("stdout: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Every row in id order from the reader; the works dealt to `workers`
/// threads a batch at a time, in turn, each thread with a queue of its
/// own so a waiting one sleeps; each counts its own, the counts added at
/// the end — examples are the first works by id whichever thread saw them.
fn census_of(store: &Store, workers: usize) -> Result<String, String> {
    let what = "survey analysis";
    let serving = serve::Serving::open(store).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let (senders, receivers): (Vec<_>, Vec<_>) = (0..workers.max(1))
        .map(|_| mpsc::sync_channel::<Vec<Item>>(2))
        .unzip();
    let (read, census) = std::thread::scope(|scope| {
        let threads: Vec<_> = receivers
            .into_iter()
            .map(|rx| {
                scope.spawn(move || {
                    let mut census = Census::default();
                    for batch in rx {
                        for item in &batch {
                            census.work(item);
                        }
                    }
                    census
                })
            })
            .collect();
        let mut turn = senders.iter().cycle();
        let mut read = Read::default();
        let mut batch = Vec::with_capacity(BATCH);
        let result = serving.each_row(|row| {
            read.rows += 1;
            if read.rows % PROGRESS == 0 {
                eprintln!(
                    "{what}: {} rows read, {:.0} s",
                    read.rows,
                    started.elapsed().as_secs_f64()
                );
            }
            if row.stub {
                read.stubs += 1;
                return Ok(());
            }
            let Some(text) = row.text else {
                read.without_text += 1;
                return Ok(());
            };
            batch.push(Item {
                id: row.id,
                public: row.public,
                lang: row.fields.language.map(|f| f.value),
                title: text.title,
                abstract_text: text.abstract_text,
            });
            if batch.len() == BATCH {
                let full = std::mem::replace(&mut batch, Vec::with_capacity(BATCH));
                // A send fails only when its thread has panicked, which
                // the join below reports.
                let _ = turn.next().map(|tx| tx.send(full));
            }
            Ok(())
        });
        if !batch.is_empty() {
            let _ = turn.next().map(|tx| tx.send(batch));
        }
        drop(turn);
        drop(senders);
        let mut census = Census::default();
        for t in threads {
            census.absorb(t.join().expect("a census thread panicked"));
        }
        result.map(|()| (read, census))
    })
    .map_err(|e| e.to_string())?;
    Ok(census_report(&serving, &read, &census, started))
}

#[allow(clippy::too_many_lines)]
fn census_report(serving: &serve::Serving, read: &Read, c: &Census, started: Instant) -> String {
    let mut out = String::new();
    let epoch = serving.epoch();
    let a = &c.all;
    let _ = writeln!(
        out,
        "survey analysis: epoch {} (batch {}), {} rows: {} held works with text, {} stubs, {} held works without text; {:.0} s",
        epoch.epoch,
        epoch.batch,
        read.rows,
        a.works,
        read.stubs,
        read.without_text,
        started.elapsed().as_secs_f64()
    );
    let _ = writeln!(
        out,
        "  read: each work's title and abstract, split as the index's analyzer splits them ({}): runs of letters and digits, a run of {CAP} bytes or more dropped",
        search::schema::DESCRIPTION
    );
    let _ = writeln!(out, "  examples: the first works by id, one example each\n");

    let _ = writeln!(out, "I1 — text that yields no terms");
    let _ = writeln!(
        out,
        "  works with a title (a letter or digit in it): {}; whose title yields no terms: {} ({:.3}%)",
        a.titled,
        a.title_none,
        share(a.title_none, a.titled)
    );
    for (_, e) in &c.title_none.examples {
        let _ = writeln!(out, "      {e}");
    }
    let _ = writeln!(
        out,
        "  works with an abstract: {}; whose abstract yields no terms: {} ({:.3}%)",
        a.abstracted,
        a.abstract_none,
        share(a.abstract_none, a.abstracted)
    );
    let _ = writeln!(
        out,
        "  works whose title and abstract, as far as they have them, yield no terms: {} ({:.3}% of works)",
        a.all_none,
        share(a.all_none, a.works)
    );
    let _ = writeln!(
        out,
        "  works with no title or abstract with a letter or digit (findable by authors and venue alone): {}\n",
        a.wordless
    );

    let _ = writeln!(
        out,
        "I5, D3 — runs the cap drops, by the run's script and its length in bytes"
    );
    let _ = write!(out, "  {:<12}", "script");
    for (_, label) in BUCKETS {
        let _ = write!(out, " {label:>12}");
    }
    let _ = writeln!(out, " {:>12}", "total");
    for (script, row) in &c.capped {
        let _ = write!(out, "  {:<12}", script.name());
        for n in row {
            let _ = write!(out, " {n:>12}");
        }
        let _ = writeln!(out, " {:>12}", row.iter().sum::<u64>());
    }
    let _ = writeln!(
        out,
        "  of the scripts written with spaces, examples by length:"
    );
    for ((_, label), class) in BUCKETS.iter().zip(&c.capped_examples) {
        for (_, e) in &class.examples {
            let _ = writeln!(out, "      {label:>8}  {e}");
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "D2 — runs of the scripts written without spaces");
    let _ = writeln!(
        out,
        "  {:<12} {:>14} {:>14} {:>14} {:>12}",
        "script", "runs", "one term each", "dropped", "mean chars"
    );
    for (script, u) in &c.unspaced {
        #[allow(clippy::cast_precision_loss)]
        let mean = if u.runs == 0 {
            0.0
        } else {
            u.chars as f64 / u.runs as f64
        };
        let _ = writeln!(
            out,
            "  {:<12} {:>14} {:>14} {:>14} {mean:>12.1}",
            script.name(),
            u.runs,
            u.kept,
            u.runs - u.kept
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "I3, D4–D6 — spellings a reader takes for one word (runs)"
    );
    let m = |mark: Mark| &c.marks[mark as usize];
    m(Mark::Accented).print(&mut out, "with accented Latin letters (D4, folding)");
    m(Mark::Compatibility).print(&mut out, "with compatibility forms (D4, NFKC)");
    m(Mark::GreekAlone).print(&mut out, "a Greek letter alone (D5)");
    m(Mark::MuJoined).print(
        &mut out,
        "mu or the micro sign alone of Greek, joined to letters or digits (µg, µm: units, D5)",
    );
    m(Mark::GreekJoined).print(
        &mut out,
        "other Greek letters joined to letters or digits (D5)",
    );
    m(Mark::GreekWord).print(&mut out, "Greek words, left as they are (D5)");
    m(Mark::Hyphen).print(&mut out, "pairs joined by a hyphen (D6)");
    m(Mark::Parts).print(&mut out, "letters beside digits or another script (D6)");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "I4, D7 — short all-capital words, 2 to 5 letters (runs)"
    );
    m(Mark::Caps).print(&mut out, "all capitals");
    let _ = writeln!(out);

    let header = |out: &mut String, first: &str| {
        let _ = write!(
            out,
            "  {first:<28} {:>11} {:>11} {:>11} {:>11}",
            "works", "title none", "abs. none", "all none"
        );
        for (name, _) in COLUMNS {
            let _ = write!(out, " {name:>10}");
        }
        let _ = writeln!(out);
    };
    let row = |out: &mut String, first: &str, t: &Tally| {
        let _ = write!(
            out,
            "  {first:<28} {:>11} {:>11} {:>11} {:>11}",
            t.works, t.title_none, t.abstract_none, t.all_none
        );
        for n in t.columns {
            let _ = write!(out, " {n:>10}");
        }
        let _ = writeln!(out);
    };
    let _ = writeln!(
        out,
        "By language (the merged record's; und: none given) — works, those whose text yields no terms, and works with each class"
    );
    header(&mut out, "language");
    let mut langs: Vec<_> = c.by_lang.iter().collect();
    langs.sort_by(|(ka, a), (kb, b)| b.works.cmp(&a.works).then(ka.cmp(kb)));
    for (lang, t) in langs {
        let name = lang.map_or_else(|| "und".to_owned(), |l| format!("{l} {}", l.name()));
        let name: String = name.chars().take(28).collect();
        row(&mut out, &name, t);
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "By the title's script (most of its letters; none: no title with a letter)"
    );
    header(&mut out, "script");
    let mut scripts: Vec<_> = c.by_script.iter().collect();
    scripts.sort_by(|(ka, a), (kb, b)| b.works.cmp(&a.works).then(ka.cmp(kb)));
    for (script, t) in scripts {
        row(&mut out, script.map_or("none", Script::name), t);
    }
    out
}

/// Where a judged work stands in the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// Relevant, ranked 1 to 10.
    Found,
    /// Relevant, ranked past 10.
    Late,
    /// Relevant, not in the run.
    Missed,
    /// Judged not relevant, ranked 1 to 10.
    Wrong,
}

const STANDINGS: [(Standing, &str); 4] = [
    (Standing::Found, "relevant, in the top 10"),
    (Standing::Late, "relevant, ranked past 10"),
    (Standing::Missed, "relevant, not in the run"),
    (Standing::Wrong, "judged not relevant, in the top 10"),
];

/// A candidate change of form, as the miss analysis asks it.
#[derive(Debug, Clone, Copy)]
enum Change {
    Cap,
    Fold,
    Greek,
    Joins,
    Stem,
    All,
}

const CHANGES: [(Change, &str); 6] = [
    (Change::Cap, "cap lifted (D3)"),
    (Change::Fold, "folding (D4)"),
    (Change::Greek, "Greek names (D5)"),
    (Change::Joins, "joins (D6)"),
    (Change::Stem, "English stem (D8)"),
    (Change::All, "all of them"),
];

/// A work's text in the form each change would index it.
struct Forms {
    today: HashSet<String>,
    uncapped: HashSet<String>,
    folded: HashSet<String>,
    greek: HashSet<String>,
    joined: HashSet<String>,
    stemmed: HashSet<String>,
    all: HashSet<String>,
}

impl Forms {
    fn of(a: &mut Analyzers, text: &str) -> Self {
        let today: HashSet<String> = a.terms(text).into_iter().collect();
        let uncapped: HashSet<String> = a.uncapped(text).into_iter().collect();
        let joins = analysis::joins_and_splits(text);
        let mut folded = HashSet::with_capacity(today.len());
        let mut stemmed = HashSet::with_capacity(today.len());
        let mut greek = today.clone();
        for t in &today {
            folded.insert(a.folded(t));
            stemmed.insert(a.stemmed(t));
            greek.insert(analysis::greek_named(t).into_owned());
        }
        let mut joined = today.clone();
        joined.extend(joins.iter().filter(|t| t.len() < CAP).cloned());
        let mut all = HashSet::with_capacity(uncapped.len() * 2);
        for t in uncapped.iter().chain(&joins) {
            all.insert(Self::every(a, t));
            all.insert(Self::every(a, &analysis::greek_named(t)));
        }
        Self {
            today,
            uncapped,
            folded,
            greek,
            joined,
            stemmed,
            all,
        }
    }

    /// A term under every change of form at once.
    fn every(a: &mut Analyzers, term: &str) -> String {
        let folded = a.folded(term);
        a.stemmed(&folded)
    }

    /// Whether the query's word `q` meets the text under `change`, or
    /// today when `change` is `None`; `kept` is whether today's analyzer
    /// keeps `q` in the query at all.
    fn met(&self, a: &mut Analyzers, change: Option<Change>, q: &str, kept: bool) -> bool {
        let today = kept && self.today.contains(q);
        let parts = || analysis::parts(q);
        today
            || match change {
                None => false,
                Some(Change::Cap) => self.uncapped.contains(q),
                Some(Change::Fold) => kept && self.folded.contains(&a.folded(q)),
                Some(Change::Greek) => {
                    kept && self.greek.contains(analysis::greek_named(q).as_ref())
                }
                Some(Change::Joins) => {
                    kept && (self.joined.contains(q) || {
                        let parts = parts();
                        parts.len() > 1 && parts.iter().all(|p| self.joined.contains(*p))
                    })
                }
                Some(Change::Stem) => kept && self.stemmed.contains(&a.stemmed(q)),
                Some(Change::All) => {
                    let named = analysis::greek_named(q);
                    self.all.contains(&Self::every(a, q))
                        || self.all.contains(&Self::every(a, &named))
                        || {
                            let parts = parts();
                            parts.len() > 1
                                && parts.iter().all(|p| {
                                    let named = analysis::greek_named(p);
                                    self.all.contains(&Self::every(a, &named))
                                })
                        }
                }
            }
    }
}

/// The pairs of one standing: a query and a judged work.
#[derive(Debug, Default)]
struct Group {
    pairs: u64,
    queries: BTreeSet<String>,
    without_abstract: u64,
    not_in_store: u64,
    without_text: u64,
    outside_years: u64,
    /// The query's words, over every pair.
    words: u64,
    met_today: u64,
    met: [u64; CHANGES.len()],
    gaining: [u64; CHANGES.len()],
    from_none: [u64; CHANGES.len()],
}

/// A missed or late pair a change reaches: query, work, the word.
type Reached = (String, String, String);

#[derive(Debug, Default)]
struct Misses {
    queries: usize,
    groups: [Group; STANDINGS.len()],
    unreached: HashMap<String, u64>,
    examples: [Vec<Reached>; CHANGES.len()],
}

/// `evaluate misses <suite> <run> [--store <root>]`.
pub(crate) fn misses(opts: &Options) -> ExitCode {
    let [suite, run] = opts.paths.as_slice() else {
        return super::usage();
    };
    if opts.out.is_some()
        || opts.limit.is_some()
        || opts.workers.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
        || opts.survey
    {
        return super::usage();
    }
    print(misses_of(&opts.store(), suite, run), "evaluate misses")
}

/// A value of a run's `.about`, named when missing.
fn about_field<'a>(
    about: &'a serde_json::Value,
    path: &[&str],
) -> Result<&'a serde_json::Value, String> {
    path.iter()
        .try_fold(about, |v, k| v.get(k))
        .ok_or_else(|| format!("the run's .about has no {}", path.join(".")))
}

#[allow(clippy::too_many_lines)]
fn misses_of(store: &Store, suite_path: &Path, run_path: &Path) -> Result<String, String> {
    use evaluation::suite::{Half, Suite};
    if !suite_path.is_dir() {
        return Err(format!(
            "{}: not a suite's directory; the suite's halves decide which queries are read",
            suite_path.display()
        ));
    }
    let suite = Suite::read(suite_path).map_err(|e| e.to_string())?;
    if suite.is_held_out() {
        return Err(format!(
            "{}: held out, read at milestones only (evaluation plan §7); misses reads working halves",
            suite_path.display()
        ));
    }
    let key = suite.key_over(Half::Working);
    let read = |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
    let run = evaluation::trec::Run::read(&read(run_path)?)
        .map_err(|e| format!("{}: {e}", run_path.display()))?;
    let about_path = std::path::PathBuf::from(format!("{}.about", run_path.display()));
    let about: serde_json::Value = serde_json::from_str(&read(&about_path)?)
        .map_err(|e| format!("{}: {e}", about_path.display()))?;
    let epoch = about_field(&about, &["epoch", "epoch"])?
        .as_u64()
        .ok_or("the run's .about: epoch.epoch is not a number")?;
    let restricted = about_field(&about, &["restricted"])?
        .as_bool()
        .ok_or("the run's .about: restricted is not true or false")?;
    let depth = about_field(&about, &["depth"])?
        .as_u64()
        .ok_or("the run's .about: depth is not a number")?;
    // The run's own epoch, so the text read is the text it searched.
    let serving = serve::Serving::open_epoch(store, epoch).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let asked: HashMap<&str, &str> = suite
        .queries
        .iter()
        .map(|(id, q)| (id.as_str(), q.as_str()))
        .collect();
    let mut a = Analyzers::default();
    let mut m = Misses::default();
    for (qid, judged) in &key.queries {
        let Some(query) = asked.get(qid.as_str()) else {
            return Err(format!("query {qid} is judged but not asked"));
        };
        if !judged.values().any(|g| *g > 0) {
            continue;
        }
        m.queries += 1;
        let mut ranked = run.queries.get(qid).cloned().unwrap_or_default();
        evaluation::trec::order(&mut ranked);
        let rank: HashMap<&str, usize> = ranked
            .iter()
            .enumerate()
            .map(|(i, s)| (s.work.as_str(), i + 1))
            .collect();
        let mut pairs = Vec::new();
        for (work, grade) in judged {
            let at = rank.get(work.as_str()).copied();
            let standing = match (*grade, at) {
                (g, Some(r)) if g > 0 && r <= 10 => Standing::Found,
                (g, Some(_)) if g > 0 => Standing::Late,
                (g, None) if g > 0 => Standing::Missed,
                (0, Some(r)) if r <= 10 => Standing::Wrong,
                _ => continue,
            };
            let id = match serving.resolve(work) {
                Ok(Some(serve::Resolved::One(id))) => Some(id),
                _ => None,
            };
            pairs.push((work, standing, id));
        }
        let ids: Vec<WorkId> = pairs.iter().filter_map(|(_, _, id)| *id).collect();
        let rows = serving.rows(&ids).map_err(|e| e.to_string())?;
        let until = suite.filters.get(qid).and_then(|f| f.until);
        let words: Vec<String> = {
            let mut seen = HashSet::new();
            a.uncapped(query)
                .into_iter()
                .filter(|w| seen.insert(w.clone()))
                .collect()
        };
        let kept: HashSet<String> = a.terms(query).into_iter().collect();
        for (work, standing, id) in pairs {
            let g = &mut m.groups[standing as usize];
            let Some(row) = id.and_then(|id| rows.get(&id)) else {
                g.not_in_store += 1;
                continue;
            };
            let Some(text) = &row.text else {
                g.without_text += 1;
                continue;
            };
            if standing == Standing::Missed && restricted {
                let year = row.fields.published.as_ref().map(|p| p.value.year.get());
                if until.is_some_and(|u| year.is_none_or(|y| y > u)) {
                    g.outside_years += 1;
                    continue;
                }
            }
            let mut body = String::new();
            for part in [
                text.title.as_deref(),
                text.abstract_text.as_deref(),
                text.venue.as_deref(),
            ]
            .into_iter()
            .flatten()
            .chain(text.authors.iter().map(String::as_str))
            {
                body.push_str(part);
                body.push('\n');
            }
            let forms = Forms::of(&mut a, &body);
            g.pairs += 1;
            g.queries.insert(qid.clone());
            g.without_abstract += u64::from(text.abstract_text.is_none());
            g.words += words.len() as u64;
            let mut today = 0u64;
            let mut under = [0u64; CHANGES.len()];
            for w in &words {
                let k = kept.contains(w);
                let met_today = forms.met(&mut a, None, w, k);
                today += u64::from(met_today);
                for (i, (change, _)) in CHANGES.iter().enumerate() {
                    let met = forms.met(&mut a, Some(*change), w, k);
                    under[i] += u64::from(met);
                    if met
                        && !met_today
                        && matches!(standing, Standing::Late | Standing::Missed)
                        && m.examples[i].len() < EXAMPLES
                        && m.examples[i].last().is_none_or(|(q, _, _)| q != qid)
                    {
                        let public = row.public.clone().unwrap_or_else(|| work.clone());
                        m.examples[i].push((qid.clone(), public, w.clone()));
                    }
                    if !met
                        && matches!(change, Change::All)
                        && matches!(standing, Standing::Late | Standing::Missed)
                    {
                        *m.unreached.entry(w.clone()).or_default() += 1;
                    }
                }
            }
            g.met_today += today;
            for (i, n) in under.into_iter().enumerate() {
                g.met[i] += n;
                g.gaining[i] += u64::from(n > today);
                g.from_none[i] += u64::from(today == 0 && n > 0);
            }
        }
    }
    Ok(misses_report(
        &m, suite_path, run_path, &serving, depth, restricted, started,
    ))
}

fn misses_report(
    m: &Misses,
    suite: &Path,
    run: &Path,
    serving: &serve::Serving,
    depth: u64,
    restricted: bool,
    started: Instant,
) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "evaluate misses: {} (working half, {} queries with a relevant work), run {} (epoch {}, depth {depth}, {}); {:.1} s",
        suite.display(),
        m.queries,
        run.display(),
        serving.epoch().epoch,
        if restricted {
            "restricted by year"
        } else {
            "open"
        },
        started.elapsed().as_secs_f64()
    );
    let _ = writeln!(
        out,
        "  relevant: graded above 0 (lenient). A query's words: its terms as the index's analyzer makes them, cap lifted, each once. \
         A word meets a work when the work's title, abstract, authors or venue yield it, today or under the change."
    );
    let _ = writeln!(
        out,
        "  A ceiling: a word meeting a work does not rank it; the judged non-relevant top 10 shows what a change also brings to the wrong works.\n"
    );
    for ((_, label), g) in STANDINGS.iter().zip(&m.groups) {
        let _ = writeln!(
            out,
            "{label}: {} pairs over {} queries ({} without an abstract); {} query words, {} met today ({:.1}%)",
            g.pairs,
            g.queries.len(),
            g.without_abstract,
            g.words,
            g.met_today,
            share(g.met_today, g.words)
        );
        let _ = writeln!(
            out,
            "  not read: {} not in the store, {} without text, {} outside the run's years",
            g.not_in_store, g.without_text, g.outside_years
        );
        let _ = writeln!(
            out,
            "  {:<20} {:>16} {:>9} {:>14} {:>9} {:>16}",
            "change", "words newly met", "", "pairs gaining", "", "pairs from none"
        );
        for (i, (_, name)) in CHANGES.iter().enumerate() {
            let newly = g.met[i] - g.met_today;
            let _ = writeln!(
                out,
                "  {name:<20} {newly:>16} {:>8.2}% {:>14} {:>8.2}% {:>16}",
                share(newly, g.words),
                g.gaining[i],
                share(g.gaining[i], g.pairs),
                g.from_none[i]
            );
        }
        let all = CHANGES.len() - 1;
        let _ = writeln!(
            out,
            "  pairs no change of form adds a word to: {} ({:.1}%)\n",
            g.pairs - g.gaining[all],
            share(g.pairs - g.gaining[all], g.pairs)
        );
    }
    let _ = writeln!(
        out,
        "Words no change reaches, relevant works late or missed (the commonest {SHOWN}; what is left for synonyms and concepts, D12):"
    );
    let mut unreached: Vec<_> = m.unreached.iter().collect();
    unreached.sort_by(|(wa, a), (wb, b)| b.cmp(a).then(wa.cmp(wb)));
    for (w, n) in unreached.into_iter().take(SHOWN) {
        let _ = writeln!(out, "  {n:>8}  {w}");
    }
    let _ = writeln!(
        out,
        "\nExamples, relevant works late or missed: a query word a change makes meet"
    );
    for ((_, name), examples) in CHANGES.iter().zip(&m.examples) {
        for (q, work, w) in examples {
            let _ = writeln!(out, "  {name:<20} query {q}, {work}: `{w}`");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(n: u32, lang: Option<&str>, title: &str, abstract_text: Option<&str>) -> Item {
        Item {
            id: WorkId::new(n).unwrap(),
            public: None,
            lang: lang.map(|l| Lang::from_terminologic(l).unwrap()),
            title: Some(title.to_owned()),
            abstract_text: abstract_text.map(str::to_owned),
        }
    }

    fn census(items: &[Item]) -> Census {
        let mut c = Census::default();
        for i in items {
            c.work(i);
        }
        c
    }

    #[test]
    fn a_chinese_title_yields_no_terms_and_counts_as_unspaced() {
        let c = census(&[
            item(1, Some("zho"), "新型冠状病毒肺炎患者的临床特征分析", None),
            item(
                2,
                Some("eng"),
                "Clinical features of pneumonia",
                Some("短 摘要"),
            ),
        ]);
        assert_eq!((c.all.works, c.all.title_none, c.all.all_none), (2, 1, 1));
        assert_eq!(c.all.abstract_none, 0);
        let zho = &c.by_lang[&Some(Lang::from_terminologic("zho").unwrap())];
        assert_eq!((zho.works, zho.title_none), (1, 1));
        assert_eq!(c.by_script[&Some(Script::Han)].title_none, 1);
        let han = c.unspaced[&Script::Han];
        assert_eq!((han.runs, han.kept), (3, 2));
        assert_eq!(c.capped[&Script::Han].iter().sum::<u64>(), 1);
    }

    #[test]
    fn greek_letters_are_told_apart_from_units_and_words() {
        let c = census(&[item(
            1,
            None,
            "TNF-α and δ13C at 5 µg in λόγος",
            Some("IL-6 and IL6 in ALL"),
        )]);
        let n = |m: Mark| c.marks[m as usize].n;
        assert_eq!(n(Mark::GreekAlone), 1);
        assert_eq!(n(Mark::GreekJoined), 1);
        assert_eq!(n(Mark::MuJoined), 1);
        assert_eq!(n(Mark::GreekWord), 1);
        assert_eq!(n(Mark::Hyphen), 2);
        // `δ13C`, `µg`, `IL6`
        assert_eq!(n(Mark::Parts), 3);
        // `TNF`, `IL`, `ALL`; `IL6` has a digit
        assert_eq!(n(Mark::Caps), 3);
        assert_eq!(c.marks[Mark::Hyphen as usize].works, 1);
        assert_eq!(c.all.title_none, 0);
    }

    #[test]
    fn examples_are_the_first_works_whichever_thread_saw_them() {
        let mut a = census(&[item(5, None, "étude", None), item(6, None, "café", None)]);
        let b = census(&[item(2, None, "naïve", None), item(9, None, "über", None)]);
        a.absorb(b);
        let ids: Vec<u32> = a.marks[Mark::Accented as usize]
            .examples
            .iter()
            .map(|(id, _)| id.get())
            .collect();
        assert_eq!(ids, vec![2, 5, 6]);
        assert_eq!(a.marks[Mark::Accented as usize].works, 4);
    }
}
