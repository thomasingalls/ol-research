//! Command line for the pipeline.
//!
//! * `id <kind>` runs one identifier parser over stdin, one value per line.
//! * `pubmed check <file.xml.gz> [--limit N]` parses a baseline or update
//!   file with the strict PubMed parser and reports counts and throughput.
//!
//! Both exit non-zero on the first failure and say exactly where it was: the
//! seed of the "reproduce against one record" command the failure report
//! promises (plan §6), and the quickest way to let a real file vote.

use std::io::{self, BufRead};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use model::{OpenAlexId, ParseError, Pmid};
use source_pubmed::ArticleSet;
use source_pubmed::record::Record;

const USAGE: &str = "usage:\n  ol-rht id <pmid|pmid-url|openalex|openalex-url> < values.txt\n  ol-rht pubmed check <file.xml.gz> [--limit N]";

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

#[allow(clippy::cast_precision_loss)] // reporting only
fn pubmed_check(path: &Path, limit: Option<u64>) -> ExitCode {
    let started = Instant::now();
    let input = match source_pubmed::gz::read_to_string(path) {
        Ok(input) => input,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return ExitCode::from(2);
        }
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
