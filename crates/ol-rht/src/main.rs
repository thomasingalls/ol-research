//! Command line for the pipeline.
//!
//! * `id <kind>` runs one identifier parser over stdin, one value per line.
//! * `pubmed check <file.xml.gz> [--limit N]` parses a baseline or update
//!   file with the strict PubMed parser and reports counts and throughput.
//! * `pubmed normalize <file.xml.gz> [--limit N]` parses and normalizes,
//!   and prints the first stats report: what every field held.
//! * `crossref check <file>...` parses API envelope pages (`.json`) or
//!   public-data-file lines (`.jsonl`, `.jsonl.gz`) with the strict Crossref
//!   types and reports counts and throughput.
//!
//! All exit non-zero on the first failure and say exactly where it was: the
//! seed of the "reproduce against one record" command the failure report
//! promises (plan §6), and the quickest way to let a real file vote.

use std::collections::BTreeMap;
use std::fmt::Display;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use model::{Claim, Doi, OpenAlexId, ParseError, Pmid};
use source_pubmed::ArticleSet;
use source_pubmed::normalize::{Citation, Emit, Reference, normalize};
use source_pubmed::record::Record;

const USAGE: &str = "usage:
  ol-rht id <pmid|pmid-url|openalex|openalex-url> < values.txt
  ol-rht pubmed check <file.xml.gz> [--limit N]
  ol-rht pubmed normalize <file.xml.gz> [--limit N]
  ol-rht crossref check <file>...";

type Parser = fn(&str) -> Result<(), ParseError>;

fn parser(kind: &str) -> Option<Parser> {
    Some(match kind {
        "pmid" => |s| Pmid::from_digits(s).map(drop),
        "pmid-url" => |s| Pmid::from_url(s).map(drop),
        "openalex" => |s| OpenAlexId::from_bare(s).map(drop),
        "openalex-url" => |s| OpenAlexId::from_url(s).map(drop),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["id", kind] => match parser(kind) {
            Some(parse) => ids(parse),
            None => usage(),
        },
        ["pubmed", "check", path] => pubmed_check(Path::new(path), None),
        ["pubmed", "check", path, "--limit", n] => match n.parse() {
            Ok(limit) => pubmed_check(Path::new(path), Some(limit)),
            Err(_) => usage(),
        },
        ["pubmed", "normalize", path] => pubmed_normalize(Path::new(path), None),
        ["pubmed", "normalize", path, "--limit", n] => match n.parse() {
            Ok(limit) => pubmed_normalize(Path::new(path), Some(limit)),
            Err(_) => usage(),
        },
        ["crossref", "check", paths @ ..] if !paths.is_empty() => crossref_check(paths),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

fn ids(parse: Parser) -> ExitCode {
    let (mut ok, mut failed) = (0u64, 0u64);
    for (index, line) in io::stdin().lock().lines().enumerate() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("stdin: {err}");
                return ExitCode::from(2);
            }
        };
        match parse(&line) {
            Ok(()) => ok += 1,
            Err(err) => {
                failed += 1;
                eprintln!("line {}: {line:?}: {err}", index + 1);
            }
        }
    }
    eprintln!("ok={ok} failed={failed}");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn read(path: &Path) -> Option<String> {
    match source_pubmed::gz::read_to_string(path) {
        Ok(input) => Some(input),
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            None
        }
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn pubmed_check(path: &Path, limit: Option<u64>) -> ExitCode {
    let started = Instant::now();
    let Some(input) = read(path) else {
        return ExitCode::from(2);
    };
    let inflated = started.elapsed();
    let bytes = input.len();

    let parsing = Instant::now();
    let mut set = ArticleSet::new(&input);
    let (mut articles, mut deletes, mut references, mut article_ids) = (0u64, 0u64, 0u64, 0u64);
    loop {
        if limit.is_some_and(|n| articles + deletes >= n) {
            break;
        }
        match set.next_record() {
            Ok(Some(Record::Article(a))) => {
                articles += 1;
                if let Some(d) = &a.pubmed_data {
                    article_ids += d.article_ids.len() as u64;
                    references += count_references(&d.reference_lists);
                }
            }
            Ok(Some(Record::DeleteCitation(d))) => deletes += d.pmids.len() as u64,
            Ok(None) => break,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                eprintln!("after {articles} articles");
                return ExitCode::FAILURE;
            }
        }
    }
    let parsed = parsing.elapsed();
    let mib = bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "{}: {articles} articles, {deletes} deletions, {article_ids} article ids, {references} references",
        path.display()
    );
    eprintln!(
        "inflate {:.2}s ({mib:.1} MiB); parse {:.2}s = {:.0} records/s, {:.0} MiB/s",
        inflated.as_secs_f64(),
        parsed.as_secs_f64(),
        (articles + deletes) as f64 / parsed.as_secs_f64(),
        mib / parsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}

fn count_references(lists: &[source_pubmed::record::ReferenceList<'_>]) -> u64 {
    lists
        .iter()
        .map(|l| l.references.len() as u64 + count_references(&l.sub_lists))
        .sum()
}

/// The first stats report (plan §6): what every normalized field held.
#[derive(Default)]
struct Stats {
    citations: u64,
    doi: [u64; 3],
    elocation_doi: [u64; 3],
    doi_lowercased: u64,
    pii: u64,
    pmc: u64,
    published: BTreeMap<&'static str, u64>,
    electronic: u64,
    revised: u64,
    completed: u64,
    status: BTreeMap<&'static str, u64>,
    languages: BTreeMap<&'static str, u64>,
    publication_types: BTreeMap<String, u64>,
    relations: BTreeMap<&'static str, u64>,
    references: u64,
    reference_ids: BTreeMap<&'static str, u64>,
    reference_doi_malformed: u64,
    malformed_samples: Vec<String>,
}

impl Stats {
    /// Counts one DOI claim as valid / malformed / absent.
    fn doi_claim<E: Display>(&mut self, slot: usize, claim: Option<&Claim<'_, Doi<'_>, E>>) {
        let counters = if slot == 0 {
            &mut self.doi
        } else {
            &mut self.elocation_doi
        };
        match claim {
            None => counters[2] += 1,
            Some(Claim::Valid(doi)) => {
                counters[0] += 1;
                if !doi.is_borrowed() {
                    self.doi_lowercased += 1;
                }
            }
            Some(Claim::Malformed { raw, error }) => {
                counters[1] += 1;
                if self.malformed_samples.len() < 8 {
                    self.malformed_samples.push(format!("{raw:?}: {error}"));
                }
            }
        }
    }

    fn report(&self) {
        eprintln!("citations {}", self.citations);
        eprintln!(
            "  doi           valid {} malformed {} none {}",
            self.doi[0], self.doi[1], self.doi[2]
        );
        eprintln!(
            "  elocation doi valid {} malformed {} none {}",
            self.elocation_doi[0], self.elocation_doi[1], self.elocation_doi[2]
        );
        eprintln!("  doi lowercased (allocations) {}", self.doi_lowercased);
        eprintln!("  pii {}  pmc {}", self.pii, self.pmc);
        eprintln!("  published {:?}", self.published);
        eprintln!(
            "  electronic {}  revised {}  completed {}",
            self.electronic, self.revised, self.completed
        );
        eprintln!("  status {:?}", self.status);
        eprintln!("  languages {:?}", self.languages);
        eprintln!(
            "  publication types: {} distinct",
            self.publication_types.len()
        );
        eprintln!("  relations {:?}", self.relations);
        eprintln!("references {}", self.references);
        eprintln!("  ids {:?}", self.reference_ids);
        eprintln!("  doi malformed {}", self.reference_doi_malformed);
        if !self.malformed_samples.is_empty() {
            eprintln!("  malformed samples:");
            for sample in &self.malformed_samples {
                eprintln!("    {sample}");
            }
        }
    }
}

impl<'n> Emit<'n> for Stats {
    fn citation(&mut self, c: &Citation<'n>) {
        self.citations += 1;
        self.doi_claim(0, c.doi.as_ref());
        self.doi_claim(1, c.elocation_doi.as_ref());
        self.pii += u64::from(c.pii.is_some());
        self.pmc += u64::from(c.pmc.is_some());
        *self.published.entry(c.published.precision()).or_default() += 1;
        self.electronic += u64::from(c.electronic.is_some());
        self.revised += u64::from(c.revised.is_some());
        self.completed += u64::from(c.completed.is_some());
        *self.status.entry(c.status.token()).or_default() += 1;
        for l in &c.languages {
            *self.languages.entry(l.alpha_3()).or_default() += 1;
        }
        for pt in &c.publication_types {
            *self
                .publication_types
                .entry(format!("{} {}", pt.ui, pt.name))
                .or_default() += 1;
        }
        for r in &c.relations {
            *self.relations.entry(r.kind.token()).or_default() += 1;
        }
    }

    fn reference(&mut self, r: &Reference<'n>) {
        self.references += 1;
        let key = match (r.pmid.is_some(), r.doi.is_some(), r.pmc.is_some()) {
            (false, false, false) => "none",
            (true, false, false) => "pmid",
            (false, true, false) => "doi",
            (false, false, true) => "pmc",
            (true, true, false) => "pmid+doi",
            (true, false, true) => "pmid+pmc",
            (false, true, true) => "doi+pmc",
            (true, true, true) => "pmid+doi+pmc",
        };
        *self.reference_ids.entry(key).or_default() += 1;
        if let Some(Claim::Malformed { raw, error }) = &r.doi {
            self.reference_doi_malformed += 1;
            if self.malformed_samples.len() < 8 {
                self.malformed_samples.push(format!("{raw:?}: {error}"));
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn pubmed_normalize(path: &Path, limit: Option<u64>) -> ExitCode {
    let Some(input) = read(path) else {
        return ExitCode::from(2);
    };
    let started = Instant::now();
    let mut set = ArticleSet::new(&input);
    let mut stats = Stats::default();
    let mut records = 0u64;
    loop {
        if limit.is_some_and(|n| records >= n) {
            break;
        }
        match set.next_record() {
            Ok(Some(Record::Article(a))) => {
                records += 1;
                if let Err(err) = normalize(&a, &mut stats) {
                    eprintln!("{}: {err}", path.display());
                    eprintln!("after {records} records");
                    stats.report();
                    return ExitCode::FAILURE;
                }
            }
            Ok(Some(Record::DeleteCitation(_))) => records += 1,
            Ok(None) => break,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let elapsed = started.elapsed();
    stats.report();
    eprintln!(
        "parse+normalize {:.2}s = {:.0} records/s",
        elapsed.as_secs_f64(),
        records as f64 / elapsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn crossref_check(paths: &[&str]) -> ExitCode {
    let started = Instant::now();
    let (mut works, mut references, mut authors, mut bytes) = (0u64, 0u64, 0u64, 0usize);
    let mut types: BTreeMap<String, u64> = BTreeMap::new();
    for path in paths {
        let path = Path::new(path);
        let input = if path.extension().is_some_and(|e| e == "gz") {
            source_crossref::gz::read_to_string(path)
        } else {
            std::fs::read_to_string(path)
        };
        let input = match input {
            Ok(input) => input,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                return ExitCode::from(2);
            }
        };
        bytes += input.len();
        let mut count = |w: &source_crossref::Work<'_>| {
            works += 1;
            references += w.reference.as_ref().map_or(0, Vec::len) as u64;
            authors += w.author.as_ref().map_or(0, Vec::len) as u64;
            *types.entry(format!("{:?}", w.r#type)).or_default() += 1;
        };
        let result = if input.trim_start().starts_with("{\"status\"") {
            match source_crossref::Page::parse(&input) {
                Ok(page) => page.works().try_for_each(|w| w.map(|w| count(&w))),
                Err(err) => Err(err),
            }
        } else {
            source_crossref::jsonl(&input).try_for_each(|w| w.map(|w| count(&w)))
        };
        if let Err(err) = result {
            eprintln!("{}: {err}", path.display());
            eprintln!("after {works} works");
            return ExitCode::FAILURE;
        }
    }
    let elapsed = started.elapsed();
    let mib = bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "{works} works, {references} references, {authors} authors from {} files",
        paths.len()
    );
    eprintln!("types {types:?}");
    eprintln!(
        "read+parse {:.2}s = {:.0} works/s, {:.0} MiB/s",
        elapsed.as_secs_f64(),
        works as f64 / elapsed.as_secs_f64(),
        mib / elapsed.as_secs_f64()
    );
    ExitCode::SUCCESS
}
