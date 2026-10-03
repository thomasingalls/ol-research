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
//! * `merge [--store <root>] [--limit N] [--survey]` runs both passes:
//!   identity and the registry as above, then every record's view and
//!   every reference through the names table (§5, §6), and writes the
//!   merged sets under the store's `merged/v<model>.<merge>/` (§8),
//!   replacing what was there; prints both reports. `--survey` counts
//!   values outside the merge's tables instead of stopping; it and
//!   `--limit` write nothing.
//!
//!   The registry's lines are of record, so both of these stage them in
//!   `merged/registry.jsonl.pending` and append them only once the run
//!   has nothing left that can fail — pass 2 runs for hours and a run
//!   that dies in it must leave the log as it found it (the user,
//!   2026-09-19; see [`merge::registry::Pending`]). Either command
//!   settles a sidecar an earlier run left behind before it reads the
//!   registry, under the merge lock.
//! * `merge --batch <path>... [--store <root>] [--public-key <file>]`
//!   merges the normalized sets at the paths — set files, set prefixes
//!   or directories of sets, any source — incrementally against the
//!   live tables (serving plan §5): registry and public lines for what
//!   it mints, the four delta logs, the journal line, the batch absorbed
//!   into the lexical index; the current epoch must be at the last full
//!   merge.
//! * `merge dump <set.zst>... [--limit N]` prints a merged set as JSON
//!   lines, for eyes.
//! * `survey placement [--store <root>] [--limit N]` counts what PubMed
//!   and Crossref say about where a work was published — volume, issue,
//!   pages, article number, dates — and compares the two records of each
//!   work the merged set pairs; it writes nothing (citation fields plan
//!   §7, CF0; see [`survey`]).
//! * `survey journals [--store <root>] [--limit N] [--out <list.tsv>]`
//!   counts the pairs of a PubMed and a Crossref record whose ISSNs do not
//!   overlap by what else says they are one journal: NLM's ISSN-L and its
//!   id on other PubMed records, Crossref's ISSNs written together; with
//!   `--out`, it lists the ISSNs NLM gives no ISSN-L (citation fields plan
//!   §11, item 3).
//!
//! `--survey` (plan §6) keeps going past records this build cannot read
//! and reports every distinct failure signature with counts and examples;
//! it writes no output and exits non-zero when anything failed. A record
//! that does not normalize costs that record, and so does one that does
//! not parse in a lines file or an API page (Crossref, OpenAlex), where
//! the next record starts on the next line or item regardless. A PubMed
//! record that does not parse costs the rest of its file, which the
//! report names as abandoned, because nothing after it in that XML stream
//! can be trusted to be a record boundary. Without `--survey` the first
//! failure of any kind stops the run.
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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
use merge::identity::Resolution;
use merge::record::{Edge, NameRow};
use merge::{Builder, Contributor, Identity, OwnedName, WorkId};
use model::{Claim, Doi, OpenAlexId, ParseError, Pmid};
use source_crossref::Crossref;
use source_crossref::normalize::LicenseStart;
use source_openalex::OpenAlex;
use source_openalex::normalize::LocationId;
use source_pubmed::Pubmed;
use source_pubmed::normalize::{Citation, HistoryDate, HistoryTime, Published, Reference};

mod survey;

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
  ol-rht pubmed genres < publication-type-uis.txt
  ol-rht merge identity [--store <root>] [--limit N] [--survey]
  ol-rht merge [--store <root>] [--limit N] [--survey] [--public-key <file>]
  ol-rht merge --batch <path>... [--store <root>] [--public-key <file>]
  ol-rht merge dump <set.zst>... [--limit N]
  ol-rht survey placement [--store <root>] [--limit N]
  ol-rht survey journals [--store <root>] [--limit N] [--out <list.tsv>]
  ol-rht graph build [--store <root>] [--held]
  ol-rht graph query <#id|name> [--store <root>] [--held] [--limit N]
  ol-rht graph check [--store <root>] [--held]
  ol-rht search build [--store <root>]
  ol-rht search query <words>... [--store <root>] [--limit N]
  ol-rht names build [--store <root>]
  ol-rht works build [--store <root>]
  ol-rht epoch publish [--store <root>]
  ol-rht epoch check [--store <root>]
  ol-rht query work <name|#id|ol:...> [--store <root>] [--limit N]
  ol-rht query search <words>... [--store <root>] [--limit N]
  ol-rht query expand <name|#id>... [--store <root>] [--limit N] [--walk <references|citers|both>]
  ol-rht query related <name|#id> --by <cites-same|cited-alongside> [--store <root>] [--limit N]
  ol-rht query path <name|#id> <name|#id> [--store <root>] [--depth N] [--limit N]
  ol-rht evaluate load <trec-pm <topics.xml> <qrels> | relish <RELISH_v1.json.gz>> --out <suite> [--store <root>] [--held-out]
  ol-rht evaluate run <suite | queries> --out <run> [--store <root>] [--limit depth] [--workers N] [--restricted]
  ol-rht evaluate score <suite | key> <run> [--test]
  ol-rht evaluate compare <suite | key> <base run> <new run> [--test]";

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
        ["merge", rest @ ..] if rest.first().is_none_or(|a| a.starts_with("--")) => {
            let Some((public_key, rest)) = strip_value(rest, "--public-key") else {
                return usage();
            };
            let batch = rest.contains(&"--batch");
            let rest: Vec<&str> = rest.into_iter().filter(|a| *a != "--batch").collect();
            match Options::parse(&rest) {
                Some(opts) if batch => merge_batch(&opts, public_key.map(Path::new)),
                Some(opts) => merge_all(&opts, public_key.map(Path::new)),
                None => usage(),
            }
        }
        [source, action, rest @ ..] => {
            let Some((held, rest)) = strip_held(source, rest) else {
                return usage();
            };
            let Some(opts) = Options::parse(&rest) else {
                return usage();
            };
            // The evaluation's own switches, each refused anywhere else.
            let evaluating = |named: &str| *source == "evaluate" && *action == named;
            if (opts.restricted && !evaluating("run"))
                || (opts.held_out && !evaluating("load"))
                || (opts.test && !(evaluating("score") || evaluating("compare")))
            {
                return usage();
            }
            match (*source, *action) {
                ("pubmed", "check") => check::<Pubmed>(&opts),
                ("pubmed", "normalize") => normalize::<Pubmed, PubmedStats>(&opts),
                ("pubmed", "dump") => dump::<Pubmed>(&opts),
                ("pubmed", "keys") => keys::<Pubmed>(&opts),
                ("pubmed", "list") => list::<Pubmed>(&opts),
                ("pubmed", "fetch") => fetch::<Pubmed>(&opts),
                ("pubmed", "ingest") => ingest::<Pubmed, PubmedStats>(&opts),
                ("pubmed", "genres") => genres(),
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
                ("merge", "dump") => merge_dump(&opts),
                ("survey", "placement") => survey::placement(&opts),
                ("survey", "journals") => survey::journals(&opts),
                ("graph", "build") => graph_build(&opts, held),
                ("graph", "query") => graph_query(&opts, held),
                ("graph", "check") => graph_check(&opts, held),
                ("search", "build") => search_build(&opts),
                ("search", "query") => search_query(&opts),
                ("names", "build") => names_build(&opts),
                ("works", "build") => works_build(&opts),
                ("epoch", "publish") => epoch_publish(&opts),
                ("epoch", "check") => epoch_check(&opts),
                ("query", "work") => query_work(&opts),
                ("query", "search") => query_search(&opts),
                ("query", "expand") => query_expand(&opts),
                ("query", "related") => query_related(&opts),
                ("query", "path") => query_path(&opts),
                ("evaluate", "load") => evaluate_load(&opts),
                ("evaluate", "run") => evaluate_run(&opts),
                ("evaluate", "score") => evaluate_score(&opts),
                ("evaluate", "compare") => evaluate_compare(&opts),
                _ => usage(),
            }
        }
        _ => usage(),
    }
}

/// The paths and trailing options of a source command.
// Each bool is one switch as typed, and each command refuses the ones it
// does not take; an enum of them would only be spelled back into these.
#[allow(clippy::struct_excessive_bools)]
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
    /// The other `--by`: which neighbourhood `query related` asks for,
    /// spelled as the JSON surface spells it. The two values no key is
    /// named after, so one option reads for both commands and neither
    /// takes the other's.
    related: Option<serve::By>,
    walk: Option<serve::Walk>,
    depth: Option<u32>,
    /// `evaluate run`'s date cutoff (evaluation plan §3.2).
    restricted: bool,
    /// `evaluate load`'s: the whole suite its test half (§7).
    held_out: bool,
    /// `evaluate score` and `compare`'s: read the test half, at a
    /// milestone (§7).
    test: bool,
}

impl Options {
    /// `None` when an option is malformed or repeated, or an unknown one
    /// appears. Paths may be absent (`list` takes none).
    // One arm an option, and they are only long together.
    #[allow(clippy::too_many_lines)]
    fn parse(args: &[&str]) -> Option<Self> {
        let mut rest = args;
        let (mut limit, mut workers, mut out, mut store, mut mailto, mut seed, mut by) =
            (None, None, None, None, None, None, None);
        let (mut walk, mut depth, mut related) = (None, None, None);
        let (mut survey, mut restricted, mut held_out, mut test) = (false, false, false, false);
        loop {
            match rest {
                [before @ .., "--survey"] => {
                    if survey {
                        return None;
                    }
                    survey = true;
                    rest = before;
                }
                [before @ .., "--restricted"] => {
                    if restricted {
                        return None;
                    }
                    restricted = true;
                    rest = before;
                }
                [before @ .., "--held-out"] => {
                    if held_out {
                        return None;
                    }
                    held_out = true;
                    rest = before;
                }
                [before @ .., "--test"] => {
                    if test {
                        return None;
                    }
                    test = true;
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
                    match *k {
                        "cites-same" | "cited-alongside" => {
                            let which = if *k == "cites-same" {
                                serve::By::CitesSame
                            } else {
                                serve::By::CitedAlongside
                            };
                            if related.replace(which).is_some() {
                                return None;
                            }
                        }
                        _ => {
                            if by.replace(Key::parse(k)?).is_some() {
                                return None;
                            }
                        }
                    }
                    rest = before;
                }
                [before @ .., "--walk", w] => {
                    let direction = match *w {
                        "references" => serve::Walk::References,
                        "citers" => serve::Walk::Citers,
                        "both" => serve::Walk::Both,
                        _ => return None,
                    };
                    if walk.replace(direction).is_some() {
                        return None;
                    }
                    rest = before;
                }
                [before @ .., "--depth", n] => {
                    if depth.replace(n.parse::<u32>().ok()?).is_some() {
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
            related,
            walk,
            depth,
            restricted,
            held_out,
            test,
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

/// Where each PubMed publication type on stdin sits in the genre table:
/// one UI a line — or a UI and anything after a tab, so a `<UI>\t<name>`
/// listing can be piped in as it comes.
///
/// The table is a `match`, so it cannot be printed; it can be asked, which
/// is what this does. A merge stops at the first type the table has never
/// seen (merge plan §5), and finding all of them takes one pass over the
/// corpus rather than one merge per surprise.
fn genres() -> ExitCode {
    let (mut known, mut unknown) = (0u64, 0u64);
    for line in io::stdin().lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("stdin: {err}");
                return ExitCode::from(2);
            }
        };
        let ui = line.split('\t').next().unwrap_or("").trim();
        if ui.is_empty() {
            continue;
        }
        if let Some(genre) = source_pubmed::view::genre_of(ui) {
            known += 1;
            println!("{ui}\t{genre}");
        } else {
            unknown += 1;
            println!("{ui}\tUNKNOWN");
        }
    }
    eprintln!("pubmed genres: {known} in the table, {unknown} not");
    if unknown == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A source's stats report (plan §6): what every normalized field held.
/// One per worker, merged at the end.
trait StatsReport: Default + Merge + Send {
    fn report(&self);

    /// Values this build cannot place and a merge would stop at. Reported
    /// by [`Self::report`]; counted here so `--survey` fails on them, as it
    /// does on a record it cannot read — both are the corpus saying the
    /// model is not ready for it.
    fn unplaced(&self) -> u64 {
        0
    }
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

/// What `--survey` found and whether it stands between this build and a
/// merge: a record it cannot read, or a value with no place in a table.
/// Both say the model is not ready for this corpus, so both fail.
fn survey_failed(name: &str, survey: &ingest::run::Survey, unplaced: u64) -> bool {
    eprint!("{name}: survey: {survey}");
    if unplaced > 0 {
        eprintln!("{name}: survey: {unplaced} records carry a value with no place in a table");
    }
    !survey.is_empty() || unplaced > 0
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
    let unplaced = stats.unplaced();
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
                    for kind in Kind::NORMALIZED {
                        let _ = std::fs::remove_file(kind.path(&w.prefix));
                    }
                    let _ = std::fs::remove_file(ingest::binary::Index::path(&w.prefix));
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
            if opts.survey && survey_failed(S::NAME, &outcome.survey, unplaced) {
                return ExitCode::FAILURE;
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
        Kind::Edges | Kind::Names | Kind::Relations | Kind::Unresolved | Kind::CitedAs => Err(
            io::Error::new(io::ErrorKind::InvalidInput, "a merged set; use merge dump"),
        ),
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
        Kind::Deletions
        | Kind::CitedAs
        | Kind::Symbols
        | Kind::Edges
        | Kind::Names
        | Kind::Relations
        | Kind::Unresolved => Err(io::Error::new(
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

/// The options no merge command takes.
fn merge_options_ok(opts: &Options) -> bool {
    opts.paths.is_empty()
        && opts.workers.is_none()
        && opts.out.is_none()
        && opts.mailto.is_none()
        && opts.seed.is_none()
        && opts.by.is_none()
}

/// What the identity pass leaves for the registry and the second pass:
/// the resolution, and per source the works and references sets read.
struct Pass1 {
    res: Resolution,
    pubmed: Sets,
    crossref: Sets,
    openalex: Sets,
    /// Which copy of each record speaks (merge plan §8), for both passes.
    copies: merge::copies::Copies,
}

/// One source's normalized sets, each with its name for the copy rule.
#[derive(Debug, Default)]
struct Sets {
    works: Vec<(PathBuf, Box<str>)>,
    references: Vec<(PathBuf, Box<str>)>,
    deletions: Vec<(PathBuf, Box<str>)>,
}

impl Sets {
    /// Every set of `S` under the store, named.
    fn find<S: Contributor>(store: &Store) -> Result<Self, merge::identity::Error> {
        let dir = store.normalized(model::VERSION, S::NAME);
        let named = |kind: Kind| -> Result<Vec<(PathBuf, Box<str>)>, merge::identity::Error> {
            let paths =
                merge::sets::find(&dir, kind).map_err(|source| merge::identity::Error::Io {
                    path: dir.clone(),
                    source,
                })?;
            paths
                .into_iter()
                .map(|path| {
                    let set = merge::copies::set_name(&dir, &path).ok_or_else(|| {
                        merge::identity::Error::Io {
                            path: path.clone(),
                            source: io::Error::other("not a set under its source's directory"),
                        }
                    })?;
                    Ok((path, set))
                })
                .collect()
        };
        Ok(Self {
            works: named(Kind::Works)?,
            references: named(Kind::References)?,
            deletions: named(Kind::Deletions)?,
        })
    }
}

/// Shards a spilled pass 2 finishes at a time: `OL_RHT_SPILL_SHARDS`, or 8
/// — 524,288 ids, about 80 ranges for the PubMed baseline. More holds more
/// of pass 2 at once; fewer opens more files and writes more, smaller
/// ones. The output is the same whatever it is.
fn spill_shards() -> u32 {
    std::env::var("OL_RHT_SPILL_SHARDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(8)
}

/// One step of a long run done: says so with the heap now and the most it
/// was during the step, in MiB so that a sample's steps are told apart,
/// then starts the peak again for the next (the serving layer's memory,
/// 2026-09-25).
#[allow(clippy::cast_precision_loss)] // reporting only
fn phase(what: &str, done: &str, started: std::time::Instant) {
    let mib = |bytes: u64| bytes as f64 / f64::from(1u32 << 20);
    eprintln!(
        "{what}: {done} at {:.1} s; heap {:.1} MiB, peak {:.1} MiB",
        started.elapsed().as_secs_f64(),
        mib(ingest::alloc::live_bytes()),
        mib(ingest::alloc::peak_bytes())
    );
    ingest::alloc::reset_peak();
}

/// The heap now and at its worst so far, for a merge phase's progress
/// line: where a merge's memory goes, phase by phase, is what says how to
/// split it (2026-09-24, the corpus survey killed at 80 GiB in pass 2).
#[allow(clippy::cast_precision_loss)] // reporting only
fn heap() -> String {
    let gib = |bytes: u64| bytes as f64 / f64::from(1u32 << 30);
    format!(
        "heap {:.2} GiB, peak {:.2} GiB",
        gib(ingest::alloc::live_bytes()),
        gib(ingest::alloc::peak_bytes())
    )
}

/// The identity pass over the store (merge plan §3, §8): the phases in
/// the plan's order, ending in the verdicts.
fn merge_pass1(
    opts: &Options,
    store: &Store,
    limit: Option<usize>,
    what: &str,
) -> Result<Pass1, ExitCode> {
    let started = std::time::Instant::now();
    let mut copies = merge::copies::Copies::new();
    // Every name the pass will read, gathered while the copies are read and
    // numbered in name order before the pass numbers any (the merge's
    // memory, step 2b, 2026-09-24): its runs go under the system's
    // temporary directory, the pod's own scratch on the box, as the spill
    // does.
    let names = std::env::temp_dir().join(format!("ol-rht-names-{}", std::process::id()));
    let sources = (|| -> Result<_, merge::identity::Error> {
        let pubmed = Sets::find::<Pubmed>(store)?;
        let crossref = Sets::find::<Crossref>(store)?;
        let openalex = Sets::find::<OpenAlex>(store)?;
        let io = |source| merge::identity::Error::Io {
            path: names.clone(),
            source,
        };
        let mut sorter = merge::dictionary::Sorter::create(&names).map_err(io)?;
        merge_copies::<Pubmed>(&mut copies, &pubmed, limit, &mut sorter)?;
        merge_copies::<Crossref>(&mut copies, &crossref, limit, &mut sorter)?;
        merge_copies::<OpenAlex>(&mut copies, &openalex, limit, &mut sorter)?;
        let read = started.elapsed();
        let pushed = sorter.pushed();
        let sorting = std::time::Instant::now();
        let dictionary = sorter.finish().map_err(io)?;
        // Every copy seen: each one's fate kept by its set and place, and
        // the copies by name let go (the merge's memory, 2026-09-26).
        copies.settle()?;
        eprintln!(
            "{what}: copies read in {:.1} s; {} distinct names of {pushed} read, a dictionary of {} MiB (the FST {} MiB) and the copies settled in {:.1} s; {}",
            read.as_secs_f64(),
            dictionary.len(),
            dictionary.bytes() >> 20,
            dictionary.fst_bytes() >> 20,
            sorting.elapsed().as_secs_f64(),
            heap()
        );
        Ok((pubmed, crossref, openalex, dictionary))
    })();
    let (pubmed, crossref, openalex, dictionary) = sources.map_err(|e| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    })?;
    let mut identity = Identity::with_dictionary(dictionary);
    identity.set_survey(opts.survey);
    let added = (|| -> Result<(), merge::identity::Error> {
        merge_add::<Pubmed>(&mut identity, &pubmed, limit, &copies)?;
        merge_add::<Crossref>(&mut identity, &crossref, limit, &copies)?;
        merge_add::<OpenAlex>(&mut identity, &openalex, limit, &copies)?;
        identity.check_dictionary()
    })();
    added.map_err(|e| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    })?;
    eprintln!(
        "{what}: {} records, {} names read in {:.1} s; {}",
        identity.records(),
        identity.names(),
        started.elapsed().as_secs_f64(),
        heap()
    );
    for (source, n) in &copies.stats.within_one_set {
        eprintln!(
            "{what}: {source}: {n} records with two copies in one set, whose references are both copies'"
        );
    }
    identity.mark();
    eprintln!(
        "{what}: {} conflict components; {}",
        identity.conflicts(),
        heap()
    );
    let phases = (|| -> Result<(), merge::identity::Error> {
        merge_titles::<Pubmed>(&mut identity, &pubmed, limit, &copies)?;
        merge_titles::<Crossref>(&mut identity, &crossref, limit, &copies)?;
        merge_titles::<OpenAlex>(&mut identity, &openalex, limit, &copies)?;
        merge_targets::<Pubmed>(&mut identity, &pubmed, limit, &copies)?;
        merge_targets::<Crossref>(&mut identity, &crossref, limit, &copies)?;
        merge_targets::<OpenAlex>(&mut identity, &openalex, limit, &copies)?;
        identity.relate();
        Ok(())
    })();
    phases.map_err(|e| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    })?;
    let mut res = identity.resolve(opts.survey).map_err(|e| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    })?;
    eprintln!(
        "{what}: identity resolved in {:.1} s; {}",
        started.elapsed().as_secs_f64(),
        heap()
    );
    merge_sort_stubs(what, &mut res)?;
    Ok(Pass1 {
        res,
        pubmed,
        crossref,
        openalex,
        copies,
    })
}

/// The registry's answer for one resolution: an id for every work, the
/// run's lines staged beside the registry, and what the rest of the run
/// needs of the registry — which is two numbers, so the registry itself
/// goes before pass 2 begins.
struct Assigned {
    ids: merge::Ids,
    /// The counts; its `lines` are empty, having been staged as made.
    assignment: merge::registry::Assignment,
    stub_works: merge::identity::StubWorks,
    /// The first merges and splits, written out for the report.
    changes: Vec<String>,
    /// `None` on a partial run.
    pending: Option<merge::registry::Pending>,
    totals: Totals,
}

/// The registry's size once this run's lines are in it: what the journal
/// line and the public ids need of it. Taken so the registry's fold — a
/// map of every name ever minted, 6.5 GB on the corpus — need not be held
/// through pass 2 for two integers (2026-09-24).
#[derive(Debug, Clone, Copy)]
struct Totals {
    minted: u32,
    lines: u64,
}

/// The stub names, found while the references were read, numbered in
/// name order and kept in a dictionary as the arena's are (the merge's
/// memory, 2026-09-26): its runs under the system's temporary directory,
/// as the arena's are.
fn merge_sort_stubs(what: &str, res: &mut Resolution) -> Result<(), ExitCode> {
    let sorting = std::time::Instant::now();
    let stubs = std::env::temp_dir().join(format!("ol-rht-stubs-{}", std::process::id()));
    merge::dictionary::Sorter::create(&stubs)
        .and_then(|sorter| res.sort_stubs(sorter))
        .map_err(|e| {
            eprintln!("{what}: {}: {e}", stubs.display());
            ExitCode::FAILURE
        })?;
    eprintln!(
        "{what}: {} stub names in a dictionary of {} MiB in {:.1} s; the resolution holds {}; {}",
        res.stubs.len(),
        res.stubs.dictionary().map_or(0, |d| d.bytes() >> 20),
        sorting.elapsed().as_secs_f64(),
        res.held(),
        heap()
    );
    Ok(())
}

/// Gives every work its id through the registry (merge plan §4): held
/// works in the registry's order, then stubs, in one assignment, so an
/// id that a held work and a stub both claim — a name a vanished record
/// held that a reference still cites — is settled by the split rule
/// rather than given twice: the held work keeps it and the stub is
/// minted anew (merge plan §11, item 3, 2026-09-19), which is why the
/// held count is what [`merge::registry::assign`] is told. On a
/// complete run, names no longer seen are dropped.
///
/// The registry is folded by name number ([`merge::registry::Ranked`]),
/// the works given as lists of numbers in the registry's order, and a
/// line's names read out only as it is staged (the merge's memory, step
/// 2d, 2026-09-24): on the corpus the registry's fold by name was 6.5 GB
/// and a copy of every work's names for the order about 5 GB, both at the
/// identity pass's peak.
fn merge_assign(
    what: &str,
    path: &Path,
    res: &Resolution,
    complete: bool,
) -> Result<Assigned, ExitCode> {
    use merge::registry::{NameIndex, Works};
    let fail = |e: io::Error| {
        eprintln!("{what}: {}: {e}", path.display());
        ExitCode::FAILURE
    };
    let mut ranked = merge::registry::Ranked::read(path, res).map_err(fail)?;
    let (order, mut works) = res.registry_works();
    // By number, a stub's names read out one at a time for their numbers
    // in the run's index, which is the arena's for a name it also holds
    // (the merge's memory, 2026-09-26).
    let stub_works = res.stub_groups();
    for names in stub_works.iter() {
        works.push(names.iter().map(|&ix| {
            res.index(&res.stubs.get(ix))
                .expect("a stub's name is the run's")
        }));
    }
    let at = ingest::time::now();
    let mut stager = merge_stager(what, path, complete)?;
    let mut computed = 0u64;
    let (mut assignment, dropped) = {
        let mut emit = |line: merge::registry::Line| -> io::Result<()> {
            computed += 1;
            stager.as_mut().map_or(Ok(()), |s| s.push(&line))
        };
        let assignment =
            merge::registry::assign_each(&mut ranked, &works, order.len(), &at, &mut emit)
                .map_err(fail)?;
        let dropped = if complete {
            let mut seen = vec![0u64; res.len().div_ceil(64)];
            for &ix in works.all() {
                seen[ix as usize / 64] |= 1 << (ix % 64);
            }
            let lines =
                ranked.drop_unseen(|ix| seen[ix as usize / 64] & (1 << (ix % 64)) != 0, &at);
            let n = lines.len() as u64;
            for line in lines {
                emit(line).map_err(fail)?;
            }
            n
        } else {
            0
        };
        (assignment, dropped)
    };
    debug_assert_eq!(works.len(), order.len() + stub_works.len());
    drop(works);
    // Counted with the assignment's own, as `Registry::assign` counts them,
    // so that the report, the journal and the batch say one number.
    assignment.dropped += dropped;
    let changes = merge_changes(res, &order, &stub_works, &assignment);
    let pending = merge_staged(what, path, stager, computed)?;
    let (held_ids, stub_ids) = assignment.ids.split_at(order.len());
    let mut ids = merge::Ids {
        held: vec![WorkId::FIRST; res.works.len()],
        stubs: stub_ids.to_vec(),
    };
    for (w, id) in order.iter().zip(held_ids) {
        ids.held[*w as usize] = *id;
    }
    let totals = Totals {
        minted: ranked.minted(),
        lines: ranked.lines(),
    };
    Ok(Assigned {
        ids,
        assignment,
        stub_works,
        changes,
        pending,
        totals,
    })
}

/// The first merges and splits an assignment made, written out for the
/// report (merge plan §11 item 27): each id retired, where it went, and
/// the work there — its records, then its names — so that what a new
/// source changes under ids of record can be seen before a merge of
/// record writes it. `order` is the held works in the order they were
/// assigned; the stubs follow.
fn merge_changes(
    res: &Resolution,
    order: &[u32],
    stubs: &merge::identity::StubWorks,
    assignment: &merge::registry::Assignment,
) -> Vec<String> {
    let describe = |w: usize| -> String {
        if let Some(&held) = order.get(w) {
            let work = res.works.get(held as usize);
            let records: Vec<String> = work
                .records
                .iter()
                .map(|&rix| {
                    let r = &res.records[rix as usize];
                    format!("{}={}", r.source, res.arena.get(r.native))
                })
                .collect();
            let names: Vec<String> = work
                .names
                .iter()
                .map(|n| res.arena.get(n.name).to_string())
                .collect();
            format!("{{{} | {}}}", records.join(" "), names.join(" "))
        } else {
            let names: Vec<String> = stubs
                .names(w - order.len())
                .iter()
                .map(|&ix| res.stubs.get(ix).to_string())
                .collect();
            format!("{{a stub | {}}}", names.join(" "))
        }
    };
    let mut changes: Vec<String> = assignment
        .merge_examples
        .iter()
        .map(|&(id, into, w)| format!("merged {id} into {into}: {}", describe(w)))
        .collect();
    for (id, parts) in &assignment.split_examples {
        let ids: Vec<String> = parts.iter().map(|(part, _)| part.to_string()).collect();
        let works: Vec<String> = parts.iter().map(|&(_, w)| describe(w)).collect();
        changes.push(format!(
            "split {id} into {}: {}",
            ids.join(", "),
            works.join(" || ")
        ));
    }
    changes
}

/// Bytes of the registry the last journal line already told readers
/// about: the serving layer tails it to exactly there (serving plan §5),
/// so recovery must never reach below it. Lines written before SP5 carry
/// a line count instead of an offset, which is what the fold turns into
/// one too.
fn registry_sealed(store: &Store) -> io::Result<u64> {
    let Some(line) = merge::journal::last(&store.journal())? else {
        return Ok(0);
    };
    match line.bytes.get("registry") {
        Some(bytes) => Ok(*bytes),
        None => merge::journal::offset_of(&store.registry(), line.registry),
    }
}

/// Settles anything an earlier run left staged beside the registry
/// before this one reads it. Under the lock, so nothing else is looking.
fn merge_recover(what: &str, store: &Store) -> Result<(), ExitCode> {
    let path = store.registry();
    let fail = |e: String| {
        eprintln!("{what}: {}: {e}", path.display());
        ExitCode::FAILURE
    };
    let sealed = registry_sealed(store).map_err(|e| fail(e.to_string()))?;
    if let Some(done) = merge::registry::recover(&path, sealed).map_err(|e| fail(e.to_string()))? {
        eprintln!("{what}: {}: {done}", path.display());
    }
    Ok(())
}

/// Begins staging the registry's new lines beside it — not yet of
/// record — unless the run is partial; [`merge_staged`] ends it. The run
/// promotes them with [`merge_promote`] once everything else it had to do
/// is done; dropping what these return throws them away, so every failure
/// between here and there leaves the registry exactly as the run found it
/// (the user, 2026-09-19).
fn merge_stager(
    what: &str,
    path: &Path,
    complete: bool,
) -> Result<Option<merge::registry::Stager>, ExitCode> {
    if !complete {
        return Ok(None);
    }
    merge::registry::Stager::begin(path).map(Some).map_err(|e| {
        eprintln!("{what}: {}: {e}", path.display());
        ExitCode::FAILURE
    })
}

/// Ends what [`merge_stager`] began: the sidecar whole under its pending
/// name, or, on a partial run, only the count of lines computed.
fn merge_staged(
    what: &str,
    path: &Path,
    stager: Option<merge::registry::Stager>,
    computed: u64,
) -> Result<Option<merge::registry::Pending>, ExitCode> {
    let Some(stager) = stager else {
        eprintln!("{what}: {computed} registry lines computed, none written (--limit or --survey)");
        return Ok(None);
    };
    let pending = stager.finish().map_err(|e| {
        eprintln!("{what}: {}: {e}", path.display());
        ExitCode::FAILURE
    })?;
    eprintln!(
        "{what}: {} registry lines staged in {}; of record when the run finishes",
        pending.lines(),
        pending.path().display()
    );
    Ok(Some(pending))
}

/// Stages a list of lines already made, through the same two steps a run
/// takes: what the driver's tests stage.
#[cfg(test)]
fn merge_stage(
    what: &str,
    path: &Path,
    lines: &[merge::registry::Line],
    complete: bool,
) -> Result<Option<merge::registry::Pending>, ExitCode> {
    let mut stager = merge_stager(what, path, complete)?;
    if let Some(s) = stager.as_mut() {
        for line in lines {
            s.push(line).map_err(|e| {
                eprintln!("{what}: {}: {e}", path.display());
                ExitCode::FAILURE
            })?;
        }
    }
    merge_staged(what, path, stager, lines.len() as u64)
}

/// Appends the staged lines to the registry — the one thing of record.
/// Nothing that can fail may come after this but the clock.
fn merge_promote(
    what: &str,
    path: &Path,
    minted: u32,
    pending: Option<merge::registry::Pending>,
) -> Result<(), ExitCode> {
    let Some(pending) = pending else {
        return Ok(());
    };
    let n = pending.commit().map_err(|e| {
        eprintln!("{what}: {}: {e}", path.display());
        ExitCode::FAILURE
    })?;
    eprintln!(
        "{what}: {n} lines appended to {}; {minted} ids minted in all",
        path.display()
    );
    Ok(())
}

/// The merge's identity pass over the store (merge plan §3, §4, §8):
/// the phases in the plan's order, then the registry, then the report.
fn merge_identity(opts: &Options) -> ExitCode {
    if !merge_options_ok(opts) {
        return usage();
    }
    let what = "merge identity";
    let store = opts.store();
    let limit = opts.limit.map(|n| records_limit(Some(n)));
    let started = std::time::Instant::now();
    let allocations = ingest::alloc::allocations();
    let _lock = match merge::layout::Lock::acquire(&store) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(code) = merge_recover(what, &store) {
        return code;
    }
    let pass = match merge_pass1(opts, &store, limit, what) {
        Ok(pass) => pass,
        Err(code) => return code,
    };
    let registry_path = store.registry();
    let complete = limit.is_none() && !opts.survey;
    let assigned = match merge_assign(what, &registry_path, &pass.res, complete) {
        Ok(assigned) => assigned,
        Err(code) => return code,
    };
    let mut out = io::stdout().lock();
    if let Err(e) = merge::report::identity(
        &mut out,
        &pass.res,
        &assigned.stub_works,
        Some(&assigned.assignment),
        &assigned.changes,
    ) {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    if let Err(code) = merge_promote(
        what,
        &registry_path,
        assigned.totals.minted,
        assigned.pending,
    ) {
        return code;
    }
    eprintln!(
        "{what}: {:.1} s, {} allocations",
        started.elapsed().as_secs_f64(),
        ingest::alloc::allocations() - allocations
    );
    if pass.res.components.unresolved.is_empty() && pass.res.stats.unmapped.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The whole merge (merge plan §8): the identity pass and the registry,
/// then every record's view and every reference through the names
/// table, the merged sets written, both reports printed.
#[allow(clippy::too_many_lines)]
fn merge_all(opts: &Options, public_key: Option<&Path>) -> ExitCode {
    if !merge_options_ok(opts) {
        return usage();
    }
    let what = "merge";
    let store = opts.store();
    let limit = opts.limit.map(|n| records_limit(Some(n)));
    let started = std::time::Instant::now();
    let allocations = ingest::alloc::allocations();
    let _lock = match merge::layout::Lock::acquire(&store) {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(code) = merge_recover(what, &store) {
        return code;
    }
    let Pass1 {
        res,
        pubmed,
        crossref,
        openalex,
        copies,
    } = match merge_pass1(opts, &store, limit, what) {
        Ok(pass) => pass,
        Err(code) => return code,
    };
    let registry_path = store.registry();
    let complete = limit.is_none() && !opts.survey;
    // Staged, not appended, and staged as made (`merge_assign`). The
    // registry is of record and pass 2 below runs for hours, so its lines
    // wait in a sidecar until the run has nothing left that can fail:
    // every `return` between here and the promotion drops `pending`,
    // which throws the sidecar away and leaves the registry exactly as
    // this run found it (the user, 2026-09-19 — a run that died in pass 2
    // left 39,907 lines of record that no run had concluded, and an
    // append-only log cannot take them back).
    let assigned = match merge_assign(what, &registry_path, &res, complete) {
        Ok(assigned) => assigned,
        Err(code) => return code,
    };
    let mut out = io::stdout().lock();
    if let Err(e) = merge::report::identity(
        &mut out,
        &res,
        &assigned.stub_works,
        Some(&assigned.assignment),
        &assigned.changes,
    ) {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    let pass1_clean = res.components.unresolved.is_empty() && res.stats.unmapped.is_empty();
    let Assigned {
        ids,
        assignment,
        stub_works,
        changes: _,
        pending,
        totals,
    } = assigned;
    let counts = Counts {
        minted: assignment.minted,
        merged: assignment.merged,
        split: assignment.split,
        dropped: assignment.dropped,
    };
    eprintln!(
        "{what}: identity and registry done in {:.1} s; {}",
        started.elapsed().as_secs_f64(),
        heap()
    );
    let mut builder = match Builder::with_stubs(res, ids, stub_works) {
        Ok(builder) => builder,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    builder.set_survey(opts.survey);
    // Pass 2's rows go to disk by work id as they are made, and `finish`
    // takes them a range at a time (the merge's memory, step 1,
    // 2026-09-24): under the system's temporary directory, which on the
    // box is the pod's own scratch, not the store.
    let spill = std::env::temp_dir().join(format!("ol-rht-spill-{}", std::process::id()));
    let width = spill_shards() * merge::layout::SHARD;
    if let Err(e) = builder.spill_to(&spill, width) {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "{what}: names table built; spilling pass 2 under {} in ranges of {width} ids; {}",
        spill.display(),
        heap()
    );
    let phases = (|| -> Result<(), merge::build::Error> {
        for (path, set) in &pubmed.works {
            builder.works::<Pubmed>(path, set, limit, &copies)?;
        }
        for (path, set) in &crossref.works {
            builder.works::<Crossref>(path, set, limit, &copies)?;
        }
        for (path, set) in &openalex.works {
            builder.works::<OpenAlex>(path, set, limit, &copies)?;
        }
        eprintln!(
            "{what}: views read in {:.1} s; {}",
            started.elapsed().as_secs_f64(),
            heap()
        );
        for (path, set) in &pubmed.references {
            builder.references::<Pubmed>(path, set, limit, &copies)?;
        }
        for (path, set) in &crossref.references {
            builder.references::<Crossref>(path, set, limit, &copies)?;
        }
        for (path, set) in &openalex.references {
            builder.references::<OpenAlex>(path, set, limit, &copies)?;
        }
        Ok(())
    })();
    if let Err(e) = phases {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "{what}: views and references read in {:.1} s; {}",
        started.elapsed().as_secs_f64(),
        heap()
    );
    if let Some((rows, bytes)) = builder.spilled() {
        eprintln!("{what}: {rows} rows spilled, {} MiB", bytes >> 20);
    }
    let dir = merge::layout::dir(&store);
    let (merged, written) = match builder.finish_spilled(complete.then_some(dir.as_path())) {
        Ok(finished) => finished,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "{what}: merged rows finished{} in {:.1} s; {}",
        if written.is_some() {
            " and written"
        } else {
            ""
        },
        started.elapsed().as_secs_f64(),
        heap()
    );
    if written.is_none() {
        eprintln!("{what}: merged sets computed, none written (--limit or --survey)");
    }
    // The point of no return: the merged sets on disk name this run's
    // ids, so from here the registry has to agree with them. Everything
    // still to come — the report, the public ids, the journal line — can
    // be redone by another run from a registry that holds these lines,
    // but not by one whose ids the sets on disk contradict.
    if let Err(code) = merge_promote(what, &registry_path, totals.minted, pending) {
        return code;
    }
    if let Err(e) = merge::report::merge(&mut out, &merged, written.as_ref()) {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    if complete && let Err(code) = merge_clock(what, &store, totals, counts, &merged, public_key) {
        return code;
    }
    eprintln!(
        "{what}: {:.1} s, {} allocations",
        started.elapsed().as_secs_f64(),
        ingest::alloc::allocations() - allocations
    );
    if pass1_clean && merged.stats.unmapped.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// What a run did to the registry, for the journal.
#[derive(Debug, Clone, Copy)]
struct Counts {
    minted: u64,
    merged: u64,
    split: u64,
    dropped: u64,
}

/// After a complete run: a public id for every id lacking one, when a
/// key was given (serving plan §6), and the journal line that is the
/// run's batch (§4).
fn merge_clock(
    what: &str,
    store: &Store,
    registry: Totals,
    counts: Counts,
    merged: &merge::Merged,
    public_key: Option<&Path>,
) -> Result<(), ExitCode> {
    let fail = |e: String| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    };
    let public_path = store.public();
    let mut public = merge::public::Public::read(&public_path)
        .map_err(|e| fail(format!("{}: {e}", public_path.display())))?;
    let at = ingest::time::now();
    match public_key {
        Some(path) => {
            let key = merge::public::Key::read(path)
                .map_err(|e| fail(format!("{}: {e}", path.display())))?;
            let ids = (1..=registry.minted).filter_map(WorkId::new);
            let lines = public.mint(ids, &key, &at);
            merge::public::Public::append(&public_path, &lines)
                .map_err(|e| fail(format!("{}: {e}", public_path.display())))?;
            eprintln!(
                "{what}: {} public ids minted; {} in all",
                lines.len(),
                public.len()
            );
        }
        None => eprintln!(
            "{what}: no --public-key: public ids not minted ({} in all)",
            public.len()
        ),
    }
    let journal_path = store.journal();
    let last = merge::journal::last(&journal_path)
        .map_err(|e| fail(format!("{}: {e}", journal_path.display())))?;
    let size = |path: &Path| -> Result<u64, ExitCode> {
        merge::delta::len(path).map_err(|e| fail(format!("{}: {e}", path.display())))
    };
    let line = merge::journal::Line {
        batch: last.map_or(1, |l| l.batch + 1),
        at,
        kind: merge::journal::Kind::Full,
        registry: registry.lines,
        public: public.lines(),
        delta: BTreeMap::new(),
        bytes: [
            ("registry".to_owned(), size(&store.registry())?),
            ("public".to_owned(), size(&public_path)?),
        ]
        .into_iter()
        .collect(),
        inputs: Vec::new(),
        records: merged.stats.viewed.values().sum(),
        minted: counts.minted,
        merged: counts.merged,
        split: counts.split,
        dropped: counts.dropped,
    };
    merge::journal::append(&journal_path, &line)
        .map_err(|e| fail(format!("{}: {e}", journal_path.display())))?;
    eprintln!("{what}: journal batch {} appended", line.batch);
    Ok(())
}

/// The sets of one prefix a batch merges, by source.
#[derive(Debug, Default)]
struct BatchSets {
    prefixes: BTreeMap<PathBuf, String>,
}

impl BatchSets {
    /// Every set prefix at or below the paths given, with the source
    /// that wrote it.
    fn find(paths: &[PathBuf]) -> io::Result<Self> {
        let mut this = Self::default();
        for path in paths {
            let files: Vec<PathBuf> = if path.is_dir() {
                let mut all = Vec::new();
                for kind in [Kind::Works, Kind::References, Kind::Deletions] {
                    all.extend(merge::sets::find(path, kind)?);
                }
                all
            } else {
                vec![path.clone()]
            };
            for file in files {
                let prefix = Kind::prefix_of(&file).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("{}: not a normalized set", file.display()),
                    )
                })?;
                if this.prefixes.contains_key(&prefix) {
                    continue;
                }
                let header = ingest::binary::header(&file)?;
                this.prefixes.insert(prefix, header.source);
            }
        }
        Ok(this)
    }
}

/// A record the batch does not carry, from the normalized layer (serving
/// plan §5, 2026-09-18): the one whose text a work's promoted field
/// wants, or the one a work keeps of a source whose twin the batch
/// withdraws or moves. The merged row names, per record, the set its
/// speaking copy came from, so one prefix is opened and scanned.
struct Normalized<'a> {
    store: &'a Store,
}

impl serve::incremental::Records for Normalized<'_> {
    fn record(
        &self,
        source: merge::SourceId,
        native: &OwnedName,
        set: &str,
    ) -> Result<Option<serve::incremental::Incoming>, String> {
        Ok(self
            .records(source, std::slice::from_ref(native), set)?
            .pop())
    }

    fn records(
        &self,
        source: merge::SourceId,
        natives: &[OwnedName],
        set: &str,
    ) -> Result<Vec<serve::incremental::Incoming>, String> {
        match source {
            merge::SourceId::Pubmed => records_in::<Pubmed>(self.store, natives, set),
            merge::SourceId::Crossref => records_in::<Crossref>(self.store, natives, set),
            merge::SourceId::OpenAlex => records_in::<OpenAlex>(self.store, natives, set),
        }
    }
}

/// A copy's rank inside one set: its stamp, its version, its place.
type InSet = (Option<model::Date>, u32, u64);

/// The records of `S` among `natives` that `set` holds: each the newest
/// copy there, joined with the others as the copy rule joins them (merge
/// plan §8), with every reference the set beside it gives. A set with an
/// index is sought, a record a seek and a frame; one without is scanned
/// whole (serving plan §11, item 35).
fn records_in<S: Contributor>(
    store: &Store,
    natives: &[OwnedName],
    set: &str,
) -> Result<Vec<serve::incremental::Incoming>, String> {
    let prefix = store.normalized(model::VERSION, S::NAME).join(set);
    let index = ingest::binary::Index::read(&prefix)
        .map_err(|e| format!("{}: {e}", ingest::binary::Index::path(&prefix).display()))?;
    let best = match index {
        Some(index) => records_sought::<S>(&prefix, set, natives, &index)?,
        None => records_scanned::<S>(&prefix, set, natives)?,
    };
    let mut out: Vec<serve::incremental::Incoming> =
        best.into_values().map(|(_, record)| record).collect();
    out.sort_by(|a, b| a.native.cmp(&b.native));
    Ok(out)
}

/// What a set holds of each record asked for: its rank among the set's
/// copies, and the copy as the copy rule joins them.
type Held = HashMap<OwnedName, (InSet, serve::incremental::Incoming)>;

/// One copy of a record, from a work of `S` read from `set`, where it is
/// the `ordinal`th work.
fn copy_of<S: Contributor>(
    work: &S::Work<'_>,
    table: &model::symbol::Table,
    set: &str,
    native: OwnedName,
    ordinal: u64,
) -> Result<(InSet, serve::incremental::Incoming), String> {
    let mut asserts = Vec::new();
    S::names(work, &mut |a| {
        asserts.push(merge::build::OwnedAssert {
            name: a.name.to_owned(),
            field: a.field.into(),
            native: a.native,
        });
    });
    let view = S::view(work, table).map_err(|e| e.to_string())?;
    let text = S::text(work, table).map_err(|e| e.to_string())?;
    let mut relations = Vec::new();
    S::relations(work, table, &mut |r| relations.push(r)).map_err(|e| e.to_string())?;
    let mut view = merge::view::Owned::resolve(&view, table);
    view.set = set.into();
    let rank = (view.revised, view.version, ordinal + 1);
    Ok((
        rank,
        serve::incremental::Incoming {
            source: S::SOURCE,
            native,
            asserts,
            view,
            text,
            relations,
            corrects_its_aliases: S::corrects_its_aliases(work),
            references: Vec::new(),
        },
    ))
}

/// Joins a copy into what the set holds of its record: the newer speaks,
/// the older fills it (merge plan §8).
fn join_copy(best: &mut Held, (rank, copy): (InSet, serve::incremental::Incoming)) {
    let native = copy.native.clone();
    let joined = match best.remove(&native) {
        None => (rank, copy),
        Some((seen, older)) if seen < rank => {
            let mut newer = copy;
            newer.absorb(&older);
            (rank, newer)
        }
        Some((seen, mut newer)) => {
            newer.absorb(&copy);
            (seen, newer)
        }
    };
    best.insert(native, joined);
}

/// One reference of `S` as a batch carries it.
fn reference_of<S: Contributor>(
    reference: &S::Reference<'_>,
    table: Option<&model::symbol::Table>,
) -> serve::incremental::Reference {
    let mut names = Vec::new();
    S::targets(reference, &mut |n| names.push(n.to_owned()));
    serve::incremental::Reference {
        names,
        citation: merge::build::OwnedCitation::of(&S::citation(reference), table),
    }
}

/// [`records_in`] for a set with no index: every work and every
/// reference read.
fn records_scanned<S: Contributor>(
    prefix: &Path,
    set: &str,
    natives: &[OwnedName],
) -> Result<Held, String> {
    let wanted: HashSet<&OwnedName> = natives.iter().collect();
    let works_path = Kind::Works.path(prefix);
    let say = |path: &Path, e: &dyn Display| format!("{}: {e}", path.display());
    let reader =
        merge::sets::open::<S>(&works_path, Kind::Works).map_err(|e| say(&works_path, &e))?;
    let table = reader
        .table()
        .ok_or_else(|| say(&works_path, &"no symbol table"))?;
    let mut best: Held = HashMap::new();
    reader.with_table(|| -> Result<(), String> {
        for (i, work) in reader.records::<S::Work<'_>>().enumerate() {
            let work = work.map_err(|e| say(&works_path, &e))?;
            let Some(native) = merge::copies::native_of::<S>(&work)
                .map(|n| n.to_owned())
                .filter(|n| wanted.contains(n))
            else {
                continue;
            };
            let copy = copy_of::<S>(&work, table, set, native, i as u64)
                .map_err(|e| say(&works_path, &e))?;
            join_copy(&mut best, copy);
        }
        Ok(())
    })?;
    if best.is_empty() {
        return Ok(best);
    }
    let references_path = Kind::References.path(prefix);
    if references_path.exists() {
        let reader = merge::sets::open::<S>(&references_path, Kind::References)
            .map_err(|e| say(&references_path, &e))?;
        let table = reader.table();
        reader.with_table(|| -> Result<(), String> {
            for row in reader.records::<(S::Id<'_>, S::Reference<'_>)>() {
                let (id, reference) = row.map_err(|e| say(&references_path, &e))?;
                if let Some((_, record)) = best.get_mut(&S::native(&id).to_owned()) {
                    record.references.push(reference_of::<S>(&reference, table));
                }
            }
            Ok(())
        })?;
    }
    Ok(best)
}

/// The frame of a table holding record `ordinal`: its place in the table
/// and its first record.
fn frame_of(frames: &[(u64, u64)], ordinal: u64) -> Option<(usize, u64)> {
    let k = frames.partition_point(|f| f.1 <= ordinal).checked_sub(1)?;
    Some((k, frames[k].1))
}

/// A frame into the cache, read unless it is there.
fn load_frame(
    frames: &mut HashMap<(Kind, usize), ingest::binary::Frame>,
    kind: Kind,
    path: &Path,
    k: usize,
    offset: u64,
) -> Result<(), String> {
    if let std::collections::hash_map::Entry::Vacant(slot) = frames.entry((kind, k)) {
        slot.insert(
            ingest::binary::frame(path, offset).map_err(|e| format!("{}: {e}", path.display()))?,
        );
    }
    Ok(())
}

/// The references of the `i`th work of a set with an index, `native`:
/// the ones that follow it in the references file, by seeks to the
/// frames that hold them.
fn sought_references<S: Contributor>(
    prefix: &Path,
    index: &ingest::binary::Index,
    table: &model::symbol::Table,
    frames: &mut HashMap<(Kind, usize), ingest::binary::Frame>,
    i: usize,
    native: &OwnedName,
) -> Result<Vec<serve::incremental::Reference>, String> {
    let references_path = Kind::References.path(prefix);
    let say = |e: &dyn Display| format!("{}: {e}", references_path.display());
    let start = index.works[i].1;
    let end = index.works.get(i + 1).map_or(index.references, |w| w.1);
    let mut out = Vec::new();
    let mut at = start;
    while at < end {
        let (k, first) = frame_of(&index.references_frames, at)
            .ok_or_else(|| say(&format!("no frame holds reference {at}")))?;
        load_frame(
            frames,
            Kind::References,
            &references_path,
            k,
            index.references_frames[k].0,
        )?;
        let frame = &frames[&(Kind::References, k)];
        let before = out.len();
        model::symbol::with_reading(table, || -> Result<(), String> {
            let skip = usize::try_from(at - first).map_err(|e| e.to_string())?;
            for row in frame
                .records::<(S::Id<'_>, S::Reference<'_>)>()
                .skip(skip)
                .take(usize::try_from(end - at).unwrap_or(usize::MAX))
            {
                let (id, reference) = row.map_err(|e| say(&e))?;
                if S::native(&id).to_owned() != *native {
                    return Err(say(&format!(
                        "a reference of {native} is another record's, where the index says"
                    )));
                }
                out.push(reference_of::<S>(&reference, Some(table)));
            }
            Ok(())
        })?;
        let read = out.len() - before;
        if read == 0 {
            return Err(say(&format!("reference {at} is not in its frame")));
        }
        at += read as u64;
    }
    Ok(out)
}

/// [`records_in`] for a set with an index: each record's works by a seek
/// to its frame, and its references — which follow it in the references
/// file — by seeks to the frames that hold them. A record the index
/// names that the file does not hold where it says, or a reference that
/// is not the record's, is a broken set and stops the read.
fn records_sought<S: Contributor>(
    prefix: &Path,
    set: &str,
    natives: &[OwnedName],
    index: &ingest::binary::Index,
) -> Result<Held, String> {
    let wanted: HashMap<String, &OwnedName> = natives.iter().map(|n| (n.to_string(), n)).collect();
    let ordinals: Vec<(usize, &OwnedName)> = index
        .works
        .iter()
        .enumerate()
        .filter_map(|(i, (key, _))| wanted.get(key.as_str()).map(|n| (i, *n)))
        .collect();
    let mut best: Held = HashMap::new();
    if ordinals.is_empty() {
        return Ok(best);
    }
    let works_path = Kind::Works.path(prefix);
    let say = |path: &Path, e: &dyn Display| format!("{}: {e}", path.display());
    let table = ingest::binary::table(prefix)
        .map_err(|e| say(prefix, &e))?
        .ok_or_else(|| say(prefix, &"no symbol table"))?;
    let mut frames: HashMap<(Kind, usize), ingest::binary::Frame> = HashMap::new();
    for &(i, native) in &ordinals {
        let (k, first) = frame_of(&index.works_frames, i as u64)
            .ok_or_else(|| say(&works_path, &format!("no frame holds work {i}")))?;
        load_frame(
            &mut frames,
            Kind::Works,
            &works_path,
            k,
            index.works_frames[k].0,
        )?;
        let frame = &frames[&(Kind::Works, k)];
        let copy = model::symbol::with_reading(&table, || -> Result<_, String> {
            let skip = usize::try_from(i as u64 - first).map_err(|e| e.to_string())?;
            let work = frame
                .records::<S::Work<'_>>()
                .nth(skip)
                .ok_or_else(|| say(&works_path, &format!("work {i} is not in its frame")))?
                .map_err(|e| say(&works_path, &e))?;
            if merge::copies::native_of::<S>(&work)
                .map(|n| n.to_owned())
                .as_ref()
                != Some(native)
            {
                return Err(say(
                    &works_path,
                    &format!("work {i} is not {native}, as the index says"),
                ));
            }
            copy_of::<S>(&work, &table, set, native.clone(), i as u64)
                .map_err(|e| say(&works_path, &e))
        })?;
        join_copy(&mut best, copy);
    }
    for &(i, native) in &ordinals {
        let references = sought_references::<S>(prefix, index, &table, &mut frames, i, native)?;
        if let Some((_, record)) = best.get_mut(native) {
            record.references.extend(references);
        }
    }
    Ok(best)
}

/// Reads one prefix's works, references and deletions sets of `S` into a
/// batch, each copy's view naming its set for the copy rule (merge plan
/// §8): a prefix under the store's normalized layer, as the full merge
/// reads it. Two copies of one record in the set are one delivery: the
/// newer's names and fields, both copies' references.
#[allow(clippy::too_many_lines)]
fn batch_add<S: Contributor>(
    store: &Store,
    prefix: &Path,
    batch: &mut serve::incremental::Batch,
) -> Result<u64, merge::build::Error> {
    let io = |path: &Path, source| merge::build::Error::Io {
        path: path.to_path_buf(),
        source,
    };
    let dir = store.normalized(model::VERSION, S::NAME);
    let set = merge::copies::prefix_name(&dir, prefix).ok_or_else(|| {
        merge::build::Error::Invariant(format!(
            "{}: not a set under {}, where the full merge reads its sets",
            prefix.display(),
            dir.display()
        ))
    })?;
    let mut by_native: BTreeMap<String, (usize, merge::copies::Delivery)> = BTreeMap::new();
    let mut read = 0u64;
    let works_path = Kind::Works.path(prefix);
    if works_path.exists() {
        let reader =
            merge::sets::open::<S>(&works_path, Kind::Works).map_err(|e| io(&works_path, e))?;
        let table = reader
            .table()
            .ok_or_else(|| merge::build::Error::NoTable(works_path.clone()))?;
        reader.with_table(|| -> Result<(), merge::build::Error> {
            for (i, work) in reader.records::<S::Work<'_>>().enumerate() {
                let work = work.map_err(|e| io(&works_path, e))?;
                let unmapped = |source| merge::build::Error::Unmapped {
                    path: works_path.clone(),
                    ordinal: i as u64 + 1,
                    source,
                };
                let mut asserts = Vec::new();
                S::names(&work, &mut |a| {
                    asserts.push(merge::build::OwnedAssert {
                        name: a.name.to_owned(),
                        field: a.field.into(),
                        native: a.native,
                    });
                });
                let Some(native) = asserts.iter().find(|a| a.native).map(|a| a.name.clone()) else {
                    return Err(merge::build::Error::Invariant(format!(
                        "{}: record {} has no native name",
                        works_path.display(),
                        i + 1
                    )));
                };
                let view = S::view(&work, table).map_err(unmapped)?;
                let text = S::text(&work, table).map_err(unmapped)?;
                let mut relations = Vec::new();
                S::relations(&work, table, &mut |r| relations.push(r)).map_err(unmapped)?;
                let mut view = merge::view::Owned::resolve(&view, table);
                view.set = set.clone();
                let delivery = merge::copies::Delivery::of(&view.borrow(), i as u64 + 1);
                let incoming = serve::incremental::Incoming {
                    source: S::SOURCE,
                    native,
                    asserts,
                    view,
                    text,
                    relations,
                    corrects_its_aliases: S::corrects_its_aliases(&work),
                    references: Vec::new(),
                };
                // Two copies of one record in the set: the newer speaks,
                // and the older's names and values join it, absence
                // being no assertion (merge plan §8, 2026-09-18).
                let key = incoming.native.to_string();
                if let Some((ix, current)) = by_native.get_mut(&key) {
                    if *current < delivery {
                        let mut newer = incoming;
                        newer.absorb(&batch.records[*ix]);
                        batch.records[*ix] = newer;
                        *current = delivery;
                    } else {
                        batch.records[*ix].absorb(&incoming);
                    }
                    continue;
                }
                by_native.insert(key, (batch.records.len(), delivery));
                batch.records.push(incoming);
                read += 1;
            }
            Ok(())
        })?;
    }
    let references_path = Kind::References.path(prefix);
    if references_path.exists() {
        let reader = merge::sets::open::<S>(&references_path, Kind::References)
            .map_err(|e| io(&references_path, e))?;
        let table = reader.table();
        reader.with_table(|| -> Result<(), merge::build::Error> {
            for record in reader.records::<(S::Id<'_>, S::Reference<'_>)>() {
                let (id, reference) = record.map_err(|e| io(&references_path, e))?;
                let native = S::native(&id).to_string();
                let Some(&(ix, _)) = by_native.get(&native) else {
                    return Err(merge::build::Error::Invariant(format!(
                        "{}: a reference of {native}, which the works set beside it does not hold",
                        references_path.display()
                    )));
                };
                let mut names = Vec::new();
                S::targets(&reference, &mut |n| names.push(n.to_owned()));
                batch.records[ix]
                    .references
                    .push(serve::incremental::Reference {
                        names,
                        citation: merge::build::OwnedCitation::of(&S::citation(&reference), table),
                    });
            }
            Ok(())
        })?;
    }
    let deletions_path = Kind::Deletions.path(prefix);
    if deletions_path.exists() {
        let reader = merge::sets::open::<S>(&deletions_path, Kind::Deletions)
            .map_err(|e| io(&deletions_path, e))?;
        for record in reader.records::<S::Id<'_>>() {
            let id = record.map_err(|e| io(&deletions_path, e))?;
            batch.deletions.push(serve::incremental::Deletion {
                source: S::SOURCE,
                native: S::native(&id).to_owned(),
                set: set.clone(),
            });
        }
    }
    Ok(read)
}

/// Every deletions set of `S` the store holds outside the batch's own
/// sets, for the copy rule: they withdraw the batch's copies in the sets
/// before theirs.
fn store_deletions<S: Contributor>(
    store: &Store,
    own: &HashSet<Box<str>>,
    out: &mut Vec<serve::incremental::Deletion>,
) -> Result<u64, merge::build::Error> {
    let sets = Sets::find::<S>(store).map_err(|e| merge::build::Error::Invariant(e.to_string()))?;
    let mut read = 0u64;
    for (path, set) in &sets.deletions {
        if own.contains(set) {
            continue;
        }
        let reader = merge::sets::open::<S>(path, Kind::Deletions).map_err(|source| {
            merge::build::Error::Io {
                path: path.clone(),
                source,
            }
        })?;
        for record in reader.records::<S::Id<'_>>() {
            let id = record.map_err(|source| merge::build::Error::Io {
                path: path.clone(),
                source,
            })?;
            out.push(serve::incremental::Deletion {
                source: S::SOURCE,
                native: S::native(&id).to_owned(),
                set: set.clone(),
            });
            read += 1;
        }
    }
    Ok(read)
}

/// The incremental merge (serving plan §5): the sets at the paths given
/// merged against the live tables and appended to the deltas.
#[allow(clippy::too_many_lines)]
fn merge_batch(opts: &Options, public_key: Option<&Path>) -> ExitCode {
    if opts.paths.is_empty()
        || opts.limit.is_some()
        || opts.survey
        || opts.workers.is_some()
        || opts.out.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
    {
        return usage();
    }
    let what = "merge --batch";
    let store = opts.store();
    let started = std::time::Instant::now();
    let fail = |e: String| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    };
    let key = match public_key {
        Some(path) => match merge::public::Key::read(path) {
            Ok(key) => Some(key),
            Err(e) => return fail(format!("{}: {e}", path.display())),
        },
        None => None,
    };
    let sets = match BatchSets::find(&opts.paths) {
        Ok(sets) => sets,
        Err(e) => return fail(e.to_string()),
    };
    if sets.prefixes.is_empty() {
        return fail("no normalized sets at the paths given".into());
    }
    let mut batch = serve::incremental::Batch::default();
    for (prefix, source) in &sets.prefixes {
        let read = match source.as_str() {
            "pubmed" => batch_add::<Pubmed>(&store, prefix, &mut batch),
            "crossref" => batch_add::<Crossref>(&store, prefix, &mut batch),
            "openalex" => batch_add::<OpenAlex>(&store, prefix, &mut batch),
            other => Err(merge::build::Error::Invariant(format!(
                "{}: written by {other:?}, which this binary does not merge",
                prefix.display()
            ))),
        };
        match read {
            Ok(n) => eprintln!("{what}: {}: {n} records", prefix.display()),
            Err(e) => return fail(e.to_string()),
        }
        let relative = prefix
            .strip_prefix(store.root())
            .unwrap_or(prefix)
            .to_string_lossy()
            .into_owned();
        batch.inputs.push(relative);
    }
    // The store's own deletions sets, which a batch of older sets than
    // they are must see as the full merge does.
    for source in sets.prefixes.values().collect::<BTreeSet<&String>>() {
        let dir = store.normalized(model::VERSION, source);
        let own: HashSet<Box<str>> = sets
            .prefixes
            .iter()
            .filter(|(_, s)| *s == source)
            .filter_map(|(prefix, _)| merge::copies::prefix_name(&dir, prefix))
            .collect();
        let read = match source.as_ref() {
            "pubmed" => store_deletions::<Pubmed>(&store, &own, &mut batch.store_deletions),
            "crossref" => store_deletions::<Crossref>(&store, &own, &mut batch.store_deletions),
            "openalex" => store_deletions::<OpenAlex>(&store, &own, &mut batch.store_deletions),
            _ => Ok(0),
        };
        if let Err(e) = read {
            return fail(e.to_string());
        }
    }
    eprintln!(
        "{what}: {} records, {} deletions read in {:.1} s",
        batch.records.len(),
        batch.deletions.len(),
        started.elapsed().as_secs_f64()
    );
    let _lock = match merge::layout::Lock::acquire(&store) {
        Ok(lock) => lock,
        Err(e) => return fail(e.to_string()),
    };
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            return fail(format!(
                "{e} (an epoch at the last full merge is the batch's base)"
            ));
        }
    };
    eprintln!(
        "{what}: epoch {} at batch {}, folded to {}, opened in {:.1} s",
        serving.epoch().epoch,
        serving.epoch().batch,
        serving.watermark().batch,
        started.elapsed().as_secs_f64()
    );
    let texts = Normalized { store: &store };
    let applied =
        match serve::incremental::apply_with(&store, &serving, batch, key.as_ref(), &texts) {
            Ok(applied) => applied,
            Err(e) => return fail(e.to_string()),
        };
    let a = &applied;
    println!("batch {} at {}", a.batch, a.at);
    println!(
        "  records: {} merged ({} copies superseded by a newer one, {} of them read for their citations alone, {} withdrawn; {} earlier copies read back for their citations, {} citing records restated for a name the batch moved, {} normalized sets read), {} deletions ({} of records no work had, {} not applying to the copy that speaks)",
        a.records,
        a.superseded,
        a.cited_only,
        a.withdrawn,
        a.earlier_copies,
        a.restated,
        a.sets_read,
        a.deleted,
        a.deletions_unknown,
        a.deletions_not_applying
    );
    println!(
        "  identity: {} components; {} ids minted ({} stubs), {} merged into an older id, {} split with no successor, {} dropped; {} names attached, {} dropped",
        a.components,
        a.minted,
        a.stubs_minted,
        a.merged,
        a.split,
        a.dropped,
        a.names_attached,
        a.names_dropped
    );
    println!(
        "  rows: {} held works, {} stubs written; {} public ids minted",
        a.works_put, a.stubs_put, a.public_minted
    );
    println!(
        "  references: {}",
        a.references
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "  arcs: {} added, {} removed, {} moved with a retired id; {} relation lists, {} unresolved lists",
        a.arcs_added, a.arcs_removed, a.arcs_moved, a.relations_put, a.unresolved_put
    );
    if a.arcs_kept > 0 || a.uncited_stubs > 0 {
        println!(
            "  {} arcs kept that a new copy no longer names; {} stubs are left uncited until a full merge",
            a.arcs_kept, a.uncited_stubs
        );
    }
    for t in &a.truncated {
        println!("  cut: {t}");
    }
    println!(
        "  lexical index: {} documents put, {} removed; {} in all",
        a.lexical.put, a.lexical.removed, a.lexical.documents
    );
    println!(
        "  folded: the serving layer answers at batch {}",
        serving.watermark().batch
    );
    eprintln!("{what}: {:.1} s", started.elapsed().as_secs_f64());
    ExitCode::SUCCESS
}

/// Prints merged sets as JSON lines.
fn merge_dump(opts: &Options) -> ExitCode {
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
        if let Err(err) = merge_dump_one(path, opts.limit, &mut out) {
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

fn merge_dump_one(path: &Path, limit: Option<u64>, out: &mut impl Write) -> io::Result<()> {
    let reader = merge::layout::open(path)?;
    let limit = records_limit(limit);
    reader.with_table(|| match reader.header().kind {
        Kind::Works => json_lines::<merge::record::Work<'_>>(&reader, limit, out),
        Kind::Edges => json_lines::<Edge>(&reader, limit, out),
        Kind::Names => json_lines::<NameRow<'_>>(&reader, limit, out),
        Kind::Relations => json_lines::<merge::record::Relation>(&reader, limit, out),
        Kind::Unresolved => json_lines::<merge::record::Unresolved<'_>>(&reader, limit, out),
        Kind::CitedAs => json_lines::<merge::record::CitedAs<'_>>(&reader, limit, out),
        Kind::Symbols => json_lines::<&str>(&reader, limit, out),
        Kind::References | Kind::Deletions => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a normalized set; use <source> dump",
        )),
    })
}

/// `--held` belongs to the graph commands alone; anywhere else it stays
/// an unknown option. `None` when it is repeated.
fn strip_held<'a>(source: &str, rest: &[&'a str]) -> Option<(bool, Vec<&'a str>)> {
    if source != "graph" {
        return Some((false, rest.to_vec()));
    }
    let kept: Vec<&str> = rest.iter().copied().filter(|a| *a != "--held").collect();
    match rest.len() - kept.len() {
        0 => Some((false, kept)),
        1 => Some((true, kept)),
        _ => None,
    }
}

/// Takes `flag <value>` out of `rest`; `None` when it is repeated or has
/// no value.
fn strip_value<'a>(rest: &[&'a str], flag: &str) -> Option<(Option<&'a str>, Vec<&'a str>)> {
    let mut value = None;
    let mut kept = Vec::with_capacity(rest.len());
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == flag {
            if value.is_some() {
                return None;
            }
            value = Some(*rest.get(i + 1)?);
            i += 2;
        } else {
            kept.push(rest[i]);
            i += 1;
        }
    }
    Some((value, kept))
}

/// The options a graph command takes: `--store` and `--held`, and for
/// `query` one work and `--limit`. `--walk` and `--depth` belong to
/// `query expand` and `query path` alone, which check them themselves.
fn graph_options_ok(opts: &Options, paths: usize, limit: bool) -> bool {
    opts.paths.len() == paths
        && (limit || opts.limit.is_none())
        && opts.out.is_none()
        && opts.workers.is_none()
        && opts.mailto.is_none()
        && opts.seed.is_none()
        && opts.by.is_none()
        && opts.related.is_none()
        && opts.walk.is_none()
        && opts.depth.is_none()
        && !opts.survey
}

/// What `query expand`, `query related` and `query path` take beyond a
/// graph command's options: `--limit` always, `--walk` for the first,
/// `--by` for the second and `--depth` for the third, each refused where
/// it means nothing. `--by` is *required* for `related`, the two
/// neighbourhoods being opposite questions.
fn query_options_ok(opts: &Options, paths: usize, walk: bool, depth: bool, related: bool) -> bool {
    (walk || opts.walk.is_none())
        && (depth || opts.depth.is_none())
        && (related == opts.related.is_some())
        && opts.paths.len() == paths
        && opts.out.is_none()
        && opts.workers.is_none()
        && opts.mailto.is_none()
        && opts.seed.is_none()
        && opts.by.is_none()
        && !opts.survey
}

/// The works of the merged directory and its edges shards.
fn graph_inputs(what: &str, merged: &Path) -> Result<(graph::Nodes, Vec<PathBuf>), ExitCode> {
    let shards = match merge::sets::find(merged, Kind::Edges) {
        Ok(shards) => shards,
        Err(e) => {
            eprintln!("{what}: {}: {e}", merged.display());
            return Err(ExitCode::FAILURE);
        }
    };
    if shards.is_empty() {
        eprintln!(
            "{what}: no merged sets under {}; run merge first",
            merged.display()
        );
        return Err(ExitCode::FAILURE);
    }
    let nodes = graph::Nodes::read(merged).map_err(|e| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    })?;
    Ok((nodes, shards))
}

/// Streams every edge of the shards in order, as often as asked.
fn graph_edges(
    shards: &[PathBuf],
) -> impl FnMut(&mut dyn FnMut(Edge) -> Result<(), graph::Error>) -> Result<(), graph::Error> + '_ {
    move |sink: &mut dyn FnMut(Edge) -> Result<(), graph::Error>| {
        for path in shards {
            let reader = merge::layout::open(path).map_err(|e| graph::Error::io(path, e))?;
            for row in reader.records::<Edge>() {
                sink(row.map_err(|e| graph::Error::io(path, e))?)?;
            }
        }
        Ok(())
    }
}

/// Reads the merged directory and writes the graph into `dir`.
fn graph_write(
    what: &str,
    store: &Store,
    dir: &Path,
    held: bool,
) -> Result<(graph::Nodes, graph::build::Built), ExitCode> {
    let merged = merge::layout::dir(store);
    let (nodes, shards) = graph_inputs(what, &merged)?;
    let input = graph::layout::Input {
        merged: merge::layout::version(),
        shards: shards.len(),
    };
    let options = graph::build::Options { held };
    let built =
        graph::build::build(dir, &nodes, input, options, graph_edges(&shards)).map_err(|e| {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        })?;
    Ok((nodes, built))
}

/// Builds both orientations from the merged sets and prints the report.
fn graph_build(opts: &Options, held: bool) -> ExitCode {
    if !graph_options_ok(opts, 0, false) {
        return usage();
    }
    let what = "graph build";
    let store = opts.store();
    let started = std::time::Instant::now();
    let allocations = ingest::alloc::allocations();
    let dir = graph::layout::dir(&store, held);
    let (nodes, built) = match graph_write(what, &store, &dir, held) {
        Ok(written) => written,
        Err(code) => return code,
    };
    eprintln!(
        "{what}: {} arcs over {} nodes written in {:.1} s",
        built.manifest.arcs,
        built.manifest.nodes,
        started.elapsed().as_secs_f64()
    );
    let base = match graph::Base::open(&built.dir) {
        Ok(base) => base,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut out = io::stdout().lock();
    if let Err(e) = graph::report::print(&mut out, &base, &nodes) {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "{what}: {:.1} s, {} allocations",
        started.elapsed().as_secs_f64(),
        ingest::alloc::allocations() - allocations
    );
    ExitCode::SUCCESS
}

/// Verifies the digests and the invariants, then rebuilds into scratch
/// and compares byte for byte.
fn graph_check(opts: &Options, held: bool) -> ExitCode {
    if !graph_options_ok(opts, 0, false) {
        return usage();
    }
    let what = "graph check";
    let store = opts.store();
    let started = std::time::Instant::now();
    let dir = graph::layout::dir(&store, held);
    let base = match graph::Base::open(&dir) {
        Ok(base) => base,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let verified = graph::check::digests(&base)
        .and_then(|files| graph::check::invariants(&base).map(|checked| (files, checked)));
    let (files, checked) = match verified {
        Ok(verified) => verified,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    drop(base);
    println!("graph check {}", graph::layout::name(held));
    println!(
        "  {files} files match their digests; {} arcs in both orientations over {} nodes hold every invariant",
        checked.arcs, checked.nodes
    );
    eprintln!(
        "{what}: invariants held in {:.1} s; rebuilding",
        started.elapsed().as_secs_f64()
    );
    let scratch = dir.with_file_name(format!("{}.check", graph::layout::name(held)));
    let (_, built) = match graph_write(what, &store, &scratch, held) {
        Ok(written) => written,
        Err(code) => return code,
    };
    let differing = graph::check::differing(&dir, &built.dir);
    if let Err(e) = std::fs::remove_dir_all(&built.dir) {
        eprintln!("{what}: {}: {e}", built.dir.display());
    }
    match differing {
        Ok(differing) if differing.is_empty() => {
            println!("  a rebuild is byte-identical");
            eprintln!("{what}: {:.1} s", started.elapsed().as_secs_f64());
            ExitCode::SUCCESS
        }
        Ok(differing) => {
            println!("  a rebuild DIFFERS in {}", differing.join(", "));
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Prints one work's names, references and citers with their asserters.
fn graph_query(opts: &Options, held: bool) -> ExitCode {
    if !graph_options_ok(opts, 1, true) {
        return usage();
    }
    let what = "graph query";
    let Some(text) = opts.paths[0].to_str() else {
        return usage();
    };
    let store = opts.store();
    let base = match graph::Base::open(&graph::layout::dir(&store, held)) {
        Ok(base) => base,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let wanted = if let Some(digits) = text.strip_prefix('#') {
        let Some(id) = digits.parse::<u32>().ok().and_then(WorkId::new) else {
            eprintln!("{what}: {text} is not an id");
            return ExitCode::FAILURE;
        };
        Ok(id)
    } else {
        match OwnedName::parse(text) {
            Ok(name) => Err(name),
            Err(e) => {
                eprintln!("{what}: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    let (id, names) = match graph_names(&merge::layout::dir(&store), wanted) {
        Ok(Some(found)) => found,
        Ok(None) => {
            eprintln!("{what}: {text}: no such work");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let limit = opts
        .limit
        .map_or(50, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let mut out = io::BufWriter::new(io::stdout().lock());
    let printed = (|| -> io::Result<()> {
        let past = if base.position(id).is_none() {
            " (past the base)"
        } else {
            ""
        };
        writeln!(out, "{id}: {}{past}", names.join(", "))?;
        graph_list(
            &mut out,
            "references",
            base.out_degree(id),
            base.references(id),
            limit,
        )?;
        graph_list(
            &mut out,
            "citers",
            base.in_degree(id),
            base.citers(id),
            limit,
        )?;
        out.flush()
    })();
    match printed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn graph_list(
    out: &mut impl Write,
    what: &str,
    degree: u64,
    arcs: impl Iterator<Item = graph::Arc>,
    limit: usize,
) -> io::Result<()> {
    writeln!(out, "  {what}: {degree}")?;
    let mut shown = 0u64;
    for arc in arcs.take(limit) {
        writeln!(out, "    {} by {}", arc.work, arc.by)?;
        shown += 1;
    }
    if shown < degree {
        writeln!(out, "    … and {} more", degree - shown)?;
    }
    Ok(())
}

/// The work a name belongs to — or the id given — and every name of that
/// work with its standing, from the names sets: a scan for the name, then
/// the one shard the work lives in.
fn graph_names(
    merged: &Path,
    wanted: Result<WorkId, OwnedName>,
) -> io::Result<Option<(WorkId, Vec<String>)>> {
    let at =
        |path: &Path, e: io::Error| io::Error::new(e.kind(), format!("{}: {e}", path.display()));
    let id = match wanted {
        Ok(id) => id,
        Err(name) => {
            let mut found = None;
            for path in merge::sets::find(merged, Kind::Names)? {
                let reader = merge::layout::open(&path).map_err(|e| at(&path, e))?;
                found = reader.with_table(|| -> io::Result<Option<WorkId>> {
                    for row in reader.records::<NameRow<'_>>() {
                        let row = row.map_err(|e| at(&path, e))?;
                        if row.name == name.as_name() {
                            return Ok(Some(row.work));
                        }
                    }
                    Ok(None)
                })?;
                if found.is_some() {
                    break;
                }
            }
            match found {
                Some(id) => id,
                None => return Ok(None),
            }
        }
    };
    let path = Kind::Names.path(&merge::layout::prefix(merged, merge::layout::shard(id)));
    if !path.exists() {
        return Ok(Some((id, Vec::new())));
    }
    let reader = merge::layout::open(&path).map_err(|e| at(&path, e))?;
    let names = reader.with_table(|| -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for row in reader.records::<NameRow<'_>>() {
            let row = row.map_err(|e| at(&path, e))?;
            if row.work == id {
                names.push(format!("{} ({})", row.name.to_owned(), row.standing));
            }
        }
        Ok(names)
    })?;
    Ok(Some((id, names)))
}

/// The options a search command takes: `--store`, and for `query` the
/// words and `--limit`.
fn search_options_ok(opts: &Options, query: bool) -> bool {
    (query != opts.paths.is_empty())
        && (query || opts.limit.is_none())
        && opts.out.is_none()
        && opts.workers.is_none()
        && opts.mailto.is_none()
        && opts.seed.is_none()
        && opts.by.is_none()
        && !opts.survey
}

/// Streams one source's normalized works sets into the assembler.
fn search_add<S: Contributor>(
    assembler: &mut merge::Assembler,
    store: &Store,
) -> Result<u64, merge::document::Error> {
    let dir = store.normalized(model::VERSION, S::NAME);
    let paths = merge::sets::find(&dir, Kind::Works).map_err(|e| merge::document::Error::Io {
        path: dir.clone(),
        source: e,
    })?;
    let mut read = 0;
    for path in &paths {
        let set =
            merge::copies::set_name(&dir, path).ok_or_else(|| merge::document::Error::Io {
                path: path.clone(),
                source: io::Error::other("not a set under its source's directory"),
            })?;
        read += assembler.add::<S>(path, &set)?;
    }
    Ok(read)
}

/// Every held work's document, from the merged and normalized sets.
fn assemble(
    what: &str,
    store: &Store,
    started: std::time::Instant,
) -> Result<merge::Assembly, ExitCode> {
    static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let merged = merge::layout::dir(store);
    // The documents and their text go to disk by id range as the join
    // makes them and come back a range at a time (the serving layer's
    // memory, 2026-09-24): under the system's temporary directory, the
    // pod's own scratch on the box, as the merge's spill does.
    let dir = std::env::temp_dir().join(format!(
        "ol-rht-documents-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let width = spill_shards() * merge::layout::SHARD;
    let assembled = (|| -> Result<_, merge::document::Error> {
        let mut assembler = merge::Assembler::read_in(&merged, &dir, width)?;
        phase(what, "documents read from the merged sets", started);
        search_add::<Pubmed>(&mut assembler, store)?;
        search_add::<Crossref>(&mut assembler, store)?;
        search_add::<OpenAlex>(&mut assembler, store)?;
        phase(what, "text read from the normalized sets", started);
        Ok(assembler.ranged())
    })();
    let assembly = assembled.map_err(|e| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    })?;
    if assembly.stats().documents == 0 {
        eprintln!(
            "{what}: no held works under {}; run merge first",
            merged.display()
        );
        return Err(ExitCode::FAILURE);
    }
    Ok(assembly)
}

/// Builds the works index into `dir` from an assembly taken a range at a
/// time, handing each range to `observe` as well — the lexical index,
/// when one is being built in the same pass.
fn works_ranged(
    what: &str,
    store: &Store,
    dir: &Path,
    assembly: &mut merge::Assembly,
    observe: impl FnMut(&[merge::Document]) -> Result<(), merge::document::Error>,
    batch: u64,
    started: std::time::Instant,
) -> Result<serve::works::Built, ExitCode> {
    let mut ranged = merge::document::Ranged::new(assembly, observe);
    let built = works_write(what, store, dir, &mut ranged, batch, started)?;
    ranged.finish().map_err(|e| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    })?;
    Ok(built)
}

/// The batch the merged sets are at: the journal's last line.
fn current_batch(what: &str, store: &Store) -> Result<merge::journal::Line, ExitCode> {
    let path = store.journal();
    match merge::journal::last(&path) {
        Ok(Some(line)) => Ok(line),
        Ok(None) => {
            eprintln!("{what}: no journal at {}; run merge first", path.display());
            Err(ExitCode::FAILURE)
        }
        Err(e) => {
            eprintln!("{what}: {}: {e}", path.display());
            Err(ExitCode::FAILURE)
        }
    }
}

/// Builds the names lookup into `dir` for a batch.
fn names_write(
    what: &str,
    store: &Store,
    dir: &Path,
    batch: u64,
) -> Result<serve::names::Built, ExitCode> {
    let fail = |e: String| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    };
    serve::names::build(
        dir,
        &merge::layout::dir(store),
        &store.registry(),
        &store.public(),
        batch,
    )
    .map_err(|e| fail(e.to_string()))
}

/// Builds the works index into `dir` for a batch.
fn works_write(
    what: &str,
    store: &Store,
    dir: &Path,
    documents: &mut dyn merge::Documents,
    batch: u64,
    started: std::time::Instant,
) -> Result<serve::works::Built, ExitCode> {
    let fail = |e: String| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    };
    let public = merge::public::ById::read(&store.public())
        .map_err(|e| fail(format!("{}: {e}", store.public().display())))?;
    phase(what, "public ids read for the works index", started);
    serve::works::build(dir, &merge::layout::dir(store), documents, &public, batch)
        .map_err(|e| fail(e.to_string()))
}

#[allow(clippy::cast_precision_loss)]
fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

/// Builds the names lookup and prints its report.
fn names_build(opts: &Options) -> ExitCode {
    if !graph_options_ok(opts, 0, false) {
        return usage();
    }
    let what = "names build";
    let store = opts.store();
    let started = std::time::Instant::now();
    let batch = match current_batch(what, &store) {
        Ok(line) => line,
        Err(code) => return code,
    };
    let dir = store.names(model::VERSION, merge::VERSION, serve::VERSION);
    let built = match names_write(what, &store, &dir, batch.batch) {
        Ok(built) => built,
        Err(code) => return code,
    };
    let m = &built.manifest;
    println!("names {} for batch {}", serve::version(), m.batch);
    println!(
        "  keys: {} ({} shared, {} public ids); retired ids: {} ({} split, answering with their parts)",
        m.keys, m.shared, m.public, m.retired, m.split
    );
    println!(
        "  files: {} {:.1} MiB, {} {:.1} MiB, {} {} bytes, {} {} bytes",
        serve::names::FST,
        mib(m.files[serve::names::FST].bytes),
        serve::names::SHARED,
        mib(m.files[serve::names::SHARED].bytes),
        serve::names::RETIRED,
        m.files[serve::names::RETIRED].bytes,
        serve::names::SPLIT,
        m.files.get(serve::names::SPLIT).map_or(0, |f| f.bytes)
    );
    eprintln!("{what}: {:.1} s", started.elapsed().as_secs_f64());
    ExitCode::SUCCESS
}

/// Builds the works index and prints its report.
fn works_build(opts: &Options) -> ExitCode {
    if !graph_options_ok(opts, 0, false) {
        return usage();
    }
    let what = "works build";
    let store = opts.store();
    let started = std::time::Instant::now();
    let batch = match current_batch(what, &store) {
        Ok(line) => line,
        Err(code) => return code,
    };
    let mut assembly = match assemble(what, &store, started) {
        Ok(assembled) => assembled,
        Err(code) => return code,
    };
    eprintln!(
        "{what}: {} documents assembled in {:.1} s",
        assembly.stats().documents,
        started.elapsed().as_secs_f64()
    );
    let dir = store.works(model::VERSION, merge::VERSION, serve::VERSION);
    let built = match works_ranged(
        what,
        &store,
        &dir,
        &mut assembly,
        |_| Ok(()),
        batch.batch,
        started,
    ) {
        Ok(built) => built,
        Err(code) => return code,
    };
    let m = &built.manifest;
    println!("works {} for batch {}", serve::version(), m.batch);
    println!(
        "  rows: {} works ({} held, {} stubs) over {} slots; {} with text, {} with a public id",
        m.works, m.held, m.stubs, m.nodes, m.with_text, m.with_public
    );
    println!(
        "  blocks: {} of {} rows, {:.1} MiB raw, {:.1} MiB compressed, offsets {:.1} MiB",
        m.blocks,
        m.block,
        mib(m.raw_bytes),
        mib(m.files[serve::works::BLOCKS].bytes),
        mib(m.files[serve::works::OFFSETS].bytes)
    );
    eprintln!("{what}: {:.1} s", started.elapsed().as_secs_f64());
    ExitCode::SUCCESS
}

/// Builds what is missing for the current batch and publishes an epoch.
/// For `epoch publish`: the works index and the lexical index from one
/// assembly, taken a range at a time and given to both, and let go before
/// the cited-as lookup and the graph are built — the documents held whole
/// were most of what the corpus's epoch would need (the serving layer's
/// memory, 2026-09-24). Either is skipped when it is already at the batch.
fn publish_documents(
    what: &str,
    store: &Store,
    batch: &merge::journal::Line,
    works_dir: &Path,
    search_dir: &Path,
    started: std::time::Instant,
) -> Result<(), ExitCode> {
    let fail = |e: String| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    };
    let works_wanted =
        !serve::works::Manifest::read(works_dir).is_ok_and(|m| m.batch == batch.batch);
    let search_wanted = !search::Manifest::read(search_dir).is_ok_and(|m| m.built_at >= batch.at);
    if !(works_wanted || search_wanted) {
        return Ok(());
    }
    let mut assembly = assemble(what, store, started)?;
    let mut lexical = if search_wanted {
        eprintln!("{what}: building the lexical index");
        Some(search::index::Build::start(search_dir).map_err(|e| fail(e.to_string()))?)
    } else {
        None
    };
    let observe = |docs: &[merge::Document]| match lexical.as_mut() {
        Some(build) => build
            .add(docs)
            .map_err(|e| merge::document::Error::Consumer(e.to_string())),
        None => Ok(()),
    };
    if works_wanted {
        eprintln!("{what}: building works for batch {}", batch.batch);
        works_ranged(
            what,
            store,
            works_dir,
            &mut assembly,
            observe,
            batch.batch,
            started,
        )?;
    } else {
        merge::document::Ranged::new(&mut assembly, observe)
            .finish()
            .map_err(|e| fail(e.to_string()))?;
    }
    drop(assembly);
    phase(what, "works index built, documents let go", started);
    if let Some(build) = lexical {
        build.finish(batch.batch).map_err(|e| fail(e.to_string()))?;
        phase(what, "lexical index committed", started);
    }
    Ok(())
}

fn epoch_publish(opts: &Options) -> ExitCode {
    if !graph_options_ok(opts, 0, false) {
        return usage();
    }
    let what = "epoch publish";
    let store = opts.store();
    let started = std::time::Instant::now();
    let batch = match current_batch(what, &store) {
        Ok(line) => line,
        Err(code) => return code,
    };
    let fail = |e: String| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    };
    if batch.kind != merge::journal::Kind::Full {
        return fail(format!(
            "batch {} is an incremental merge; an epoch is published at a full one (serving plan §4): run merge first",
            batch.batch
        ));
    }
    let published = (|| -> Result<serve::epoch::Manifest, ExitCode> {
        let names_dir = store.names(model::VERSION, merge::VERSION, serve::VERSION);
        if !serve::names::Manifest::read(&names_dir).is_ok_and(|m| m.batch == batch.batch) {
            eprintln!("{what}: building names for batch {}", batch.batch);
            names_write(what, &store, &names_dir, batch.batch)?;
            phase(what, "names built", started);
        }
        let works_dir = store.works(model::VERSION, merge::VERSION, serve::VERSION);
        let search_dir = search::dir(&store);
        publish_documents(what, &store, &batch, &works_dir, &search_dir, started)?;
        let cited_dir = store.cited(model::VERSION, merge::VERSION, serve::VERSION);
        if !serve::cited::Manifest::read(&cited_dir).is_ok_and(|m| m.batch == batch.batch) {
            eprintln!(
                "{what}: building the cited-as lookup for batch {}",
                batch.batch
            );
            serve::cited::build(&cited_dir, &merge::layout::dir(&store), batch.batch)
                .map_err(|e| fail(e.to_string()))?;
            phase(what, "cited-as lookup built", started);
        }
        let graph_dir = graph::layout::dir(&store, false);
        if !graph::Manifest::read(&graph_dir).is_ok_and(|m| m.built_at >= batch.at) {
            eprintln!("{what}: building the graph");
            graph_write(what, &store, &graph_dir, false)?;
            phase(what, "graph built", started);
        }
        let n = serve::epoch::current(&store)
            .map_err(|e| fail(e.to_string()))?
            .map_or(1, |c| c + 1);
        let artifact = |dir: &Path, manifest: &str| {
            serve::epoch::artifact(&store, dir, manifest).map_err(|e| fail(e.to_string()))
        };
        let manifest = serve::epoch::Manifest {
            epoch: n,
            batch: batch.batch,
            at: ingest::time::now(),
            merged: merge::layout::version(),
            artifacts: [
                ("names", artifact(&names_dir, serve::names::MANIFEST)?),
                ("works", artifact(&works_dir, serve::works::MANIFEST)?),
                ("cited", artifact(&cited_dir, serve::cited::MANIFEST)?),
                ("graph", artifact(&graph_dir, graph::layout::MANIFEST)?),
                ("search", artifact(&search_dir, search::index::MANIFEST)?),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
        };
        serve::epoch::publish(&store, &manifest).map_err(|e| fail(e.to_string()))?;
        Ok(manifest)
    })();
    let manifest = match published {
        Ok(manifest) => manifest,
        Err(code) => return code,
    };
    println!(
        "epoch {} published: batch {} of {}",
        manifest.epoch, manifest.batch, manifest.merged
    );
    for (name, artifact) in &manifest.artifacts {
        println!("  {name}: {}", artifact.dir);
    }
    eprintln!("{what}: {:.1} s", started.elapsed().as_secs_f64());
    ExitCode::SUCCESS
}

/// Verifies the current epoch: every artifact's digests, the names and
/// works rebuilt byte for byte, and the artifacts against each other.
fn epoch_check(opts: &Options) -> ExitCode {
    if !graph_options_ok(opts, 0, false) {
        return usage();
    }
    let what = "epoch check";
    let store = opts.store();
    let started = std::time::Instant::now();
    let fail = |e: String| {
        eprintln!("{what}: {e}");
        ExitCode::FAILURE
    };
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => return fail(e.to_string()),
    };
    let epoch = serving.epoch().clone();
    println!("epoch check {}: batch {}", epoch.epoch, epoch.batch);
    let checked = (|| -> Result<serve::core::Checked, ExitCode> {
        let files = graph::check::digests(serving.graph()).map_err(|e| fail(e.to_string()))?;
        println!("  graph: {files} files match their digests");
        let names_dir = store.root().join(&epoch.artifacts["names"].dir);
        let scratch = names_dir.with_file_name(format!("{}.check", serve::version()));
        let rebuilt = names_write(what, &store, &scratch, epoch.batch)?;
        let same = [
            serve::names::FST,
            serve::names::SHARED,
            serve::names::RETIRED,
            serve::names::SPLIT,
        ]
        .iter()
        .all(|f| rebuilt.manifest.files.get(*f) == serving.names().manifest().files.get(*f));
        let _ = std::fs::remove_dir_all(&scratch);
        if !same {
            return Err(fail("the names lookup rebuilt differently".into()));
        }
        println!("  names: a rebuild is byte-identical");
        phase(what, "names rebuilt", started);
        let works_dir = store.root().join(&epoch.artifacts["works"].dir);
        let scratch = works_dir.with_file_name(format!("{}.check", serve::version()));
        let mut assembly = assemble(what, &store, started)?;
        let rebuilt = works_ranged(
            what,
            &store,
            &scratch,
            &mut assembly,
            |_| Ok(()),
            epoch.batch,
            started,
        )?;
        drop(assembly);
        let same = [serve::works::BLOCKS, serve::works::OFFSETS]
            .iter()
            .all(|f| rebuilt.manifest.files[*f] == serving.works().manifest().files[*f]);
        let _ = std::fs::remove_dir_all(&scratch);
        if !same {
            return Err(fail("the works index rebuilt differently".into()));
        }
        println!("  works: a rebuild is byte-identical");
        phase(what, "works rebuilt", started);
        let checked = serving.cross_check().map_err(|e| fail(e.to_string()));
        phase(what, "cross-checked", started);
        checked
    })();
    match checked {
        Ok(c) => {
            println!(
                "  {} rows: {} promoted names and {} public ids resolve to their work; the graph and the works index agree on {} nodes; the lexical index holds the {} held works",
                c.rows,
                c.names,
                c.public,
                serving.works().nodes(),
                serving.works().manifest().held
            );
            eprintln!("{what}: {:.1} s", started.elapsed().as_secs_f64());
            ExitCode::SUCCESS
        }
        Err(code) => code,
    }
}

/// A partial date as text.
fn bound_text(b: model::date::Bound) -> String {
    use std::fmt::Write as _;
    let mut s = format!("{}", b.year.get());
    if let Some(month) = b.month {
        let _ = write!(s, "-{:02}", month as u8);
        if let Some(day) = b.day {
            let _ = write!(s, "-{:02}", day.get());
        }
    }
    s
}

/// Which work a query argument means: `#id`, or a name or public id
/// through the names lookup.
fn query_resolve(
    what: &str,
    serving: &serve::Serving,
    text: &str,
) -> Result<Option<WorkId>, ExitCode> {
    if let Some(digits) = text.strip_prefix('#') {
        return digits
            .parse::<u32>()
            .ok()
            .and_then(WorkId::new)
            .map(Some)
            .ok_or_else(|| {
                eprintln!("{what}: {text} is not an id");
                ExitCode::FAILURE
            });
    }
    match serving.resolve(text) {
        Ok(Some(serve::Resolved::One(id))) => Ok(Some(id)),
        Ok(Some(serve::Resolved::Shared(ids))) => {
            println!(
                "{text}: a shared name, accepted into {}",
                ids.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Ok(None)
        }
        Ok(Some(serve::Resolved::Split { id, into })) => {
            println!(
                "{text}: {id}, retired by a split; no one work answers for it. Split into {}",
                into.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            Ok(None)
        }
        Ok(None) => {
            eprintln!("{what}: {text}: no such work");
            Err(ExitCode::FAILURE)
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            Err(ExitCode::FAILURE)
        }
    }
}

fn print_row(
    out: &mut impl Write,
    serving: &serve::Serving,
    row: &serve::Row,
    limit: usize,
) -> io::Result<()> {
    writeln!(
        out,
        "{}{}{}",
        row.id,
        row.public
            .as_deref()
            .map_or(String::new(), |p| format!(" {p}")),
        if row.stub { " (stub)" } else { "" }
    )?;
    writeln!(
        out,
        "  names: {}",
        row.names
            .iter()
            .map(|n| format!("{} ({})", n.name, n.standing))
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    if !row.records.is_empty() {
        writeln!(
            out,
            "  records: {}",
            row.records
                .iter()
                .map(|r| format!(
                    "{}{}",
                    r.native,
                    if r.promoted { "" } else { " (not promoted)" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )?;
    }
    if let Some(text) = &row.text {
        if let Some(title) = &text.title {
            writeln!(out, "  title: {title}")?;
        }
        if !text.authors.is_empty() {
            writeln!(out, "  authors: {}", text.authors.join(", "))?;
        }
        if let Some(venue) = &text.venue {
            writeln!(out, "  venue: {venue}")?;
        }
    }
    print_facts(out, row)?;
    if let Some(text) = &row.text
        && let Some(a) = &text.abstract_text
    {
        let short: String = a.chars().take(300).collect();
        let more = if a.chars().count() > 300 { "…" } else { "" };
        writeln!(out, "  abstract: {short}{more}")?;
    }
    graph_list(
        out,
        "references",
        serving.references(row.id).count() as u64,
        serving.references(row.id),
        limit,
    )?;
    graph_list(
        out,
        "citers",
        serving.citers(row.id).count() as u64,
        serving.citers(row.id),
        limit,
    )
}

/// The promoted fields on one line, and which are disputed.
/// The promoted placement on a line of its own, and how its records
/// joined (merge version 17).
fn print_placement(out: &mut impl Write, f: &merge::build::OwnedFields) -> io::Result<()> {
    if let Some(p) = &f.placement {
        let mut parts: Vec<String> = p
            .levels
            .iter()
            .map(|l| match &l.value.label {
                Some(label) => format!("{} {label}", l.value.kind.as_str()),
                None => l.value.kind.as_str().to_owned(),
            })
            .collect();
        if let Some(w) = &p.within {
            parts.push(format!("pages {}", w.value));
        }
        if let Some(d) = &p.date {
            parts.push(format!("issue date {}", d.value.year.get()));
        }
        writeln!(
            out,
            "  placement: {} ({})",
            parts.join(", "),
            p.join.as_str()
        )?;
    }
    Ok(())
}

/// The placement's parts where another source differs (merge version 17).
fn placement_disputed(f: &merge::build::OwnedFields, disputed: &mut Vec<&'static str>) {
    if let Some(p) = &f.placement {
        if p.journal
            .as_ref()
            .is_some_and(merge::record::Field::disputed)
        {
            disputed.push("journal");
        }
        for level in &p.levels {
            if level.disputed() {
                disputed.push(level.value.kind.as_str());
            }
        }
        if p.within
            .as_ref()
            .is_some_and(merge::record::Field::disputed)
        {
            disputed.push("pages");
        }
        if p.date.as_ref().is_some_and(merge::record::Field::disputed) {
            disputed.push("issue date");
        }
    }
}

fn print_facts(out: &mut impl Write, row: &serve::Row) -> io::Result<()> {
    let f = &row.fields;
    let mut facts = Vec::new();
    if let Some(p) = &f.published {
        facts.push(format!(
            "published {} (from {})",
            bound_text(p.value),
            p.from
        ));
    }
    if let Some(l) = &f.language {
        facts.push(format!("language {}", l.value));
    }
    if let Some(x) = &f.form {
        facts.push(format!("form {}", x.value));
    }
    if let Some(x) = &f.genre {
        facts.push(format!("genre {}", x.value));
    }
    for (flag, by) in [
        ("retracted", f.status.retracted),
        ("corrected", f.status.corrected),
        ("concern", f.status.concern),
        ("duplicate", f.status.duplicate),
    ] {
        if !by.is_empty() {
            facts.push(format!("{flag} by {by}"));
        }
    }
    if let Some(a) = &f.access {
        facts.push(format!(
            "access {}",
            if a.value { "open" } else { "closed" }
        ));
    }
    if !facts.is_empty() {
        writeln!(out, "  {}", facts.join("; "))?;
    }
    print_placement(out, f)?;
    let mut disputed = Vec::new();
    if f.title.as_ref().is_some_and(merge::record::Field::disputed) {
        disputed.push("title");
    }
    if f.authors
        .as_ref()
        .is_some_and(merge::record::Field::disputed)
    {
        disputed.push("authors");
    }
    placement_disputed(f, &mut disputed);
    if f.online
        .as_ref()
        .is_some_and(merge::record::Field::disputed)
    {
        disputed.push("online");
    }
    if f.published
        .as_ref()
        .is_some_and(merge::record::Field::disputed)
    {
        disputed.push("published");
    }
    if f.language
        .as_ref()
        .is_some_and(merge::record::Field::disputed)
    {
        disputed.push("language");
    }
    if f.form.as_ref().is_some_and(merge::record::Field::disputed) {
        disputed.push("form");
    }
    if f.genre.as_ref().is_some_and(merge::record::Field::disputed) {
        disputed.push("genre");
    }
    if f.access
        .as_ref()
        .is_some_and(merge::record::Field::disputed)
    {
        disputed.push("access");
    }
    if !disputed.is_empty() {
        writeln!(out, "  disputed: {}", disputed.join(", "))?;
    }
    Ok(())
}

/// Prints one work through the query core.
fn query_work(opts: &Options) -> ExitCode {
    if !graph_options_ok(opts, 1, true) {
        return usage();
    }
    let what = "query work";
    let Some(text) = opts.paths[0].to_str() else {
        return usage();
    };
    let store = opts.store();
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let id = match query_resolve(what, &serving, text) {
        Ok(Some(id)) => id,
        Ok(None) => return ExitCode::SUCCESS,
        Err(code) => return code,
    };
    let row = match serving.work(id) {
        Ok(Some(row)) => row,
        Ok(None) => {
            eprintln!("{what}: {id}: no row; retired, or past the epoch");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let limit = opts
        .limit
        .map_or(10, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let mut out = io::BufWriter::new(io::stdout().lock());
    match print_row(&mut out, &serving, &row, limit).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Searches through the query core, printing each hit's public id.
fn query_search(opts: &Options) -> ExitCode {
    if !search_options_ok(opts, true) {
        return usage();
    }
    let what = "query search";
    let store = opts.store();
    let words: Vec<&str> = opts.paths.iter().filter_map(|p| p.to_str()).collect();
    let text = words.join(" ");
    let limit = opts
        .limit
        .map_or(10, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let asked = match query_parser::parse(&text) {
        Ok(asked) => asked,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let started = std::time::Instant::now();
    let hits = match serving.search(&asked, limit) {
        Ok(hits) => hits,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed();
    let mut out = io::BufWriter::new(io::stdout().lock());
    let printed = (|| -> io::Result<()> {
        writeln!(
            out,
            "{text:?}: {} hits in {:.1} ms (epoch {})",
            hits.total,
            elapsed.as_secs_f64() * 1000.0,
            serving.epoch().epoch
        )?;
        for hit in &hits.hits {
            let public = serving
                .work(hit.id)
                .ok()
                .flatten()
                .and_then(|r| r.public)
                .unwrap_or_default();
            writeln!(
                out,
                "  {:>6.2}  {}  {public}  {}",
                hit.score,
                hit.id,
                hit.title.as_deref().unwrap_or("(no title)")
            )?;
        }
        for (root, values) in &hits.facets {
            if values.is_empty() {
                continue;
            }
            writeln!(
                out,
                "  {root}: {}",
                values
                    .iter()
                    .map(|(v, n)| format!("{v} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        out.flush()
    })();
    match printed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// How deep a run goes unless `--limit` says (evaluation plan §3.3).
const RUN_DEPTH: usize = 1000;

/// How many times its depth a restricted run asks the search for, since
/// the date cutoff is applied after the search (evaluation plan §3.2).
const RESTRICTED_ASK: usize = 2;

/// One query's part of a run.
struct Answer {
    /// The works kept, with their scores.
    works: Vec<evaluation::trec::Scored>,
    /// How long the search took.
    took: std::time::Duration,
    /// Whether the date cutoff left the query short of the depth though
    /// the search had more to give.
    short: bool,
}

type Searched = Result<Answer, String>;

/// What a run searches: a suite's queries and filters, or a queries
/// file's queries and no filters.
struct Asked {
    queries: Vec<(String, String)>,
    filters: BTreeMap<String, evaluation::suite::Filter>,
    /// Of the queries file, and of the filters when there are any.
    digest: String,
}

/// Reads what a run searches from a suite's directory or a queries file.
fn evaluate_asked(path: &Path) -> Result<Asked, String> {
    if path.is_dir() {
        let suite = evaluation::suite::Suite::read(path).map_err(|e| e.to_string())?;
        let mut hasher = blake3::Hasher::new();
        for name in [evaluation::suite::QUERIES, evaluation::suite::FILTERS] {
            let bytes = std::fs::read(path.join(name))
                .map_err(|e| format!("{}: {e}", path.join(name).display()))?;
            hasher.update(&bytes);
        }
        return Ok(Asked {
            queries: suite.queries,
            filters: suite.filters,
            digest: hasher.finalize().to_hex().to_string(),
        });
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let queries =
        evaluation::trec::read_queries(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Asked {
        queries,
        filters: BTreeMap::new(),
        digest: blake3::hash(text.as_bytes()).to_hex().to_string(),
    })
}

/// `evaluate run <suite | queries> --out <run> [--restricted]`: every
/// query through the store's search, in process — the parser and the
/// translation the search box uses, with no time limit and no page cap —
/// written as a TREC run, and beside it `<run>.about` naming what was
/// searched (evaluation plan §6). A suite's filters leave a query's own
/// seed out of its results, and with `--restricted` every work published
/// after the suite's year (§3.2). The queries are searched in parallel and
/// written in the file's order, so the run never depends on which search
/// finished first. A run is kept: an existing `--out` is refused.
// Read, search, stamp, write: one step after another, and long together.
#[allow(clippy::too_many_lines)]
fn evaluate_run(opts: &Options) -> ExitCode {
    let what = "evaluate run";
    let ([asked_path], Some(out)) = (opts.paths.as_slice(), opts.out.as_ref()) else {
        return usage();
    };
    if opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
        || opts.survey
        || opts.limit == Some(0)
    {
        return usage();
    }
    let depth = opts
        .limit
        .map_or(RUN_DEPTH, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let about_path = PathBuf::from(format!("{}.about", out.display()));
    for path in [out, &about_path] {
        if path.exists() {
            eprintln!(
                "{what}: {} exists; a run is kept, so name a new one",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    }
    let asked = match evaluate_asked(asked_path) {
        Ok(asked) => asked,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if opts.restricted && !asked.filters.values().any(|f| f.until.is_some()) {
        eprintln!(
            "{what}: {} names no year for any query; --restricted needs a suite that does",
            asked_path.display()
        );
        return ExitCode::FAILURE;
    }
    let mut jobs = Vec::with_capacity(asked.queries.len());
    for (id, query) in &asked.queries {
        match query_parser::parse(query) {
            Ok(parsed) => jobs.push((parsed, asked.filters.get(id).cloned())),
            Err(e) => {
                eprintln!("{what}: query {id}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let store = opts.store();
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let started = std::time::Instant::now();
    let searched = evaluate_search(&serving, &jobs, depth, opts.restricted, opts.workers());
    let mut run = evaluation::trec::Run::default();
    let mut slowest: Option<(std::time::Duration, &str)> = None;
    let (mut returned, mut empty, mut short) = (0_usize, 0_usize, 0_usize);
    for ((id, _), result) in asked.queries.iter().zip(searched) {
        let mut answer = match result {
            Ok(answer) => answer,
            Err(e) => {
                eprintln!("{what}: query {id}: {e}");
                return ExitCode::FAILURE;
            }
        };
        if slowest.is_none_or(|(t, _)| answer.took > t) {
            slowest = Some((answer.took, id));
        }
        returned += answer.works.len();
        empty += usize::from(answer.works.is_empty());
        short += usize::from(answer.short);
        evaluation::trec::order(&mut answer.works);
        run.queries.insert(id.clone(), answer.works);
    }
    let elapsed = started.elapsed();

    let epoch = serving.epoch();
    let index = serving.search_index();
    let absorbed = match index.batch() {
        Ok(batch) => batch,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let about = serde_json::json!({
        "asked": asked_path.display().to_string(),
        "asked_blake3": asked.digest,
        "queries_count": asked.queries.len(),
        "depth": depth,
        "restricted": opts.restricted,
        "short_of_depth": short,
        "epoch": epoch,
        "search": index.manifest(),
        "search_absorbed_batch": absorbed,
        "ol_rht": env!("CARGO_PKG_VERSION"),
    });
    let tag = format!("ol-rht-epoch{}", epoch.epoch);
    if let Err(e) = evaluate_write(out, &about_path, &run, &tag, &about) {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    println!(
        "{what}: {} queries, {returned} works returned, {empty} queries with none, \
         depth {depth}{}, epoch {}, {:.1} s",
        asked.queries.len(),
        if opts.restricted {
            format!(", restricted by year ({short} queries short of the depth)")
        } else {
            String::new()
        },
        epoch.epoch,
        elapsed.as_secs_f64()
    );
    if let Some((took, id)) = slowest {
        println!(
            "  slowest: query {id}, {:.1} ms",
            took.as_secs_f64() * 1000.0
        );
    }
    println!("  run: {}", out.display());
    ExitCode::SUCCESS
}

/// Searches every query on `workers` threads, taking the next query as
/// each finishes one, and returns the answers in the queries' order.
fn evaluate_search(
    serving: &serve::Serving,
    jobs: &[(query_parser::Query, Option<evaluation::suite::Filter>)],
    depth: usize,
    restricted: bool,
    workers: usize,
) -> Vec<Searched> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut answered: Vec<(usize, Searched)> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..workers.max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((query, filter)) = jobs.get(i) else {
                            break mine;
                        };
                        mine.push((
                            i,
                            evaluate_one(serving, query, filter.as_ref(), depth, restricted),
                        ));
                    }
                })
            })
            .collect();
        threads
            .into_iter()
            .flat_map(|t| t.join().expect("a search thread panicked"))
            .collect()
    });
    answered.sort_unstable_by_key(|(i, _)| *i);
    answered.into_iter().map(|(_, a)| a).collect()
}

/// One query's works, each named by its public id: its own seed left out,
/// and in a restricted run every work published after its year, or of no
/// known year, left out too.
fn evaluate_one(
    serving: &serve::Serving,
    query: &query_parser::Query,
    filter: Option<&evaluation::suite::Filter>,
    depth: usize,
    restricted: bool,
) -> Searched {
    let exclude = filter.and_then(|f| f.exclude.as_deref());
    let until = filter.and_then(|f| f.until).filter(|_| restricted);
    let ask = if until.is_some() {
        depth.saturating_mul(RESTRICTED_ASK)
    } else {
        depth
    }
    .saturating_add(usize::from(exclude.is_some()));
    let started = std::time::Instant::now();
    let hits = serving
        .search_facets(query, ask, 0, None)
        .map_err(|e| e.to_string())?;
    let took = started.elapsed();
    let rows = match until {
        Some(_) => {
            let ids: Vec<WorkId> = hits.hits.iter().map(|h| h.id).collect();
            serving.rows(&ids).map_err(|e| e.to_string())?
        }
        None => HashMap::new(),
    };
    let mut works = Vec::with_capacity(depth.min(hits.hits.len()));
    for hit in &hits.hits {
        if works.len() == depth {
            break;
        }
        let Some(public) = serving.public_of(hit.id) else {
            return Err(format!(
                "work {} has no public id; a run names works by public id only",
                hit.id
            ));
        };
        if exclude == Some(public.as_str()) {
            continue;
        }
        if let Some(until) = until {
            let year = rows
                .get(&hit.id)
                .and_then(|row| row.fields.published.as_ref())
                .map(|p| p.value.year.get());
            if year.is_none_or(|y| y > until) {
                continue;
            }
        }
        works.push(evaluation::trec::Scored {
            work: public,
            score: f64::from(hit.score),
        });
    }
    let short = until.is_some() && works.len() < depth && hits.hits.len() == ask;
    Ok(Answer { works, took, short })
}

/// Writes the run and its sidecar through `.partial` names, renamed only
/// once both are whole, so a failed run leaves nothing behind.
fn evaluate_write(
    out: &Path,
    about_path: &Path,
    run: &evaluation::trec::Run,
    tag: &str,
    about: &serde_json::Value,
) -> io::Result<()> {
    let run_partial = PathBuf::from(format!("{}.partial", out.display()));
    let about_partial = PathBuf::from(format!("{}.partial", about_path.display()));
    let written = (|| -> io::Result<()> {
        let mut file = io::BufWriter::new(std::fs::File::create(&run_partial)?);
        run.write(tag, &mut file)?;
        file.into_inner()
            .map_err(io::IntoInnerError::into_error)?
            .sync_all()?;
        let mut text = serde_json::to_string_pretty(about).map_err(io::Error::other)?;
        text.push('\n');
        std::fs::write(&about_partial, text)?;
        std::fs::rename(&run_partial, out)?;
        std::fs::rename(&about_partial, about_path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&run_partial);
        let _ = std::fs::remove_file(&about_partial);
    }
    written
}

/// What a set's loader makes: the suite before its halves are drawn, what
/// the corpus holds of its key, the queries left out and why, and what
/// `about.json` says of how it was read.
struct Loaded {
    suite: evaluation::suite::Suite,
    coverage: evaluation::sets::Coverage,
    left_out: Vec<(String, &'static str)>,
    details: serde_json::Value,
}

/// What the store holds of a PMID, and the work when one work has it.
fn evaluate_find(
    serving: &serve::Serving,
    pmid: &str,
) -> Result<(evaluation::sets::Found, Option<WorkId>), String> {
    use evaluation::sets::Found;
    match serving
        .resolve(&format!("pmid:{pmid}"))
        .map_err(|e| e.to_string())?
    {
        None => Ok((Found::Missing, None)),
        Some(serve::Resolved::One(id)) => {
            let public = serving
                .public_of(id)
                .ok_or_else(|| format!("work {id} has no public id"))?;
            if serving.is_stub(id) {
                Ok((Found::Stub(public), Some(id)))
            } else {
                Ok((Found::Held(public), Some(id)))
            }
        }
        Some(serve::Resolved::Shared(_) | serve::Resolved::Split { .. }) => {
            Ok((Found::Ambiguous, None))
        }
    }
}

/// The year TREC Precision Medicine's judges saw PubMed up to: 2019 and
/// 2020 both searched the MEDLINE 2019 baseline, a snapshot of mid-December
/// 2018 (benchmarks note §2.4). Other years searched another snapshot,
/// with conference abstracts beside it, not yet read.
fn trec_pm_until(year: u16) -> Result<u16, String> {
    match year {
        2019 | 2020 => Ok(2018),
        other => Err(format!(
            "TREC Precision Medicine {other}: its snapshot year is not written down yet"
        )),
    }
}

/// TREC Precision Medicine, scientific abstracts: `<topics> <qrels>`.
fn evaluate_load_trec_pm(
    serving: &serve::Serving,
    files: &[(PathBuf, Vec<u8>)],
) -> Result<Loaded, String> {
    let [(topics_path, topics), (qrels_path, qrels)] = files else {
        return Err("trec-pm takes its topics file and its trec_eval qrels".to_owned());
    };
    let text = |path: &Path, bytes: &[u8]| {
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|e| format!("{}: {e}", path.display()))
    };
    let topics = evaluation::sets::read_pm_topics(&text(topics_path, topics)?)
        .map_err(|e| format!("{}: {e}", topics_path.display()))?;
    let judged = evaluation::trec::Key::read(&text(qrels_path, qrels)?)
        .map_err(|e| format!("{}: {e}", qrels_path.display()))?;
    if let Some(stray) = judged
        .queries
        .keys()
        .find(|q| !topics.topics.iter().any(|t| &&t.number == q))
    {
        return Err(format!(
            "{}: topic {stray} is not in the topics file",
            qrels_path.display()
        ));
    }
    let until = trec_pm_until(topics.year)?;
    let judgments = judged.queries.iter().flat_map(|(q, works)| {
        works
            .iter()
            .map(move |(pmid, grade)| (q.as_str(), pmid.as_str(), *grade))
    });
    let (key, coverage) = evaluation::sets::key_of(judgments, |pmid| {
        evaluate_find(serving, pmid).map(|(f, _)| f)
    })
    .map_err(|e| e.to_string())?;
    let mut suite = evaluation::suite::Suite::default();
    for topic in &topics.topics {
        suite.queries.push((topic.number.clone(), topic.query()));
        suite.filters.insert(
            topic.number.clone(),
            evaluation::suite::Filter {
                until: Some(until),
                exclude: None,
            },
        );
    }
    suite.key = key;
    let left_out = topics
        .topics
        .iter()
        .filter(|t| !suite.key.queries.contains_key(&t.number))
        .map(|t| (t.number.clone(), "the key judges nothing for it"))
        .collect();
    Ok(Loaded {
        suite,
        coverage,
        left_out,
        details: serde_json::json!({
            "year": topics.year,
            "topics": topics.topics.len(),
            "query": "the disease and the gene, and the treatment where the topic names one (evaluation plan §12, item 15)",
            "until": until,
        }),
    })
}

/// RELISH v1: `<RELISH_v1.json.gz>`.
fn evaluate_load_relish(
    serving: &serve::Serving,
    files: &[(PathBuf, Vec<u8>)],
) -> Result<Loaded, String> {
    use evaluation::sets::Found;
    /// RELISH's candidates came from PubMed as of 2018-07-03.
    const UNTIL: u16 = 2018;
    let [(path, bytes)] = files else {
        return Err("relish takes RELISH_v1.json.gz".to_owned());
    };
    let records =
        evaluation::sets::read_relish_gz(bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let annotations = records.len();
    let (kept, dropped) = evaluation::sets::first_per_seed(records);
    let mut suite = evaluation::suite::Suite::default();
    let mut left_out = Vec::new();
    let mut judgments = Vec::new();
    for record in &kept {
        let seed = record.pmid.as_str();
        let (found, id) = evaluate_find(serving, seed)?;
        let (public, id) = match (found, id) {
            (Found::Held(public), Some(id)) => (public, id),
            (Found::Stub(_), _) => {
                left_out.push((seed.to_owned(), "the seed is known only as cited"));
                continue;
            }
            (Found::Missing, _) => {
                left_out.push((seed.to_owned(), "the seed is not in the corpus"));
                continue;
            }
            _ => {
                left_out.push((seed.to_owned(), "the seed's PMID names more than one work"));
                continue;
            }
        };
        let title = serving
            .work(id)
            .map_err(|e| e.to_string())?
            .and_then(|row| row.text)
            .and_then(|text| text.title)
            .map(|t| t.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|t| !t.is_empty());
        let Some(title) = title else {
            left_out.push((seed.to_owned(), "the seed has no title"));
            continue;
        };
        suite.queries.push((seed.to_owned(), title));
        suite.filters.insert(
            seed.to_owned(),
            evaluation::suite::Filter {
                until: Some(UNTIL),
                exclude: Some(public),
            },
        );
        judgments.extend(
            record
                .judgments()
                .into_iter()
                .map(|(pmid, grade)| (seed, pmid, grade)),
        );
    }
    let (key, coverage) = evaluation::sets::key_of(judgments, |pmid| {
        evaluate_find(serving, pmid).map(|(f, _)| f)
    })
    .map_err(|e| e.to_string())?;
    suite.key = key;
    Ok(Loaded {
        suite,
        coverage,
        left_out,
        details: serde_json::json!({
            "annotations": annotations,
            "seeds": kept.len(),
            "later_annotations_of_a_seed_left_out": dropped,
            "query": "the seed paper's title as the works index holds it (evaluation plan §12, item 15)",
            "until": UNTIL,
        }),
    })
}

/// `evaluate load <trec-pm | relish> <file>... --out <suite>`: a published
/// set's own files read into a suite (evaluation plan §6) — its queries,
/// its key with every PMID found through the store's names, the filters,
/// the halves when it is large enough to halve — and beside it
/// `about.json`: the files' digests, the epoch, what the corpus holds of
/// the key and which queries were left out and why.
// Read, find, write, report: one step after another, and long together.
#[allow(clippy::too_many_lines)]
fn evaluate_load(opts: &Options) -> ExitCode {
    let what = "evaluate load";
    let (Some((set, files)), Some(out)) = (opts.paths.split_first(), opts.out.as_ref()) else {
        return usage();
    };
    if opts.limit.is_some()
        || opts.workers.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
        || opts.survey
    {
        return usage();
    }
    if out.exists() {
        eprintln!(
            "{what}: {} exists; a suite is kept, so name a new version",
            out.display()
        );
        return ExitCode::FAILURE;
    }
    let mut read = Vec::with_capacity(files.len());
    for path in files {
        match std::fs::read(path) {
            Ok(bytes) => read.push((path.clone(), bytes)),
            Err(e) => {
                eprintln!("{what}: {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    let store = opts.store();
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let loaded = match set.to_str() {
        Some("trec-pm") => evaluate_load_trec_pm(&serving, &read),
        Some("relish") => evaluate_load_relish(&serving, &read),
        _ => return usage(),
    };
    let Loaded {
        mut suite,
        coverage,
        left_out,
        details,
    } = match loaded {
        Ok(loaded) => loaded,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let scored = suite.scored();
    suite.halves = if opts.held_out {
        evaluation::suite::held_out(&scored)
    } else {
        evaluation::suite::halves(&scored)
    };
    let working = suite
        .halves
        .values()
        .filter(|h| **h == evaluation::suite::Half::Working)
        .count();
    let coverage_json: serde_json::Map<String, serde_json::Value> = coverage
        .by_grade
        .iter()
        .map(|(grade, [held, stub, missing, ambiguous])| {
            (
                grade.to_string(),
                serde_json::json!({
                    "held": held, "stub": stub, "missing": missing, "ambiguous": ambiguous,
                }),
            )
        })
        .collect();
    let about = serde_json::json!({
        "set": set.display().to_string(),
        "files": read.iter().map(|(path, bytes)| serde_json::json!({
            "path": path.display().to_string(),
            "bytes": bytes.len(),
            "blake3": blake3::hash(bytes).to_hex().to_string(),
        })).collect::<Vec<_>>(),
        "epoch": serving.epoch(),
        "details": details,
        "queries": suite.queries.len(),
        "scored": scored.len(),
        "halves": if suite.is_held_out() {
            serde_json::json!("held out whole: its total is read at milestones")
        } else if suite.halves.is_empty() {
            serde_json::json!("whole: too few queries to halve")
        } else {
            serde_json::json!({ "working": working, "test": suite.halves.len() - working })
        },
        "left_out": left_out.iter().map(|(q, why)| serde_json::json!({ "query": q, "why": why })).collect::<Vec<_>>(),
        "coverage_by_grade": coverage_json,
        "collided": coverage.collided,
        "ol_rht": env!("CARGO_PKG_VERSION"),
    });
    let written = suite.write(out).and_then(|()| {
        let mut text = serde_json::to_string_pretty(&about).map_err(io::Error::other)?;
        text.push('\n');
        std::fs::write(out.join("about.json"), text)
    });
    if let Err(e) = written {
        eprintln!("{what}: {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    println!(
        "{what}: {} queries, {} with a relevant work, {} left out; {}",
        suite.queries.len(),
        scored.len(),
        left_out.len(),
        if suite.is_held_out() {
            "held out whole".to_owned()
        } else if suite.halves.is_empty() {
            "kept whole, too few to halve".to_owned()
        } else {
            format!("halved, {working} working")
        }
    );
    println!("  judged works by grade: held (searchable), stub (cited only), missing, ambiguous");
    for (grade, [held, stub, missing, ambiguous]) in &coverage.by_grade {
        println!(
            "    grade {grade}: {held} held, {stub} stub, {missing} missing, {ambiguous} ambiguous"
        );
    }
    if coverage.collided > 0 {
        println!(
            "  {} judgments landed on a work judged already",
            coverage.collided
        );
    }
    println!("  suite: {}", out.display());
    ExitCode::SUCCESS
}

/// The half of a suite `score` and `compare` read.
fn evaluate_half(opts: &Options) -> evaluation::suite::Half {
    if opts.test {
        evaluation::suite::Half::Test
    } else {
        evaluation::suite::Half::Working
    }
}

/// The options `evaluate score` and `evaluate compare` do not take: they
/// read files and nothing else.
fn evaluate_reads_only(opts: &Options) -> bool {
    opts.limit.is_none()
        && opts.workers.is_none()
        && opts.out.is_none()
        && opts.store.is_none()
        && opts.mailto.is_none()
        && opts.seed.is_none()
        && opts.by.is_none()
        && opts.related.is_none()
        && opts.walk.is_none()
        && opts.depth.is_none()
        && !opts.survey
}

/// What `score` and `compare` read their answers from: a suite, or a
/// bare answer key.
enum Judged {
    Suite(evaluation::suite::Suite),
    Key(evaluation::trec::Key),
}

/// Reads a suite's directory or an answer key, and runs, naming the file
/// that fails.
fn evaluate_read(
    judged: &Path,
    runs: &[PathBuf],
) -> Result<(Judged, Vec<evaluation::trec::Run>), String> {
    let read =
        |path: &Path| std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()));
    let judged = if judged.is_dir() {
        Judged::Suite(evaluation::suite::Suite::read(judged).map_err(|e| e.to_string())?)
    } else {
        Judged::Key(
            evaluation::trec::Key::read(&read(judged)?)
                .map_err(|e| format!("{}: {e}", judged.display()))?,
        )
    };
    let runs = runs
        .iter()
        .map(|path| {
            evaluation::trec::Run::read(&read(path)?)
                .map_err(|e| format!("{}: {e}", path.display()))
        })
        .collect::<Result<_, _>>()?;
    Ok((judged, runs))
}

/// `evaluate score <suite | key> <run> [--test]`: the run's report
/// (evaluation plan §4) — of a suite, its working half, or with `--test`
/// its test half at a milestone, open and condensed.
fn evaluate_score(opts: &Options) -> ExitCode {
    let what = "evaluate score";
    let [judged, run] = opts.paths.as_slice() else {
        return usage();
    };
    if !evaluate_reads_only(opts) {
        return usage();
    }
    let (judged, runs) = match evaluate_read(judged, std::slice::from_ref(run)) {
        Ok(read) => read,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut out = io::BufWriter::new(io::stdout().lock());
    let written = match &judged {
        Judged::Suite(suite) => {
            evaluation::report::score_suite(suite, &runs[0], evaluate_half(opts), &mut out)
        }
        Judged::Key(_) if opts.test => return usage(),
        Judged::Key(key) => evaluation::report::score(key, &runs[0], &mut out),
    };
    match written.and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `evaluate compare <suite | key> <base run> <new run> [--test]`: both
/// runs query by query, the interval of every difference, and the
/// acceptance rule's verdict (evaluation plan §4) — exit 1 when the new
/// run is refused, and when a level has too few queries for the rule to
/// speak. Of a suite, the working half, or with `--test` the test half.
fn evaluate_compare(opts: &Options) -> ExitCode {
    let what = "evaluate compare";
    let [judged, base, new] = opts.paths.as_slice() else {
        return usage();
    };
    if !evaluate_reads_only(opts) {
        return usage();
    }
    let (judged, runs) = match evaluate_read(judged, &[base.clone(), new.clone()]) {
        Ok(read) => read,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut out = io::BufWriter::new(io::stdout().lock());
    let verdict = match &judged {
        Judged::Suite(suite) => evaluation::report::compare_suite(
            suite,
            &runs[0],
            &runs[1],
            evaluate_half(opts),
            &mut out,
        ),
        Judged::Key(_) if opts.test => return usage(),
        Judged::Key(key) => evaluation::report::compare(key, &runs[0], &runs[1], &mut out),
    }
    .and_then(|verdict| out.flush().map(|()| verdict));
    match verdict {
        Ok(evaluation::report::Verdict::Passes) => ExitCode::SUCCESS,
        Ok(evaluation::report::Verdict::Refused | evaluation::report::Verdict::TooFewQueries) => {
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The titles of a whole listing, read in one walk of the works index
/// rather than one lookup a row (serving plan §11, item 31) — the same
/// call the transport hydrates a page with.
fn query_titles(serving: &serve::Serving, ids: &[WorkId]) -> HashMap<WorkId, String> {
    let rows = serving.rows(ids).unwrap_or_default();
    ids.iter()
        .map(|id| {
            let text = match rows.get(id) {
                None => "(no row)",
                Some(row) => row
                    .text
                    .as_ref()
                    .and_then(|t| t.title.as_deref())
                    .unwrap_or(if row.stub { "(stub)" } else { "(no title)" }),
            };
            (*id, text.to_owned())
        })
        .collect()
}

/// A work's title for a listing, or what it is instead.
fn query_title(serving: &serve::Serving, id: WorkId) -> String {
    let row = serving.work(id).ok().flatten();
    row.as_ref()
        .and_then(|r| r.text.as_ref())
        .and_then(|t| t.title.as_deref())
        .unwrap_or_else(|| {
            if row.as_ref().is_some_and(|r| r.stub) {
                "(stub)"
            } else {
                "(no title)"
            }
        })
        .to_owned()
}

/// One hop out from a seed set along `--walk`, best-scored first. The
/// raw overlap and the degree it is out of are printed beside the score,
/// since those are the numbers a reader can weigh.
fn query_expand(opts: &Options) -> ExitCode {
    if opts.paths.is_empty() || !query_options_ok(opts, opts.paths.len(), true, false, false) {
        return usage();
    }
    let what = "query expand";
    let store = opts.store();
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut seeds = Vec::new();
    for path in &opts.paths {
        let Some(text) = path.to_str() else {
            return usage();
        };
        match query_resolve(what, &serving, text) {
            Ok(Some(id)) => seeds.push(id),
            Ok(None) => {}
            Err(code) => return code,
        }
    }
    let budget = opts
        .limit
        .map_or(20, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let walk = opts.walk.unwrap_or_default();
    let started = std::time::Instant::now();
    let found = serving.expand(&seeds, walk, budget, None);
    let elapsed = started.elapsed();
    let mut out = io::BufWriter::new(io::stdout().lock());
    let printed = (|| -> io::Result<()> {
        writeln!(
            out,
            "one hop from {} seeds along {walk:?}: {} of {} reached, in {:.1} ms",
            found.seeds_used,
            found.neighbours.len(),
            found.total,
            elapsed.as_secs_f64() * 1000.0
        )?;
        print_neighbours(&mut out, &serving, &found.neighbours)?;
        out.flush()
    })();
    match printed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// One neighbour a line: the score, the raw overlap it is out of, and
/// the title, the whole listing's rows read in one call.
fn print_neighbours(
    out: &mut impl Write,
    serving: &serve::Serving,
    found: &[serve::Neighbour],
) -> io::Result<()> {
    let ids: Vec<WorkId> = found.iter().map(|n| n.work).collect();
    let titles = query_titles(serving, &ids);
    for n in found {
        writeln!(
            out,
            "  {:.4}  {:>3} of {:<6} {}  {}",
            n.score,
            n.touches,
            n.degree,
            n.work,
            titles.get(&n.work).map_or("(no row)", String::as_str)
        )?;
    }
    Ok(())
}

/// The two questions a reader asks of one work's neighbourhood, each in
/// one call, with the seeds gathered here and not handed in: what else
/// cites what this cites, and what else is cited by what cites this. The
/// seeding is capped, and the line says what it used of what there was.
fn query_related(opts: &Options) -> ExitCode {
    if !query_options_ok(opts, 1, false, false, true) {
        return usage();
    }
    let what = "query related";
    let store = opts.store();
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(text) = opts.paths[0].to_str() else {
        return usage();
    };
    let id = match query_resolve(what, &serving, text) {
        Ok(Some(id)) => id,
        Ok(None) => return ExitCode::FAILURE,
        Err(code) => return code,
    };
    let Some(by) = opts.related else {
        return usage();
    };
    let budget = opts
        .limit
        .map_or(20, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let started = std::time::Instant::now();
    let found = serving.related(id, by, budget);
    let elapsed = started.elapsed();
    let mut out = io::BufWriter::new(io::stdout().lock());
    let printed = (|| -> io::Result<()> {
        writeln!(
            out,
            "{id} {by:?}: {} of {} reached, from {} seeds of {}, in {:.1} ms",
            found.neighbours.len(),
            found.total,
            found.seeds_used,
            found.seeds_total,
            elapsed.as_secs_f64() * 1000.0
        )?;
        print_neighbours(&mut out, &serving, &found.neighbours)?;
        out.flush()
    })();
    match printed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The shortest directed paths between two works: "does the first cite
/// something that cites … the second". No path within the bound is the
/// ordinary answer on a corpus this sparse (graph plan §7), and it is
/// printed as one.
fn query_path(opts: &Options) -> ExitCode {
    if !query_options_ok(opts, 2, false, true, false) {
        return usage();
    }
    let what = "query path";
    let store = opts.store();
    let serving = match serve::Serving::open(&store) {
        Ok(serving) => serving,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut ends = Vec::new();
    for path in &opts.paths {
        let Some(text) = path.to_str() else {
            return usage();
        };
        match query_resolve(what, &serving, text) {
            Ok(Some(id)) => ends.push(id),
            Ok(None) => return ExitCode::FAILURE,
            Err(code) => return code,
        }
    }
    let bounds = graph::Bounds {
        depth: opts.depth.unwrap_or(graph::Bounds::DEFAULT.depth),
        paths: opts.limit.map_or(graph::Bounds::DEFAULT.paths, |n| {
            usize::try_from(n).unwrap_or(usize::MAX)
        }),
        visits: graph::Bounds::DEFAULT.visits,
    };
    let started = std::time::Instant::now();
    let found = serving.path(ends[0], ends[1], bounds);
    let elapsed = started.elapsed();
    let mut out = io::BufWriter::new(io::stdout().lock());
    let printed = (|| -> io::Result<()> {
        let ms = elapsed.as_secs_f64() * 1000.0;
        match &found {
            graph::Found::Paths {
                hops,
                paths,
                meetings,
                reached,
            } => {
                writeln!(
                    out,
                    "{} -> {}: {hops} hops, {meetings} shortest, {} shown; {} works forward, {} backward, {} expanded, in {ms:.1} ms",
                    ends[0],
                    ends[1],
                    paths.len(),
                    reached.forward,
                    reached.backward,
                    reached.expanded
                )?;
                for path in paths {
                    writeln!(out, "  --")?;
                    for step in path {
                        writeln!(
                            out,
                            "  {}  {}  {}",
                            step.work,
                            if step.by.is_empty() {
                                "(the start)".to_owned()
                            } else {
                                step.by.to_string()
                            },
                            query_title(&serving, step.work)
                        )?;
                    }
                }
            }
            graph::Found::None { depth, reached } => writeln!(
                out,
                "{} -> {}: no path within {depth} hops, searched to that bound; {} works forward, {} backward, {} expanded, in {ms:.1} ms",
                ends[0], ends[1], reached.forward, reached.backward, reached.expanded
            )?,
            graph::Found::Stopped {
                depth,
                proved,
                visits,
                reached,
            } => writeln!(
                out,
                "{} -> {}: no path within {proved} hops; {depth} was asked and the budget of {visits} works ran out after {} forward and {} backward, in {ms:.1} ms",
                ends[0], ends[1], reached.forward, reached.backward
            )?,
        }
        out.flush()
    })();
    match printed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Assembles every held work's document from the merged and normalized
/// sets, indexes them, and prints the report.
fn search_build(opts: &Options) -> ExitCode {
    if !search_options_ok(opts, false) {
        return usage();
    }
    let what = "search build";
    let store = opts.store();
    let started = std::time::Instant::now();
    let allocations = ingest::alloc::allocations();
    let mut assembly = match assemble(what, &store, started) {
        Ok(assembled) => assembled,
        Err(code) => return code,
    };
    eprintln!(
        "{what}: {} documents assembled in {:.1} s",
        assembly.stats().documents,
        started.elapsed().as_secs_f64()
    );
    let batch = match current_batch(what, &store) {
        Ok(line) => line,
        Err(code) => return code,
    };
    let dir = search::dir(&store);
    let built = (|| -> Result<search::index::Built, String> {
        let mut build = search::index::Build::start(&dir).map_err(|e| e.to_string())?;
        merge::document::Ranged::new(&mut assembly, |docs| {
            build
                .add(docs)
                .map_err(|e| merge::document::Error::Consumer(e.to_string()))
        })
        .finish()
        .map_err(|e| e.to_string())?;
        build.finish(batch.batch).map_err(|e| e.to_string())
    })();
    let built = match built {
        Ok(built) => built,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let stats = assembly.stats().clone();
    eprintln!(
        "{what}: {} documents indexed in {:.1} s",
        built.manifest.documents,
        started.elapsed().as_secs_f64()
    );
    let reported = search::Opened::open(&built.dir).and_then(|opened| {
        let mut out = io::stdout().lock();
        search::report::print(&mut out, &opened, &stats)
    });
    if let Err(e) = reported {
        eprintln!("{what}: {e}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "{what}: {:.1} s, {} allocations",
        started.elapsed().as_secs_f64(),
        ingest::alloc::allocations() - allocations
    );
    ExitCode::SUCCESS
}

/// Prints the top hits for a query and the facet counts over its matches.
fn search_query(opts: &Options) -> ExitCode {
    if !search_options_ok(opts, true) {
        return usage();
    }
    let what = "search query";
    let store = opts.store();
    let words: Vec<&str> = opts.paths.iter().filter_map(|p| p.to_str()).collect();
    let text = words.join(" ");
    let limit = opts
        .limit
        .map_or(10, |n| usize::try_from(n).unwrap_or(usize::MAX));
    let opened = match search::Opened::open(&search::dir(&store)) {
        Ok(opened) => opened,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let asked = match query_parser::parse(&text) {
        Ok(asked) => asked,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let started = std::time::Instant::now();
    let hits = match opened.search(&asked, limit, 5, None) {
        Ok(hits) => hits,
        Err(e) => {
            eprintln!("{what}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let elapsed = started.elapsed();
    let mut out = io::BufWriter::new(io::stdout().lock());
    let printed = (|| -> io::Result<()> {
        writeln!(
            out,
            "{text:?}: {} hits in {:.1} ms",
            hits.total,
            elapsed.as_secs_f64() * 1000.0
        )?;
        for hit in &hits.hits {
            writeln!(
                out,
                "  {:>6.2}  {}  {}",
                hit.score,
                hit.id,
                hit.title.as_deref().unwrap_or("(no title)")
            )?;
        }
        for (root, values) in &hits.facets {
            if values.is_empty() {
                continue;
            }
            writeln!(
                out,
                "  {root}: {}",
                values
                    .iter()
                    .map(|(v, n)| format!("{v} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        out.flush()
    })();
    match printed {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Reads one source's deletions sets, then every copy of its records, into
/// the copy rule (merge plan §8): before the identity pass, which asks it
/// about each copy.
fn merge_copies<S: Contributor>(
    copies: &mut merge::copies::Copies,
    sets: &Sets,
    limit: Option<usize>,
    names: &mut merge::dictionary::Sorter,
) -> Result<(), merge::identity::Error> {
    let mut withdrawn = 0u64;
    for (path, set) in &sets.deletions {
        withdrawn += copies.read_deletions::<S>(path, set)?;
    }
    if withdrawn > 0 {
        eprintln!(
            "merge: {}: {withdrawn} withdrawals in deletions sets",
            S::NAME
        );
    }
    for (path, set) in &sets.works {
        copies.read_works::<S>(path, set, limit, Some(&mut *names))?;
    }
    Ok(())
}

/// Reads every works set of one source into the identity pass, each copy
/// that does not speak for its record left out.
fn merge_add<S: Contributor>(
    identity: &mut Identity,
    sets: &Sets,
    limit: Option<usize>,
    copies: &merge::copies::Copies,
) -> Result<(), merge::identity::Error> {
    for (path, set) in &sets.works {
        identity.add::<S>(path, set, limit, copies)?;
    }
    Ok(())
}

fn merge_titles<S: Contributor>(
    identity: &mut Identity,
    sets: &Sets,
    limit: Option<usize>,
    copies: &merge::copies::Copies,
) -> Result<(), merge::identity::Error> {
    for (path, set) in &sets.works {
        if identity.needs_titles(path) {
            identity.titles::<S>(path, set, limit, copies)?;
        }
    }
    Ok(())
}

fn merge_targets<S: Contributor>(
    identity: &mut Identity,
    sets: &Sets,
    limit: Option<usize>,
    copies: &merge::copies::Copies,
) -> Result<(), merge::identity::Error> {
    for (path, set) in &sets.references {
        identity.targets::<S>(path, set, limit, copies)?;
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

impl<T: Absorb, const N: usize> Absorb for [T; N] {
    fn absorb(&mut self, other: Self) {
        for (a, b) in self.iter_mut().zip(other) {
            a.absorb(b);
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

/// Page fields as model 13 reads them (citation fields plan D8): how many
/// were read, gave none, or were kept as written and why; and the kept by
/// shape, each with a few examples, the list the grammar is taught from
/// next, as the kept `MedlineDate`s are.
#[derive(Debug, Default)]
struct PageCounts {
    how: BTreeMap<&'static str, u64>,
    kept: BTreeMap<String, u64>,
    examples: BTreeMap<String, Vec<String>>,
    /// Named pages (`IFC`), in lower case.
    named: Names,
}

impl PageCounts {
    const EXAMPLES: usize = 3;
    const SHOWN: usize = 40;

    fn see(&mut self, pages: Option<&model::pages::Pages<'_>>, who: impl FnOnce() -> String) {
        use model::pages::Pages;
        match pages {
            None => *self.how.entry("none").or_default() += 1,
            Some(Pages::Read(_)) => *self.how.entry("read").or_default() += 1,
            Some(Pages::Kept { raw, error, .. }) => {
                *self.how.entry(error.as_str()).or_default() += 1;
                let mut shape = String::new();
                survey::signature(raw, &mut shape);
                *self.kept.entry(shape.clone()).or_default() += 1;
                let examples = self.examples.entry(shape).or_default();
                if examples.len() < Self::EXAMPLES {
                    examples.push(format!("{} {raw:?}", who()));
                }
            }
            Some(Pages::Named(name)) => {
                *self.how.entry("named").or_default() += 1;
                self.named
                    .see(name.to_lowercase(), || format!("{} {name:?}", who()));
            }
        }
    }

    fn report(&self, name: &str) {
        eprintln!("  {name} {:?}", self.how);
        let mut kept: Vec<(&String, &u64)> = self.kept.iter().collect();
        kept.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (shape, n) in kept.iter().take(Self::SHOWN) {
            let examples = self
                .examples
                .get(*shape)
                .map_or(String::new(), |e| e.join("; "));
            eprintln!("    kept {shape:<20} {n:>10}  {examples}");
        }
        if kept.len() > Self::SHOWN {
            let rest: u64 = kept[Self::SHOWN..].iter().map(|k| *k.1).sum();
            eprintln!(
                "    kept, and {} more shapes, {rest} fields",
                kept.len() - Self::SHOWN
            );
        }
        self.named.report("named", Self::SHOWN);
    }
}

impl Absorb for PageCounts {
    fn absorb(&mut self, other: Self) {
        self.how.absorb(other.how);
        self.kept.absorb(other.kept);
        self.named.absorb(other.named);
        for (shape, examples) in other.examples {
            let mine = self.examples.entry(shape).or_default();
            for e in examples {
                if mine.len() < Self::EXAMPLES {
                    mine.push(e);
                }
            }
        }
    }
}

/// What became of volume or issue fields (model 13, plan D12): each
/// counted as none, plain, read into levels or kept; the levels read, by
/// the kinds they name; the kept, by shape; and every word in a plain
/// label, to find the markers not read yet.
#[derive(Debug, Default)]
struct LabelCounts {
    how: BTreeMap<&'static str, u64>,
    /// By the kinds of the levels read, with examples, rendered.
    read: BTreeMap<String, (u64, Vec<String>)>,
    /// By shape, with examples.
    kept: BTreeMap<String, (u64, Vec<String>)>,
    /// Each run of letters in a plain label, in lower case.
    words: Names,
    /// The words of labels saying the work awaits its issue, as written.
    pending: Names,
}

/// Names counted (words, named pages, pending labels), each with the
/// first place it was seen.
#[derive(Debug, Default)]
struct Names(BTreeMap<String, (u64, String)>);

impl Names {
    fn see(&mut self, name: String, first: impl FnOnce() -> String) {
        self.0.entry(name).or_insert_with(|| (0, first())).0 += 1;
    }

    /// The commonest `shown`, then how many more and how often.
    fn report(&self, what: &str, shown: usize) {
        let mut names: Vec<_> = self.0.iter().collect();
        names.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.0.cmp(b.0)));
        for (name, (n, first)) in names.iter().take(shown) {
            eprintln!("    {what} {name:<20} {n:>10}  {first}");
        }
        if names.len() > shown {
            let rest: u64 = names[shown..].iter().map(|w| w.1.0).sum();
            eprintln!("    {what}, and {} more, {rest} times", names.len() - shown);
        }
    }
}

impl Absorb for Names {
    fn absorb(&mut self, other: Self) {
        for (name, (n, first)) in other.0 {
            self.0.entry(name).or_insert((0, first)).0 += n;
        }
    }
}

impl LabelCounts {
    const EXAMPLES: usize = 3;
    const SHOWN: usize = 40;

    fn see(
        &mut self,
        label: Option<&model::label::Label<'_>>,
        own: model::label::LevelKind,
        who: impl Fn() -> String,
    ) {
        use model::label::Label;
        let (key, seen) = match label {
            None => {
                *self.how.entry("none").or_default() += 1;
                return;
            }
            Some(Label::Plain(text)) => {
                *self.how.entry("plain").or_default() += 1;
                for word in text.split(|c: char| !c.is_alphabetic()) {
                    if !word.is_empty() {
                        self.words
                            .see(word.to_lowercase(), || format!("{} {text:?}", who()));
                    }
                }
                return;
            }
            Some(Label::Pending(text)) => {
                *self.how.entry("pending").or_default() += 1;
                self.pending
                    .see((*text).to_owned(), || format!("{} {text:?}", who()));
                return;
            }
            Some(read @ Label::Read(_)) => {
                *self.how.entry("read").or_default() += 1;
                let kinds: Vec<&str> = read.levels(own).map(|l| l.kind.as_str()).collect();
                (kinds.join(", "), &mut self.read)
            }
            Some(Label::Kept(text)) => {
                *self.how.entry("kept").or_default() += 1;
                let mut shape = String::new();
                survey::signature(text, &mut shape);
                (shape, &mut self.kept)
            }
        };
        let (n, examples) = seen.entry(key).or_default();
        *n += 1;
        if examples.len() < Self::EXAMPLES
            && let Some(label) = label
        {
            examples.push(format!("{} {:?}", who(), label.to_string()));
        }
    }

    fn report(&self, name: &str) {
        eprintln!("  {name} {:?}", self.how);
        for (what, seen) in [("read", &self.read), ("kept", &self.kept)] {
            let mut rows: Vec<_> = seen.iter().collect();
            rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then_with(|| a.0.cmp(b.0)));
            for (key, (n, examples)) in rows.iter().take(Self::SHOWN) {
                eprintln!("    {what} {key:<32} {n:>10}  {}", examples.join("; "));
            }
            if rows.len() > Self::SHOWN {
                let rest: u64 = rows[Self::SHOWN..].iter().map(|r| r.1.0).sum();
                eprintln!(
                    "    {what}, and {} more, {rest} fields",
                    rows.len() - Self::SHOWN
                );
            }
        }
        self.pending.report("pending", Self::SHOWN);
        self.words.report("word", Self::SHOWN);
    }
}

/// A record's pages, volume and issue, as model 13 reads them.
fn report_placement(pages: &PageCounts, volume: &LabelCounts, issue: &LabelCounts) {
    pages.report("pages");
    volume.report("volume");
    issue.report("issue");
}

impl Absorb for LabelCounts {
    fn absorb(&mut self, other: Self) {
        self.how.absorb(other.how);
        for (mine, theirs) in [(&mut self.read, other.read), (&mut self.kept, other.kept)] {
            for (key, (n, examples)) in theirs {
                let (m, e) = mine.entry(key).or_default();
                *m += n;
                let room = Self::EXAMPLES.saturating_sub(e.len());
                e.extend(examples.into_iter().take(room));
            }
        }
        self.words.absorb(other.words);
        self.pending.absorb(other.pending);
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

/// What became of a record's publisher ids (merge plan §3.1), for the
/// normalize surveys: each value counted by the name it became or by the
/// reason it became none.
#[derive(Debug, Default)]
struct PublisherIds {
    outcomes: BTreeMap<&'static str, u64>,
}

impl PublisherIds {
    fn see(&mut self, scope: &merge::publisher::Scope<'_>, value: &str) {
        let outcome = match scope.name(value) {
            Ok(_) => "a name",
            Err(why) => why.reason(),
        };
        *self.outcomes.entry(outcome).or_default() += 1;
    }

    fn report(&self) {
        if self.outcomes.is_empty() {
            return;
        }
        eprintln!("  publisher ids {:?}", self.outcomes);
    }
}

impl Absorb for PublisherIds {
    fn absorb(&mut self, other: Self) {
        self.outcomes.absorb(other.outcomes);
    }
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
    /// Values read as other than written, by field and how.
    corrected: BTreeMap<(&'static str, &'static str), u64>,
    /// Records the checked-in exclusion list kept out, by reason.
    excluded: BTreeMap<&'static str, u64>,
    doi: [u64; 3],
    elocation_doi: [u64; 3],
    doi_lowercased: u64,
    /// Records whose two DOI slots hold different valid DOIs (2026-09-18).
    doi_disagree: u64,
    pii: u64,
    /// Publisher item ids past the first on one record.
    pii_extra: u64,
    /// What those ids became as names (merge plan §3.1).
    publisher: PublisherIds,
    pmc: u64,
    /// `ArticleId[pmc]` values that are not PMC ids, kept as written.
    pmc_malformed: u64,
    /// Reference ids of a kind that is no name of ours, by kind.
    reference_loose: BTreeMap<&'static str, u64>,
    published: BTreeMap<&'static str, u64>,
    /// `MedlineDate`s kept rather than read (model version 7), by the text
    /// as written: the list the grammar is taught from next.
    published_kept: BTreeMap<String, u64>,
    /// The same, by why the grammar did not read them and by what of them
    /// was certain.
    published_kept_why: BTreeMap<String, u64>,
    published_kept_certain: BTreeMap<&'static str, u64>,
    electronic: u64,
    revised: u64,
    completed: u64,
    status: BTreeMap<&'static str, u64>,
    languages: BTreeMap<&'static str, u64>,
    publication_types: BTreeMap<String, u64>,
    /// Publication types with no place in the genre table, by `UI name`.
    /// A merge stops at the first of these (merge plan §5), so a survey
    /// that found one has found a reason the corpus cannot be merged yet.
    unplaced_types: BTreeMap<String, u64>,
    title: [u64; 3],
    abstracts: u64,
    sections: u64,
    sections_markup: u64,
    sections_math: u64,
    other_abstracts: u64,
    journal_issn: u64,
    journal_issn_linking: u64,
    journal_issn_malformed: u64,
    /// `Pagination`, read (model 13).
    pages: PageCounts,
    volume: LabelCounts,
    issue: LabelCounts,
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
    /// History dates, and times, that name none and are kept as written
    /// (model version 6), by `PubStatus`: which one says whether it
    /// matters — a publisher's `received` is provenance, NLM's `entrez`
    /// would be an arrival time.
    history_date_malformed: BTreeMap<&'static str, u64>,
    history_time_malformed: BTreeMap<&'static str, u64>,
    publication_status: BTreeMap<&'static str, u64>,
    relations: BTreeMap<&'static str, u64>,
    /// Relations with no PMID whose `RefSource` is a DOI, which names the
    /// other record (model version 12), by kind.
    relation_dois: BTreeMap<&'static str, u64>,
    references: u64,
    reference_ids: BTreeMap<&'static str, u64>,
    reference_doi_malformed: u64,
    reference_pmid_malformed: u64,
    reference_pmc_malformed: u64,
    malformed_samples: Vec<String>,
}

impl PubmedStats {
    /// The relations, by kind, and those a DOI in `RefSource` names.
    fn report_relations(&self) {
        eprintln!("  relations {:?}", self.relations);
        if !self.relation_dois.is_empty() {
            eprintln!(
                "  relations naming the other record by a DOI in RefSource, no PMID: {:?}",
                self.relation_dois
            );
        }
    }

    /// The `MedlineDate`s kept rather than read, with the commonest of
    /// them as written: what the grammar is taught from next.
    fn report_kept(&self) {
        if self.published_kept.is_empty() {
            return;
        }
        let kept: u64 = self.published_kept.values().sum();
        eprintln!(
            "  published kept as written: {kept} records, {} distinct; certain {:?}; why {:?}",
            self.published_kept.len(),
            self.published_kept_certain,
            self.published_kept_why
        );
        let mut commonest: Vec<(&String, &u64)> = self.published_kept.iter().collect();
        commonest.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        for (raw, n) in commonest.into_iter().take(40) {
            eprintln!("    {n:>7}  {raw:?}");
        }
    }

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
    fn unplaced(&self) -> u64 {
        self.unplaced_types.values().sum()
    }

    fn report(&self) {
        eprintln!("citations {}  deletions {}", self.citations, self.deletions);
        if !self.corrected.is_empty() {
            eprintln!("  read as other than written {:?}", self.corrected);
        }
        for (reason, n) in &self.excluded {
            eprintln!("  excluded by the checked-in list: {n}, {reason}");
        }
        eprintln!(
            "  doi           valid {} malformed {} none {}",
            self.doi[0], self.doi[1], self.doi[2]
        );
        eprintln!(
            "  elocation doi valid {} malformed {} none {}",
            self.elocation_doi[0], self.elocation_doi[1], self.elocation_doi[2]
        );
        eprintln!("  doi lowercased (allocations) {}", self.doi_lowercased);
        eprintln!(
            "  pii {} (extra on one record {})  pmc {} (not a PMC id, kept {})  two valid dois {}",
            self.pii, self.pii_extra, self.pmc, self.pmc_malformed, self.doi_disagree
        );
        self.publisher.report();
        eprintln!("  published {:?}", self.published);
        self.report_kept();
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
        if !self.unplaced_types.is_empty() {
            eprintln!(
                "  publication types with no genre, which stop a merge and fail this survey: {:?}",
                self.unplaced_types
            );
        }
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
        report_placement(&self.pages, &self.volume, &self.issue);
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
        if !self.history_date_malformed.is_empty() || !self.history_time_malformed.is_empty() {
            eprintln!(
                "  history kept as written: dates {:?}  times {:?}",
                self.history_date_malformed, self.history_time_malformed
            );
        }
        eprintln!("  publication status {:?}", self.publication_status);
        self.report_relations();
        eprintln!("references {}", self.references);
        eprintln!("  ids {:?}", self.reference_ids);
        if !self.reference_loose.is_empty() {
            eprintln!("  ids of no name of ours, kept: {:?}", self.reference_loose);
        }
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
    #[allow(clippy::too_many_lines)]
    fn work(&mut self, c: &Citation<'n>) {
        use source_pubmed::normalize::AuthorName;
        self.citations += 1;
        self.doi_claim(0, c.ids.doi.as_ref());
        for claim in &c.ids.elocation_doi {
            self.doi_claim(1, Some(claim));
        }
        self.pii += u64::from(!c.ids.pii.is_empty());
        self.pii_extra += c.ids.pii.len().saturating_sub(1) as u64;
        let scope = source_pubmed::view::publisher_scope(c);
        for (_, value) in source_pubmed::view::publisher_ids(c) {
            self.publisher.see(&scope, value);
        }
        match c.ids.pmc.as_ref() {
            Some(Claim::Valid(_)) => self.pmc += 1,
            Some(Claim::Malformed { .. }) => self.pmc_malformed += 1,
            None => {}
        }
        if let Some(a) = c.ids.doi.as_ref().and_then(Claim::valid) {
            self.doi_disagree += u64::from(
                c.ids
                    .elocation_doi
                    .iter()
                    .filter_map(Claim::valid)
                    .any(|b| b != a),
            );
        }
        match &c.dates.published {
            Published::Read(date) => *self.published.entry(date.precision()).or_default() += 1,
            Published::Kept {
                certain,
                raw,
                error,
            } => {
                *self.published.entry("kept").or_default() += 1;
                bump(&mut self.published_kept, raw);
                bump(&mut self.published_kept_why, &error.to_string());
                *self
                    .published_kept_certain
                    .entry(certain.precision())
                    .or_default() += 1;
            }
        }
        self.electronic += u64::from(c.dates.electronic.is_some());
        self.revised += u64::from(c.dates.revised.is_some());
        self.completed += u64::from(c.dates.completed.is_some());
        *self.status.entry(c.status.token()).or_default() += 1;
        for l in &c.languages {
            *self.languages.entry(l.alpha_3()).or_default() += 1;
        }
        for pt in &c.publication_types {
            let name = pt.name.as_text().unwrap_or("");
            bump(&mut self.publication_types, name);
            if source_pubmed::view::genre_of(pt.ui.as_str()).is_none() {
                bump(
                    &mut self.unplaced_types,
                    &format!("{} {name}", pt.ui.as_str()),
                );
            }
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
        self.pages
            .see(c.journal.pages.as_ref(), || format!("pmid:{}", c.pmid));
        self.volume.see(
            c.journal.volume.as_ref(),
            model::label::LevelKind::Volume,
            || format!("pmid:{}", c.pmid),
        );
        self.issue.see(
            c.journal.issue.as_ref(),
            model::label::LevelKind::Issue,
            || format!("pmid:{}", c.pmid),
        );
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
            let status = h.status.token();
            *self.history.entry(status).or_default() += 1;
            if let HistoryDate::Malformed { error, .. } = &h.date {
                *self.history_date_malformed.entry(status).or_default() += 1;
                sample(&mut self.malformed_samples, &h.date.to_string(), error);
            }
            if let Some(time @ HistoryTime::Malformed { error, .. }) = &h.time {
                *self.history_time_malformed.entry(status).or_default() += 1;
                sample(&mut self.malformed_samples, &time.to_string(), error);
            }
        }
        if let Some(ps) = c.publication_status {
            *self.publication_status.entry(ps.token()).or_default() += 1;
        }
        for r in &c.relations {
            *self.relations.entry(r.kind.token()).or_default() += 1;
            if r.doi.is_some() {
                *self.relation_dois.entry(r.kind.token()).or_default() += 1;
            }
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
        for id in &r.loose {
            *self.reference_loose.entry(id.kind.token()).or_default() += 1;
        }
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

    fn excluded(&mut self, _: &Pmid, reason: &'static str) {
        *self.excluded.entry(reason).or_default() += 1;
    }

    fn corrected(&mut self, _: &Pmid, field: &'static str, how: &'static str) {
        *self.corrected.entry((field, how)).or_default() += 1;
    }
}

/// The Crossref stats report.
#[derive(Default)]
struct CrossrefStats {
    works: u64,
    deletions: u64,
    types: BTreeMap<source_crossref::vocab::WorkType, u64>,
    /// Records with no `type`.
    untyped: u64,
    /// Records the checked-in exclusion list kept out, by reason.
    excluded: BTreeMap<&'static str, u64>,
    /// Values read and not kept, by field and value.
    set_aside: BTreeMap<(&'static str, &'static str), u64>,
    /// Values read as other than written, by field and how.
    corrected: BTreeMap<(&'static str, &'static str), u64>,
    /// `aliases`: valid, malformed.
    aliases: [u64; 2],
    /// Works whose DOI has an ASCII spelling of its own.
    ascii_dois: u64,
    doi_lowercased: u64,
    owner_differs: u64,
    /// What `alternative-id` became as a name (merge plan §3.1).
    publisher: PublisherIds,
    issued: BTreeMap<&'static str, u64>,
    title: [u64; 3],
    alternate_titles: u64,
    subtitle_empty: u64,
    abstracts: [u64; 3],
    persons: u64,
    orgs: u64,
    /// Authors with a given name and nothing else.
    given_only: u64,
    with_orcid: u64,
    orcid_malformed: u64,
    /// `<institution>`s: authors' affiliations, then works' own.
    institutions: [InstitutionCounts; 2],
    issns: u64,
    /// ISBNs: valid, malformed.
    isbns: [u64; 2],
    languages: BTreeMap<&'static str, u64>,
    references: u64,
    reference_doi: [u64; 3],
    reference_empty: u64,
    updates: BTreeMap<source_crossref::vocab::UpdateType, u64>,
    /// Updates written in an older word than Crossmark's, by the word.
    update_words: BTreeMap<&'static str, u64>,
    relations: BTreeMap<String, u64>,
    licenses: BTreeMap<source_crossref::vocab::ContentVersion, u64>,
    /// License starts: from publication, on a day, unreadable.
    license_starts: [u64; 3],
    /// The dates beside the publication dates, by what each is.
    dates: BTreeMap<&'static str, u64>,
    /// Works with a version label, and with a part number.
    version_part: [u64; 2],
    /// Works with no `member`, and with no `publisher`.
    unowned: [u64; 2],
    /// `page`, read (model 13).
    pages: PageCounts,
    volume: LabelCounts,
    issue: LabelCounts,
    /// Container titles a work gives, by how many, journal articles apart
    /// (model 13 keeps them all; citation fields plan §4).
    container_titles: BTreeMap<&'static str, u64>,
    /// Abbreviations (`short-container-title`) a work gives, likewise.
    short_container_titles: BTreeMap<&'static str, u64>,
    /// The issue's own print and online dates against the work's.
    issue_dates: BTreeMap<&'static str, u64>,
    malformed_samples: Vec<String>,
}

/// How many entries a list holds, journal articles first, then the rest.
const COUNTED: [[&str; 4]; 2] = [
    [
        "journal articles: none",
        "journal articles: one",
        "journal articles: two",
        "journal articles: three or more",
    ],
    [
        "other types: none",
        "other types: one",
        "other types: two",
        "other types: three or more",
    ],
];

impl CrossrefStats {
    /// Where the work says it was published: its pages as read, its
    /// container titles and abbreviations by how many, and the issue's own
    /// dates against the work's (citation fields plan §4, model 13).
    fn placement(&mut self, w: &source_crossref::normalize::Work<'_>) {
        self.pages
            .see(w.pages.as_ref(), || format!("doi:{}", w.doi));
        self.volume
            .see(w.volume.as_ref(), model::label::LevelKind::Volume, || {
                format!("doi:{}", w.doi)
            });
        self.issue
            .see(w.issue.as_ref(), model::label::LevelKind::Issue, || {
                format!("doi:{}", w.doi)
            });
        let counted =
            &COUNTED[usize::from(w.kind != Some(source_crossref::vocab::WorkType::JournalArticle))];
        *self
            .container_titles
            .entry(counted[w.container_titles.len().min(3)])
            .or_default() += 1;
        *self
            .short_container_titles
            .entry(counted[w.short_container_titles.len().min(3)])
            .or_default() += 1;
        for (labels, issue, own) in [
            (
                [
                    "print: the work's own",
                    "print: differs from the work's",
                    "print: the work gives none",
                ],
                w.issue_published_print,
                w.published_print,
            ),
            (
                [
                    "online: the work's own",
                    "online: differs from the work's",
                    "online: the work gives none",
                ],
                w.issue_published_online,
                w.published_online,
            ),
        ] {
            if let Some(issue) = issue {
                let label = match own {
                    Some(own) if own == issue => labels[0],
                    Some(_) => labels[1],
                    None => labels[2],
                };
                *self.issue_dates.entry(label).or_default() += 1;
            }
        }
    }
}

impl<'n> Sink<'n, Crossref> for CrossrefStats {
    fn work(&mut self, w: &source_crossref::normalize::Work<'n>) {
        self.works += 1;
        match w.kind {
            Some(kind) => *self.types.entry(kind).or_default() += 1,
            None => self.untyped += 1,
        }
        self.doi_lowercased += u64::from(!w.doi.is_borrowed());
        self.owner_differs += u64::from(w.owner != w.doi.registrant());
        let scope = source_crossref::view::publisher_scope(w);
        for value in &w.alternative_ids {
            self.publisher.see(&scope, value);
        }
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
                source_crossref::normalize::AuthorName::Given(_) => self.given_only += 1,
            }
            match &a.orcid {
                None => {}
                Some(Claim::Valid(_)) => self.with_orcid += 1,
                Some(Claim::Malformed { raw, error }) => {
                    self.orcid_malformed += 1;
                    sample(&mut self.malformed_samples, raw, error);
                }
            }
            for i in &a.affiliations {
                self.institutions[0].see(i, &mut self.malformed_samples);
            }
        }
        for i in &w.institutions {
            self.institutions[1].see(i, &mut self.malformed_samples);
        }
        self.issns += w.issns.len() as u64;
        self.placement(w);
        for t in &w.isbns {
            match &t.isbn {
                Claim::Valid(_) => self.isbns[0] += 1,
                Claim::Malformed { raw, error } => {
                    self.isbns[1] += 1;
                    sample(&mut self.malformed_samples, raw, error);
                }
            }
        }
        if let Some(l) = w.language {
            *self.languages.entry(l.alpha_3()).or_default() += 1;
        }
        for u in w.updates_to.iter().chain(&w.updated_by) {
            *self.updates.entry(u.kind).or_default() += 1;
            if !u.word.is_crossmark() {
                *self.update_words.entry(u.word.as_str()).or_default() += 1;
            }
        }
        for r in &w.relations {
            bump(&mut self.relations, r.kind.as_text().unwrap_or(""));
        }
        for l in &w.licenses {
            *self.licenses.entry(l.version).or_default() += 1;
            match l.start {
                LicenseStart::FromPublication => self.license_starts[0] += 1,
                LicenseStart::On(_) => self.license_starts[1] += 1,
                LicenseStart::Unreadable { raw, error } => {
                    self.license_starts[2] += 1;
                    sample(&mut self.malformed_samples, raw, &error);
                }
            }
        }
        for d in &w.dates {
            *self.dates.entry(d.kind.field()).or_default() += 1;
        }
        self.version_part[0] += u64::from(w.version.is_some());
        self.version_part[1] += u64::from(w.part_number.is_some());
        for a in &w.aliases {
            match a {
                Claim::Valid(_) => self.aliases[0] += 1,
                Claim::Malformed { raw, error } => {
                    self.aliases[1] += 1;
                    sample(&mut self.malformed_samples, raw, error);
                }
            }
        }
        self.ascii_dois += u64::from(w.ascii_doi.is_some());
        self.unowned[0] += u64::from(w.member.is_none());
        self.unowned[1] += u64::from(w.publisher.is_none());
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

    fn excluded(&mut self, _: &Doi<'n>, reason: &'static str) {
        *self.excluded.entry(reason).or_default() += 1;
    }

    fn set_aside(&mut self, _: &Doi<'n>, field: &'static str, value: &'static str) {
        *self.set_aside.entry((field, value)).or_default() += 1;
    }

    fn corrected(&mut self, _: &Doi<'n>, field: &'static str, how: &'static str) {
        *self.corrected.entry((field, how)).or_default() += 1;
    }
}

impl StatsReport for CrossrefStats {
    fn report(&self) {
        eprintln!("works {}  deletions {}", self.works, self.deletions);
        if !self.excluded.is_empty() {
            eprintln!("  kept out by the exclusion list {:?}", self.excluded);
        }
        eprintln!("  types {:?}  untyped {}", self.types, self.untyped);
        eprintln!(
            "  doi lowercased (allocations) {}  owner prefix != registrant {}",
            self.doi_lowercased, self.owner_differs
        );
        self.publisher.report();
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
            "  authors persons {} orgs {} given name only {} with orcid {} malformed {}",
            self.persons, self.orgs, self.given_only, self.with_orcid, self.orcid_malformed
        );
        eprintln!(
            "  affiliations {}  the works' own institutions {}",
            self.institutions[0], self.institutions[1]
        );
        eprintln!(
            "  issns {}  isbns {} malformed {}",
            self.issns, self.isbns[0], self.isbns[1]
        );
        eprintln!("  container titles {:?}", self.container_titles);
        eprintln!("  abbreviations {:?}", self.short_container_titles);
        eprintln!("  the issue's own dates {:?}", self.issue_dates);
        report_placement(&self.pages, &self.volume, &self.issue);
        eprintln!("  languages {:?}", self.languages);
        eprintln!(
            "  updates {:?}  from older words {:?}",
            self.updates, self.update_words
        );
        if !self.set_aside.is_empty() {
            eprintln!("  set aside {:?}", self.set_aside);
        }
        if !self.corrected.is_empty() {
            eprintln!("  read as other than written {:?}", self.corrected);
        }
        eprintln!(
            "  aliases {} malformed {}  DOIs with an ASCII spelling {}",
            self.aliases[0], self.aliases[1], self.ascii_dois
        );
        eprintln!("  relations {:?}", self.relations);
        eprintln!(
            "  licenses {:?}  from publication {} on a day {} unreadable {}",
            self.licenses, self.license_starts[0], self.license_starts[1], self.license_starts[2]
        );
        eprintln!("  dates {:?}", self.dates);
        eprintln!(
            "  version labels {}  part numbers {}  no member {}  no publisher {}",
            self.version_part[0], self.version_part[1], self.unowned[0], self.unowned[1]
        );
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

/// What Crossref's `<institution>`s in one place carry.
#[derive(Default, Clone, Copy)]
struct InstitutionCounts {
    all: u64,
    ror: u64,
    ror_malformed: u64,
    isni: u64,
    isni_malformed: u64,
    wikidata: u64,
    acronym: u64,
    nameless: u64,
}

impl InstitutionCounts {
    fn see(&mut self, i: &source_crossref::normalize::Institution<'_>, samples: &mut Vec<String>) {
        self.all += 1;
        match i.ror.map(|r| r.id) {
            None => {}
            Some(Claim::Valid(_)) => self.ror += 1,
            Some(Claim::Malformed { raw, error }) => {
                self.ror_malformed += 1;
                sample(samples, raw, &error);
            }
        }
        match i.isni.map(|a| a.id) {
            None => {}
            Some(Claim::Valid(_)) => self.isni += 1,
            Some(Claim::Malformed { raw, error }) => {
                self.isni_malformed += 1;
                sample(samples, raw, &error);
            }
        }
        self.wikidata += u64::from(i.wikidata.is_some());
        self.acronym += u64::from(!i.acronyms.is_empty());
        self.nameless += u64::from(i.name.is_none());
    }
}

impl Absorb for InstitutionCounts {
    fn absorb(&mut self, o: Self) {
        self.all += o.all;
        self.ror += o.ror;
        self.ror_malformed += o.ror_malformed;
        self.isni += o.isni;
        self.isni_malformed += o.isni_malformed;
        self.wikidata += o.wikidata;
        self.acronym += o.acronym;
        self.nameless += o.nameless;
    }
}

impl Display for InstitutionCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (ror {} malformed {}, isni {} malformed {}, wikidata {}, with acronym {}, nameless {})",
            self.all,
            self.ror,
            self.ror_malformed,
            self.isni,
            self.isni_malformed,
            self.wikidata,
            self.acronym,
            self.nameless
        )
    }
}

/// The OpenAlex stats report.
#[derive(Default)]
struct OpenAlexStats {
    works: u64,
    deletions: u64,
    /// Values read as other than written, by field and how.
    corrected: BTreeMap<(&'static str, &'static str), u64>,
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
        if !self.corrected.is_empty() {
            eprintln!("  read as other than written {:?}", self.corrected);
        }
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

    fn corrected(&mut self, _: &OpenAlexId, field: &'static str, how: &'static str) {
        *self.corrected.entry((field, how)).or_default() += 1;
    }
}

impl Merge for PubmedStats {
    fn merge(&mut self, other: Self) {
        absorb_all!(self, other: PubmedStats { citations, deletions, corrected, excluded, doi, elocation_doi, doi_lowercased, doi_disagree, pii, pii_extra, publisher, pmc, pmc_malformed, published, published_kept, published_kept_why, published_kept_certain, electronic, revised, completed, status, languages, publication_types, unplaced_types, title, abstracts, sections, sections_markup, sections_math, other_abstracts, journal_issn, journal_issn_linking, journal_issn_malformed, pages, volume, issue, persons, collectives, orcid, affiliations, affiliation_ids, investigators, mesh_headings, mesh_major, chemicals, keywords, grants, grants_with_country, data_banks, history, history_date_malformed, history_time_malformed, publication_status, relations, relation_dois, references, reference_ids, reference_loose, reference_doi_malformed, reference_pmid_malformed, reference_pmc_malformed, malformed_samples });
    }
}

impl Merge for CrossrefStats {
    fn merge(&mut self, other: Self) {
        absorb_all!(self, other: CrossrefStats { works, deletions, types, untyped, excluded, set_aside, corrected, aliases, ascii_dois, doi_lowercased, owner_differs, publisher, issued, title, alternate_titles, subtitle_empty, abstracts, persons, orgs, given_only, with_orcid, orcid_malformed, institutions, issns, isbns, languages, references, reference_doi, reference_empty, updates, update_words, relations, licenses, license_starts, dates, version_part, unowned, pages, volume, issue, container_titles, short_container_titles, issue_dates, malformed_samples });
    }
}

impl Merge for OpenAlexStats {
    fn merge(&mut self, other: Self) {
        absorb_all!(self, other: OpenAlexStats { works, deletions, corrected, types, doi, doi_lowercased, pmid, mag, languages, language_malformed, title, oa_status, oa_url, with_best_oa, authorships, authors_with_id, with_orcid, with_raw_orcid, orcid_conflicts, corresponding, corresponding_institutions_disagree, institutions, institution_countries, institutions_without_ror, authorship_countries, affiliations, locations, provenance, location_ids, with_source, issn_l, issn, host_lineage, licenses, versions, topics, keywords, concepts, mesh, sdgs, awards, funders, funders_with_ror, apc, abstracts, counts_by_year, indexed_in, flags, references, self_references, related, malformed_samples });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use ingest::Store;
    use merge::registry::Line;
    use merge::{OwnedName, Registry, WorkId};

    use super::{merge_promote, merge_recover, merge_stage, registry_sealed};

    fn store(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!(
            "ol-rht-driver-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("merged")).unwrap();
        Store::new(dir)
    }

    fn a_run(store: &Store) -> (Registry, Vec<Line>) {
        let works = vec![
            vec![OwnedName::parse("doi:10.1000/a").unwrap()],
            vec![OwnedName::parse("pmid:2").unwrap()],
        ];
        let mut registry = Registry::read(&store.registry()).unwrap();
        let lines = registry.assign(&works, works.len(), "t0", true).lines;
        (registry, lines)
    }

    fn journal_line(batch: u64, bytes: Option<u64>, lines: u64) -> merge::journal::Line {
        merge::journal::Line {
            batch,
            at: "t".into(),
            kind: merge::journal::Kind::Full,
            registry: lines,
            public: 0,
            delta: BTreeMap::new(),
            bytes: bytes
                .map(|b| [("registry".to_owned(), b)].into_iter().collect())
                .unwrap_or_default(),
            inputs: Vec::new(),
            records: 0,
            minted: 0,
            merged: 0,
            split: 0,
            dropped: 0,
        }
    }

    /// The driver's registry handling, end to end: what a merge that
    /// dies in pass 2 leaves behind (the user, 2026-09-19).
    #[test]
    fn a_merge_that_dies_in_pass_two_leaves_the_registry_alone() {
        let store = store("pass2");
        let path = store.registry();
        let (registry, lines) = a_run(&store);
        // A first run that concluded.
        let pending = merge_stage("t", &path, &lines, true).unwrap();
        merge_promote("t", &path, registry.minted(), pending).unwrap();
        let before = fs::read(&path).unwrap();
        assert_eq!(before.split(|b| *b == b'\n').count() - 1, 2);

        // A second run: stage, then pass 2 fails for real.
        let (registry, lines) = {
            let mut r = Registry::read(&path).unwrap();
            let works = vec![vec![
                OwnedName::parse("doi:10.1000/a").unwrap(),
                OwnedName::parse("pmid:2").unwrap(),
            ]];
            let lines = r.assign(&works, 1, "t1", true).lines;
            (r, lines)
        };
        assert!(
            matches!(lines.first(), Some(Line::Merge { .. })),
            "{lines:?}"
        );
        let pending = merge_stage("t", &path, &lines, true).unwrap();
        let occupied = store.root().join("merged").join("occupied");
        fs::write(&occupied, b"a file").unwrap();
        let e = fs::create_dir_all(occupied.join("v2.8")).unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotADirectory, "{e}");
        drop(pending); // as every `return` in merge_all does

        assert_eq!(fs::read(&path).unwrap(), before, "byte for byte");
        let read = Registry::read(&path).unwrap();
        assert_eq!(
            read.live(WorkId::new(2).unwrap()),
            Some(WorkId::new(2).unwrap()),
            "id 2 was never retired"
        );
        merge_recover("t", &store).unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);

        // And the same run, finishing, does put them of record.
        let pending = merge_stage("t", &path, &lines, true).unwrap();
        merge_promote("t", &path, registry.minted(), pending).unwrap();
        let read = Registry::read(&path).unwrap();
        assert_eq!(read.live(WorkId::new(2).unwrap()), WorkId::new(1));
        assert_eq!(read, registry);
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn a_partial_run_stages_nothing_to_promote() {
        let store = store("partial");
        let path = store.registry();
        let (registry, lines) = a_run(&store);
        assert!(
            merge_stage("t", &path, &lines, false).unwrap().is_none(),
            "--limit or --survey"
        );
        assert!(!path.exists(), "not even a sidecar");
        merge_promote("t", &path, registry.minted(), None).unwrap();
        assert!(!path.exists());
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn the_sealed_offset_comes_from_the_journal() {
        let store = store("sealed");
        let path = store.registry();
        assert_eq!(registry_sealed(&store).unwrap(), 0, "no journal");
        let (registry, lines) = a_run(&store);
        let pending = merge_stage("t", &path, &lines, true).unwrap();
        merge_promote("t", &path, registry.minted(), pending).unwrap();
        let len = fs::metadata(&path).unwrap().len();

        // A line that records the offset outright.
        let journal = store.journal();
        merge::journal::append(&journal, &journal_line(1, Some(len), 2)).unwrap();
        assert_eq!(registry_sealed(&store).unwrap(), len);

        // One from before SP5, whose line count the offset is read from.
        fs::remove_file(&journal).unwrap();
        merge::journal::append(&journal, &journal_line(1, None, 1)).unwrap();
        let first = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .len() as u64
            + 1;
        assert_eq!(registry_sealed(&store).unwrap(), first);
        fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn recovery_will_not_reach_below_what_a_batch_sealed() {
        let store = store("floor");
        let path = store.registry();
        let (registry, lines) = a_run(&store);
        let pending = merge_stage("t", &path, &lines, true).unwrap();
        merge_promote("t", &path, registry.minted(), pending).unwrap();
        let len = fs::metadata(&path).unwrap().len();
        merge::journal::append(&store.journal(), &journal_line(1, Some(len), 2)).unwrap();

        // A sidecar staged against a shorter registry than the journal
        // has already told readers about: the driver refuses it.
        let sidecar = PathBuf::from(format!("{}.pending", path.display()));
        fs::write(
            &sidecar,
            "{\"base\":0,\"lines\":1,\"at\":\"t\"}\n{\"op\":\"drop\",\"name\":\"pmid:2\",\"at\":\"t\"}\n",
        )
        .unwrap();
        assert!(merge_recover("t", &store).is_err());
        assert_eq!(fs::metadata(&path).unwrap().len(), len, "untouched");
        assert!(sidecar.exists(), "left for a person");
        fs::remove_dir_all(store.root()).unwrap();
    }
}
