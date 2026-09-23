//! The runner: every record of every file through a [`Source`] and into a
//! [`Sink`], stopping at the first failure with enough context to reproduce
//! it (plan §6): the source, the file, the record's ordinal, the stage, and
//! the source's own error with its native id and field.
//!
//! Parallelism is over files (plan §3.4): a work queue of raw files, one
//! worker per core, each with its own sink, sequential within a file. A
//! record limit is claimed through one shared counter, so it is exact
//! across workers; the first failure raises a halt flag every worker checks
//! between records. Every worker's sink comes back to the caller either
//! way, to be finished or discarded.

use std::collections::BTreeMap;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, BufReader, Read};
use std::ops::ControlFlow;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::frame::ReadError;
use crate::source::{Signature, Sink, Source};
use crate::{alloc, raw};

/// The framing reader's buffer: how much text is pulled from the file at
/// once. Records run to a few hundred KiB; one read amortizes many.
const FRAME_BUFFER: usize = 1024 * 1024;

/// The stage a run failed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Reading or inflating the file.
    Read,
    /// The strict native parse.
    Parse,
    /// The strict value-level normalization.
    Normalize,
    /// Writing to the sink, or making one.
    Write,
    /// Transferring the file in, or verifying what arrived (`ingest`).
    Fetch,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "read",
            Self::Parse => "parse",
            Self::Normalize => "normalize",
            Self::Write => "write",
            Self::Fetch => "fetch",
        })
    }
}

/// Why a run stopped, and where.
#[derive(Debug)]
pub struct Failure {
    /// [`Source::NAME`].
    pub source: &'static str,
    /// The raw file; empty when no file was involved (a sink that could
    /// not be made).
    pub path: PathBuf,
    /// The record's ordinal in the file, from one; for a failure of the
    /// stream itself, the records read before it; zero when the file could
    /// not be opened, or opened for.
    pub record: u64,
    #[allow(missing_docs)]
    pub stage: Stage,
    /// The source's error: what, and the native id where it was readable.
    pub error: Box<dyn Error + Send + Sync>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.source, self.stage)?;
        if !self.path.as_os_str().is_empty() {
            write!(f, ": {}", self.path.display())?;
        }
        if self.record > 0 {
            write!(f, ": record {}", self.record)?;
        }
        write!(f, ": {}", self.error)
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error.as_ref())
    }
}

/// What a run did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Report {
    /// Files read.
    pub files: usize,
    /// Records parsed (and normalized, when normalizing).
    pub records: u64,
    /// Bytes of decompressed input.
    pub bytes: u64,
    /// Wall time for reading, parsing and normalizing.
    pub elapsed: Duration,
    /// Allocations during the run, when the counting allocator is
    /// installed (plan §5, rule 13); zero otherwise.
    pub allocations: u64,
    /// Bytes those allocations requested.
    pub allocated_bytes: u64,
    /// Worker threads.
    pub workers: usize,
}

#[allow(clippy::cast_precision_loss)] // reporting only
impl Report {
    /// Records per second of wall time.
    #[must_use]
    pub fn records_per_second(&self) -> f64 {
        self.records as f64 / self.elapsed.as_secs_f64()
    }

    /// Decompressed mebibytes per second of wall time.
    #[must_use]
    pub fn mib_per_second(&self) -> f64 {
        self.bytes as f64 / (1024.0 * 1024.0) / self.elapsed.as_secs_f64()
    }
}

impl fmt::Display for Report {
    #[allow(clippy::cast_precision_loss)] // reporting only
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} records from {} files on {} worker{}, {:.1} MiB in {:.2}s = {:.0} records/s, {:.0} MiB/s",
            self.records,
            self.files,
            self.workers,
            if self.workers == 1 { "" } else { "s" },
            self.bytes as f64 / (1024.0 * 1024.0),
            self.elapsed.as_secs_f64(),
            self.records_per_second(),
            self.mib_per_second()
        )?;
        if self.allocations > 0 && self.records > 0 {
            write!(
                f,
                "; {:.1} allocations/record, {:.0} B/record",
                self.allocations as f64 / self.records as f64,
                self.allocated_bytes as f64 / self.records as f64
            )?;
        }
        Ok(())
    }
}

/// A parallel run's outcome: the report or the failure, and every worker's
/// sink either way, so the caller can finish or discard them.
#[derive(Debug)]
pub struct Outcome<K> {
    #[allow(missing_docs)]
    pub result: Result<Report, Failure>,
    /// One per worker that got a sink, in no particular order.
    pub sinks: Vec<K>,
    /// What `--survey` found; empty unless surveying.
    pub survey: Survey,
}

/// What `--survey` collects (plan §6): every distinct failure signature,
/// how often it fired, and a few examples, so one run surfaces many warts
/// instead of one.
///
/// Parse failures are surveyed too, and were not until 2026-09-23. The
/// old rule — structural, so stop — was written when a survey read two
/// sample files on this machine. Stopping costs a code change, a rebuild,
/// an image push and a re-run for every wart found, which is affordable
/// over two files and not over 1,334 on a cluster: the first survey of
/// the whole PubMed baseline died partway through the first of eight
/// shares, one wart the wiser. Collapsing that loop is the mode's whole
/// purpose, and a parse failure is a wart like any other.
///
/// What a parse failure costs is the rest of its file. A normalize
/// failure is one record's; a parse failure leaves the reader's place in
/// the byte stream untrustworthy, and [`Source::records`] has no way to
/// be asked for the next record after it — so the file is abandoned and
/// the next one taken. That is a hole in the run, and a survey that hid
/// it would be worse than one that stopped, so every abandoned file is
/// named in [`Survey::abandoned`] and in the report.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Survey {
    /// Records that failed to parse or to normalize.
    pub failed: u64,
    /// By signature.
    pub seen: BTreeMap<String, Seen>,
    /// Files given up on at a record that would not parse; their later
    /// records were never read, and are in no count here.
    pub abandoned: Vec<Abandoned>,
}

/// A file a survey stopped reading partway through, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abandoned {
    /// The raw file.
    pub path: PathBuf,
    /// The ordinal, from one, of the record that would not parse.
    pub record: u64,
}

/// One signature's tally.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Seen {
    #[allow(missing_docs)]
    pub count: u64,
    /// The first few failures' full messages.
    pub examples: Vec<String>,
}

impl Survey {
    const EXAMPLES: usize = 3;

    fn note(&mut self, signature: String, example: String) {
        self.failed += 1;
        let seen = self.seen.entry(signature).or_default();
        seen.count += 1;
        if seen.examples.len() < Self::EXAMPLES {
            seen.examples.push(example);
        }
    }

    /// A file given up on where a record would not parse: the failure is
    /// tallied like any other, and the file is named, because everything
    /// after that record went unread.
    fn abandon(&mut self, path: &Path, record: u64, signature: String, example: String) {
        self.note(signature, example);
        self.abandoned.push(Abandoned {
            path: path.to_path_buf(),
            record,
        });
    }

    /// Adds another worker's survey.
    pub fn merge(&mut self, other: Self) {
        self.failed += other.failed;
        self.abandoned.extend(other.abandoned);
        for (signature, seen) in other.seen {
            let mine = self.seen.entry(signature).or_default();
            mine.count += seen.count;
            for example in seen.examples {
                if mine.examples.len() < Self::EXAMPLES {
                    mine.examples.push(example);
                }
            }
        }
    }

    /// Nothing failed, and nothing went unread.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.failed == 0 && self.abandoned.is_empty()
    }

    /// The abandoned files, by path.
    fn abandoned_in_order(&self) -> Vec<&Abandoned> {
        let mut sorted: Vec<&Abandoned> = self.abandoned.iter().collect();
        sorted.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.record.cmp(&b.record)));
        sorted
    }
}

impl fmt::Display for Survey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} record{} failed, {} distinct signature{}",
            self.failed,
            if self.failed == 1 { "" } else { "s" },
            self.seen.len(),
            if self.seen.len() == 1 { "" } else { "s" }
        )?;
        // The count goes in the first line as well as the list below it:
        // a run that read only part of its corpus must say so where the
        // eye lands, not under a page of signatures.
        if !self.abandoned.is_empty() {
            write!(
                f,
                ", {} file{} abandoned",
                self.abandoned.len(),
                if self.abandoned.len() == 1 { "" } else { "s" }
            )?;
        }
        writeln!(f)?;
        let mut by_count: Vec<(&String, &Seen)> = self.seen.iter().collect();
        by_count.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(b.0)));
        for (signature, seen) in by_count {
            writeln!(f, "{:>8}  {signature}", seen.count)?;
            for example in &seen.examples {
                writeln!(f, "          e.g. {example}")?;
            }
        }
        if !self.abandoned.is_empty() {
            writeln!(
                f,
                "{} file{} given up on at a record that would not parse; nothing after that record was read:",
                self.abandoned.len(),
                if self.abandoned.len() == 1 { "" } else { "s" }
            )?;
            for a in self.abandoned_in_order() {
                writeln!(f, "          {} from record {}", a.path.display(), a.record)?;
            }
        }
        Ok(())
    }
}

/// What one record's step failed at.
pub(crate) enum Step<E> {
    Normalize(E),
    Write(io::Error),
}

/// Shared across a run's workers: the record limit and the halt flag.
///
/// A worker claims a record before reading it, so the record after the
/// limit is never parsed, and gives the claim back if the file ends
/// first. `done` counts claims consumed or still held; `pending` counts
/// the held ones. A claim that finds the limit reached waits while claims
/// are pending — one may come back — and only then answers no, so the
/// limit is exact across workers.
pub(crate) struct Progress<'a> {
    limit: Option<u64>,
    done: &'a AtomicU64,
    pending: &'a AtomicU64,
    halt: &'a AtomicBool,
    /// Tally failures instead of stopping at the first: a normalize
    /// failure costs its record, a parse failure the rest of its file.
    survey: bool,
}

impl<'a> Progress<'a> {
    pub(crate) fn new(
        limit: Option<u64>,
        done: &'a AtomicU64,
        pending: &'a AtomicU64,
        halt: &'a AtomicBool,
        survey: bool,
    ) -> Self {
        Self {
            limit,
            done,
            pending,
            halt,
            survey,
        }
    }

    /// Claims the next record: false when the run is stopped or the limit
    /// is reached, so at most `limit` records are ever processed.
    pub(crate) fn claim(&self) -> bool {
        let Some(n) = self.limit else {
            return !self.halt.load(Ordering::Relaxed);
        };
        loop {
            if self.halt.load(Ordering::Relaxed) {
                return false;
            }
            if self.done.fetch_add(1, Ordering::AcqRel) < n {
                self.pending.fetch_add(1, Ordering::AcqRel);
                return true;
            }
            self.done.fetch_sub(1, Ordering::AcqRel);
            if self.pending.load(Ordering::Acquire) == 0 {
                return false;
            }
            std::thread::yield_now();
        }
    }

    /// The claim held for this record is used: the record is being
    /// processed.
    fn consume(&self) {
        if self.limit.is_some() {
            self.pending.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Whether the run is over: stopped, or the limit reached with no
    /// claim that could still come back.
    pub(crate) fn over(&self) -> bool {
        self.halt.load(Ordering::Relaxed)
            || self.limit.is_some_and(|n| {
                self.done.load(Ordering::Acquire) >= n && self.pending.load(Ordering::Acquire) == 0
            })
    }

    /// Gives back a claim that went unused: the file ended before the
    /// record it was for.
    fn unclaim(&self) {
        if self.limit.is_some() {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            self.done.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// Counts the bytes read through it.
struct Counted<R> {
    inner: R,
    bytes: u64,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes += n as u64;
        Ok(n)
    }
}

/// What one raw file yielded.
pub(crate) struct FileRun {
    pub(crate) records: u64,
    pub(crate) bytes: u64,
    pub(crate) survey: Survey,
    /// The read ended before the end of the stream: the record limit was
    /// reached, or another worker's failure stopped the run.
    pub(crate) stopped: bool,
}

/// A [`Failure`] of `S` at `path`.
pub(crate) fn failure<S: Source>(
    path: &Path,
    record: u64,
    stage: Stage,
    error: Box<dyn Error + Send + Sync>,
) -> Failure {
    Failure {
        source: S::NAME,
        path: path.to_path_buf(),
        record,
        stage,
        error,
    }
}

/// One raw file through the source and into the sink; `None` when the run
/// was over before the file's first record, which is then not counted as
/// read.
fn run_file<S, K, T>(
    path: &Path,
    index: usize,
    sink: &mut K,
    progress: &Progress<'_>,
    step: &mut T,
) -> Result<Option<FileRun>, Failure>
where
    S: Source,
    K: for<'n> Sink<'n, S>,
    T: FnMut(&S::Native<'_>, &mut K) -> Result<(), Step<S::NormalizeError>>,
{
    let mut raw = raw::open(path).map_err(|e| failure::<S>(path, 0, Stage::Read, Box::new(e)))?;
    if !progress.claim() {
        return Ok(None);
    }
    sink.begin_file(path, index)
        .map_err(|e| failure::<S>(path, 0, Stage::Write, Box::new(e)))?;
    let run = run_stream::<S, K, T>(&mut raw, path, sink, progress, step)?;
    sink.end_file()
        .map_err(|e| failure::<S>(path, run.records, Stage::Write, Box::new(e)))?;
    Ok(Some(run))
}

/// The records of `input` through the source and into the sink, a claim
/// for the first record already held.
///
/// A claim is held for the next record before it is read, so the record
/// after the limit is never parsed; the claim left over when the stream
/// ends goes back.
pub(crate) fn run_stream<S, K, T>(
    input: &mut impl Read,
    path: &Path,
    sink: &mut K,
    progress: &Progress<'_>,
    step: &mut T,
) -> Result<FileRun, Failure>
where
    S: Source,
    K: for<'n> Sink<'n, S>,
    T: FnMut(&S::Native<'_>, &mut K) -> Result<(), Step<S::NormalizeError>>,
{
    let fail = |record: u64, stage: Stage, error: Box<dyn Error + Send + Sync>| {
        failure::<S>(path, record, stage, error)
    };
    let mut counted = Counted {
        inner: input,
        bytes: 0,
    };
    let mut records = 0u64;
    let mut held = true;
    let mut failed = None;
    let mut survey = Survey::default();
    let read = S::records(
        BufReader::with_capacity(FRAME_BUFFER, &mut counted),
        |native| {
            held = false;
            progress.consume();
            records += 1;
            match step(native, sink) {
                Ok(()) => {}
                Err(Step::Normalize(e)) if progress.survey => {
                    survey.note(e.signature(), e.to_string());
                }
                Err(Step::Normalize(e)) => {
                    failed = Some(fail(records, Stage::Normalize, Box::new(e)));
                    return ControlFlow::Break(());
                }
                Err(Step::Write(e)) => {
                    failed = Some(fail(records, Stage::Write, Box::new(e)));
                    return ControlFlow::Break(());
                }
            }
            held = progress.claim();
            if held {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        },
    );
    let stopped = !held;
    if held {
        progress.unclaim();
    }
    if let Some(failure) = failed {
        return Err(failure);
    }
    match read {
        Ok(()) => {}
        // A survey gives up on the file, not on the run: nothing after
        // this record was read, so the file is named along with the
        // signature. Every other mode stops here, as always.
        Err(ReadError::Parse(e)) if progress.survey => {
            survey.abandon(path, records + 1, e.signature(), e.to_string());
        }
        Err(ReadError::Io(e)) => return Err(fail(records, Stage::Read, Box::new(e))),
        Err(ReadError::Parse(e)) => return Err(fail(records + 1, Stage::Parse, Box::new(e))),
    }
    Ok(FileRun {
        records,
        bytes: counted.bytes,
        survey,
        stopped,
    })
}

/// Normalizes one record and asks the sink whether it could write it.
pub(crate) fn normalize_step<S: Source, K: for<'n> Sink<'n, S>>(
    native: &S::Native<'_>,
    sink: &mut K,
) -> Result<(), Step<S::NormalizeError>> {
    S::normalize(native, sink).map_err(Step::Normalize)?;
    sink.failed().map_or(Ok(()), |e| Err(Step::Write(e)))
}

fn drive<S, K, T>(
    paths: &[impl AsRef<Path>],
    limit: Option<u64>,
    sink: &mut K,
    mut step: T,
) -> Result<Report, Failure>
where
    S: Source,
    K: for<'n> Sink<'n, S>,
    T: FnMut(&S::Native<'_>, &mut K) -> Result<(), Step<S::NormalizeError>>,
{
    let started = Instant::now();
    let (allocations, allocated_bytes) = (alloc::allocations(), alloc::allocated_bytes());
    let (done, pending, halt) = (AtomicU64::new(0), AtomicU64::new(0), AtomicBool::new(false));
    let progress = Progress::new(limit, &done, &pending, &halt, false);
    let mut report = Report {
        workers: 1,
        ..Report::default()
    };
    for (index, path) in paths.iter().enumerate() {
        if progress.over() {
            break;
        }
        let Some(run) = run_file::<S, K, T>(path.as_ref(), index, sink, &progress, &mut step)?
        else {
            break;
        };
        report.files += 1;
        report.records += run.records;
        report.bytes += run.bytes;
    }
    report.elapsed = started.elapsed();
    report.allocations = alloc::allocations() - allocations;
    report.allocated_bytes = alloc::allocated_bytes() - allocated_bytes;
    Ok(report)
}

/// Parses every record of every file and normalizes none of them: the
/// structural check alone, on this thread. `limit` stops the run after
/// that many records across all files.
///
/// # Errors
///
/// The first file that cannot be read or record that does not parse.
pub fn parse<S: Source>(paths: &[impl AsRef<Path>], limit: Option<u64>) -> Result<Report, Failure> {
    drive::<S, (), _>(paths, limit, &mut (), |_, &mut ()| Ok(()))
}

/// Parses and normalizes every record of every file into `sink`, on this
/// thread. `limit` stops the run after that many records across all
/// files.
///
/// # Errors
///
/// The first file that cannot be read, record that does not parse, value
/// that does not fit the model, or write the sink could not make. The sink
/// has received everything before it.
pub fn normalize<S, K>(
    paths: &[impl AsRef<Path>],
    limit: Option<u64>,
    sink: &mut K,
) -> Result<Report, Failure>
where
    S: Source,
    K: for<'n> Sink<'n, S>,
{
    drive::<S, K, _>(paths, limit, sink, normalize_step::<S, K>)
}

/// [`parse`] over `workers` threads.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_parallel<S: Source, P: AsRef<Path> + Sync>(
    paths: &[P],
    limit: Option<u64>,
    workers: usize,
) -> Result<Report, Failure> {
    run_parallel::<S, (), _, P, _>(
        paths,
        limit,
        workers,
        false,
        |_| Ok(()),
        |_, &mut ()| Ok(()),
    )
    .result
}

/// [`normalize`] over `workers` threads, each with the sink `make` gives
/// it (called with the worker's index), each taking the next raw file
/// from a shared queue until none is left. Never more workers than files.
/// With `survey`, a record that does not normalize is tallied in the
/// outcome's [`Survey`] instead of stopping the run; the sink may have
/// received part of it. So is a record that does not parse, at the price
/// of the rest of its file, which the survey then names as abandoned.
pub fn normalize_parallel<S, K, F, P>(
    paths: &[P],
    limit: Option<u64>,
    workers: usize,
    survey: bool,
    make: F,
) -> Outcome<K>
where
    S: Source,
    K: for<'n> Sink<'n, S> + Send,
    F: Fn(usize) -> io::Result<K> + Sync,
    P: AsRef<Path> + Sync,
{
    run_parallel::<S, K, F, P, _>(paths, limit, workers, survey, make, normalize_step::<S, K>)
}

fn run_parallel<S, K, F, P, T>(
    paths: &[P],
    limit: Option<u64>,
    workers: usize,
    survey: bool,
    make: F,
    step: T,
) -> Outcome<K>
where
    S: Source,
    K: for<'n> Sink<'n, S> + Send,
    F: Fn(usize) -> io::Result<K> + Sync,
    P: AsRef<Path> + Sync,
    T: Fn(&S::Native<'_>, &mut K) -> Result<(), Step<S::NormalizeError>> + Sync,
{
    let workers = workers.clamp(1, paths.len().max(1));
    let started = Instant::now();
    let (allocations, allocated_bytes) = (alloc::allocations(), alloc::allocated_bytes());
    let (next, done, pending, halt) = (
        AtomicUsize::new(0),
        AtomicU64::new(0),
        AtomicU64::new(0),
        AtomicBool::new(false),
    );
    let progress = Progress::new(limit, &done, &pending, &halt, survey);
    let outcomes: Vec<(Report, Option<K>, Option<Failure>, Survey)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let (progress, make, step, next, halt) = (&progress, &make, &step, &next, &halt);
                scope.spawn(move || {
                    worker::<S, K, F, P, T>(w, paths, progress, next, halt, make, step)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    let mut report = Report {
        workers,
        ..Report::default()
    };
    let mut sinks = Vec::with_capacity(workers);
    let mut failure = None;
    let mut survey = Survey::default();
    for (r, sink, f, s) in outcomes {
        report.files += r.files;
        report.records += r.records;
        report.bytes += r.bytes;
        sinks.extend(sink);
        survey.merge(s);
        if failure.is_none() {
            failure = f;
        }
    }
    report.elapsed = started.elapsed();
    report.allocations = alloc::allocations() - allocations;
    report.allocated_bytes = alloc::allocated_bytes() - allocated_bytes;
    Outcome {
        result: failure.map_or(Ok(report), Err),
        sinks,
        survey,
    }
}

fn worker<S, K, F, P, T>(
    index: usize,
    paths: &[P],
    progress: &Progress<'_>,
    next: &AtomicUsize,
    halt: &AtomicBool,
    make: &F,
    step: &T,
) -> (Report, Option<K>, Option<Failure>, Survey)
where
    S: Source,
    K: for<'n> Sink<'n, S>,
    F: Fn(usize) -> io::Result<K>,
    P: AsRef<Path>,
    T: Fn(&S::Native<'_>, &mut K) -> Result<(), Step<S::NormalizeError>>,
{
    let mut sink = match make(index) {
        Ok(sink) => sink,
        Err(e) => {
            halt.store(true, Ordering::Relaxed);
            let failure = Failure {
                source: S::NAME,
                path: PathBuf::new(),
                record: 0,
                stage: Stage::Write,
                error: Box::new(e),
            };
            return (Report::default(), None, Some(failure), Survey::default());
        }
    };
    let mut report = Report::default();
    let mut survey = Survey::default();
    let mut step = step;
    loop {
        if progress.over() {
            break;
        }
        let i = next.fetch_add(1, Ordering::Relaxed);
        let Some(path) = paths.get(i) else { break };
        match run_file::<S, K, &T>(path.as_ref(), i, &mut sink, progress, &mut step) {
            Ok(Some(run)) => {
                report.files += 1;
                report.records += run.records;
                report.bytes += run.bytes;
                survey.merge(run.survey);
            }
            Ok(None) => break,
            Err(failure) => {
                halt.store(true, Ordering::Relaxed);
                return (report, Some(sink), Some(failure), survey);
            }
        }
    }
    (report, Some(sink), None, survey)
}

/// Two raw files that would write the same output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    #[allow(missing_docs)]
    pub first: PathBuf,
    #[allow(missing_docs)]
    pub second: PathBuf,
    /// The output prefix both would use.
    pub prefix: PathBuf,
}

impl fmt::Display for Collision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} and {} would both write {}",
            self.first.display(),
            self.second.display(),
            self.prefix.display()
        )
    }
}

impl Error for Collision {}

/// Output prefixes for raw files, mirroring their paths below the
/// directory they all share into `out`, without `.gz` and one more
/// extension: `raw/openalex/updated_date=2025-07-25/part_0000.gz` and
/// `raw/openalex/updated_date=2025-07-26/part_0000.gz` become
/// `out/updated_date=2025-07-25/part_0000` and `…26/part_0000`; a lone
/// `raw/pubmed/pubmed26n1334.xml.gz` becomes `out/pubmed26n1334`.
///
/// # Errors
///
/// Two raw files that would name the same output.
pub fn mirror(paths: &[impl AsRef<Path>], out: &Path) -> Result<Vec<PathBuf>, Collision> {
    let parents: Vec<Vec<Component<'_>>> = paths
        .iter()
        .map(|p| {
            p.as_ref()
                .parent()
                .map_or_else(Vec::new, |d| d.components().collect())
        })
        .collect();
    let shared = parents.first().map_or(0, |first| {
        parents.iter().fold(first.len(), |n, p| {
            n.min(p.iter().zip(first).take_while(|(a, b)| a == b).count())
        })
    });
    let mut seen: BTreeMap<PathBuf, usize> = BTreeMap::new();
    let mut prefixes = Vec::with_capacity(paths.len());
    for (i, (path, parent)) in paths.iter().zip(&parents).enumerate() {
        let path = path.as_ref();
        let mut prefix = out.to_path_buf();
        for component in &parent[shared..] {
            prefix.push(component.as_os_str());
        }
        prefix.push(stem(path));
        if let Some(&first) = seen.get(&prefix) {
            return Err(Collision {
                first: paths[first].as_ref().to_path_buf(),
                second: path.to_path_buf(),
                prefix,
            });
        }
        seen.insert(prefix.clone(), i);
        prefixes.push(prefix);
    }
    Ok(prefixes)
}

/// The file name without `.gz` and one more extension:
/// `part_0000.gz` → `part_0000`, `pubmed26n1334.xml.gz` → `pubmed26n1334`.
#[must_use]
pub fn stem(path: &Path) -> OsString {
    let name = Path::new(path.file_name().unwrap_or_default());
    let name = if name.extension().is_some_and(|e| e == "gz") {
        name.file_stem().map_or(name, Path::new)
    } else {
        name
    };
    name.file_stem().unwrap_or(name.as_os_str()).to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fmt;
    use std::io::{self, BufRead, Write};
    use std::ops::ControlFlow;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::{Abandoned, Stage, mirror, normalize, normalize_parallel, parse, parse_parallel};
    use crate::binary::{Binary, Kind, Reader, read_all};
    use crate::frame::{self, ReadError};
    use crate::source::{Count, Sink, Source};

    /// A toy source: one record per line. `3: 4 5` is work 3 citing 4 and
    /// 5; `del 7` withdraws 7; a line starting with `#` fails to parse; a
    /// line that is not a number fails to normalize.
    struct Lines;

    #[derive(Debug)]
    struct BadLine(String);

    impl fmt::Display for BadLine {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "bad line {:?}", self.0)
        }
    }

    impl std::error::Error for BadLine {}

    impl crate::source::Signature for BadLine {
        fn signature(&self) -> String {
            "bad line".to_owned()
        }
    }

    impl Source for Lines {
        const NAME: &'static str = "lines";
        type Native<'a> = &'a str;
        type Id<'n> = u32;
        type Work<'n> = u32;
        type Reference<'n> = u32;
        type ParseError = BadLine;
        type NormalizeError = BadLine;

        fn records<R: BufRead, F>(input: R, mut each: F) -> Result<(), ReadError<BadLine>>
        where
            F: FnMut(&&str) -> ControlFlow<()>,
        {
            frame::lines(input, |_, line| {
                if line.starts_with('#') {
                    Err(BadLine(line.to_owned()))
                } else {
                    Ok(each(&line))
                }
            })
        }

        fn normalize<'n, K: Sink<'n, Self>>(line: &'n &str, sink: &mut K) -> Result<(), BadLine> {
            let bad = || BadLine((*line).to_owned());
            if let Some(id) = line.strip_prefix("del ") {
                sink.deletion(&id.parse().map_err(|_| bad())?);
                return Ok(());
            }
            let (from, targets) = line.split_once(':').ok_or_else(bad)?;
            let from: u32 = from.parse().map_err(|_| bad())?;
            sink.work(&from);
            for to in targets.split_whitespace() {
                sink.reference(&from, &to.parse().map_err(|_| bad())?);
            }
            Ok(())
        }
    }

    static NEXT: AtomicU32 = AtomicU32::new(0);

    fn unique(what: &str, ext: &str) -> PathBuf {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ol-rht-ingest-{what}-{}-{n}{ext}",
            std::process::id()
        ))
    }

    /// Writes `text` to a fresh file in the temp dir, gzipped when asked.
    fn file(text: &str, gz: bool) -> PathBuf {
        let path = unique("in", if gz { ".txt.gz" } else { ".txt" });
        let mut out = std::fs::File::create(&path).unwrap();
        if gz {
            let mut enc = flate2::write::GzEncoder::new(out, flate2::Compression::fast());
            enc.write_all(text.as_bytes()).unwrap();
            enc.finish().unwrap();
        } else {
            out.write_all(text.as_bytes()).unwrap();
        }
        path
    }

    fn total(sinks: &[Count]) -> Count {
        sinks.iter().fold(Count::default(), |mut a, c| {
            a.works += c.works;
            a.references += c.references;
            a.deletions += c.deletions;
            a
        })
    }

    #[test]
    fn counts_works_edges_and_deletions_across_plain_and_gzipped_files() {
        const PLAIN: &str = "1: 2 3\n2:\n";
        const GZ: &str = "3: 1\ndel 9\n";
        let plain = file(PLAIN, false);
        let gz = file(GZ, true);
        let mut count = Count::default();
        let report = normalize::<Lines, _>(&[&plain, &gz], None, &mut count).unwrap();
        assert_eq!((report.files, report.records, report.workers), (2, 4, 1));
        assert_eq!(report.bytes, (PLAIN.len() + GZ.len()) as u64);
        assert_eq!(
            count,
            Count {
                works: 3,
                references: 3,
                deletions: 1
            }
        );
        assert!(
            report
                .to_string()
                .contains("4 records from 2 files on 1 worker,")
        );
        assert_eq!(parse::<Lines>(&[&plain, &gz], None).unwrap().records, 4);
        std::fs::remove_file(plain).unwrap();
        std::fs::remove_file(gz).unwrap();
    }

    #[test]
    fn limit_stops_across_files_before_parsing_the_next_record() {
        let a = file("1: 2\n2: 3\n", false);
        let b = file("#never read\n", false);
        let mut count = Count::default();
        let report = normalize::<Lines, _>(&[&a, &b], Some(2), &mut count).unwrap();
        assert_eq!((report.records, report.files), (2, 1));
        assert_eq!(count.works, 2);
        let mut count = Count::default();
        let report = normalize::<Lines, _>(&[&a, &b], Some(1), &mut count).unwrap();
        assert_eq!((report.records, report.files), (1, 1));
        std::fs::remove_file(a).unwrap();
        std::fs::remove_file(b).unwrap();
    }

    #[test]
    fn the_record_after_the_limit_is_never_parsed() {
        let path = file("1: 2\n#bad\n", false);
        let mut count = Count::default();
        let report = normalize::<Lines, _>(&[&path], Some(1), &mut count).unwrap();
        assert_eq!((report.records, count.works), (1, 1));
        let err = normalize::<Lines, _>(&[&path], None, &mut count).unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Parse, 2));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_truncated_gzip_stream_fails_at_read_after_the_records_before_it() {
        use std::fmt::Write as _;
        let mut text = String::new();
        for i in 0..5000 {
            writeln!(text, "{i}: {}", i * 7).unwrap();
        }
        let path = file(&text, true);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
        let err = parse::<Lines>(&[&path], None).unwrap_err();
        assert_eq!(err.stage, Stage::Read, "{err}");
        assert!(err.record > 0, "records before the break were read: {err}");
        assert!(err.record < 5000, "{err}");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn failures_say_which_file_record_and_stage() {
        let path = file("1: 2\n#nope\n3: 4\n", false);
        let err = parse::<Lines>(&[&path], None).unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Parse, 2));
        assert_eq!(err.source, "lines");
        assert_eq!(err.path, path);
        assert!(
            err.to_string().contains("record 2: bad line \"#nope\""),
            "{err}"
        );
        std::fs::remove_file(&path).unwrap();

        let path = file("1: 2\n2: x\n", false);
        let mut count = Count::default();
        let err = normalize::<Lines, _>(&[&path], None, &mut count).unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Normalize, 2));
        assert_eq!(
            count.works, 2,
            "the sink had the work before its edge failed"
        );
        assert!(err.to_string().starts_with("lines normalize: "), "{err}");
        std::fs::remove_file(&path).unwrap();

        let missing = unique("missing", ".txt");
        let err = parse::<Lines>(&[&missing], None).unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Read, 0));
        assert!(!err.to_string().contains("record"), "{err}");
    }

    #[test]
    fn workers_share_the_files_and_come_back_with_their_sinks() {
        let files: Vec<PathBuf> = (0..6)
            .map(|i| file(&format!("{i}: 1 2\n{i}0:\n"), i % 2 == 0))
            .collect();
        let outcome = normalize_parallel::<Lines, Count, _, _>(&files, None, 3, false, |_| {
            Ok(Count::default())
        });
        let report = outcome.result.unwrap();
        assert_eq!((report.files, report.records, report.workers), (6, 12, 3));
        assert_eq!(outcome.sinks.len(), 3);
        assert_eq!(
            total(&outcome.sinks),
            Count {
                works: 12,
                references: 12,
                deletions: 0
            }
        );
        assert!(report.to_string().contains("on 3 workers,"), "{report}");

        let outcome = normalize_parallel::<Lines, Count, _, _>(&files, Some(5), 3, false, |_| {
            Ok(Count::default())
        });
        assert_eq!(
            outcome.result.unwrap().records,
            5,
            "the limit is exact across workers"
        );
        assert_eq!(total(&outcome.sinks).works, 5);

        let report = parse_parallel::<Lines, _>(&files, None, 64).unwrap();
        assert_eq!(report.workers, 6, "never more workers than files");
        assert_eq!(report.records, 12);
        for f in files {
            std::fs::remove_file(f).unwrap();
        }
    }

    #[test]
    fn a_survey_tallies_normalize_failures_and_goes_on() {
        let a = file("1: 2\n2: x\n3: y\n4:\n", false);
        let b = file("del z\n5: 6\n", false);
        let outcome = normalize_parallel::<Lines, Count, _, _>(&[&a, &b], None, 2, true, |_| {
            Ok(Count::default())
        });
        let report = outcome.result.unwrap();
        assert_eq!(report.records, 6, "failed records still count as read");
        assert_eq!((outcome.survey.failed, outcome.survey.seen.len()), (3, 1));
        let seen = &outcome.survey.seen["bad line"];
        assert_eq!((seen.count, seen.examples.len()), (3, 3));
        assert!(
            outcome.survey.to_string().starts_with(
                "3 records failed, 1 distinct signature\n       3  bad line\n          e.g. bad line"
            ),
            "{}",
            outcome.survey
        );
        assert!(
            outcome.survey.abandoned.is_empty(),
            "a normalize failure costs its record and no more"
        );
        for f in [a, b] {
            std::fs::remove_file(f).unwrap();
        }
    }

    #[test]
    fn a_survey_tallies_a_parse_failure_and_goes_on_to_the_next_file() {
        let a = file("1: 2\n#bad\n3: 4\n", false);
        let b = file("5: 6\n7: 8\n", false);
        // One worker, so the files are read in order and the assertions
        // below are about what one worker did with both.
        let outcome = normalize_parallel::<Lines, Count, _, _>(&[&a, &b], None, 1, true, |_| {
            Ok(Count::default())
        });
        let report = outcome
            .result
            .expect("a parse failure does not stop a survey");
        assert_eq!(report.files, 2, "the next file was still read");
        assert_eq!(
            (report.records, total(&outcome.sinks).works),
            (3, 3),
            "one record before the bad one, two from the next file; \
             the record after the bad one was never read"
        );
        assert_eq!((outcome.survey.failed, outcome.survey.seen.len()), (1, 1));
        assert_eq!(outcome.survey.seen["bad line"].count, 1);
        assert_eq!(
            outcome.survey.abandoned,
            [Abandoned {
                path: a.clone(),
                record: 2
            }],
            "the file and the record it was given up at"
        );
        assert!(!outcome.survey.is_empty(), "a survey that found this fails");
        let report = outcome.survey.to_string();
        assert!(
            report.starts_with("1 record failed, 1 distinct signature, 1 file abandoned\n"),
            "{report}"
        );
        assert!(
            report.contains(&format!(
                "1 file given up on at a record that would not parse; \
                 nothing after that record was read:\n          {} from record 2\n",
                a.display()
            )),
            "{report}"
        );

        // The default path is untouched: without a survey the same file
        // stops the run at the record that would not parse.
        let outcome = normalize_parallel::<Lines, Count, _, _>(&[&a], None, 1, false, |_| {
            Ok(Count::default())
        });
        let err = outcome.result.unwrap_err();
        assert_eq!((err.stage, err.record, &err.path), (Stage::Parse, 2, &a));
        for f in [a, b] {
            std::fs::remove_file(f).unwrap();
        }
    }

    #[test]
    fn a_failure_in_one_file_stops_the_run_and_returns_the_sinks() {
        let files = [
            file("1: 2\n", false),
            file("#bad\n", false),
            file("3: 4\n", false),
        ];
        let outcome = normalize_parallel::<Lines, Count, _, _>(&files, None, 2, false, |_| {
            Ok(Count::default())
        });
        let err = outcome.result.unwrap_err();
        assert_eq!((err.stage, &err.path), (Stage::Parse, &files[1]));
        assert_eq!(outcome.sinks.len(), 2);

        // Only good files here, so the one failure possible is the sink
        // that could not be made; with the bad file in, worker 0 could
        // reach it before worker 1's sink fails.
        let good = [&files[0], &files[2]];
        let outcome = normalize_parallel::<Lines, Count, _, _>(&good, None, 2, false, |w| {
            if w == 0 {
                Ok(Count::default())
            } else {
                Err(io::Error::other("no sink for you"))
            }
        });
        let err = outcome.result.unwrap_err();
        assert_eq!((err.stage, err.record), (Stage::Write, 0));
        assert!(err.to_string().starts_with("lines write: no sink"), "{err}");
        for f in files {
            std::fs::remove_file(f).unwrap();
        }
    }

    #[test]
    fn the_binary_sink_writes_one_set_per_raw_file() {
        let a = file("1: 2 3\ndel 9\n", false);
        let b = file("2:\n", true);
        let out = unique("out", "");
        let prefixes: Arc<[PathBuf]> = mirror(&[&a, &b], &out).unwrap().into();
        let outcome = normalize_parallel::<Lines, _, _, _>(&[&a, &b], None, 2, false, |_| {
            let p = Arc::clone(&prefixes);
            Ok((
                Count::default(),
                Binary::new("lines", 7, move |i, _: &Path| p[i].clone()),
            ))
        });
        outcome.result.unwrap();
        let mut written = Vec::new();
        for (_, binary) in outcome.sinks {
            written.extend(binary.finish().unwrap());
        }
        assert_eq!(written.len(), 2);
        let by_prefix: BTreeMap<_, _> = written
            .iter()
            .map(|w| (w.prefix.clone(), w.records))
            .collect();
        assert_eq!(by_prefix[&prefixes[0]], [1, 2, 1, 0]);
        assert_eq!(by_prefix[&prefixes[1]], [1, 0, 0, 0]);
        assert!(written.iter().all(|w| w.bytes > 0));

        let works = Reader::open(&Kind::Works.path(&prefixes[0])).unwrap();
        assert_eq!(works.header().source, "lines");
        assert_eq!(works.header().model, 7);
        assert_eq!(works.header().kind, Kind::Works);
        let ws: Vec<u32> = works.records().map(Result::unwrap).collect();
        assert_eq!(ws, [1]);
        let refs: Vec<(u32, u32)> = read_all(&Kind::References.path(&prefixes[0])).unwrap();
        assert_eq!(refs, [(1, 2), (1, 3)]);
        let dels: Vec<u32> = read_all(&Kind::Deletions.path(&prefixes[0])).unwrap();
        assert_eq!(dels, [9]);
        let ws: Vec<u32> = read_all(&Kind::Works.path(&prefixes[1])).unwrap();
        assert_eq!(ws, [2]);
        assert!(
            Reader::open(&a).is_err(),
            "a plain text file is not this layout"
        );
        std::fs::remove_dir_all(out).unwrap();
        std::fs::remove_file(a).unwrap();
        std::fs::remove_file(b).unwrap();
    }

    #[test]
    fn a_failed_run_leaves_no_output() {
        let a = file("1: 2\n", false);
        let b = file("bad\n", false);
        let out = unique("out", "");
        let prefixes: Arc<[PathBuf]> = mirror(&[&a, &b], &out).unwrap().into();
        let outcome = normalize_parallel::<Lines, _, _, _>(&[&a, &b], None, 1, false, |_| {
            let p = Arc::clone(&prefixes);
            Ok(Binary::new("lines", 1, move |i, _: &Path| p[i].clone()))
        });
        assert_eq!(outcome.result.unwrap_err().stage, Stage::Normalize);
        let binary = outcome.sinks.into_iter().next().unwrap();
        assert_eq!(
            binary.written().len(),
            1,
            "the first file's set was finished"
        );
        assert!(Kind::Works.path(&prefixes[0]).exists());
        assert!(
            Kind::Works.path(&prefixes[1]).exists(),
            "the failed file's set is still open"
        );
        binary.discard();
        assert!(!Kind::Works.path(&prefixes[0]).exists());
        assert!(!Kind::Works.path(&prefixes[1]).exists());
        std::fs::remove_dir_all(out).unwrap();
        std::fs::remove_file(a).unwrap();
        std::fs::remove_file(b).unwrap();
    }

    #[test]
    fn mirror_names_outputs_below_the_common_directory() {
        let out = Path::new("out");
        assert_eq!(
            mirror(&["data/raw/pubmed/pubmed26n1334.xml.gz"], out).unwrap(),
            [PathBuf::from("out/pubmed26n1334")]
        );
        assert_eq!(
            mirror(
                &[
                    "raw/openalex/updated_date=2025-07-25/part_0000.gz",
                    "raw/openalex/updated_date=2025-07-26/part_0000.gz",
                ],
                out
            )
            .unwrap(),
            [
                PathBuf::from("out/updated_date=2025-07-25/part_0000"),
                PathBuf::from("out/updated_date=2025-07-26/part_0000"),
            ]
        );
        assert_eq!(
            mirror(
                &[
                    "api/sample-random-1.json",
                    "api/sample-has-references-1.json"
                ],
                out
            )
            .unwrap(),
            [
                PathBuf::from("out/sample-random-1"),
                PathBuf::from("out/sample-has-references-1"),
            ]
        );
        let err = mirror(&["a/x.gz", "b/x.gz", "a/x.json"], out).unwrap_err();
        assert_eq!(
            (err.first, err.second, err.prefix),
            (
                PathBuf::from("a/x.gz"),
                PathBuf::from("a/x.json"),
                PathBuf::from("out/a/x")
            )
        );
    }
}
