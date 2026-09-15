//! Command line for the pipeline.
//!
//! * `id <kind>` runs one identifier parser over stdin, one value per line.
//! * `<source> check <file>... [--limit N] [--workers N]` reads raw files
//!   through the source's strict native types and reports counts and
//!   throughput.
//! * `<source> normalize <file>... [--limit N] [--workers N] [--out <dir>]
//!   [--survey]` parses and normalizes, prints the source's stats report —
//!   what every field held — and, with `--out`, writes the normalized
//!   layer: one set
//!   of `<prefix>.works.zst`, `<prefix>.references.zst` and
//!   `<prefix>.deletions.zst` per raw file, the prefix mirroring the raw
//!   path below the inputs' common directory (plan §5, rule 12), tagged
//!   with the model version.
//! * `<source> dump <file.zst>... [--limit N]` prints a normalized file as
//!   JSON lines, for eyes.
//! * `<source> keys <set.zst>... --by <doi|pmid|pmc|openalex> [--limit N]`
//!   prints the identifiers a normalized set holds — a work's own, or
//!   those its references point at — one per line, distinct: the seed of
//!   a lookup listing (plan §8, item 8).
//! * `<source> list [--store <root>]` writes what the source offers now as
//!   a listing under the store (plan §3.5), with the index document it was
//!   read from beside it. With `--seed <file> --by <key>` it writes a
//!   lookup listing instead: the API requests answering a seed of ids, a
//!   batch per request, the seed kept beside the listing.
//! * `<source> fetch <listing> [--store <root>] [--workers N] [--limit N]`
//!   brings in every entry of the listing that has no receipt yet,
//!   verified, and appends a receipt per file. Lookups go one request at
//!   a time at the source's own pace, wait once for what a 429 asks, and
//!   stop at anything else; `--mailto` reaches the API as the source's
//!   contact parameter.
//! * `<source> ingest <listing> [--store <root>] [--workers N] [--limit N]
//!   [--out <dir>]` does the same while normalizing, each download
//!   streamed straight through the framer on its own worker; the output
//!   sets land under the store's normalized layer unless `--out` says
//!   where. `--limit` counts records; a file it cuts short gets neither
//!   receipt nor output set, and its partial bytes wait for a resume.
//!
//! * `merge identity [--store <root>] [--limit N] [--survey]` runs the
//!   identity pass of the merge plan (§3, §8) over every normalized set in
//!   the store: names, components, verdicts, works and stubs; assigns
//!   every work its `WorkId` through the registry (§4), appending the
//!   registry's lines unless `--survey` or `--limit` is given; prints
//!   the identity report.
//!
//! `--survey` (plan §6) keeps going past records that do not normalize and
//! reports every distinct failure signature with counts and examples; it
//! writes no output and exits non-zero when anything failed.
//! `--store` is the root of the store of record, `data` by default.
//! `--mailto <address>` puts an address in the User-Agent for sources
//! that reward it; none is sent unless given.
//!
//! Sources are `pubmed`, `crossref` and `openalex`. Every one goes through
//! the same runner (`ingest::normalize_parallel`, one worker per core by
//! default, files shared through a queue); what differs per source is one
//! registration line and its stats sink. All exit non-zero on the first
//! failure and say exactly where it was — source, file, record ordinal,
//! stage, native id, field, raw value — the seed of the "reproduce against
//! one record" command the failure report promises (plan §6). A failed run
//! leaves no output: partial files are removed.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;
use std::io::{self, BufRead, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use ingest::list::Lister;
use ingest::{
    Binary, Client, Failure, Key, Keys, Kind, Reader, Sink, Source, Stage, Store, Written,
};
use merge::{Contributor, Identity, OwnedName, Registry};
use model::{Claim, Doi, OpenAlexId, ParseError, Pmid};
use source_crossref::Crossref;
use source_openalex::OpenAlex;
use source_openalex::normalize::LocationId;
use source_pubmed::Pubmed;
use source_pubmed::normalize::{Citation, Reference};

/// Allocations per record are part of every report (plan §5, rule 13).
#[global_allocator]
static ALLOC: ingest::alloc::Counting = ingest::alloc::Counting;

const USAGE: &str = "usage:
  ol-rht id <pmid|pmid-url|openalex|openalex-url> < values.txt
  ol-rht <pubmed|crossref|openalex> check <file>... [--limit N] [--workers N]
  ol-rht <pubmed|crossref|openalex> normalize <file>... [--limit N] [--workers N] [--out <dir> | --survey]
  ol-rht <pubmed|crossref|openalex> dump <file.zst>... [--limit N]
  ol-rht <pubmed|crossref|openalex> keys <set.zst>... --by <doi|pmid|pmc|openalex> [--limit N]
  ol-rht <pubmed|crossref|openalex> list [--store <root>] [--mailto <address>] [--seed <file> --by <doi|pmid|pmc|openalex>]
  ol-rht <pubmed|crossref|openalex> fetch <listing> [--store <root>] [--workers N] [--limit N] [--mailto <address>]
  ol-rht <pubmed|crossref|openalex> ingest <listing> [--store <root>] [--workers N] [--limit N] [--out <dir>] [--mailto <address>]
  ol-rht merge identity [--store <root>] [--limit N] [--survey]";

/// Who we say we are; `--mailto` adds an address inside the parenthesis.
const USER_AGENT: &str = "ol-rht/0.1 (citation-graph ingestion";

/// Parallel transfers unless `--workers` says otherwise.
const FETCH_WORKERS: usize = 4;

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
        [source, action, rest @ ..] => {
            let Some(opts) = Options::parse(rest) else {
                return usage();
            };
            match (*source, *action) {
                ("pubmed", "check") => check::<Pubmed>(&opts),
                ("pubmed", "normalize") => normalize::<Pubmed, PubmedStats>(&opts),
                ("pubmed", "dump") => dump::<Pubmed>(&opts),
                ("pubmed", "keys") => keys::<Pubmed>(&opts),
                ("pubmed", "list") => list::<Pubmed>(&opts),
                ("pubmed", "fetch") => fetch::<Pubmed>(&opts),
                ("pubmed", "ingest") => ingest::<Pubmed, PubmedStats>(&opts),
                ("crossref", "check") => check::<Crossref>(&opts),
                ("crossref", "normalize") => normalize::<Crossref, CrossrefStats>(&opts),
                ("crossref", "dump") => dump::<Crossref>(&opts),
                ("crossref", "keys") => keys::<Crossref>(&opts),
                ("crossref", "list") => list::<Crossref>(&opts),
                ("crossref", "fetch") => fetch::<Crossref>(&opts),
                ("crossref", "ingest") => ingest::<Crossref, CrossrefStats>(&opts),
                ("openalex", "check") => check::<OpenAlex>(&opts),
                ("openalex", "normalize") => normalize::<OpenAlex, OpenAlexStats>(&opts),
                ("openalex", "dump") => dump::<OpenAlex>(&opts),
                ("openalex", "keys") => keys::<OpenAlex>(&opts),
                ("openalex", "list") => list::<OpenAlex>(&opts),
                ("openalex", "fetch") => fetch::<OpenAlex>(&opts),
                ("openalex", "ingest") => ingest::<OpenAlex, OpenAlexStats>(&opts),
                ("merge", "identity") => merge_identity(&opts),
                _ => usage(),
            }
        }
        _ => usage(),
    }
}

/// The paths and trailing options of a source command.
struct Options {
    paths: Vec<PathBuf>,
    limit: Option<u64>,
    workers: Option<usize>,
    out: Option<PathBuf>,
    store: Option<PathBuf>,
    mailto: Option<String>,
    survey: bool,
    seed: Option<PathBuf>,
    by: Option<Key>,
}

impl Options {
    /// `None` when an option is malformed or repeated, or an unknown one
    /// appears. Paths may be absent (`list` takes none).
    fn parse(args: &[&str]) -> Option<Self> {
        let mut rest = args;
        let (mut limit, mut workers, mut out, mut store, mut mailto, mut seed, mut by) =
            (None, None, None, None, None, None, None);
        let mut survey = false;
        loop {
            match rest {
                [before @ .., "--survey"] => {
                    if survey {
                        return None;
                    }
                    survey = true;
                    rest = before;
                }
                [before @ .., "--store", p] => {
                    if store.replace(PathBuf::from(p)).is_some() {
                        return None;
                    }
                    rest = before;
                }
                [before @ .., "--mailto", m] => {
                    if mailto.replace((*m).to_owned()).is_some() {
                        return None;
                    }
                    rest = before;
                }
                [before @ .., "--limit", n] => {
                    if limit.replace(n.parse::<u64>().ok()?).is_some() {
                        return None;
                    }
                    rest = before;
                }
                [before @ .., "--workers", n] => {
                    if workers
                        .replace(n.parse::<NonZeroUsize>().ok()?.get())
                        .is_some()
                    {
                        return None;
                    }
                    rest = before;
                }
                [before @ .., "--out", p] => {
                    if out.replace(PathBuf::from(p)).is_some() {
                        return None;
                    }
                    rest = before;
                }
                [before @ .., "--seed", p] => {
                    if seed.replace(PathBuf::from(p)).is_some() {
                        return None;
                    }
                    rest = before;
                }
                [before @ .., "--by", k] => {
                    if by.replace(Key::parse(k)?).is_some() {
                        return None;
                    }
                    rest = before;
                }
                _ => break,
            }
        }
        if rest.iter().any(|a| a.starts_with("--")) {
            return None;
        }
        Some(Self {
            paths: rest.iter().map(PathBuf::from).collect(),
            limit,
            workers,
            out,
            store,
            mailto,
            survey,
            seed,
            by,
        })
    }

    /// `--workers`, or one per core.
    fn workers(&self) -> usize {
        self.workers
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, NonZeroUsize::get))
    }

    /// `--store`, or `data`.
    fn store(&self) -> Store {
        Store::new(self.store.clone().unwrap_or_else(|| PathBuf::from("data")))
    }

    /// The User-Agent, with `--mailto` when given.
    fn user_agent(&self) -> String {
        match &self.mailto {
            Some(address) => format!("{USER_AGENT}; mailto:{address})"),
            None => format!("{USER_AGENT})"),
        }
    }

    /// The options a reading command does not take.
    fn reads_only(&self) -> bool {
        self.store.is_none()
            && self.mailto.is_none()
            && self.seed.is_none()
            && !self.paths.is_empty()
    }

    /// For lookups: the contact parameter `--mailto` becomes for `S`.
    fn contact<S: Lister>(&self) -> Option<String> {
        let address = self.mailto.as_ref()?;
        S::CONTACT.map(|param| format!("{param}={}", ingest::list::encode(address)))
    }

    /// The options a command that does not normalize does not take.
    fn no_survey(&self) -> bool {
        !self.survey
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

/// A source's stats report (plan §6): what every normalized field held.
/// One per worker, merged at the end.
trait StatsReport: Default + Merge + Send {
    fn report(&self);
}

fn check<S: Source>(opts: &Options) -> ExitCode {
    if opts.out.is_some() || opts.by.is_some() || !opts.reads_only() || !opts.no_survey() {
        return usage();
    }
    match ingest::run::parse_parallel::<S, _>(&opts.paths, opts.limit, opts.workers()) {
        Ok(report) => {
            eprintln!("{}: {report}", S::NAME);
            ExitCode::SUCCESS
        }
        Err(failure) => failed(&failure),
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn normalize<S, K>(opts: &Options) -> ExitCode
where
    S: Source,
    K: StatsReport + for<'n> Sink<'n, S>,
{
    if !opts.reads_only() || opts.by.is_some() || (opts.survey && opts.out.is_some()) {
        return usage();
    }
    let prefixes: Option<Arc<[PathBuf]>> = match &opts.out {
        Some(dir) => match ingest::run::mirror(&opts.paths, dir) {
            Ok(prefixes) => Some(prefixes.into()),
            Err(collision) => {
                eprintln!("{collision}");
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    let make = |_: usize| {
        let binary = prefixes.clone().map(|prefixes| {
            Binary::new(S::NAME, model::VERSION, move |i, _: &Path| {
                prefixes[i].clone()
            })
        });
        Ok((K::default(), binary))
    };
    let outcome = ingest::run::normalize_parallel::<S, _, _, _>(
        &opts.paths,
        opts.limit,
        opts.workers(),
        opts.survey,
        make,
    );
    let mut stats = K::default();
    let mut binaries = Vec::new();
    for (s, b) in outcome.sinks {
        stats.merge(s);
        binaries.extend(b);
    }
    stats.report();
    match outcome.result {
        Ok(report) => {
            let mut written: Vec<Written> = Vec::new();
            let mut error = None;
            for binary in binaries {
                match binary.finish() {
                    Ok(sets) => written.extend(sets),
                    Err(e) => error = Some(e),
                }
            }
            if let Some(e) = error {
                for w in &written {
                    for kind in Kind::ALL {
                        let _ = std::fs::remove_file(kind.path(&w.prefix));
                    }
                }
                eprintln!("{} write: {e}", S::NAME);
                eprintln!("no output written");
                return ExitCode::FAILURE;
            }
            if let Some(dir) = &opts.out {
                let records = written.iter().fold([0u64; 4], |a, w| {
                    [
                        a[0] + w.records[0],
                        a[1] + w.records[1],
                        a[2] + w.records[2],
                        a[3] + w.records[3],
                    ]
                });
                let bytes: u64 = written.iter().map(|w| w.bytes).sum();
                eprintln!(
                    "wrote {} works, {} references, {} deletions, {} symbols in {} sets under {}: {:.1} MiB",
                    records[0],
                    records[1],
                    records[2],
                    records[3],
                    written.len(),
                    dir.display(),
                    bytes as f64 / (1024.0 * 1024.0)
                );
            }
            eprintln!("{}: {report}", S::NAME);
            if opts.survey {
                eprint!("{}: survey: {}", S::NAME, outcome.survey);
                if !outcome.survey.is_empty() {
                    return ExitCode::FAILURE;
                }
            }
            ExitCode::SUCCESS
        }
        Err(failure) => {
            for binary in binaries {
                binary.discard();
            }
            if opts.out.is_some() {
                eprintln!("no output written");
            }
            failed(&failure)
        }
    }
}

fn dump<S: Source>(opts: &Options) -> ExitCode {
    if opts.out.is_some()
        || opts.workers.is_some()
        || opts.by.is_some()
        || !opts.reads_only()
        || !opts.no_survey()
    {
        return usage();
    }
    let mut out = io::BufWriter::new(io::stdout().lock());
    for path in &opts.paths {
        if let Err(err) = dump_one::<S>(path, opts.limit, &mut out) {
            eprintln!("{}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    }
    match out.flush() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("stdout: {err}");
            ExitCode::FAILURE
        }
    }
}

/// A normalized set of `S` at this binary's model version, open.
fn open_set<S: Source>(path: &Path) -> io::Result<Reader> {
    let reader = Reader::open(path)?;
    let header = reader.header();
    if header.source != S::NAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("written by {:?}, not {}", header.source, S::NAME),
        ));
    }
    if header.model != model::VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "model version {} is not this binary's {}",
                header.model,
                model::VERSION
            ),
        ));
    }
    Ok(reader)
}

fn records_limit(limit: Option<u64>) -> usize {
    limit.map_or(usize::MAX, |n| usize::try_from(n).unwrap_or(usize::MAX))
}

fn dump_one<S: Source>(path: &Path, limit: Option<u64>, out: &mut impl Write) -> io::Result<()> {
    let reader = open_set::<S>(path)?;
    let header = reader.header();
    let limit = records_limit(limit);
    // Interned strings resolve through the set's table while it is installed.
    reader.with_table(|| match header.kind {
        Kind::Works => json_lines::<S::Work<'_>>(&reader, limit, out),
        Kind::References => json_lines::<(S::Id<'_>, S::Reference<'_>)>(&reader, limit, out),
        Kind::Deletions => json_lines::<S::Id<'_>>(&reader, limit, out),
        Kind::Symbols => json_lines::<&str>(&reader, limit, out),
    })
}

fn json_lines<'a, T: serde::Deserialize<'a> + serde::Serialize>(
    reader: &'a Reader,
    limit: usize,
    out: &mut impl Write,
) -> io::Result<()> {
    for record in reader.records::<T>().take(limit) {
        serde_json::to_writer(&mut *out, &record?)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// The `--by` keys of every set given, distinct, in first-seen order, one
/// per line on stdout; non-zero when there are none, since an empty seed
/// is a mistake.
fn keys<S: Keys>(opts: &Options) -> ExitCode {
    let Some(key) = opts.by else {
        return usage();
    };
    if opts.out.is_some() || opts.workers.is_some() || !opts.reads_only() || !opts.no_survey() {
        return usage();
    }
    let mut seen = HashSet::new();
    let mut order = Vec::new();
    let mut records = 0u64;
    for path in &opts.paths {
        let mut emit = |id: String| {
            if seen.insert(id.clone()) {
                order.push(id);
            }
        };
        match keys_one::<S>(path, key, opts.limit, &mut emit) {
            Ok(n) => records += n,
            Err(err) => {
                eprintln!("{}: {err}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let mut out = io::BufWriter::new(io::stdout().lock());
    let written = order
        .iter()
        .try_for_each(|id| writeln!(out, "{id}"))
        .and_then(|()| out.flush());
    if let Err(err) = written {
        eprintln!("stdout: {err}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "{}: {} distinct {key} keys from {records} records in {} sets",
        S::NAME,
        order.len(),
        opts.paths.len()
    );
    if order.is_empty() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The `key` keys of one works or references set, each to `emit`; how
/// many records were read.
fn keys_one<S: Keys>(
    path: &Path,
    key: Key,
    limit: Option<u64>,
    emit: &mut dyn FnMut(String),
) -> io::Result<u64> {
    let reader = open_set::<S>(path)?;
    let limit = records_limit(limit);
    let mut records = 0u64;
    reader.with_table(|| match reader.header().kind {
        Kind::Works => {
            for record in reader.records::<S::Work<'_>>().take(limit) {
                S::work_keys(&record?, key, emit);
                records += 1;
            }
            Ok(records)
        }
        Kind::References => {
            for record in reader
                .records::<(S::Id<'_>, S::Reference<'_>)>()
                .take(limit)
            {
                let (_, reference) = record?;
                S::reference_keys(&reference, key, emit);
                records += 1;
            }
            Ok(records)
        }
        Kind::Deletions | Kind::Symbols => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "keys come from a works or references set",
        )),
    })
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn list<S: Lister>(opts: &Options) -> ExitCode {
    if !opts.paths.is_empty()
        || opts.limit.is_some()
        || opts.workers.is_some()
        || opts.out.is_some()
        || opts.survey
        || opts.seed.is_some() != opts.by.is_some()
    {
        return usage();
    }
    let store = opts.store();
    let listed = if let (Some(seed), Some(key)) = (&opts.seed, opts.by) {
        ingest::list::list_seed::<S>(&store, key, seed)
    } else {
        let client = Client::new(&opts.user_agent());
        ingest::list::list::<S>(&store, &client)
    };
    match listed {
        Ok(listed) if opts.seed.is_some() => {
            let ids: u64 = listed.entries.iter().filter_map(|e| e.records).sum();
            eprintln!(
                "{}: listed {} lookups of {ids} ids to {}",
                S::NAME,
                listed.entries.len(),
                listed.path.display()
            );
            ExitCode::SUCCESS
        }
        Ok(listed) => {
            let bytes: u64 = listed.entries.iter().filter_map(|e| e.bytes).sum();
            let records: u64 = listed.entries.iter().filter_map(|e| e.records).sum();
            let sized = listed.entries.iter().filter(|e| e.bytes.is_some()).count();
            eprint!(
                "{}: listed {} entries to {}",
                S::NAME,
                listed.entries.len(),
                listed.path.display()
            );
            if sized > 0 {
                eprint!(
                    "; {:.1} GiB in the {sized} with a size",
                    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                );
            }
            if records > 0 {
                eprint!("; {records} records declared");
            }
            eprintln!();
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{} list: {e}", S::NAME);
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::cast_precision_loss)] // reporting only
fn fetch<S: Lister>(opts: &Options) -> ExitCode {
    let [listing] = opts.paths.as_slice() else {
        return usage();
    };
    if opts.out.is_some() || opts.survey || opts.seed.is_some() || opts.by.is_some() {
        return usage();
    }
    let store = opts.store();
    let entries = match ingest::listing::read(listing) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("{} fetch: {e}", S::NAME);
            return ExitCode::from(2);
        }
    };
    let client = Client::new(&opts.user_agent());
    let run = ingest::fetch::Run {
        store: &store,
        source: S::NAME,
        listing,
        workers: opts.workers.unwrap_or(FETCH_WORKERS),
        limit: opts.limit.map(|n| usize::try_from(n).unwrap_or(usize::MAX)),
        pace: S::PACE,
        contact: opts.contact::<S>(),
    };
    let result = ingest::fetch::fetch(&run, &entries, &client, |entry, receipt| {
        eprintln!(
            "fetched {} ({:.1} MiB in {:.1}s{}; {})",
            entry.path,
            receipt.bytes as f64 / (1024.0 * 1024.0),
            receipt.took_ms as f64 / 1000.0,
            receipt
                .resumed_from
                .map_or(String::new(), |b| format!(", resumed at {b}")),
            if receipt.checks.is_empty() {
                "no check available".to_owned()
            } else {
                format!("{:?}", receipt.checks)
            }
        );
    });
    match result {
        Ok(fetched) => {
            let bytes: u64 = fetched.receipts.iter().map(|r| r.bytes).sum();
            eprintln!(
                "{}: fetched {} files, {:.1} MiB in {:.1}s; {} already receipted",
                S::NAME,
                fetched.receipts.len(),
                bytes as f64 / (1024.0 * 1024.0),
                fetched.elapsed.as_secs_f64(),
                fetched.skipped
            );
            ExitCode::SUCCESS
        }
        Err(ingest::fetch::Error::Torrent(torrents)) => {
            eprintln!(
                "{} fetch: torrents are fetched by aria2c, run by you:",
                S::NAME
            );
            for t in &torrents {
                eprintln!(
                    "  aria2c --file-allocation=none --seed-time=0 -d {} {}",
                    store.raw(S::NAME, &t.path).display(),
                    t.url
                );
            }
            ExitCode::from(2)
        }
        Err(e) => {
            eprintln!("{} fetch: {e}", S::NAME);
            ExitCode::FAILURE
        }
    }
}

/// The output prefix of a listing entry under `out`: its own path with
/// `.gz` and one more extension dropped, so the normalized layer mirrors
/// the raw one.
fn prefix_of(out: &Path, entry_path: &str) -> PathBuf {
    let raw = Path::new(entry_path);
    let mut prefix = out.to_path_buf();
    if let Some(parent) = raw.parent() {
        prefix.push(parent);
    }
    prefix.push(ingest::run::stem(raw));
    prefix
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)] // reporting only; one command
fn ingest<S, K>(opts: &Options) -> ExitCode
where
    S: Lister,
    K: StatsReport + for<'n> Sink<'n, S>,
{
    let [listing] = opts.paths.as_slice() else {
        return usage();
    };
    if opts.survey || opts.seed.is_some() || opts.by.is_some() {
        return usage();
    }
    let store = opts.store();
    let entries = match ingest::listing::read(listing) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("{} ingest: {e}", S::NAME);
            return ExitCode::from(2);
        }
    };
    let out = opts
        .out
        .clone()
        .unwrap_or_else(|| store.normalized(model::VERSION, S::NAME));
    let prefixes: Arc<[PathBuf]> = entries.iter().map(|e| prefix_of(&out, &e.path)).collect();
    let make = |_: usize| {
        let prefixes = Arc::clone(&prefixes);
        Ok((
            K::default(),
            Binary::new(S::NAME, model::VERSION, move |i, _: &Path| {
                prefixes[i].clone()
            }),
        ))
    };
    let client = Client::new(&opts.user_agent());
    let run = ingest::fetch::Run {
        store: &store,
        source: S::NAME,
        listing,
        workers: opts.workers.unwrap_or(FETCH_WORKERS),
        limit: None,
        pace: S::PACE,
        contact: opts.contact::<S>(),
    };
    let result = ingest::stream::ingest_parallel::<S, _, _, _>(
        &run,
        &entries,
        &client,
        opts.workers.unwrap_or(FETCH_WORKERS),
        opts.limit,
        make,
        |entry, receipt| {
            eprintln!(
                "ingested {} ({:.1} MiB in {:.1}s{}; {:?})",
                entry.path,
                receipt.bytes as f64 / (1024.0 * 1024.0),
                receipt.took_ms as f64 / 1000.0,
                receipt
                    .resumed_from
                    .map_or(String::new(), |b| format!(", resumed at {b}")),
                receipt.checks
            );
        },
    );
    let ingested = match result {
        Ok(ingested) => ingested,
        Err(ingest::fetch::Error::Torrent(torrents)) => {
            eprintln!(
                "{} ingest: torrents are fetched by aria2c, run by you:",
                S::NAME
            );
            for t in &torrents {
                eprintln!(
                    "  aria2c --file-allocation=none --seed-time=0 -d {} {}",
                    store.raw(S::NAME, &t.path).display(),
                    t.url
                );
            }
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("{} ingest: {e}", S::NAME);
            return ExitCode::FAILURE;
        }
    };
    let mut stats = K::default();
    let mut written: Vec<Written> = Vec::new();
    let mut write_error = None;
    for (s, mut binary) in ingested.outcome.sinks {
        stats.merge(s);
        // Sets of receipted files are complete and stay; the one left
        // open by a failure or the limit is abandoned with its files.
        binary.abandon_open();
        match binary.finish() {
            Ok(sets) => written.extend(sets),
            Err(e) => write_error = Some(e),
        }
    }
    stats.report();
    let records = written.iter().fold([0u64; 4], |a, w| {
        [
            a[0] + w.records[0],
            a[1] + w.records[1],
            a[2] + w.records[2],
            a[3] + w.records[3],
        ]
    });
    let bytes: u64 = written.iter().map(|w| w.bytes).sum();
    eprintln!(
        "wrote {} works, {} references, {} deletions, {} symbols in {} sets under {}: {:.1} MiB",
        records[0],
        records[1],
        records[2],
        records[3],
        written.len(),
        out.display(),
        bytes as f64 / (1024.0 * 1024.0)
    );
    let fetched: u64 = ingested.receipts.iter().map(|r| r.bytes).sum();
    eprintln!(
        "{}: ingested {} files, {:.1} MiB; {} already receipted",
        S::NAME,
        ingested.receipts.len(),
        fetched as f64 / (1024.0 * 1024.0),
        ingested.skipped
    );
    if let Some(path) = &ingested.cut_short {
        eprintln!(
            "left unfinished: {} (the run stopped inside it): partial transfer kept, no receipt, no output set",
            path.display()
        );
    }
    let code = match ingested.outcome.result {
        Ok(report) => {
            eprintln!("{}: {report}", S::NAME);
            ExitCode::SUCCESS
        }
        Err(failure) => failed(&failure),
    };
    if let Some(e) = write_error {
        eprintln!("{} write: {e}", S::NAME);
        return ExitCode::FAILURE;
    }
    code
}

/// The merge's identity pass over the store (merge plan §3, §4, §8):
/// the phases in the plan's order, then the registry, then the report.
#[allow(clippy::too_many_lines)]
fn merge_identity(opts: &Options) -> ExitCode {
    if !opts.paths.is_empty()
        || opts.workers.is_some()
        || opts.out.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
    {
        return usage();
    }
    let store = opts.store();
    let limit = opts.limit.map(|n| records_limit(Some(n)));
    let started = std::time::Instant::now();
    let allocations = ingest::alloc::allocations();
    let mut identity = Identity::new();
    let sources = (|| -> Result<_, merge::identity::Error> {
        let pubmed = merge_add::<Pubmed>(&mut identity, &store, limit)?;
        let crossref = merge_add::<Crossref>(&mut identity, &store, limit)?;
        let openalex = merge_add::<OpenAlex>(&mut identity, &store, limit)?;
        Ok((pubmed, crossref, openalex))
    })();
    let (pubmed, crossref, openalex) = match sources {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("merge identity: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "merge identity: {} records, {} names read in {:.1} s",
        identity.records(),
        identity.names(),
        started.elapsed().as_secs_f64()
    );
    identity.mark();
    eprintln!(
        "merge identity: {} conflict components",
        identity.conflicts()
    );
    let phases = (|| -> Result<(), merge::identity::Error> {
        merge_titles::<Pubmed>(&mut identity, &pubmed.0, limit)?;
        merge_titles::<Crossref>(&mut identity, &crossref.0, limit)?;
        merge_titles::<OpenAlex>(&mut identity, &openalex.0, limit)?;
        merge_targets::<Pubmed>(&mut identity, &pubmed.1, limit)?;
        merge_targets::<Crossref>(&mut identity, &crossref.1, limit)?;
        merge_targets::<OpenAlex>(&mut identity, &openalex.1, limit)?;
        Ok(())
    })();
    if let Err(e) = phases {
        eprintln!("merge identity: {e}");
        return ExitCode::FAILURE;
    }
    let res = match identity.resolve(opts.survey) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("merge identity: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The registry: held works in a deterministic order, then stubs.
    let registry_path = store.registry();
    let mut registry = match Registry::read(&registry_path) {
        Ok(registry) => registry,
        Err(e) => {
            eprintln!("merge identity: {}: {e}", registry_path.display());
            return ExitCode::FAILURE;
        }
    };
    let mut held: Vec<Vec<OwnedName>> = res
        .works
        .iter()
        .map(|w| w.registry_names(&res.arena))
        .collect();
    held.sort();
    let stubs = res.stub_works();
    let at = ingest::time::now();
    let complete = limit.is_none() && !opts.survey;
    let held_assignment = registry.assign(&held, &at, false);
    let stub_assignment = registry.assign(&stubs, &at, false);
    let dropped = if complete {
        let seen: HashSet<&OwnedName> = held.iter().chain(&stubs).flatten().collect();
        registry.drop_unseen(|n| seen.contains(n), &at)
    } else {
        Vec::new()
    };
    let mut out = io::stdout().lock();
    if let Err(e) = merge::report::identity(
        &mut out,
        &res,
        &stubs,
        Some(&held_assignment),
        Some(&stub_assignment),
    ) {
        eprintln!("merge identity: {e}");
        return ExitCode::FAILURE;
    }
    let lines = held_assignment.lines.len() + stub_assignment.lines.len() + dropped.len();
    if complete {
        let all: Vec<merge::registry::Line> = held_assignment
            .lines
            .into_iter()
            .chain(stub_assignment.lines)
            .chain(dropped)
            .collect();
        if let Err(e) = Registry::append(&registry_path, &all) {
            eprintln!("merge identity: {}: {e}", registry_path.display());
            return ExitCode::FAILURE;
        }
        eprintln!(
            "merge identity: {lines} lines appended to {}; {} ids minted in all",
            registry_path.display(),
            registry.minted()
        );
    } else {
        eprintln!(
            "merge identity: {lines} registry lines computed, none written (--limit or --survey)"
        );
    }
    eprintln!(
        "merge identity: {:.1} s, {} allocations",
        started.elapsed().as_secs_f64(),
        ingest::alloc::allocations() - allocations
    );
    if res.components.unresolved.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Reads every works set of one source into the identity pass; returns
/// the works and references sets found.
fn merge_add<S: Contributor>(
    identity: &mut Identity,
    store: &Store,
    limit: Option<usize>,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), merge::identity::Error> {
    let dir = store.normalized(model::VERSION, S::NAME);
    let io = |source| merge::identity::Error::Io {
        path: dir.clone(),
        source,
    };
    let works = merge::sets::find(&dir, Kind::Works).map_err(io)?;
    let references = merge::sets::find(&dir, Kind::References).map_err(io)?;
    for path in &works {
        identity.add::<S>(path, limit)?;
    }
    Ok((works, references))
}

fn merge_titles<S: Contributor>(
    identity: &mut Identity,
    works: &[PathBuf],
    limit: Option<usize>,
) -> Result<(), merge::identity::Error> {
    for path in works {
        if identity.needs_titles(path) {
            identity.titles::<S>(path, limit)?;
        }
    }
    Ok(())
}

fn merge_targets<S: Contributor>(
    identity: &mut Identity,
    references: &[PathBuf],
    limit: Option<usize>,
) -> Result<(), merge::identity::Error> {
    for path in references {
        identity.targets::<S>(path, limit)?;
    }
    Ok(())
}

fn failed(failure: &Failure) -> ExitCode {
    eprintln!("{failure}");
    if failure.stage == Stage::Read {
        ExitCode::from(2)
    } else {
        ExitCode::FAILURE
    }
}

/// Merging one worker's stats into another's.
trait Merge {
    fn merge(&mut self, other: Self);
}

/// Adding one worker's counter, map or sample list into another's.
trait Absorb {
    fn absorb(&mut self, other: Self);
}

impl Absorb for u64 {
    fn absorb(&mut self, other: Self) {
        *self += other;
    }
}

impl<const N: usize> Absorb for [u64; N] {
    fn absorb(&mut self, other: Self) {
        for (a, b) in self.iter_mut().zip(other) {
            *a += b;
        }
    }
}

impl<K: Ord> Absorb for BTreeMap<K, u64> {
    fn absorb(&mut self, other: Self) {
        for (k, v) in other {
            *self.entry(k).or_default() += v;
        }
    }
}

impl Absorb for Vec<String> {
    fn absorb(&mut self, other: Self) {
        for s in other {
            if self.len() < 8 {
                self.push(s);
            }
        }
    }
}

/// Destructures `other` exhaustively — a new field is a compile error here
/// until it is listed — and absorbs every field.
macro_rules! absorb_all {
    ($self:ident, $other:ident: $ty:ident { $($field:ident),* $(,)? }) => {
        let $ty { $($field),* } = $other;
        $( $self.$field.absorb($field); )*
    };
}

/// Counts `key` in a map of the data's own strings, allocating only the
/// first time a key is seen.
fn bump(map: &mut BTreeMap<String, u64>, key: &str) {
    if let Some(n) = map.get_mut(key) {
        *n += 1;
    } else {
        map.insert(key.to_owned(), 1);
    }
}

/// Keeps the first few malformed values for the report.
fn sample(samples: &mut Vec<String>, raw: &str, error: &impl Display) {
    if samples.len() < 8 {
        samples.push(format!("{raw:?}: {error}"));
    }
}

/// The PubMed stats report.
#[derive(Default)]
struct PubmedStats {
    citations: u64,
    deletions: u64,
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
    title: [u64; 3],
    abstracts: u64,
    sections: u64,
    sections_markup: u64,
    sections_math: u64,
    other_abstracts: u64,
    journal_issn: u64,
    journal_issn_linking: u64,
    journal_issn_malformed: u64,
    persons: u64,
    collectives: u64,
    orcid: [u64; 3],
    affiliations: u64,
    affiliation_ids: BTreeMap<&'static str, u64>,
    investigators: u64,
    mesh_headings: u64,
    mesh_major: u64,
    chemicals: u64,
    keywords: u64,
    grants: u64,
    grants_with_country: u64,
    data_banks: u64,
    history: BTreeMap<&'static str, u64>,
    publication_status: BTreeMap<&'static str, u64>,
    relations: BTreeMap<&'static str, u64>,
    references: u64,
    reference_ids: BTreeMap<&'static str, u64>,
    reference_doi_malformed: u64,
    reference_pmid_malformed: u64,
    reference_pmc_malformed: u64,
    malformed_samples: Vec<String>,
}

impl PubmedStats {
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
                sample(&mut self.malformed_samples, raw, error);
            }
        }
    }
}

impl StatsReport for PubmedStats {
    fn report(&self) {
        eprintln!("citations {}  deletions {}", self.citations, self.deletions);
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
        eprintln!(
            "  title plain {} markup {} empty {}",
            self.title[0], self.title[1], self.title[2]
        );
        eprintln!(
            "  abstracts {} sections {} (markup {} math {})  other abstracts {}",
            self.abstracts,
            self.sections,
            self.sections_markup,
            self.sections_math,
            self.other_abstracts
        );
        eprintln!(
            "  journal issn {}  issn linking {}  malformed {}",
            self.journal_issn, self.journal_issn_linking, self.journal_issn_malformed
        );
        eprintln!(
            "  authors persons {} collectives {}  orcid valid {} malformed {} none {}",
            self.persons, self.collectives, self.orcid[0], self.orcid[1], self.orcid[2]
        );
        eprintln!(
            "  affiliations {}  ids {:?}  investigators {}",
            self.affiliations, self.affiliation_ids, self.investigators
        );
        eprintln!(
            "  mesh headings {} (major {})  chemicals {}  keywords {}  grants {} (with country {})  data banks {}",
            self.mesh_headings,
            self.mesh_major,
            self.chemicals,
            self.keywords,
            self.grants,
            self.grants_with_country,
            self.data_banks
        );
        eprintln!("  history {:?}", self.history);
        eprintln!("  publication status {:?}", self.publication_status);
        eprintln!("  relations {:?}", self.relations);
        eprintln!("references {}", self.references);
        eprintln!("  ids {:?}", self.reference_ids);
        eprintln!(
            "  malformed: doi {} pmid {} pmc {}",
            self.reference_doi_malformed,
            self.reference_pmid_malformed,
            self.reference_pmc_malformed
        );
        if !self.malformed_samples.is_empty() {
            eprintln!("  malformed samples:");
            for sample in &self.malformed_samples {
                eprintln!("    {sample}");
            }
        }
    }
}

impl<'n> Sink<'n, Pubmed> for PubmedStats {
    fn work(&mut self, c: &Citation<'n>) {
        use source_pubmed::normalize::AuthorName;
        self.citations += 1;
        self.doi_claim(0, c.ids.doi.as_ref());
        for claim in &c.ids.elocation_doi {
            self.doi_claim(1, Some(claim));
        }
        self.pii += u64::from(c.ids.pii.is_some());
        self.pmc += u64::from(c.ids.pmc.is_some());
        *self
            .published
            .entry(c.dates.published.precision())
            .or_default() += 1;
        self.electronic += u64::from(c.dates.electronic.is_some());
        self.revised += u64::from(c.dates.revised.is_some());
        self.completed += u64::from(c.dates.completed.is_some());
        *self.status.entry(c.status.token()).or_default() += 1;
        for l in &c.languages {
            *self.languages.entry(l.alpha_3()).or_default() += 1;
        }
        for pt in &c.publication_types {
            bump(&mut self.publication_types, pt.name.as_text().unwrap_or(""));
        }
        if c.title.is_empty() {
            self.title[2] += 1;
        } else if c.title.contains('<') {
            self.title[1] += 1;
        } else {
            self.title[0] += 1;
        }
        if let Some(a) = &c.r#abstract {
            self.abstracts += 1;
            for s in &a.sections {
                self.sections += 1;
                self.sections_markup += u64::from(s.text.contains('<'));
                self.sections_math += u64::from(s.text.contains("<mml:"));
            }
        }
        self.other_abstracts += c.other_abstracts.len() as u64;
        self.journal_issn += u64::from(c.journal.issn.is_some());
        self.journal_issn_linking += u64::from(c.journal.issn_linking.is_some());
        for claim in c
            .journal
            .issn
            .as_ref()
            .map(|i| &i.issn)
            .into_iter()
            .chain(c.journal.issn_linking.as_ref())
        {
            if let Claim::Malformed { raw, error } = claim {
                self.journal_issn_malformed += 1;
                sample(&mut self.malformed_samples, raw, error);
            }
        }
        for a in &c.authors {
            match a.name {
                AuthorName::Person(_) => self.persons += 1,
                AuthorName::Collective { .. } => self.collectives += 1,
            }
            match &a.orcid {
                None => self.orcid[2] += 1,
                Some(Claim::Valid(_)) => self.orcid[0] += 1,
                Some(Claim::Malformed { raw, error }) => {
                    self.orcid[1] += 1;
                    sample(&mut self.malformed_samples, raw, error);
                }
            }
            self.affiliations += a.affiliations.len() as u64;
            for af in &a.affiliations {
                for id in &af.ids {
                    *self.affiliation_ids.entry(id.source.token()).or_default() += 1;
                }
            }
        }
        self.investigators += c
            .investigator_lists
            .iter()
            .map(|l| l.investigators.len() as u64)
            .sum::<u64>();
        self.mesh_headings += c.mesh.len() as u64;
        self.mesh_major += c.mesh.iter().filter(|h| h.major).count() as u64;
        self.chemicals += c.chemicals.len() as u64;
        self.keywords += c.keywords.len() as u64;
        self.grants += c.grants.len() as u64;
        self.grants_with_country += c.grants.iter().filter(|g| g.country.is_some()).count() as u64;
        self.data_banks += c.data_banks.len() as u64;
        for h in &c.history {
            *self.history.entry(h.status.token()).or_default() += 1;
        }
        if let Some(ps) = c.publication_status {
            *self.publication_status.entry(ps.token()).or_default() += 1;
        }
        for r in &c.relations {
            *self.relations.entry(r.kind.token()).or_default() += 1;
        }
    }

    fn reference(&mut self, _from: &Pmid, r: &Reference<'n>) {
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
            sample(&mut self.malformed_samples, raw, error);
        }
        if let Some(Claim::Malformed { raw, error }) = &r.pmid {
            self.reference_pmid_malformed += 1;
            sample(&mut self.malformed_samples, raw, error);
        }
        if let Some(Claim::Malformed { raw, error }) = &r.pmc {
            self.reference_pmc_malformed += 1;
            sample(&mut self.malformed_samples, raw, error);
        }
    }

    fn deletion(&mut self, _: &Pmid) {
        self.deletions += 1;
    }
}

/// The Crossref stats report.
#[derive(Default)]
struct CrossrefStats {
    works: u64,
    deletions: u64,
    types: BTreeMap<source_crossref::vocab::WorkType, u64>,
    doi_lowercased: u64,
    owner_differs: u64,
    issued: BTreeMap<&'static str, u64>,
    title: [u64; 3],
    alternate_titles: u64,
    subtitle_empty: u64,
    abstracts: [u64; 3],
    persons: u64,
    orgs: u64,
    with_orcid: u64,
    affiliations_with_ror: u64,
    issns: u64,
    isbns: u64,
    languages: BTreeMap<&'static str, u64>,
    references: u64,
    reference_doi: [u64; 3],
    reference_empty: u64,
    updates: BTreeMap<source_crossref::vocab::UpdateType, u64>,
    relations: BTreeMap<String, u64>,
    licenses: BTreeMap<source_crossref::vocab::ContentVersion, u64>,
    malformed_samples: Vec<String>,
}

impl<'n> Sink<'n, Crossref> for CrossrefStats {
    fn work(&mut self, w: &source_crossref::normalize::Work<'n>) {
        self.works += 1;
        *self.types.entry(w.kind).or_default() += 1;
        self.doi_lowercased += u64::from(!w.doi.is_borrowed());
        self.owner_differs += u64::from(w.owner != w.doi.registrant());
        *self
            .issued
            .entry(w.issued.map_or("none", |d| d.precision()))
            .or_default() += 1;
        match w.title {
            None => self.title[2] += 1,
            Some(t) if t.contains('<') => self.title[1] += 1,
            Some(_) => self.title[0] += 1,
        }
        self.alternate_titles += w.alternate_titles.len() as u64;
        self.subtitle_empty += u64::from(w.subtitle.is_some_and(str::is_empty));
        match w.r#abstract {
            None => self.abstracts[2] += 1,
            Some(a) if a.trim_start().starts_with("<jats:") => self.abstracts[0] += 1,
            Some(_) => self.abstracts[1] += 1,
        }
        for a in &w.authors {
            match a.name {
                source_crossref::normalize::AuthorName::Person { .. } => self.persons += 1,
                source_crossref::normalize::AuthorName::Org(_) => self.orgs += 1,
            }
            self.with_orcid += u64::from(a.orcid.is_some());
            self.affiliations_with_ror +=
                a.affiliations.iter().filter(|x| x.ror.is_some()).count() as u64;
        }
        self.issns += w.issns.len() as u64;
        self.isbns += w.isbns.len() as u64;
        if let Some(l) = w.language {
            *self.languages.entry(l.alpha_3()).or_default() += 1;
        }
        for u in w.updates_to.iter().chain(&w.updated_by) {
            *self.updates.entry(u.kind).or_default() += 1;
        }
        for r in &w.relations {
            bump(&mut self.relations, r.kind.as_text().unwrap_or(""));
        }
        for l in &w.licenses {
            *self.licenses.entry(l.version).or_default() += 1;
        }
    }

    fn reference(&mut self, _from: &Doi<'n>, r: &source_crossref::normalize::Reference<'n>) {
        self.references += 1;
        match &r.doi {
            None => self.reference_doi[2] += 1,
            Some(Claim::Valid(_)) => self.reference_doi[0] += 1,
            Some(Claim::Malformed { raw, error }) => {
                self.reference_doi[1] += 1;
                sample(&mut self.malformed_samples, raw, error);
            }
        }
        if r.doi.is_none()
            && r.unstructured.is_none()
            && r.article_title.is_none()
            && r.journal_title.is_none()
            && r.volume_title.is_none()
            && r.author.is_none()
        {
            self.reference_empty += 1;
        }
    }

    fn deletion(&mut self, _: &Doi<'n>) {
        self.deletions += 1;
    }
}

impl StatsReport for CrossrefStats {
    fn report(&self) {
        eprintln!("works {}  deletions {}", self.works, self.deletions);
        eprintln!("  types {:?}", self.types);
        eprintln!(
            "  doi lowercased (allocations) {}  owner prefix != registrant {}",
            self.doi_lowercased, self.owner_differs
        );
        eprintln!("  issued {:?}", self.issued);
        eprintln!(
            "  title plain {} markup {} none {}  alternate titles {}  empty subtitles {}",
            self.title[0], self.title[1], self.title[2], self.alternate_titles, self.subtitle_empty
        );
        eprintln!(
            "  abstract jats {} other {} none {}",
            self.abstracts[0], self.abstracts[1], self.abstracts[2]
        );
        eprintln!(
            "  authors persons {} orgs {} with orcid {}  affiliations with ror {}",
            self.persons, self.orgs, self.with_orcid, self.affiliations_with_ror
        );
        eprintln!("  issns {}  isbns {}", self.issns, self.isbns);
        eprintln!("  languages {:?}", self.languages);
        eprintln!("  updates {:?}", self.updates);
        eprintln!("  relations {:?}", self.relations);
        eprintln!("  licenses {:?}", self.licenses);
        eprintln!("references {}", self.references);
        eprintln!(
            "  doi valid {} malformed {} none {}  empty {}",
            self.reference_doi[0],
            self.reference_doi[1],
            self.reference_doi[2],
            self.reference_empty
        );
        if !self.malformed_samples.is_empty() {
            eprintln!("  malformed samples:");
            for s in &self.malformed_samples {
                eprintln!("    {s}");
            }
        }
    }
}

/// The OpenAlex stats report.
#[derive(Default)]
struct OpenAlexStats {
    works: u64,
    deletions: u64,
    types: BTreeMap<source_openalex::vocab::WorkType, u64>,
    doi: [u64; 3],
    doi_lowercased: u64,
    pmid: u64,
    mag: [u64; 2],
    languages: BTreeMap<&'static str, u64>,
    language_malformed: u64,
    title: [u64; 3],
    oa_status: BTreeMap<source_openalex::vocab::OaStatus, u64>,
    oa_url: u64,
    with_best_oa: u64,
    authorships: u64,
    authors_with_id: u64,
    with_orcid: u64,
    with_raw_orcid: u64,
    orcid_conflicts: u64,
    corresponding: u64,
    corresponding_institutions_disagree: u64,
    institutions: u64,
    institution_countries: u64,
    institutions_without_ror: u64,
    authorship_countries: u64,
    affiliations: u64,
    locations: u64,
    provenance: BTreeMap<Option<source_openalex::vocab::Provenance>, u64>,
    location_ids: BTreeMap<&'static str, u64>,
    with_source: u64,
    issn_l: u64,
    issn: [u64; 2],
    host_lineage: u64,
    licenses: BTreeMap<&'static str, u64>,
    versions: BTreeMap<source_openalex::vocab::Version, u64>,
    topics: u64,
    keywords: u64,
    concepts: u64,
    mesh: u64,
    sdgs: u64,
    awards: u64,
    funders: u64,
    funders_with_ror: u64,
    apc: [u64; 2],
    abstracts: u64,
    counts_by_year: u64,
    indexed_in: BTreeMap<source_openalex::vocab::Index, u64>,
    flags: [u64; 3],
    references: u64,
    self_references: u64,
    related: u64,
    malformed_samples: Vec<String>,
}

impl StatsReport for OpenAlexStats {
    fn report(&self) {
        eprintln!("works {}  deletions {}", self.works, self.deletions);
        eprintln!("  types {:?}", self.types);
        eprintln!(
            "  doi valid {} malformed {} none {}  lowercased (allocations) {}",
            self.doi[0], self.doi[1], self.doi[2], self.doi_lowercased
        );
        eprintln!(
            "  pmid {}  mag {} (not the W number {})",
            self.pmid, self.mag[0], self.mag[1]
        );
        eprintln!(
            "  languages {:?}  malformed {}",
            self.languages, self.language_malformed
        );
        eprintln!(
            "  title plain {} markup {} empty {}",
            self.title[0], self.title[1], self.title[2]
        );
        eprintln!(
            "  oa status {:?}  oa_url {}  with best oa location {}",
            self.oa_status, self.oa_url, self.with_best_oa
        );
        eprintln!(
            "  authorships {}  with author id {}  orcid {} raw orcid {} conflicting {}  corresponding {} (institutions kept as the source's on {})",
            self.authorships,
            self.authors_with_id,
            self.with_orcid,
            self.with_raw_orcid,
            self.orcid_conflicts,
            self.corresponding,
            self.corresponding_institutions_disagree
        );
        eprintln!(
            "  institutions {} (with country {}, without ror {})  authorship countries {}  affiliations {}",
            self.institutions,
            self.institution_countries,
            self.institutions_without_ror,
            self.authorship_countries,
            self.affiliations
        );
        eprintln!(
            "  locations {}  provenance {:?}  ids {:?}",
            self.locations, self.provenance, self.location_ids
        );
        eprintln!(
            "  sources {}  issn_l {}  issn valid {} malformed {}  host lineage entries {}",
            self.with_source, self.issn_l, self.issn[0], self.issn[1], self.host_lineage
        );
        eprintln!("  licenses {:?}", self.licenses);
        eprintln!("  versions {:?}", self.versions);
        eprintln!(
            "  topics {}  keywords {}  concepts {}  mesh {}  sdgs {}",
            self.topics, self.keywords, self.concepts, self.mesh, self.sdgs
        );
        eprintln!(
            "  awards {}  funders {} (with ror {})  apc list {} paid {}",
            self.awards, self.funders, self.funders_with_ror, self.apc[0], self.apc[1]
        );
        eprintln!(
            "  abstracts {}  counts_by_year entries {}  indexed_in {:?}",
            self.abstracts, self.counts_by_year, self.indexed_in
        );
        eprintln!(
            "  retracted {}  xpac {}  has_fulltext {}  related works {}",
            self.flags[0], self.flags[1], self.flags[2], self.related
        );
        eprintln!(
            "references {} (self references {})",
            self.references, self.self_references
        );
        if !self.malformed_samples.is_empty() {
            eprintln!("  malformed samples:");
            for s in &self.malformed_samples {
                eprintln!("    {s}");
            }
        }
    }
}

impl<'n> Sink<'n, OpenAlex> for OpenAlexStats {
    #[allow(clippy::too_many_lines)] // one counter per field group
    fn work(&mut self, w: &source_openalex::normalize::Work<'n>) {
        self.works += 1;
        *self.types.entry(w.kind).or_default() += 1;
        match &w.doi {
            None => self.doi[2] += 1,
            Some(Claim::Valid(d)) => {
                self.doi[0] += 1;
                self.doi_lowercased += u64::from(!d.is_borrowed());
            }
            Some(Claim::Malformed { raw, error }) => {
                self.doi[1] += 1;
                sample(&mut self.malformed_samples, raw, error);
            }
        }
        self.pmid += u64::from(w.pmid.is_some());
        if let Some(m) = w.mag {
            self.mag[0] += 1;
            self.mag[1] += u64::from(u64::from(m.get()) != w.id.get());
        }
        match &w.language {
            None => {}
            Some(Claim::Valid(l)) => *self.languages.entry(l.alpha_3()).or_default() += 1,
            Some(Claim::Malformed { raw, error }) => {
                self.language_malformed += 1;
                sample(&mut self.malformed_samples, raw, error);
            }
        }
        match w.title {
            None | Some("") => self.title[2] += 1,
            Some(t) if t.contains('<') => self.title[1] += 1,
            Some(_) => self.title[0] += 1,
        }
        *self.oa_status.entry(w.open_access.status).or_default() += 1;
        self.oa_url += u64::from(w.open_access.oa_url.is_some());
        self.with_best_oa += u64::from(w.best_oa_location().is_some());
        for a in &w.authorships {
            self.authorships += 1;
            self.authors_with_id += u64::from(a.author.id.is_some());
            let orcid = a.author.orcid.as_ref().and_then(Claim::valid);
            self.with_orcid += u64::from(orcid.is_some());
            self.with_raw_orcid += u64::from(a.raw_orcid.is_some());
            self.orcid_conflicts += u64::from(matches!(
                (orcid, a.raw_orcid.as_ref()),
                (Some(x), Some(y)) if x != y
            ));
            self.corresponding += u64::from(a.is_corresponding);
            self.institutions += a.institutions.len() as u64;
            self.institution_countries += a
                .institutions
                .iter()
                .filter(|i| i.country.is_some())
                .count() as u64;
            self.institutions_without_ror +=
                a.institutions.iter().filter(|i| i.ror.is_none()).count() as u64;
            self.authorship_countries += a.countries.len() as u64;
            self.affiliations += a.affiliations.len() as u64;
        }
        self.corresponding_institutions_disagree +=
            u64::from(w.corresponding_institutions.is_some());
        for l in &w.locations {
            self.locations += 1;
            *self.provenance.entry(l.provenance).or_default() += 1;
            let kind = match l.id {
                Some(LocationId::Mag(_)) => "mag",
                Some(LocationId::Pmid(_)) => "pmid",
                Some(LocationId::Doi(_)) => "doi",
                Some(LocationId::Repo(_)) => "repo",
                None => "none",
            };
            *self.location_ids.entry(kind).or_default() += 1;
            if let Some(s) = &l.source {
                self.with_source += 1;
                self.issn_l += u64::from(s.issn_l.is_some());
                for c in &s.issns {
                    match c {
                        Claim::Valid(_) => self.issn[0] += 1,
                        Claim::Malformed { raw, error } => {
                            self.issn[1] += 1;
                            sample(&mut self.malformed_samples, raw, error);
                        }
                    }
                }
                self.host_lineage += s.host_organization_lineage.len() as u64;
            }
            if let Some(license) = l.license {
                *self.licenses.entry(license.token()).or_default() += 1;
            }
            if let Some(v) = l.version {
                *self.versions.entry(v).or_default() += 1;
            }
        }
        self.topics += w.topics.len() as u64;
        self.keywords += w.keywords.len() as u64;
        self.concepts += w.concepts.len() as u64;
        self.mesh += w.mesh.len() as u64;
        self.sdgs += w.sustainable_development_goals.len() as u64;
        self.awards += w.awards.len() as u64;
        self.funders += w.funders.len() as u64;
        self.funders_with_ror += w.funders.iter().filter(|f| f.ror.is_some()).count() as u64;
        self.apc[0] += u64::from(w.apc_list.is_some());
        self.apc[1] += u64::from(w.apc_paid.is_some());
        self.abstracts += u64::from(w.abstract_inverted_index.is_some());
        self.counts_by_year += w.counts_by_year.len() as u64;
        for i in w.indexed_in.iter() {
            *self.indexed_in.entry(i).or_default() += 1;
        }
        self.flags[0] += u64::from(w.is_retracted == Some(true));
        self.flags[1] += u64::from(w.is_xpac);
        self.flags[2] += u64::from(w.has_fulltext);
        self.related += w.related_works.len() as u64;
    }

    fn reference(&mut self, from: &OpenAlexId, to: &OpenAlexId) {
        self.references += 1;
        self.self_references += u64::from(from == to);
    }

    fn deletion(&mut self, _: &OpenAlexId) {
        self.deletions += 1;
    }
}

impl Merge for PubmedStats {
    fn merge(&mut self, other: Self) {
        absorb_all!(self, other: PubmedStats { citations, deletions, doi, elocation_doi, doi_lowercased, pii, pmc, published, electronic, revised, completed, status, languages, publication_types, title, abstracts, sections, sections_markup, sections_math, other_abstracts, journal_issn, journal_issn_linking, journal_issn_malformed, persons, collectives, orcid, affiliations, affiliation_ids, investigators, mesh_headings, mesh_major, chemicals, keywords, grants, grants_with_country, data_banks, history, publication_status, relations, references, reference_ids, reference_doi_malformed, reference_pmid_malformed, reference_pmc_malformed, malformed_samples });
    }
}

impl Merge for CrossrefStats {
    fn merge(&mut self, other: Self) {
        absorb_all!(self, other: CrossrefStats { works, deletions, types, doi_lowercased, owner_differs, issued, title, alternate_titles, subtitle_empty, abstracts, persons, orgs, with_orcid, affiliations_with_ror, issns, isbns, languages, references, reference_doi, reference_empty, updates, relations, licenses, malformed_samples });
    }
}

impl Merge for OpenAlexStats {
    fn merge(&mut self, other: Self) {
        absorb_all!(self, other: OpenAlexStats { works, deletions, types, doi, doi_lowercased, pmid, mag, languages, language_malformed, title, oa_status, oa_url, with_best_oa, authorships, authors_with_id, with_orcid, with_raw_orcid, orcid_conflicts, corresponding, corresponding_institutions_disagree, institutions, institution_countries, institutions_without_ror, authorship_countries, affiliations, locations, provenance, location_ids, with_source, issn_l, issn, host_lineage, licenses, versions, topics, keywords, concepts, mesh, sdgs, awards, funders, funders_with_ror, apc, abstracts, counts_by_year, indexed_in, flags, references, self_references, related, malformed_samples });
    }
}
