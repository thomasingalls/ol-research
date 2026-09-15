//! `ingest`: a download straight through the framer and the normalizer,
//! in one pass and on one thread per file (plan §3.2, §3.5). Bytes arrive
//! from the network, go to `<path>.part` and the hashers, are inflated,
//! framed, parsed, normalized and written — nothing waits for the
//! transfer to finish, and nothing is handed between threads: one worker
//! owns one file from first byte to receipt, so workers never contend
//! with each other or with themselves. When the stream ends the file is
//! verified as `fetch` would, renamed and receipted, and only then is its
//! output set closed. A file that fails verification, or whose transfer
//! the record limit cut short, gets no receipt and no output set; its
//! partial bytes stay for a resume.

use std::error::Error as StdError;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Chain, Read, Take, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use flate2::bufread::MultiGzDecoder;

use crate::fetch::{self, Hashers, Promised, Run, Transfer, begin, etag_md5, part_of};
use crate::http::{self, Client};
use crate::listing::Entry;
use crate::receipt::{self, Receipt};
use crate::run::{
    Failure, FileRun, Outcome, Progress, Report, Stage, Step, Survey, failure, normalize_step,
    run_stream,
};
use crate::source::{Sink, Source};
use crate::{alloc, time};

/// How much of a gzipped transfer the inflater reads at once.
const FILE_BUFFER: usize = 256 * 1024;

/// What a run of `ingest` did.
#[derive(Debug)]
pub struct Ingested<K> {
    /// The runner's outcome: report or failure, and every worker's sink.
    pub outcome: Outcome<K>,
    /// The receipts written, in completion order.
    pub receipts: Vec<Receipt>,
    /// Entries that already had a matching receipt and file.
    pub skipped: usize,
    /// A transfer the run stopped inside — the record limit, or another
    /// worker's failure: no receipt, no output set, partial bytes kept.
    pub cut_short: Option<PathBuf>,
}

/// Tees the response body to the partial file and the hashers, counting.
struct Tee<'a> {
    body: ureq::BodyReader<'static>,
    file: File,
    hashers: &'a mut Hashers,
    received: u64,
}

impl Read for Tee<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.body.read(buf)?;
        if n > 0 {
            self.file.write_all(&buf[..n])?;
            self.hashers.update(&buf[..n]);
            self.received += n as u64;
        }
        Ok(n)
    }
}

/// The bytes in order: what a partial file already holds, replayed from
/// disk, then the network from where it left off — so a resumed transfer
/// still runs through the framer from its first byte.
type Bytes<'a> = Chain<Take<File>, Tee<'a>>;

/// The transfer as the framer reads it: inflated when the file is gzipped.
enum Input<'a> {
    Plain(Bytes<'a>),
    Gz(MultiGzDecoder<BufReader<Bytes<'a>>>),
}

impl Read for Input<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(bytes) => bytes.read(buf),
            Self::Gz(decoder) => decoder.read(buf),
        }
    }
}

impl<'a> Input<'a> {
    fn into_tee(self) -> Tee<'a> {
        let bytes = match self {
            Self::Plain(bytes) => bytes,
            Self::Gz(decoder) => decoder.into_inner().into_inner(),
        };
        bytes.into_inner().1
    }
}

/// How one entry ended.
#[allow(clippy::large_enum_variant)] // one at a time, never stored
enum One {
    /// The run was over before the entry's first record; nothing started.
    Skipped,
    /// The run stopped inside the transfer: the record limit, or another
    /// worker's failure.
    CutShort(FileRun, PathBuf),
    /// Transferred, verified, receipted, and its output set closed.
    Done(FileRun, Receipt),
}

/// Fetches and normalizes every entry that has no matching receipt,
/// `workers` at a time, calling `done` after each receipt is written.
/// `limit` is a record limit, exact across workers, as `normalize`'s.
///
/// # Errors
///
/// Torrent entries or an unreceipted file, before anything starts. A
/// transfer, verification, parse, normalize or write failure comes back
/// in the outcome, with the run stopped.
#[allow(clippy::too_many_arguments)] // a run's knobs, each named
pub fn ingest_parallel<S, K, F, D>(
    run: &Run<'_>,
    entries: &[Entry],
    client: &Client,
    workers: usize,
    limit: Option<u64>,
    make: F,
    done: D,
) -> Result<Ingested<K>, fetch::Error>
where
    S: Source,
    K: for<'n> Sink<'n, S> + Send,
    F: Fn(usize) -> io::Result<K> + Sync,
    D: Fn(&Entry, &Receipt) + Sync,
{
    let (todo, skipped) = fetch::plan(run, entries)?;
    let workers = workers.clamp(1, todo.len().max(1));
    let started = Instant::now();
    let (allocations, allocated_bytes) = (alloc::allocations(), alloc::allocated_bytes());
    let (next, count, halt) = (
        AtomicUsize::new(0),
        AtomicU64::new(0),
        AtomicBool::new(false),
    );
    let progress = Progress::new(limit, &count, &halt, false);
    let shared = Shared {
        receipts_path: run.store.receipts(run.source),
        appending: Mutex::new(()),
        written: Mutex::new(Vec::new()),
        cut_short: Mutex::new(None),
    };
    let outcomes: Vec<(Report, Option<K>, Option<Failure>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let (todo, progress, next, halt, make, done, shared) =
                    (&todo, &progress, &next, &halt, &make, &done, &shared);
                scope.spawn(move || {
                    worker::<S, K, F, D>(
                        w, run, todo, client, progress, next, halt, make, done, shared,
                    )
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
    let mut failed = None;
    for (r, sink, f) in outcomes {
        report.files += r.files;
        report.records += r.records;
        report.bytes += r.bytes;
        sinks.extend(sink);
        if failed.is_none() {
            failed = f;
        }
    }
    report.elapsed = started.elapsed();
    report.allocations = alloc::allocations() - allocations;
    report.allocated_bytes = alloc::allocated_bytes() - allocated_bytes;
    Ok(Ingested {
        outcome: Outcome {
            result: failed.map_or(Ok(report), Err),
            sinks,
            survey: Survey::default(),
        },
        receipts: shared
            .written
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner),
        skipped,
        cut_short: shared
            .cut_short
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner),
    })
}

/// What the workers share besides the queue.
struct Shared {
    receipts_path: PathBuf,
    appending: Mutex<()>,
    written: Mutex<Vec<Receipt>>,
    cut_short: Mutex<Option<PathBuf>>,
}

#[allow(clippy::too_many_arguments)] // the run's shared state, borrowed
fn worker<S, K, F, D>(
    index: usize,
    run: &Run<'_>,
    todo: &[(usize, &Entry)],
    client: &Client,
    progress: &Progress<'_>,
    next: &AtomicUsize,
    halt: &AtomicBool,
    make: &F,
    done: &D,
    shared: &Shared,
) -> (Report, Option<K>, Option<Failure>)
where
    S: Source,
    K: for<'n> Sink<'n, S>,
    F: Fn(usize) -> io::Result<K>,
    D: Fn(&Entry, &Receipt),
{
    let mut sink = match make(index) {
        Ok(sink) => sink,
        Err(e) => {
            halt.store(true, Ordering::Relaxed);
            return (
                Report::default(),
                None,
                Some(failure::<S>(Path::new(""), 0, Stage::Write, Box::new(e))),
            );
        }
    };
    let mut report = Report::default();
    let mut step = normalize_step::<S, K>;
    loop {
        if progress.over() {
            break;
        }
        let i = next.fetch_add(1, Ordering::Relaxed);
        let Some(&(listed, entry)) = todo.get(i) else {
            break;
        };
        match ingest_one::<S, K, _>(
            run, entry, listed, client, &mut sink, progress, &mut step, shared,
        ) {
            Ok(One::Done(file_run, receipt)) => {
                report.files += 1;
                report.records += file_run.records;
                report.bytes += file_run.bytes;
                done(entry, &receipt);
                shared
                    .written
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(receipt);
            }
            Ok(One::CutShort(file_run, path)) => {
                report.files += 1;
                report.records += file_run.records;
                report.bytes += file_run.bytes;
                shared
                    .cut_short
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .get_or_insert(path);
                break;
            }
            Ok(One::Skipped) => break,
            Err(failure) => {
                halt.store(true, Ordering::Relaxed);
                return (report, Some(sink), Some(failure));
            }
        }
    }
    (report, Some(sink), None)
}

/// One entry: sidecar, transfer through the framer and normalizer,
/// checks, rename, receipt, and the output set closed.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // one transfer, start to receipt
fn ingest_one<S, K, T>(
    run: &Run<'_>,
    entry: &Entry,
    index: usize,
    client: &Client,
    sink: &mut K,
    progress: &Progress<'_>,
    step: &mut T,
    shared: &Shared,
) -> Result<One, Failure>
where
    S: Source,
    K: for<'n> Sink<'n, S>,
    T: FnMut(&S::Native<'_>, &mut K) -> Result<(), Step<S::NormalizeError>>,
{
    let path = run.store.raw(run.source, &entry.path);
    let part = part_of(&path);
    let fail = |record: u64, stage: Stage, error: Box<dyn StdError + Send + Sync>| {
        failure::<S>(&path, record, stage, error)
    };
    let fetch_fail = |e: fetch::Error| fail(0, Stage::Fetch, Box::new(e));
    let io = |error: io::Error| fetch::Error::Io {
        path: part.clone(),
        error,
    };
    let sidecar = fetch::sidecar(entry, client).map_err(fetch_fail)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| fetch_fail(io(e)))?;
    }
    if !progress.claim() {
        return Ok(One::Skipped);
    }
    sink.begin_file(&path, index)
        .map_err(|e| fail(0, Stage::Write, Box::new(e)))?;
    let fetched_at = time::now();
    let started = Instant::now();
    let Transfer {
        response,
        mut hashers,
        resumed_from,
    } = begin(entry, &part, client).map_err(fetch_fail)?;
    let etag = http::header(&response, "etag");
    let last_modified = http::header(&response, "last-modified");
    let (_, body) = response.into_parts();
    let content_length = body.content_length();
    // On a resume the partial file is read back first — through its own
    // handle, up to the bytes the response continues from — then appended
    // to as the rest arrives.
    let (replay, file) = if let Some(existing) = resumed_from {
        (
            File::open(&part)
                .map_err(|e| fetch_fail(io(e)))?
                .take(existing),
            OpenOptions::new()
                .append(true)
                .open(&part)
                .map_err(|e| fetch_fail(io(e)))?,
        )
    } else {
        let file = File::create(&part).map_err(|e| fetch_fail(io(e)))?;
        (
            File::open(&part).map_err(|e| fetch_fail(io(e)))?.take(0),
            file,
        )
    };
    let tee = Tee {
        body: body.into_reader(),
        file,
        hashers: &mut hashers,
        received: resumed_from.unwrap_or(0),
    };
    let bytes = replay.chain(tee);
    let gz = Path::new(&entry.path)
        .extension()
        .is_some_and(|e| e == "gz");
    let mut input = if gz {
        Input::Gz(MultiGzDecoder::new(BufReader::with_capacity(
            FILE_BUFFER,
            bytes,
        )))
    } else {
        Input::Plain(bytes)
    };
    let file_run = run_stream::<S, K, T>(&mut input, &path, sink, progress, step)?;
    let Tee {
        body,
        file,
        received,
        hashers: _,
    } = input.into_tee();
    drop(body);
    if file_run.stopped {
        drop(file);
        return Ok(One::CutShort(file_run, path));
    }
    file.sync_all().map_err(|e| fetch_fail(io(e)))?;
    drop(file);
    let (sha256, md5) = hashers.finish();
    let promised = Promised {
        content_length: content_length.map(|n| n + resumed_from.unwrap_or(0)),
        listing_bytes: entry.bytes,
        sidecar,
        etag_md5: etag.as_deref().and_then(etag_md5),
    };
    let checks = promised
        .verify(received, &md5)
        .map_err(|(check, expected, actual)| {
            let _ = fs::remove_file(&part);
            fetch_fail(fetch::Error::Verify {
                path: path.clone(),
                check,
                expected,
                actual,
            })
        })?;
    fs::rename(&part, &path).map_err(|e| fetch_fail(io(e)))?;
    let receipt = Receipt {
        path: entry.path.clone(),
        url: entry.url.clone(),
        bytes: received,
        sha256,
        md5,
        fetched_at,
        took_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        checks,
        resumed_from,
        etag,
        last_modified,
        listing: Some(run.listing.display().to_string()),
        note: None,
    };
    {
        let _guard = shared
            .appending
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        receipt::append(&shared.receipts_path, &receipt)
    }
    .map_err(|error| {
        fetch_fail(fetch::Error::Io {
            path: shared.receipts_path.clone(),
            error,
        })
    })?;
    sink.end_file()
        .map_err(|e| fail(file_run.records, Stage::Write, Box::new(e)))?;
    Ok(One::Done(file_run, receipt))
}
