//! Trying many changes cheaply (evaluation plan §14; tokenizer plan T1.3
//! and T1.4): a sample of a suite's working half, the works a screening
//! index is built over, their documents saved so that each variant's
//! index is built without another pass over the corpus, and whether two
//! runs are one answer.
//!
//! Both draws keep the items whose seeded hash is smallest, as the
//! language key's does: an item's place is its id's and the seed's, so a
//! larger draw holds every smaller one, and nothing depends on the order
//! the items came in. Each is drawn once and kept — its `--out` is
//! refused when it exists — with an `about.json` saying what it was drawn
//! from.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use evaluation::screening::{self, Why};
use evaluation::suite::{self, Suite};
use ingest::Store;
use merge::WorkId;

use super::Options;

/// What a query's place in its suite's sample is drawn with.
const SAMPLE_SEED: &str = "ol-rht suite sample v1";
/// What a work's place in a screening set's sample is drawn with.
const SET_SEED: &str = "ol-rht screening set v1";

/// An item's place: its id and the seed, nothing else.
fn draw(seed: &str, id: &str) -> u64 {
    let mut h = blake3::Hasher::new();
    h.update(seed.as_bytes());
    h.update(id.as_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&h.finalize().as_bytes()[..8]);
    u64::from_le_bytes(first)
}

/// A file's BLAKE3, in hex, read through and never held whole: a set's
/// documents are gigabytes.
fn digest(path: &Path) -> Result<String, String> {
    let named = |e: io::Error| format!("{}: {e}", path.display());
    let mut file = std::fs::File::open(path).map_err(named)?;
    let mut hasher = blake3::Hasher::new();
    io::copy(&mut file, &mut hasher).map_err(named)?;
    Ok(hasher.finalize().to_hex().to_string())
}

/// The options none of these commands takes.
fn strays(opts: &Options) -> bool {
    opts.workers.is_some()
        || opts.mailto.is_some()
        || opts.seed.is_some()
        || opts.by.is_some()
        || opts.related.is_some()
        || opts.walk.is_some()
        || opts.depth.is_some()
        || opts.survey
}

/// Prints what a command made, or why it stopped.
fn report(what: &str, made: Result<String, String>) -> ExitCode {
    match made {
        Ok(text) => {
            println!("{what}: {text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn kept_out(out: &Path, kept: &str) -> Result<(), String> {
    if out.exists() {
        return Err(format!(
            "{} exists; {kept}, so name a new one",
            out.display()
        ));
    }
    Ok(())
}

fn write_about(dir: &Path, about: &serde_json::Value) -> io::Result<()> {
    let mut text = serde_json::to_string_pretty(about).map_err(io::Error::other)?;
    text.push('\n');
    std::fs::write(dir.join("about.json"), text)
}

/// `evaluate sample <suite> --out <suite> --limit N`: the N queries of
/// the suite's working half whose draw is smallest, as a suite of its own
/// (evaluation plan §14.2) — what variants are screened on, the same
/// queries every time. None of the test half is in it.
pub(crate) fn sample(opts: &Options) -> ExitCode {
    let ([of], Some(out), Some(n)) = (opts.paths.as_slice(), opts.out.as_ref(), opts.limit) else {
        return super::usage();
    };
    if strays(opts) || opts.store.is_some() {
        return super::usage();
    }
    report("evaluate sample", sample_of(of, out, n))
}

fn sample_of(of: &Path, out: &Path, n: u64) -> Result<String, String> {
    kept_out(out, "a suite is kept")?;
    let whole = Suite::read(of).map_err(|e| e.to_string())?;
    let n = usize::try_from(n).map_err(|_| format!("a sample of {n} queries"))?;
    let working = whole.working().len();
    let sampled = whole
        .sample(n, |q| draw(SAMPLE_SEED, q))
        .map_err(|e| format!("{}: {e}", of.display()))?;
    let mut files = serde_json::Map::new();
    for name in [suite::QUERIES, suite::KEY, suite::FILTERS, suite::HALVES] {
        if of.join(name).exists() {
            files.insert(name.to_owned(), digest(&of.join(name))?.into());
        }
    }
    let about = serde_json::json!({
        "sample_of": of.display().to_string(),
        "of_blake3": files,
        "seed": SAMPLE_SEED,
        "working": working,
        "queries": n,
        "ol_rht": env!("CARGO_PKG_VERSION"),
    });
    sampled
        .write(out)
        .and_then(|()| write_about(out, &about))
        .map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(format!(
        "{n} of the working half's {working} queries, drawn with {SAMPLE_SEED:?}\n  suite: {}",
        out.display()
    ))
}

/// `evaluate screening-set <suite>... --out <dir> --limit N`: the works a
/// screening index is built over (evaluation plan §14.3) — every work the
/// suites judged or named as a seed that the epoch's index holds, and the
/// N others it holds whose draw is smallest — each by its public id, in
/// `works.tsv`.
pub(crate) fn set(opts: &Options) -> ExitCode {
    let (Some(out), Some(n)) = (opts.out.as_ref(), opts.limit) else {
        return super::usage();
    };
    if opts.paths.is_empty() || strays(opts) {
        return super::usage();
    }
    report(
        "evaluate screening-set",
        set_of(&opts.store(), &opts.paths, out, n),
    )
}

fn set_of(
    store: &Store,
    suites: &[std::path::PathBuf],
    out: &Path,
    n: u64,
) -> Result<String, String> {
    kept_out(out, "a screening set is kept")?;
    let n = usize::try_from(n).map_err(|_| format!("a sample of {n} works"))?;
    let read: Vec<Suite> = suites
        .iter()
        .map(|path| Suite::read(path).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let serving = serve::Serving::open(store).map_err(|e| e.to_string())?;
    let index = serving.search_index();
    let mut drawing = screening::Draw::new(&read, n, |work| draw(SET_SEED, work));
    let mut nameless = None;
    index
        .each_id(|id| match serving.public_of(id) {
            Some(public) => drawing.held(&public),
            None => {
                nameless.get_or_insert(id);
            }
        })
        .map_err(|e| e.to_string())?;
    if let Some(id) = nameless {
        return Err(format!(
            "work {id} has no public id; a set names works by public id only"
        ));
    }
    let drawn = drawing.finish();
    let file = out.join(screening::WORKS);
    let written = (|| -> io::Result<()> {
        std::fs::create_dir_all(out)?;
        let mut to = io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&file)?,
        );
        drawn.set.write(&mut to)?;
        to.flush()
    })();
    written.map_err(|e| format!("{}: {e}", file.display()))?;
    let (judged, seeds, sampled) = (
        drawn.set.count(Why::Judged),
        drawn.set.count(Why::Seed),
        drawn.set.count(Why::Sample),
    );
    let about = serde_json::json!({
        "suites": suites
            .iter()
            .map(|path| Ok(serde_json::json!({
                "path": path.display().to_string(),
                "key_blake3": digest(&path.join(suite::KEY))?,
                "filters_blake3": digest(&path.join(suite::FILTERS))?,
            })))
            .collect::<Result<Vec<_>, String>>()?,
        "seed": SET_SEED,
        "sample_asked": n,
        "works": drawn.set.works.len(),
        "judged": judged,
        "seeds": seeds,
        "sample": sampled,
        "held": drawn.held,
        "judged_or_seed_not_held": drawn.not_held,
        "works_blake3": digest(&file)?,
        "epoch": serving.epoch(),
        "search_absorbed_batch": index.batch().map_err(|e| e.to_string())?,
        "ol_rht": env!("CARGO_PKG_VERSION"),
    });
    write_about(out, &about).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(format!(
        "{} works of the {} epoch {} holds: {judged} judged, {seeds} seeds, \
         {sampled} drawn from the rest with {SET_SEED:?}; {} judged or seeds not held, left out\n  \
         set: {}",
        drawn.set.works.len(),
        drawn.held,
        serving.epoch().epoch,
        drawn.not_held,
        file.display()
    ))
}

/// A screening set's works as the store's epoch has them.
struct Listed {
    /// The set's file.
    list: String,
    /// Its BLAKE3, in hex.
    digest: String,
    works: HashSet<WorkId>,
    /// The epoch they are works of, and the batch its index is at.
    epoch: u64,
    batch: u64,
}

/// The works a screening set names — its file, or the directory it is in
/// — each public id as the work it is in the store's epoch: the epoch the
/// set was drawn from, or a later one that still has every work.
fn listed(store: &Store, path: &Path) -> Result<Listed, String> {
    let file = if path.is_dir() {
        path.join(screening::WORKS)
    } else {
        path.to_path_buf()
    };
    let named = |e: &dyn std::fmt::Display| format!("{}: {e}", file.display());
    let bytes = std::fs::read(&file).map_err(|e| named(&e))?;
    let text = std::str::from_utf8(&bytes).map_err(|e| named(&e))?;
    let set = screening::Set::read(text).map_err(|e| named(&e))?;
    if set.works.is_empty() {
        return Err(named(&"names no works"));
    }
    let serving = serve::Serving::open(store).map_err(|e| e.to_string())?;
    let mut works = HashSet::with_capacity(set.works.len());
    for public in set.works.keys() {
        let Ok(Some(serve::Resolved::One(id))) = serving.resolve(public) else {
            return Err(named(&format!(
                "{public} is not one work of epoch {}",
                serving.epoch().epoch
            )));
        };
        if !works.insert(id) {
            return Err(named(&format!(
                "{public} is a work the list has under another id too"
            )));
        }
    }
    Ok(Listed {
        list: file.display().to_string(),
        digest: blake3::hash(&bytes).to_hex().to_string(),
        works,
        epoch: serving.epoch().epoch,
        batch: serving.search_index().batch().map_err(|e| e.to_string())?,
    })
}

/// `search documents --works <screening set> --out <file>`: the set's
/// works' documents, assembled from the merged and normalized sets as the
/// published index's are — the one pass over the corpus a screening set
/// costs — and saved, so that every treatment's index is built from the
/// file (`search build --documents`).
pub(crate) fn documents(opts: &Options) -> ExitCode {
    let what = "search documents";
    let (Some(list), Some(out)) = (opts.works.as_ref(), opts.out.as_ref()) else {
        return super::usage();
    };
    if !opts.paths.is_empty() || opts.limit.is_some() || strays(opts) {
        return super::usage();
    }
    let store = opts.store();
    let started = Instant::now();
    let listed =
        match kept_out(out, "a set's documents are kept").and_then(|()| listed(&store, list)) {
            Ok(listed) => listed,
            Err(e) => {
                eprintln!("{what}: {e}");
                return ExitCode::FAILURE;
            }
        };
    let batch = match super::current_batch(what, &store) {
        Ok(line) => line.batch,
        Err(code) => return code,
    };
    // The ids are the epoch's; the documents are the merged sets'.
    if batch != listed.batch {
        eprintln!(
            "{what}: the merged sets are at batch {batch} and epoch {}'s index at batch {}; \
             a set's documents are assembled from the merged sets its epoch was",
            listed.epoch, listed.batch
        );
        return ExitCode::FAILURE;
    }
    let mut assembly = match super::assemble(what, &store, started) {
        Ok(assembly) => assembly,
        Err(code) => return code,
    };
    let works = listed.works.len();
    let saved = (|| -> Result<u64, String> {
        let mut writer =
            search::documents::Writer::create(out, listed.list, listed.digest, batch, listed.works)
                .map_err(|e| e.to_string())?;
        merge::document::Ranged::new(&mut assembly, |docs| {
            writer
                .add(docs)
                .map_err(|e| merge::document::Error::Consumer(e.to_string()))
        })
        .finish()
        .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())
    })();
    match saved {
        Ok(bytes) => {
            #[allow(clippy::cast_precision_loss)]
            let mib = bytes as f64 / (1024.0 * 1024.0);
            println!(
                "{what}: {works} works' documents of the {} assembled, at batch {batch}: \
                 {mib:.1} MiB in {:.1} s\n  documents: {}",
                assembly.stats().documents,
                started.elapsed().as_secs_f64(),
                out.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{what}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `search build --documents <file>`: an index of the saved documents and
/// no others, at the batch they were assembled at, its manifest naming
/// them and the set they are.
pub(crate) fn build_from(
    dir: &Path,
    analyzer: Option<search::analyzer::Switches>,
    saved: &Path,
) -> Result<search::index::Built, String> {
    let blake3 = digest(saved)?;
    let mut build = search::index::Build::start(dir, analyzer).map_err(|e| e.to_string())?;
    let header =
        search::documents::read(saved, |document| build.add(std::slice::from_ref(&document)))
            .map_err(|e| e.to_string())?;
    build.of(search::Subset {
        documents: saved.display().to_string(),
        blake3,
        list: header.list,
        list_blake3: header.list_blake3,
        works: header.works,
    });
    build.finish(header.batch).map_err(|e| e.to_string())
}

/// `evaluate same <run> <run>`: whether two runs of the same queries are
/// one answer (tokenizer plan T1.4) — no difference but what a build's
/// segments decide, which is counted apart. Fails when they are not.
pub(crate) fn same(opts: &Options) -> ExitCode {
    let [a, b] = opts.paths.as_slice() else {
        return super::usage();
    };
    if !super::evaluate_reads_only(opts) {
        return super::usage();
    }
    match same_of(a, b) {
        Ok((text, true)) => {
            println!("evaluate same: {text}");
            ExitCode::SUCCESS
        }
        Ok((text, false)) => {
            println!("evaluate same: {text}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("evaluate same: {e}");
            ExitCode::FAILURE
        }
    }
}

fn same_of(a: &Path, b: &Path) -> Result<(String, bool), String> {
    let read = |path: &Path| -> Result<(evaluation::trec::Run, serde_json::Value), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let run =
            evaluation::trec::Run::read(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let about = format!("{}.about", path.display());
        let text = std::fs::read_to_string(&about).map_err(|e| format!("{about}: {e}"))?;
        let about = serde_json::from_str(&text).map_err(|e| format!("{about}: {e}"))?;
        Ok((run, about))
    };
    let ((run_a, about_a), (run_b, about_b)) = (read(a)?, read(b)?);
    // Runs of other queries, or to another depth, are not to be compared.
    for field in ["asked_blake3", "depth", "restricted"] {
        if about_a.get(field).is_none() || about_a.get(field) != about_b.get(field) {
            return Err(format!(
                "the runs' .about files differ in {field}: {} against {}; \
                 two runs are compared when they asked the same",
                about_a.get(field).unwrap_or(&serde_json::Value::Null),
                about_b.get(field).unwrap_or(&serde_json::Value::Null)
            ));
        }
    }
    let d = evaluation::same::compare(&run_a, &run_b);
    let mut text = format!("{} against {}\n", a.display(), b.display());
    let _ = writeln!(text, "  {} lines; {} real differences", d.lines, d.real);
    for what in &d.described {
        let _ = writeln!(text, "    {what}");
    }
    if d.real > d.described.len() {
        let _ = writeln!(text, "    and {} more", d.real - d.described.len());
    }
    let _ = writeln!(
        text,
        "  what a build's segments decide, counted apart: {} scores apart in their last digits \
         (within {:e} of the score), {} ranks holding another of tied works, {} works tied with \
         the last a run returned",
        d.rounded,
        evaluation::same::TOLERANCE,
        d.tied,
        d.at_the_end
    );
    text.push_str(if d.same() {
        "  the same"
    } else {
        "  NOT the same"
    });
    Ok((text, d.same()))
}
