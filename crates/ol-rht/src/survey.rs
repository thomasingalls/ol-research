//! `survey placement` (citation fields plan §7, CF0): what the sources say
//! about where a work was published, counted before the chain of
//! containers is designed. It reads the store's normalized sets and, when
//! the store has one at this binary's versions, its merged set, and writes
//! nothing.
//!
//! - Part 1, per source: the shapes of volume, issue, pages and article
//!   number; page ranges by kind; the dates a record gives; and for
//!   Crossref, the serials a record names, by work type.
//! - Part 2, over each work whose merged row holds a PubMed and a Crossref
//!   record, paired as the merge paired them: the values the two agree
//!   on, one giving more, or conflicting (D2); PubMed's shortened page
//!   ranges expanded against Crossref's (D8); the two sources' dates (D4);
//!   the venue's promoted source against the record D3 would choose.
//!
//! Every record is read at its speaking copy (merge plan §8), as the merge
//! reads it. Part 3, what Crossref's reader reads and does not keep, is
//! counted by `crossref normalize --survey`.

use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Write as _};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use ingest::{Kind, Store};
use merge::copies::{Copies, Fate, delivery_of, native_of};
use merge::view::bound;
use merge::{Contributor, Name, SourceId};
use model::Pmid;
use model::date::{Bound, Year};
use model::pages::Pages;
use source_crossref::Crossref;
use source_crossref::normalize::Work as CrossrefWork;
use source_crossref::vocab::{SerialKind, WorkType};
use source_pubmed::Pubmed;
use source_pubmed::attr::PubModel;
use source_pubmed::normalize::{Citation, Published};

use super::{Options, Sets};

/// Examples kept of each shape, kind or class.
const EXAMPLES: usize = 3;
/// Shapes shown of each field, the commonest first.
const SHOWN: usize = 25;
/// A shape's length before it is cut: free text makes long ones.
const SIGNATURE: usize = 40;

/// `survey placement [--store <root>] [--limit N]`.
pub(crate) fn placement(opts: &Options) -> ExitCode {
    if !super::merge_options_ok(opts)
        || opts.survey
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
    {
        return super::usage();
    }
    let limit = opts.limit.map(|n| usize::try_from(n).unwrap_or(usize::MAX));
    let report = match survey(&opts.store(), limit) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("survey placement: {e}");
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

fn survey(store: &Store, limit: Option<usize>) -> Result<String, String> {
    let what = "survey placement";
    let started = Instant::now();
    let pubmed = Sets::find::<Pubmed>(store).map_err(|e| e.to_string())?;
    let crossref = Sets::find::<Crossref>(store).map_err(|e| e.to_string())?;
    let mut copies = Copies::new();
    read_copies::<Pubmed>(&mut copies, &pubmed, limit)?;
    read_copies::<Crossref>(&mut copies, &crossref, limit)?;
    copies.settle().map_err(|e| e.to_string())?;
    super::phase(what, "copies read", started);
    let mut pairs = Pairs::read(store, limit)?;
    super::phase(what, &format!("{} pairs read", pairs.found.len()), started);
    let mut one = PartOne::default();
    for (path, set) in &pubmed.works {
        each::<Pubmed>(path, set, limit, &copies, |w| {
            one.pubmed(w);
            pairs.pubmed(w);
        })?;
    }
    super::phase(what, "PubMed read", started);
    for (path, set) in &crossref.works {
        each::<Crossref>(path, set, limit, &copies, |w| {
            one.crossref(w);
            pairs.crossref(w);
        })?;
    }
    super::phase(what, "Crossref read", started);
    let mut out = String::new();
    let written = (|| -> fmt::Result {
        writeln!(out, "survey placement: {}", store.root().display())?;
        if limit.is_some() {
            writeln!(out, "(--limit: each set read only in part)")?;
        }
        one.report(&mut out)?;
        pairs.report(&mut out)
    })();
    written.map_err(|e| e.to_string())?;
    Ok(out)
}

/// Every copy of one source's records, so that each is read once, at the
/// copy that speaks for it.
fn read_copies<S: Contributor>(
    copies: &mut Copies,
    sets: &Sets,
    limit: Option<usize>,
) -> Result<(), String> {
    for (path, set) in &sets.deletions {
        copies
            .read_deletions::<S>(path, set)
            .map_err(|e| e.to_string())?;
    }
    for (path, set) in &sets.works {
        copies
            .read_works::<S>(path, set, limit, None)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Calls `f` with each record of one works set whose copy there speaks.
fn each<S: Contributor>(
    path: &Path,
    set: &str,
    limit: Option<usize>,
    copies: &Copies,
    mut f: impl FnMut(&S::Work<'_>),
) -> Result<(), String> {
    let io = |e: io::Error| format!("{}: {e}", path.display());
    let reader = merge::sets::open::<S>(path, Kind::Works).map_err(io)?;
    reader.with_table(|| {
        for (i, work) in reader
            .records::<S::Work<'_>>()
            .take(limit.unwrap_or(usize::MAX))
            .enumerate()
        {
            let work = work.map_err(io)?;
            let ordinal = i as u64 + 1;
            let native = native_of::<S>(&work).ok_or_else(|| {
                format!("{}: record {ordinal} names no native id", path.display())
            })?;
            let delivery = delivery_of::<S>(&work, set, ordinal);
            if copies.fate(S::SOURCE, native, &delivery) == Fate::Speaks {
                f(&work);
            }
        }
        Ok(())
    })
}

// --- shapes ---------------------------------------------------------------

/// A value's shape: each run of digits `9`, of letters `a`, of spaces one
/// space, anything else as itself. `123-9` is `9-9`, `S12-S15` `a9-a9`,
/// `iii-xiv` `a-a`.
pub(crate) fn signature(value: &str, out: &mut String) {
    out.clear();
    let mut last = None;
    let mut len = 0;
    for c in value.chars() {
        let class = if c.is_ascii_digit() {
            '9'
        } else if c.is_alphabetic() {
            'a'
        } else if c.is_whitespace() {
            ' '
        } else {
            c
        };
        if matches!(class, '9' | 'a' | ' ') && last == Some(class) {
            continue;
        }
        if len == SIGNATURE {
            out.push('…');
            return;
        }
        out.push(class);
        len += 1;
        last = Some(class);
    }
}

/// How often something was seen, and the first few times.
#[derive(Debug, Default)]
struct Seen {
    count: u64,
    examples: Vec<String>,
}

impl Seen {
    fn see(&mut self, example: impl FnOnce() -> String) {
        self.count += 1;
        if self.examples.len() < EXAMPLES {
            self.examples.push(example());
        }
    }
}

/// One field's values, counted by shape.
#[derive(Debug, Default)]
struct Shapes {
    absent: u64,
    by: HashMap<String, Seen>,
}

impl Shapes {
    fn see(&mut self, value: Option<&str>, who: &dyn Fn() -> String, buf: &mut String) {
        let Some(value) = value else {
            self.absent += 1;
            return;
        };
        signature(value, buf);
        let example = || format!("{} {value:?}", who());
        if let Some(seen) = self.by.get_mut(buf.as_str()) {
            seen.see(example);
        } else {
            let mut seen = Seen::default();
            seen.see(example);
            self.by.insert(buf.clone(), seen);
        }
    }

    fn report(&self, out: &mut String, name: &str) -> fmt::Result {
        let given: u64 = self.by.values().map(|s| s.count).sum();
        writeln!(out, "    {name}: {given} given, {} not", self.absent)?;
        let mut shapes: Vec<(&String, &Seen)> = self.by.iter().collect();
        shapes.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(b.0)));
        for (shape, seen) in shapes.iter().take(SHOWN) {
            writeln!(
                out,
                "      {shape:<18} {:>12}  {}",
                seen.count,
                seen.examples.join("; ")
            )?;
        }
        if shapes.len() > SHOWN {
            let rest: u64 = shapes[SHOWN..].iter().map(|s| s.1.count).sum();
            writeln!(
                out,
                "      and {} more shapes, {rest} values",
                shapes.len() - SHOWN
            )?;
        }
        Ok(())
    }
}

// --- page ranges ------------------------------------------------------------

/// A page string's kind, as far as plain digit ranges go (D8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Range {
    /// No separator: one page, or an article number written as one.
    Single,
    /// Digits to digits, the end written whole and after the start.
    Whole,
    /// Digits to the same digits.
    OnePage,
    /// The end shorter than the start, expanding forwards: `123-9`.
    Shortened,
    /// The end shorter than the start, expanding to before it: `998-12`.
    ShortenedBefore,
    /// The end written whole and before the start.
    Backwards,
    /// Anything else: letters, lists, other dashes.
    Other,
}

impl Range {
    const fn name(self) -> &'static str {
        match self {
            Self::Single => "no separator",
            Self::Whole => "digits to digits, whole",
            Self::OnePage => "digits to the same digits",
            Self::Shortened => "shortened end",
            Self::ShortenedBefore => "shortened end, expanding before the start",
            Self::Backwards => "whole end before the start",
            Self::Other => "another shape",
        }
    }
}

/// `start-end` when both are ASCII digits, spaces around the hyphen
/// allowed.
fn digit_range(s: &str) -> Option<(&str, &str)> {
    let (a, b) = s.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    let digits = |x: &str| !x.is_empty() && x.len() <= 18 && x.bytes().all(|b| b.is_ascii_digit());
    (digits(a) && digits(b)).then_some((a, b))
}

/// The end of a shortened range written whole, NLM's notation read as
/// the plan's rule under test reads it: `123-9` ends at `129`. `None`
/// unless the end is shorter than the start.
fn expanded(start: &str, end: &str) -> Option<String> {
    (end.len() < start.len()).then(|| format!("{}{end}", &start[..start.len() - end.len()]))
}

/// Digits as a number; at most 18 of them, as [`digit_range`] allows.
fn number(digits: &str) -> u64 {
    digits.parse().unwrap_or(0)
}

fn range(s: &str) -> Range {
    let Some((start, end)) = digit_range(s) else {
        let separated = s.contains(|c: char| {
            matches!(c, '-' | ',' | ';' | '\u{2010}'..='\u{2015}') || c.is_whitespace()
        });
        return if separated {
            Range::Other
        } else {
            Range::Single
        };
    };
    match expanded(start, end) {
        Some(whole) if number(&whole) >= number(start) => Range::Shortened,
        Some(_) => Range::ShortenedBefore,
        None => match number(end).cmp(&number(start)) {
            std::cmp::Ordering::Greater => Range::Whole,
            std::cmp::Ordering::Equal => Range::OnePage,
            std::cmp::Ordering::Less => Range::Backwards,
        },
    }
}

// --- dates -------------------------------------------------------------------

/// A PubMed record's online date, at the day it gives.
fn online(w: &Citation<'_>) -> Option<Bound> {
    w.dates.electronic.map(|d| Bound {
        year: d.year(),
        month: Some(d.month()),
        day: Some(d.day()),
    })
}

/// A PubMed record's issue date: read, or what of it is certain.
fn issue_date(w: &Citation<'_>) -> Bound {
    match &w.dates.published {
        Published::Read(date) | Published::Kept { certain: date, .. } => bound(date),
    }
}

/// How far `b`'s year is from `a`'s, as an index into [`GAP`]: two or
/// more earlier, one earlier, the same, one later, two or more later.
fn gap(a: Year, b: Year) -> usize {
    let apart = (i32::from(b.get()) - i32::from(a.get())).clamp(-2, 2) + 2;
    usize::try_from(apart).unwrap_or(2)
}

const GAP: [&str; 5] = [
    "two years or more earlier",
    "a year earlier",
    "the same year",
    "a year later",
    "two years or more later",
];

/// How close `b` is to `a`: the years apart, or the same as far as both
/// give.
fn closeness(a: Bound, b: Bound) -> &'static str {
    if a.year != b.year {
        return GAP[gap(a.year, b.year)];
    }
    match (a.month, b.month) {
        (Some(x), Some(y)) if x != y => return "the same year, another month",
        (Some(_), Some(_)) => {}
        _ => return "the same, as far as both give",
    }
    match (a.day, b.day) {
        (Some(x), Some(y)) if x != y => "the same month, another day",
        _ => "the same, as far as both give",
    }
}

fn type_name(kind: Option<WorkType>) -> String {
    kind.map_or_else(
        || "untyped".to_owned(),
        |k| {
            serde_json::to_string(&k)
                .map_or_else(|_| format!("{k:?}"), |s| s.trim_matches('"').to_owned())
        },
    )
}

// --- part 1 ------------------------------------------------------------------

#[derive(Debug, Default)]
struct PartOne {
    buf: String,
    pubmed: PubmedCounts,
    crossref: BTreeMap<Option<WorkType>, CrossrefCounts>,
}

#[derive(Debug, Default)]
struct PubmedCounts {
    records: u64,
    /// The volume and the issue as model 13 reads them, rendered.
    volume: Shapes,
    issue: Shapes,
    /// The pages as model 13 reads them, rendered.
    pages: Shapes,
    /// Read, kept as written, or none.
    pagination: BTreeMap<&'static str, u64>,
    /// The rendered pages, by kind.
    ranges: BTreeMap<(&'static str, Range), Seen>,
    /// By `PubModel`: the issue's date read or kept, and the online date
    /// against it.
    dates: BTreeMap<(&'static str, &'static str), u64>,
}

#[derive(Debug, Default)]
struct CrossrefCounts {
    records: u64,
    /// The volume and the issue as model 13 reads them, rendered.
    volume: Shapes,
    issue: Shapes,
    page: Shapes,
    article_number: Shapes,
    part_number: Shapes,
    ranges: BTreeMap<Range, Seen>,
    page_and_number: u64,
    /// Print ISSNs, electronic ISSNs and ISBNs a record names, each
    /// counted to three.
    serials: BTreeMap<(u8, u8, u8), u64>,
    /// Which of print, online and `issued` a record gives.
    dates: BTreeMap<&'static str, u64>,
    /// Where both are given, online's year against print's, by [`gap`].
    apart: [u64; 5],
}

fn upto3(n: usize) -> u8 {
    u8::try_from(n.min(3)).unwrap_or(3)
}

impl PartOne {
    fn pubmed(&mut self, w: &Citation<'_>) {
        let who = || format!("pmid:{}", w.pmid);
        let c = &mut self.pubmed;
        let buf = &mut self.buf;
        c.records += 1;
        let volume = w.journal.volume.as_ref().map(ToString::to_string);
        let issue = w.journal.issue.as_ref().map(ToString::to_string);
        c.volume.see(volume.as_deref(), &who, buf);
        c.issue.see(issue.as_deref(), &who, buf);
        let rendered = w.journal.pages.as_ref().map(ToString::to_string);
        c.pages.see(rendered.as_deref(), &who, buf);
        let how = match &w.journal.pages {
            None => "none",
            Some(Pages::Read(_)) => "read",
            Some(Pages::Kept { .. }) => "kept as written",
        };
        *c.pagination.entry(how).or_default() += 1;
        if let Some(pages) = &rendered {
            c.ranges
                .entry(("pages", range(pages)))
                .or_default()
                .see(|| format!("{} {pages:?}", who()));
        }
        let model = w.journal.pub_model.token();
        let kept = match w.dates.published {
            Published::Read(_) => "issue date read",
            Published::Kept { .. } => "issue date kept, its year certain",
        };
        *c.dates.entry((model, kept)).or_default() += 1;
        let against = online(w).map_or("no online date", |o| {
            [
                "online date two years or more before the issue's",
                "online date a year before the issue's",
                "online date in the issue's year",
                "online date a year after the issue's",
                "online date two years or more after the issue's",
            ][gap(issue_date(w).year, o.year)]
        });
        *c.dates.entry((model, against)).or_default() += 1;
    }

    fn crossref(&mut self, w: &CrossrefWork<'_>) {
        let who = || format!("doi:{}", w.doi);
        let c = self.crossref.entry(w.kind).or_default();
        let buf = &mut self.buf;
        c.records += 1;
        let volume = w.volume.as_ref().map(ToString::to_string);
        let issue = w.issue.as_ref().map(ToString::to_string);
        c.volume.see(volume.as_deref(), &who, buf);
        c.issue.see(issue.as_deref(), &who, buf);
        let rendered = w.pages.as_ref().map(ToString::to_string);
        c.page.see(rendered.as_deref(), &who, buf);
        c.article_number.see(w.article_number, &who, buf);
        c.part_number.see(w.part_number, &who, buf);
        if let Some(page) = &rendered {
            c.ranges
                .entry(range(page))
                .or_default()
                .see(|| format!("{} {page:?}", who()));
        }
        c.page_and_number += u64::from(w.pages.is_some() && w.article_number.is_some());
        let print = w
            .issns
            .iter()
            .filter(|i| i.kind == SerialKind::Print)
            .count();
        let electronic = w.issns.len() - print;
        *c.serials
            .entry((upto3(print), upto3(electronic), upto3(w.isbns.len())))
            .or_default() += 1;
        let given = match (
            w.published_print.is_some(),
            w.published_online.is_some(),
            w.issued.is_some(),
        ) {
            (true, true, true) => "print, online and issued",
            (true, true, false) => "print and online, no issued",
            (true, false, true) => "print and issued",
            (true, false, false) => "print alone",
            (false, true, true) => "online and issued",
            (false, true, false) => "online alone",
            (false, false, true) => "issued alone, of no stated kind",
            (false, false, false) => "no date",
        };
        *c.dates.entry(given).or_default() += 1;
        if let (Some(p), Some(o)) = (&w.published_print, &w.published_online) {
            c.apart[gap(bound(p).year, bound(o).year)] += 1;
        }
    }

    fn report(&self, out: &mut String) -> fmt::Result {
        writeln!(out)?;
        writeln!(
            out,
            "Part 1. What each source gives, every record read once, at its speaking copy."
        )?;
        let p = &self.pubmed;
        writeln!(out)?;
        writeln!(out, "PubMed: {} records", p.records)?;
        p.volume.report(out, "Volume")?;
        p.issue.report(out, "Issue")?;
        p.pages.report(out, "pages, as read")?;
        writeln!(out, "    Pagination: read, kept as written, or none:")?;
        for (parts, n) in &p.pagination {
            writeln!(out, "      {parts:<40} {n:>12}")?;
        }
        writeln!(out, "    page ranges, by kind:")?;
        for ((field, kind), seen) in &p.ranges {
            writeln!(
                out,
                "      {field:<18} {:<42} {:>12}  {}",
                kind.name(),
                seen.count,
                seen.examples.join("; ")
            )?;
        }
        writeln!(out, "    dates, by PubModel:")?;
        for ((model, what), n) in &p.dates {
            writeln!(out, "      {model:<24} {what:<52} {n:>12}")?;
        }
        let mut types: Vec<(&Option<WorkType>, &CrossrefCounts)> = self.crossref.iter().collect();
        types.sort_by(|a, b| b.1.records.cmp(&a.1.records).then_with(|| a.0.cmp(b.0)));
        let records: u64 = types.iter().map(|t| t.1.records).sum();
        writeln!(out)?;
        writeln!(
            out,
            "Crossref: {records} records in {} types, the commonest first",
            types.len()
        )?;
        for (kind, c) in types {
            writeln!(out)?;
            writeln!(out, "  {}: {} records", type_name(*kind), c.records)?;
            c.volume.report(out, "volume")?;
            c.issue.report(out, "issue")?;
            c.page.report(out, "page")?;
            c.article_number.report(out, "article-number")?;
            c.part_number.report(out, "part-number")?;
            writeln!(
                out,
                "    page and article-number both: {}",
                c.page_and_number
            )?;
            if !c.ranges.is_empty() {
                writeln!(out, "    page ranges, by kind:")?;
                for (kind, seen) in &c.ranges {
                    writeln!(
                        out,
                        "      {:<42} {:>12}  {}",
                        kind.name(),
                        seen.count,
                        seen.examples.join("; ")
                    )?;
                }
            }
            writeln!(
                out,
                "    serials named (print ISSNs, electronic ISSNs, ISBNs; 3 is 3 or more):"
            )?;
            for ((print, electronic, isbns), n) in &c.serials {
                writeln!(out, "      {print} {electronic} {isbns} {n:>12}")?;
            }
            writeln!(out, "    dates given:")?;
            for (what, n) in &c.dates {
                writeln!(out, "      {what:<40} {n:>12}")?;
            }
            if c.apart.iter().any(|n| *n > 0) {
                writeln!(out, "    online's year against print's:")?;
                for (how, n) in GAP.iter().zip(c.apart) {
                    writeln!(out, "      {how:<40} {n:>12}")?;
                }
            }
        }
        Ok(())
    }
}

// --- part 2 ------------------------------------------------------------------

/// Two records' values for one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Agree {
    Neither,
    OnlyPubmed,
    OnlyCrossref,
    Same,
    /// The same once space, case, dots, leading zeros, dashes and the
    /// spellings of supplement and part are set aside.
    Notation,
    Differ,
}

impl Agree {
    const fn name(self) -> &'static str {
        match self {
            Self::Neither => "neither gives it",
            Self::OnlyPubmed => "PubMed alone gives it",
            Self::OnlyCrossref => "Crossref alone gives it",
            Self::Same => "the same",
            Self::Notation => "the same but for notation",
            Self::Differ => "different",
        }
    }
}

/// The value with its notation set aside, for [`Agree::Notation`].
fn fold(s: &str) -> String {
    let lower = s
        .to_lowercase()
        .replace("supplement", "suppl")
        .replace("part", "pt");
    let mut out = String::with_capacity(lower.len());
    let mut digits = String::new();
    let flush = |digits: &mut String, out: &mut String| {
        let kept = digits.trim_start_matches('0');
        out.push_str(if kept.is_empty() && !digits.is_empty() {
            "0"
        } else {
            kept
        });
        digits.clear();
    };
    for c in lower.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        flush(&mut digits, &mut out);
        if c.is_whitespace() || c == '.' {
            continue;
        }
        out.push(if matches!(c, '\u{2010}'..='\u{2015}') {
            '-'
        } else {
            c
        });
    }
    flush(&mut digits, &mut out);
    out
}

/// A value, trimmed, unless nothing is left.
fn given(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

fn agree(pubmed: Option<&str>, crossref: Option<&str>) -> Agree {
    match (given(pubmed), given(crossref)) {
        (None, None) => Agree::Neither,
        (Some(_), None) => Agree::OnlyPubmed,
        (None, Some(_)) => Agree::OnlyCrossref,
        (Some(p), Some(c)) if p == c => Agree::Same,
        (Some(p), Some(c)) if fold(p) == fold(c) => Agree::Notation,
        _ => Agree::Differ,
    }
}

/// A pair, over volume, issue and where the work sits (D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Same,
    /// Everything both give agrees, and Crossref gives more.
    CrossrefMore,
    /// Everything both give agrees, and PubMed gives more.
    PubmedMore,
    /// Everything both give agrees, and each gives something the other
    /// does not.
    EachMore,
    /// A value both give differs.
    Conflicting,
    /// Each gives something, and nothing the other gives: D2's rule would
    /// join them unchecked.
    Disjoint,
    /// Only Crossref gives any.
    CrossrefAlone,
    /// Only PubMed gives any.
    PubmedAlone,
    /// Neither gives any.
    Empty,
}

impl Class {
    const fn name(self) -> &'static str {
        match self {
            Self::Same => "same",
            Self::CrossrefMore => "compatible: Crossref gives more",
            Self::PubmedMore => "compatible: PubMed gives more",
            Self::EachMore => "compatible: each gives what the other does not",
            Self::Conflicting => "conflicting",
            Self::Disjoint => "nothing to compare: each gives, none shared",
            Self::CrossrefAlone => "only Crossref gives any",
            Self::PubmedAlone => "only PubMed gives any",
            Self::Empty => "neither gives any",
        }
    }
}

/// The class, and whether agreeing took setting notation aside.
fn class(agreed: &[Agree]) -> (Class, bool) {
    let notation = agreed.contains(&Agree::Notation);
    let shared = agreed
        .iter()
        .any(|a| matches!(a, Agree::Same | Agree::Notation | Agree::Differ));
    let pubmed_more = agreed.contains(&Agree::OnlyPubmed);
    let crossref_more = agreed.contains(&Agree::OnlyCrossref);
    let class = if agreed.contains(&Agree::Differ) {
        Class::Conflicting
    } else {
        match (shared, pubmed_more, crossref_more) {
            (true, false, false) => Class::Same,
            (true, false, true) => Class::CrossrefMore,
            (true, true, false) => Class::PubmedMore,
            (true, true, true) => Class::EachMore,
            (false, true, true) => Class::Disjoint,
            (false, false, true) => Class::CrossrefAlone,
            (false, true, false) => Class::PubmedAlone,
            (false, false, false) => Class::Empty,
        }
    };
    (class, notation)
}

/// The fields a pair is compared on; the last is what D2's classes read
/// beside volume and issue.
const FIELDS: [&str; 5] = [
    "volume",
    "issue",
    "first page",
    "Crossref's article-number against PubMed's first page",
    "where the work sits: Crossref's first page, else its article-number",
];

/// PubMed's written range against Crossref's `page` (D8).
fn expansion(pubmed: Option<&str>, crossref: Option<&str>) -> &'static str {
    let Some((start, end)) = pubmed.and_then(digit_range) else {
        return "PubMed gives no digit range";
    };
    let Some(whole) = expanded(start, end) else {
        return "PubMed's range is written whole";
    };
    if number(&whole) < number(start) {
        return "PubMed's shortened range expands before its start";
    }
    match crossref.and_then(digit_range) {
        None => "PubMed's shortened; Crossref gives no digit range",
        Some((a, b)) if b.len() < a.len() => "PubMed's shortened; Crossref's shortened too",
        Some((a, b)) if a == start && b == whole => {
            "PubMed's shortened; expanding gives Crossref's"
        }
        Some(_) => "PubMed's shortened; expanding differs from Crossref's",
    }
}

fn source_name(source: Option<SourceId>) -> &'static str {
    match source {
        Some(SourceId::Pubmed) => "PubMed",
        Some(SourceId::Crossref) => "Crossref",
        Some(SourceId::OpenAlex) => "OpenAlex",
        None => "no venue",
    }
}

/// PubMed's values of a pair, kept until its Crossref record is read.
#[derive(Debug)]
struct Side {
    pmid: Pmid,
    /// The volume and the issue as model 13 reads them, rendered.
    volume: Option<Box<str>>,
    issue: Option<Box<str>>,
    first: Option<Box<str>>,
    /// The pages as model 13 reads them, rendered.
    written: Option<Box<str>>,
    issue_date: Bound,
    online: Option<Bound>,
    model: PubModel,
}

impl Side {
    fn of(w: &Citation<'_>) -> Self {
        let pages = w.journal.pages.as_ref();
        let first = pages.and_then(Pages::first).map(Into::into);
        let written = pages.map(|p| p.to_string().into());
        Self {
            pmid: w.pmid,
            volume: w.journal.volume.as_ref().map(|l| l.to_string().into()),
            issue: w.journal.issue.as_ref().map(|l| l.to_string().into()),
            first,
            written,
            issue_date: issue_date(w),
            online: online(w),
            model: w.journal.pub_model,
        }
    }
}

#[derive(Debug)]
struct Pair {
    venue: Option<SourceId>,
    pubmed: Option<Side>,
    compared: bool,
}

#[derive(Debug, Default)]
struct Pairs {
    /// The merged directory read, if the store has one.
    merged: Option<PathBuf>,
    by_pmid: HashMap<Pmid, u32>,
    by_doi: HashMap<Box<str>, u32>,
    found: Vec<Pair>,
    /// Works holding two records of one of the sources, whose promoted
    /// records were paired.
    two_of_a_source: u64,
    /// A Crossref record read before its pair's PubMed record was: the
    /// merged set is older than the normalized sets, or `--limit`.
    unpaired: u64,
    by_type: BTreeMap<Option<WorkType>, u64>,
    /// By the field's place in [`FIELDS`].
    fields: BTreeMap<(usize, Agree), Seen>,
    class: BTreeMap<(Class, bool), Seen>,
    expansion: BTreeMap<&'static str, Seen>,
    dates: BTreeMap<(&'static str, &'static str), u64>,
    issue_by_model: BTreeMap<(&'static str, &'static str), u64>,
    venue: BTreeMap<(&'static str, &'static str), u64>,
}

impl Pairs {
    /// Every work of the merged set holding a PubMed and a Crossref record.
    fn read(store: &Store, limit: Option<usize>) -> Result<Self, String> {
        let mut pairs = Self::default();
        let dir = merge::layout::dir(store);
        if !dir.is_dir() {
            return Ok(pairs);
        }
        let io = |path: &Path, e: io::Error| format!("{}: {e}", path.display());
        let paths = merge::sets::find(&dir, Kind::Works).map_err(|e| io(&dir, e))?;
        for path in &paths {
            let reader = merge::layout::open(path).map_err(|e| io(path, e))?;
            reader.with_table(|| -> Result<(), String> {
                for work in reader
                    .records::<merge::record::Work<'_>>()
                    .take(limit.unwrap_or(usize::MAX))
                {
                    let work = work.map_err(|e| io(path, e))?;
                    pairs.add(&work)?;
                }
                Ok(())
            })?;
        }
        pairs.merged = Some(dir);
        Ok(pairs)
    }

    fn add(&mut self, work: &merge::record::Work<'_>) -> Result<(), String> {
        let of = |source: SourceId| {
            let mut records = work.records.iter().filter(|r| r.source == source);
            match (records.next(), records.next()) {
                (None, _) => (None, false),
                (Some(one), None) => (Some(one), false),
                (Some(_), Some(_)) => (
                    work.records
                        .iter()
                        .find(|r| r.source == source && r.promoted),
                    true,
                ),
            }
        };
        let (pubmed, two_p) = of(SourceId::Pubmed);
        let (crossref, two_c) = of(SourceId::Crossref);
        let (Some(pubmed), Some(crossref)) = (pubmed, crossref) else {
            return Ok(());
        };
        let (Name::Pmid(pmid), Name::Doi(doi)) = (pubmed.native, crossref.native) else {
            return Err(format!(
                "work {}: native names {} and {} are not a PMID and a DOI",
                work.id.get(),
                pubmed.native,
                crossref.native
            ));
        };
        self.two_of_a_source += u64::from(two_p || two_c);
        let ix = u32::try_from(self.found.len()).map_err(|_| "over 4 billion pairs".to_owned())?;
        self.by_pmid.insert(pmid, ix);
        self.by_doi.insert(doi.into(), ix);
        self.found.push(Pair {
            venue: work.fields.venue.as_ref().map(|f| f.from),
            pubmed: None,
            compared: false,
        });
        Ok(())
    }

    fn pubmed(&mut self, w: &Citation<'_>) {
        if let Some(&ix) = self.by_pmid.get(&w.pmid) {
            self.found[ix as usize].pubmed = Some(Side::of(w));
        }
    }

    fn crossref(&mut self, w: &CrossrefWork<'_>) {
        let Some(Name::Doi(doi)) = native_of::<Crossref>(w) else {
            return;
        };
        let Some(&ix) = self.by_doi.get(doi) else {
            return;
        };
        let pair = &mut self.found[ix as usize];
        pair.compared = true;
        let venue = pair.venue;
        let Some(p) = pair.pubmed.take() else {
            self.unpaired += 1;
            return;
        };
        self.compare(&p, w, venue);
    }

    fn compare(&mut self, p: &Side, w: &CrossrefWork<'_>, venue: Option<SourceId>) {
        *self.by_type.entry(w.kind).or_default() += 1;
        let who = || format!("pmid:{} doi:{}", p.pmid, w.doi);
        let c_first = w.pages.as_ref().and_then(Pages::first);
        let c_written = w.pages.as_ref().map(ToString::to_string);
        let c_volume = w.volume.as_ref().map(ToString::to_string);
        let c_issue = w.issue.as_ref().map(ToString::to_string);
        // In the order of [`FIELDS`].
        let values = [
            (p.volume.as_deref(), c_volume.as_deref()),
            (p.issue.as_deref(), c_issue.as_deref()),
            (p.first.as_deref(), c_first),
            (p.first.as_deref(), w.article_number),
            (p.first.as_deref(), c_first.or(w.article_number)),
        ];
        let mut agreed = [Agree::Neither; FIELDS.len()];
        for (i, &(pv, cv)) in values.iter().enumerate() {
            let a = agree(pv, cv);
            agreed[i] = a;
            self.fields
                .entry((i, a))
                .or_default()
                .see(|| format!("{} {pv:?} / {cv:?}", who()));
        }
        // Volume, issue, and where the work sits.
        let (class, notation) = class(&[agreed[0], agreed[1], agreed[4]]);
        self.class.entry((class, notation)).or_default().see(|| {
            format!(
                "{} {:?} {:?} {:?} / {:?} {:?} {:?}",
                who(),
                p.volume,
                p.issue,
                p.first,
                c_volume,
                c_issue,
                c_first.or(w.article_number)
            )
        });
        let verdict = expansion(p.written.as_deref(), c_written.as_deref());
        self.expansion
            .entry(verdict)
            .or_default()
            .see(|| format!("{} {:?} / {:?}", who(), p.written, c_written));
        let print = w.published_print.as_ref().map(bound);
        let c_online = w.published_online.as_ref().map(bound);
        let date = |ours: Option<&Bound>, theirs: Option<&Bound>| match (ours, theirs) {
            (None, None) => "neither gives it",
            (Some(_), None) => "Crossref gives none",
            (None, Some(_)) => "PubMed gives none",
            (Some(a), Some(b)) => closeness(*a, *b),
        };
        let issue_print = date(Some(&p.issue_date), print.as_ref());
        for (what, how) in [
            (
                "Crossref's print date against PubMed's issue date",
                issue_print,
            ),
            (
                "Crossref's online date against PubMed's issue date",
                date(Some(&p.issue_date), c_online.as_ref()),
            ),
            (
                "Crossref's online date against PubMed's online date",
                date(p.online.as_ref(), c_online.as_ref()),
            ),
        ] {
            *self.dates.entry((what, how)).or_default() += 1;
        }
        *self
            .issue_by_model
            .entry((p.model.token(), issue_print))
            .or_default() += 1;
        let d3 = if w.volume.is_some() || w.article_number.is_some() {
            "Crossref"
        } else if p.volume.is_some() {
            "PubMed"
        } else {
            "Crossref (neither gives a volume)"
        };
        *self.venue.entry((source_name(venue), d3)).or_default() += 1;
    }

    fn report(&self, out: &mut String) -> fmt::Result {
        writeln!(out)?;
        writeln!(
            out,
            "Part 2. Works whose merged row holds a PubMed and a Crossref record."
        )?;
        let Some(dir) = &self.merged else {
            writeln!(
                out,
                "  not run: the store has no merged set at this binary's versions"
            )?;
            return Ok(());
        };
        let compared: u64 = self.by_type.values().sum();
        let unread = self.found.iter().filter(|p| !p.compared).count();
        writeln!(out, "  merged set: {}", dir.display())?;
        writeln!(
            out,
            "  pairs {}; compared {compared}; with two records of one source (the promoted ones paired) {}",
            self.found.len(),
            self.two_of_a_source
        )?;
        if self.unpaired > 0 || unread > 0 {
            writeln!(
                out,
                "  not compared: {} Crossref records whose PubMed record was not read, {unread} pairs whose Crossref record was not read (an older merged set, or --limit)",
                self.unpaired
            )?;
        }
        writeln!(out, "  Crossref's type:")?;
        for (kind, n) in &self.by_type {
            writeln!(out, "    {:<30} {n:>12}", type_name(*kind))?;
        }
        writeln!(out, "  each field (PubMed / Crossref in the examples):")?;
        let mut last = None;
        for (&(field, a), seen) in &self.fields {
            if last != Some(field) {
                writeln!(out, "    {}:", FIELDS[field])?;
                last = Some(field);
            }
            writeln!(
                out,
                "      {:<28} {:>12}  {}",
                a.name(),
                seen.count,
                seen.examples.join("; ")
            )?;
        }
        writeln!(
            out,
            "  each pair over volume, issue and where the work sits (D2); examples PubMed's then Crossref's:"
        )?;
        for ((class, notation), seen) in &self.class {
            let how = if *notation {
                ", notation set aside"
            } else {
                ""
            };
            writeln!(
                out,
                "    {:<44} {:>12}  {}",
                format!("{}{how}", class.name()),
                seen.count,
                seen.examples.join("; ")
            )?;
        }
        writeln!(
            out,
            "  PubMed's written range against Crossref's page (D8):"
        )?;
        for (verdict, seen) in &self.expansion {
            writeln!(
                out,
                "    {verdict:<56} {:>12}  {}",
                seen.count,
                seen.examples.join("; ")
            )?;
        }
        writeln!(out, "  dates (D4):")?;
        for ((what, how), n) in &self.dates {
            writeln!(out, "    {what:<54} {how:<32} {n:>12}")?;
        }
        writeln!(
            out,
            "  Crossref's print date against PubMed's issue date, by PubModel:"
        )?;
        for ((model, how), n) in &self.issue_by_model {
            writeln!(out, "    {model:<24} {how:<32} {n:>12}")?;
        }
        writeln!(
            out,
            "  the venue's promoted source against the record D3 would take (§4, open):"
        )?;
        for ((venue, d3), n) in &self.venue {
            writeln!(out, "    venue from {venue:<10} D3 takes {d3:<36} {n:>12}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(value: &str) -> String {
        let mut out = String::new();
        signature(value, &mut out);
        out
    }

    #[test]
    fn signatures() {
        assert_eq!(shape("123-9"), "9-9");
        assert_eq!(shape("S12-S15"), "a9-a9");
        assert_eq!(shape("iii-xiv"), "a-a");
        assert_eq!(shape("e0123456"), "a9");
        assert_eq!(shape("45  Suppl 1"), "9 a 9");
        assert_eq!(shape("123\u{2013}129"), "9\u{2013}9");
        assert_eq!(shape(&"a1".repeat(30)).chars().count(), SIGNATURE + 1);
    }

    #[test]
    fn ranges() {
        assert_eq!(range("123-129"), Range::Whole);
        assert_eq!(range("98-102"), Range::Whole);
        assert_eq!(range("123-9"), Range::Shortened);
        assert_eq!(range("1299-301"), Range::Shortened);
        assert_eq!(range("998-12"), Range::ShortenedBefore);
        assert_eq!(range("123-123"), Range::OnePage);
        assert_eq!(range("129-123"), Range::Backwards);
        assert_eq!(range(" 123 - 129 "), Range::Whole);
        assert_eq!(range("e0123456"), Range::Single);
        assert_eq!(range("123"), Range::Single);
        assert_eq!(range("S12-S15"), Range::Other);
        assert_eq!(range("123-9; 131"), Range::Other);
        assert_eq!(range("123\u{2013}129"), Range::Other);
    }

    #[test]
    fn expanding() {
        assert_eq!(expanded("123", "9").as_deref(), Some("129"));
        assert_eq!(expanded("1299", "301").as_deref(), Some("1301"));
        assert_eq!(expanded("123", "129"), None);
    }

    #[test]
    fn agreement() {
        assert_eq!(agree(None, Some(" ")), Agree::Neither);
        assert_eq!(agree(Some("45"), None), Agree::OnlyPubmed);
        assert_eq!(agree(Some("45"), Some("45")), Agree::Same);
        assert_eq!(agree(Some("045"), Some("45")), Agree::Notation);
        assert_eq!(agree(Some("3 Pt 2"), Some("3 part 2")), Agree::Notation);
        assert_eq!(
            agree(Some("45 Suppl"), Some("45 Supplement")),
            Agree::Notation
        );
        assert_eq!(agree(Some("0"), Some("00")), Agree::Notation);
        assert_eq!(agree(Some("45"), Some("46")), Agree::Differ);
    }

    #[test]
    fn classes() {
        use Agree::{Differ, Neither, Notation, OnlyCrossref, OnlyPubmed, Same};
        assert_eq!(class(&[Same, Same, Same]), (Class::Same, false));
        assert_eq!(class(&[Same, Notation, Neither]), (Class::Same, true));
        assert_eq!(class(&[Same, OnlyPubmed, Same]), (Class::PubmedMore, false));
        assert_eq!(
            class(&[Same, OnlyCrossref, Neither]),
            (Class::CrossrefMore, false)
        );
        assert_eq!(
            class(&[OnlyPubmed, Same, OnlyCrossref]),
            (Class::EachMore, false)
        );
        assert_eq!(
            class(&[Same, OnlyPubmed, Differ]),
            (Class::Conflicting, false)
        );
        assert_eq!(
            class(&[OnlyPubmed, OnlyCrossref, Neither]),
            (Class::Disjoint, false)
        );
        assert_eq!(
            class(&[OnlyCrossref, OnlyCrossref, Neither]),
            (Class::CrossrefAlone, false)
        );
        assert_eq!(
            class(&[Neither, OnlyPubmed, Neither]),
            (Class::PubmedAlone, false)
        );
        assert_eq!(class(&[Neither, Neither, Neither]), (Class::Empty, false));
    }

    #[test]
    fn expansions() {
        assert_eq!(
            expansion(Some("123-9"), Some("123-129")),
            "PubMed's shortened; expanding gives Crossref's"
        );
        assert_eq!(
            expansion(Some("123-9"), Some("123-130")),
            "PubMed's shortened; expanding differs from Crossref's"
        );
        assert_eq!(
            expansion(Some("123-9"), Some("123-9")),
            "PubMed's shortened; Crossref's shortened too"
        );
        assert_eq!(
            expansion(Some("123-9"), None),
            "PubMed's shortened; Crossref gives no digit range"
        );
        assert_eq!(
            expansion(Some("123-129"), Some("123-129")),
            "PubMed's range is written whole"
        );
        assert_eq!(
            expansion(Some("998-12"), Some("998-1012")),
            "PubMed's shortened range expands before its start"
        );
        assert_eq!(
            expansion(Some("e0123"), Some("e0123")),
            "PubMed gives no digit range"
        );
    }

    #[test]
    fn closenesses() {
        let b = |year: u32, month: Option<u32>, day: Option<u32>| {
            bound(&model::PartialDate::from_numbers(year, month, day).expect("a date"))
        };
        assert_eq!(
            closeness(b(2020, Some(3), None), b(2019, Some(12), Some(5))),
            "a year earlier"
        );
        assert_eq!(
            closeness(b(2020, None, None), b(2020, Some(3), None)),
            "the same, as far as both give"
        );
        assert_eq!(
            closeness(b(2020, Some(3), None), b(2020, Some(4), None)),
            "the same year, another month"
        );
        assert_eq!(
            closeness(b(2020, Some(3), Some(1)), b(2020, Some(3), Some(2))),
            "the same month, another day"
        );
        assert_eq!(
            closeness(b(2018, None, None), b(2021, None, None)),
            "two years or more later"
        );
    }
}
