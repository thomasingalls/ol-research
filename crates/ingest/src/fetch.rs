//! `fetch`: every entry of a listing that has no receipt yet, into the
//! store, verified against everything the source and the response offer,
//! then receipted (plan §3.5). Each file is its own transaction — bytes
//! stream into `<path>.part`, are hashed on the way, checked, synced,
//! renamed, and only then receipted — so a run that stops at its first
//! failure leaves the files before it complete and the one in flight
//! resumable. Nothing discovers a URL here: the listing is the input.
//!
//! A lookup entry (plan §8, item 8) is an API page answering a batch of
//! ids: paced, one request in flight, asked once more after the wait a
//! 429 asks for, kept whole and never resumed, receipted like any file.

use std::error::Error as StdError;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use md5::Md5;
use sha2::{Digest, Sha256};

use ureq::Body;
use ureq::http::Response;

use crate::http::{self, Client};
use crate::listing::{Entry, Kind};
use crate::receipt::{self, Check, Receipt};
use crate::store::Store;
use crate::time;

/// How much is read from the response at once.
pub(crate) const CHUNK: usize = 256 * 1024;

/// How long a lookup waits after a 429 or 503 with no usable
/// `Retry-After`.
const RETRY_DEFAULT: Duration = Duration::from_secs(30);
/// The longest `Retry-After` a lookup honours; past it the run stops
/// and says when to come back.
const RETRY_LIMIT: Duration = Duration::from_secs(300);

/// What a run of `fetch` did.
#[derive(Debug, Default)]
pub struct Fetched {
    /// The receipts written, in completion order.
    pub receipts: Vec<Receipt>,
    /// Entries that already had a matching receipt and file.
    pub skipped: usize,
    #[allow(missing_docs)]
    pub elapsed: Duration,
}

/// Why a run stopped.
#[derive(Debug)]
pub enum Error {
    /// The listing holds torrent entries, which `fetch` does not do.
    Torrent(Vec<Entry>),
    /// A raw file exists with no receipt: its provenance is unknown, and
    /// `fetch` neither overwrites nor adopts it.
    Unreceipted(PathBuf),
    #[allow(missing_docs)]
    Http(http::Error),
    /// I/O on a store path.
    Io {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        error: io::Error,
    },
    /// The MD5 sidecar was not what NCBI writes.
    Sidecar {
        #[allow(missing_docs)]
        url: String,
        #[allow(missing_docs)]
        reason: String,
    },
    /// A resumed transfer got a response that does not continue it.
    Range {
        #[allow(missing_docs)]
        url: String,
        #[allow(missing_docs)]
        content_range: Option<String>,
    },
    /// A lookup request that was not answered with a page, after the one
    /// wait a `Retry-After` may ask for.
    Lookup {
        #[allow(missing_docs)]
        url: String,
        #[allow(missing_docs)]
        status: u16,
        /// The `Retry-After` the response carried, verbatim.
        retry_after: Option<String>,
    },
    /// Lookups given to `ingest`, which keeps no page a parser rejects:
    /// fetch them, then normalize.
    LookupIngest,
    /// The bytes received are not what was promised.
    Verify {
        #[allow(missing_docs)]
        path: PathBuf,
        #[allow(missing_docs)]
        check: Check,
        #[allow(missing_docs)]
        expected: String,
        #[allow(missing_docs)]
        actual: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Torrent(entries) => write!(
                f,
                "{} torrent entr{} in the listing; fetch does not do torrents",
                entries.len(),
                if entries.len() == 1 { "y" } else { "ies" }
            ),
            Self::Unreceipted(path) => write!(
                f,
                "{} exists without a receipt; remove it or adopt it, fetch will not overwrite it",
                path.display()
            ),
            Self::Http(e) => write!(f, "{e}"),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
            Self::Sidecar { url, reason } => write!(f, "{url}: {reason}"),
            Self::Range { url, content_range } => write!(
                f,
                "{url}: resumed, but the response does not continue the file (Content-Range {content_range:?})"
            ),
            Self::Lookup {
                url,
                status,
                retry_after,
            } => {
                write!(f, "{url}: HTTP {status}")?;
                match retry_after {
                    Some(after) => write!(f, ", Retry-After {after}"),
                    None => Ok(()),
                }
            }
            Self::LookupIngest => f.write_str(
                "the listing holds lookups, which are fetched, then normalized: a page the parser rejects must stay on disk",
            ),
            Self::Verify {
                path,
                check,
                expected,
                actual,
            } => write!(
                f,
                "{}: {check:?} failed: expected {expected}, got {actual}",
                path.display()
            ),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Http(e) => Some(e),
            Self::Io { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Self {
        Self::Http(e)
    }
}

/// One run: which entries to fetch, where, and how.
#[derive(Debug)]
pub struct Run<'a> {
    #[allow(missing_docs)]
    pub store: &'a Store,
    /// [`Source::NAME`](crate::Source::NAME).
    pub source: &'a str,
    /// The listing the entries came from, recorded in each receipt.
    pub listing: &'a Path,
    /// Parallel transfers; clamped to the entries to fetch.
    pub workers: usize,
    /// At most this many transfers this run.
    pub limit: Option<usize>,
    /// For lookups: the least interval between two requests
    /// ([`Lister::PACE`](crate::list::Lister::PACE)).
    pub pace: Duration,
    /// For lookups: the query parameter carrying a contact address,
    /// `mailto=…`, when one was given on the command line; never in a
    /// listing or a receipt.
    pub contact: Option<String>,
}

/// Fetches every entry that has no matching receipt, `workers` at a time,
/// calling `done` after each receipt is written. Stops at the first
/// failure; what completed before it stays.
///
/// # Errors
///
/// Torrent entries or an unreceipted file, before anything is fetched;
/// then the first transfer that failed.
pub fn fetch<F>(
    run: &Run<'_>,
    entries: &[Entry],
    client: &Client,
    done: F,
) -> Result<Fetched, Error>
where
    F: Fn(&Entry, &Receipt) + Sync,
{
    let started = Instant::now();
    let (todo, skipped) = plan(run, entries)?;
    let receipts_path = run.store.receipts(run.source);
    // Lookups go one at a time: one request in flight per provider.
    let workers = if todo.iter().any(|(_, e)| e.kind == Kind::Lookup) {
        1
    } else {
        run.workers.clamp(1, todo.len().max(1))
    };
    let pace = Pace::new(run.pace);
    let next = AtomicUsize::new(0);
    let halt = AtomicBool::new(false);
    let failure: Mutex<Option<Error>> = Mutex::new(None);
    let written: Mutex<Vec<Receipt>> = Mutex::new(Vec::new());
    let appending = Mutex::new(());
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while !halt.load(Ordering::Relaxed) {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((_, entry)) = todo.get(i) else { break };
                    match fetch_one(run, entry, client, &pace) {
                        Ok(receipt) => {
                            let appended = {
                                let _guard =
                                    appending.lock().unwrap_or_else(PoisonError::into_inner);
                                receipt::append(&receipts_path, &receipt)
                            };
                            if let Err(error) = appended {
                                halt.store(true, Ordering::Relaxed);
                                let mut f = failure.lock().unwrap_or_else(PoisonError::into_inner);
                                f.get_or_insert(Error::Io {
                                    path: receipts_path.clone(),
                                    error,
                                });
                                break;
                            }
                            done(entry, &receipt);
                            written
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .push(receipt);
                        }
                        Err(e) => {
                            halt.store(true, Ordering::Relaxed);
                            failure
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .get_or_insert(e);
                            break;
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = failure.into_inner().unwrap_or_else(PoisonError::into_inner) {
        return Err(e);
    }
    Ok(Fetched {
        receipts: written.into_inner().unwrap_or_else(PoisonError::into_inner),
        skipped,
        elapsed: started.elapsed(),
    })
}

/// Entries with their index in the listing.
pub(crate) type Todo<'e> = Vec<(usize, &'e Entry)>;

/// The entries to fetch — those with no matching receipt and file, in
/// listing order with their listing index, up to the run's limit — and
/// how many were skipped.
///
/// # Errors
///
/// Torrent entries, or a raw file without a receipt; nothing is fetched.
pub(crate) fn plan<'e>(run: &Run<'_>, entries: &'e [Entry]) -> Result<(Todo<'e>, usize), Error> {
    let torrents: Vec<Entry> = entries
        .iter()
        .filter(|e| e.kind == Kind::Torrent)
        .cloned()
        .collect();
    if !torrents.is_empty() {
        return Err(Error::Torrent(torrents));
    }
    let receipts_path = run.store.receipts(run.source);
    let receipts = receipt::read(&receipts_path).map_err(|error| Error::Io {
        path: receipts_path.clone(),
        error,
    })?;
    let latest = receipt::latest(&receipts);
    let mut todo = Vec::new();
    let mut skipped = 0;
    for (index, entry) in entries.iter().enumerate() {
        let path = run.store.raw(run.source, &entry.path);
        let on_disk = fs::metadata(&path).ok().map(|m| m.len());
        match (latest.get(entry.path.as_str()), on_disk) {
            (Some(receipt), Some(len))
                if receipt.url == entry.url
                    && receipt.bytes == len
                    && entry.bytes.is_none_or(|b| b == len) =>
            {
                skipped += 1;
            }
            (None, Some(_)) => return Err(Error::Unreceipted(path)),
            _ => todo.push((index, entry)),
        }
    }
    if let Some(limit) = run.limit {
        todo.truncate(limit);
    }
    Ok((todo, skipped))
}

/// The entry's MD5 sidecar, fetched and read, when the listing names one.
pub(crate) fn sidecar(entry: &Entry, client: &Client) -> Result<Option<String>, Error> {
    match &entry.md5_url {
        Some(url) => Ok(Some(
            sidecar_md5(&client.text(url)?, name_of(&entry.path)).map_err(|reason| {
                Error::Sidecar {
                    url: url.clone(),
                    reason,
                }
            })?,
        )),
        None => Ok(None),
    }
}

/// Hashes as the bytes go by.
pub(crate) struct Hashers {
    sha256: Sha256,
    md5: Md5,
}

impl Hashers {
    pub(crate) fn new() -> Self {
        Self {
            sha256: Sha256::new(),
            md5: Md5::new(),
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        self.sha256.update(bytes);
        self.md5.update(bytes);
    }

    /// The SHA-256 and the MD5, hex.
    pub(crate) fn finish(self) -> (String, String) {
        (hex(&self.sha256.finalize()), hex(&self.md5.finalize()))
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// A transfer that has started: the response, and the partial file's
/// bytes already hashed when it resumes.
pub(crate) struct Transfer {
    pub(crate) response: Response<Body>,
    pub(crate) hashers: Hashers,
    /// Bytes on disk the response continues from.
    pub(crate) resumed_from: Option<u64>,
}

/// Starts the transfer of `entry`, continuing a partial file when there
/// is one and the server agrees to; otherwise from the start.
pub(crate) fn begin(entry: &Entry, part: &Path, client: &Client) -> Result<Transfer, Error> {
    let io = |error| Error::Io {
        path: part.to_path_buf(),
        error,
    };
    let fresh = |client: &Client| -> Result<Transfer, Error> {
        Ok(Transfer {
            response: client.get(&entry.url, None)?,
            hashers: Hashers::new(),
            resumed_from: None,
        })
    };
    let existing = fs::metadata(part).map_or(0, |m| m.len());
    if existing == 0 {
        return fresh(client);
    }
    let mut hashers = Hashers::new();
    rehash(part, &mut hashers).map_err(io)?;
    match client.get(&entry.url, Some(existing)) {
        Ok(response) if response.status() == ureq::http::StatusCode::PARTIAL_CONTENT => {
            let content_range = http::header(&response, "content-range");
            if !content_range
                .as_deref()
                .is_some_and(|r| r.starts_with(&format!("bytes {existing}-")))
            {
                return Err(Error::Range {
                    url: entry.url.clone(),
                    content_range,
                });
            }
            Ok(Transfer {
                response,
                hashers,
                resumed_from: Some(existing),
            })
        }
        // The server ignored the range: start over.
        Ok(response) => Ok(Transfer {
            response,
            hashers: Hashers::new(),
            resumed_from: None,
        }),
        // Nothing to resume from that the server agrees with.
        Err(http::Error {
            kind: http::ErrorKind::Status(416),
            ..
        }) => {
            fs::remove_file(part).map_err(io)?;
            fresh(client)
        }
        Err(e) => Err(e.into()),
    }
}

/// Everything the listing, the sidecar and the response promised about
/// the bytes; each present promise is checked.
pub(crate) struct Promised {
    /// `Content-Length` plus what was already on disk.
    pub(crate) content_length: Option<u64>,
    pub(crate) listing_bytes: Option<u64>,
    /// The sidecar's MD5, hex.
    pub(crate) sidecar: Option<String>,
    /// The `ETag` as MD5, hex, when it is one.
    pub(crate) etag_md5: Option<String>,
}

impl Promised {
    /// The checks that passed, or the first that did not with what it
    /// expected and what it got.
    pub(crate) fn verify(
        &self,
        received: u64,
        md5: &str,
    ) -> Result<Vec<Check>, (Check, String, String)> {
        let promises = [
            (
                Check::ContentLength,
                self.content_length.map(|n| n.to_string()),
                received.to_string(),
            ),
            (
                Check::ListingBytes,
                self.listing_bytes.map(|n| n.to_string()),
                received.to_string(),
            ),
            (Check::Md5Sidecar, self.sidecar.clone(), md5.to_owned()),
            (Check::EtagMd5, self.etag_md5.clone(), md5.to_owned()),
        ];
        let mut passed = Vec::new();
        for (check, expected, actual) in promises {
            let Some(expected) = expected else { continue };
            if expected != actual {
                return Err((check, expected, actual));
            }
            passed.push(check);
        }
        Ok(passed)
    }
}

/// One entry: sidecar, transfer (resumed if a partial file exists),
/// checks, rename, receipt.
fn fetch_one(run: &Run<'_>, entry: &Entry, client: &Client, pace: &Pace) -> Result<Receipt, Error> {
    if entry.kind == Kind::Lookup {
        return fetch_page(run, entry, client, pace);
    }
    let path = run.store.raw(run.source, &entry.path);
    let part = part_of(&path);
    let io = |error| Error::Io {
        path: part.clone(),
        error,
    };
    let sidecar = sidecar(entry, client)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let fetched_at = time::now();
    let started = Instant::now();
    let Transfer {
        response,
        mut hashers,
        resumed_from,
    } = begin(entry, &part, client)?;
    let mut received = resumed_from.unwrap_or(0);
    let etag = http::header(&response, "etag");
    let last_modified = http::header(&response, "last-modified");
    let (_, body) = response.into_parts();
    let content_length = body.content_length();
    let mut file = if resumed_from.is_some() {
        OpenOptions::new().append(true).open(&part).map_err(io)?
    } else {
        File::create(&part).map_err(io)?
    };
    let mut reader = body.into_reader();
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| {
            Error::Http(http::Error {
                url: entry.url.clone(),
                kind: http::ErrorKind::Body(e),
            })
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&chunk[..n]).map_err(io)?;
        hashers.update(&chunk[..n]);
        received += n as u64;
    }
    file.sync_all().map_err(io)?;
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
            Error::Verify {
                path: path.clone(),
                check,
                expected,
                actual,
            }
        })?;
    fs::rename(&part, &path).map_err(io)?;
    Ok(Receipt {
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
    })
}

/// The pace of lookups: at least `min` between one request and the next,
/// run-wide.
pub(crate) struct Pace {
    min: Duration,
    last: Mutex<Option<Instant>>,
}

impl Pace {
    pub(crate) const fn new(min: Duration) -> Self {
        Self {
            min,
            last: Mutex::new(None),
        }
    }

    /// Waits until `min` has passed since the last request, then counts
    /// this one.
    pub(crate) fn wait(&self) {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = *last {
            let since = at.elapsed();
            if since < self.min {
                thread::sleep(self.min.saturating_sub(since));
            }
        }
        *last = Some(Instant::now());
    }
}

/// One lookup: paced, asked once more after the wait a 429 or 503 asks
/// for, kept whole, receipted with what the response promised. A partial
/// page from an earlier run is overwritten, never resumed.
fn fetch_page(
    run: &Run<'_>,
    entry: &Entry,
    client: &Client,
    pace: &Pace,
) -> Result<Receipt, Error> {
    let path = run.store.raw(run.source, &entry.path);
    let part = part_of(&path);
    let io = |error| Error::Io {
        path: part.clone(),
        error,
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let url = with_contact(&entry.url, run.contact.as_deref());
    let fetched_at = time::now();
    let started = Instant::now();
    let response = answer(&url, entry, client, pace)?;
    let etag = http::header(&response, "etag");
    let last_modified = http::header(&response, "last-modified");
    let (_, body) = response.into_parts();
    let content_length = body.content_length();
    let mut file = File::create(&part).map_err(io)?;
    let mut reader = body.into_reader();
    let mut hashers = Hashers::new();
    let mut received = 0u64;
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| {
            Error::Http(http::Error {
                url: entry.url.clone(),
                kind: http::ErrorKind::Body(e),
            })
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&chunk[..n]).map_err(io)?;
        hashers.update(&chunk[..n]);
        received += n as u64;
    }
    file.sync_all().map_err(io)?;
    drop(file);
    let (sha256, md5) = hashers.finish();
    let promised = Promised {
        content_length,
        listing_bytes: None,
        sidecar: None,
        etag_md5: None,
    };
    let checks = promised
        .verify(received, &md5)
        .map_err(|(check, expected, actual)| {
            let _ = fs::remove_file(&part);
            Error::Verify {
                path: path.clone(),
                check,
                expected,
                actual,
            }
        })?;
    fs::rename(&part, &path).map_err(io)?;
    Ok(Receipt {
        path: entry.path.clone(),
        url: entry.url.clone(),
        bytes: received,
        sha256,
        md5,
        fetched_at,
        took_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        checks,
        resumed_from: None,
        etag,
        last_modified,
        listing: Some(run.listing.display().to_string()),
        note: None,
    })
}

/// The response to a lookup: the request, paced; on a 429 or 503, the
/// wait `Retry-After` asks for, then the request once more. Anything but
/// a 200 after that is the error.
fn answer(url: &str, entry: &Entry, client: &Client, pace: &Pace) -> Result<Response<Body>, Error> {
    pace.wait();
    let response = client.request(url)?;
    let status = response.status().as_u16();
    if status == 200 {
        return Ok(response);
    }
    let retry_after = http::header(&response, "retry-after");
    let refused = |status, retry_after| Error::Lookup {
        url: entry.url.clone(),
        status,
        retry_after,
    };
    if status != 429 && status != 503 {
        return Err(refused(status, retry_after));
    }
    let Some(wait) = retry_wait(retry_after.as_deref()) else {
        return Err(refused(status, retry_after));
    };
    drop(response);
    thread::sleep(wait);
    pace.wait();
    let response = client.request(url)?;
    let status = response.status().as_u16();
    if status == 200 {
        return Ok(response);
    }
    let retry_after = http::header(&response, "retry-after");
    Err(refused(status, retry_after))
}

/// How long to wait before asking again: what `Retry-After` says when it
/// is a number of seconds within the limit, the default when it is
/// absent or a date, and `None` when it is past the limit — the run
/// stops and says so.
fn retry_wait(retry_after: Option<&str>) -> Option<Duration> {
    match retry_after.map(str::trim) {
        None => Some(RETRY_DEFAULT),
        Some(text) => match text.parse::<u64>() {
            Ok(secs) if secs <= RETRY_LIMIT.as_secs() => Some(Duration::from_secs(secs)),
            Ok(_) => None,
            Err(_) => Some(RETRY_DEFAULT),
        },
    }
}

/// `url` with the contact parameter appended, when there is one.
fn with_contact(url: &str, contact: Option<&str>) -> String {
    match contact {
        Some(contact) => {
            let joiner = if url.contains('?') { '&' } else { '?' };
            format!("{url}{joiner}{contact}")
        }
        None => url.to_owned(),
    }
}

/// `<path>.part`.
pub(crate) fn part_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

/// The file name of an entry path.
fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Feeds a partial file's bytes to the hashers before resuming after them.
fn rehash(part: &Path, hashers: &mut Hashers) -> io::Result<()> {
    let mut file = File::open(part)?;
    let mut chunk = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        hashers.update(&chunk[..n]);
    }
}

/// `MD5(pubmed26n1334.xml.gz)= ea5f99dc…`, as NCBI writes a sidecar; the
/// name must be the file's.
fn sidecar_md5(text: &str, name: &str) -> Result<String, String> {
    let line = text.trim();
    let rest = line
        .strip_prefix("MD5(")
        .ok_or_else(|| format!("does not start with MD5(: {line:?}"))?;
    let (named, hex) = rest
        .split_once(")= ")
        .ok_or_else(|| format!("no ')= ' separator: {line:?}"))?;
    if named != name {
        return Err(format!("names {named:?}, not {name:?}"));
    }
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("not 32 hex digits: {hex:?}"));
    }
    Ok(hex.to_ascii_lowercase())
}

/// An `ETag` that is an MD5 — S3's for single-part objects — as hex.
pub(crate) fn etag_md5(etag: &str) -> Option<String> {
    let hex = etag.trim().trim_matches('"');
    (hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use std::time::{Duration, Instant};

    use super::{Pace, etag_md5, hex, name_of, part_of, retry_wait, sidecar_md5, with_contact};

    #[test]
    fn sidecars_and_etags_are_read_strictly() {
        let text = "MD5(pubmed26n1334.xml.gz)= EA5F99DC5A597B55E05E9129BC401F9D\n";
        assert_eq!(
            sidecar_md5(text, "pubmed26n1334.xml.gz").unwrap(),
            "ea5f99dc5a597b55e05e9129bc401f9d"
        );
        assert!(
            sidecar_md5(text, "pubmed26n1335.xml.gz")
                .unwrap_err()
                .contains("names")
        );
        assert!(
            sidecar_md5("ea5f99dc5a597b55e05e9129bc401f9d", "x")
                .unwrap_err()
                .contains("MD5(")
        );
        assert!(
            sidecar_md5("MD5(x)= abc", "x")
                .unwrap_err()
                .contains("32 hex")
        );
        assert_eq!(
            etag_md5("\"dec333abf34c775ead7cd61a415ef5df\"").as_deref(),
            Some("dec333abf34c775ead7cd61a415ef5df")
        );
        assert_eq!(etag_md5("\"dec333abf34c775ead7cd61a415ef5df-3\""), None);
        assert_eq!(etag_md5("W/\"abc\""), None);
    }

    #[test]
    fn paths_and_hex() {
        assert_eq!(
            part_of(Path::new("raw/x/a/b.gz")),
            Path::new("raw/x/a/b.gz.part")
        );
        assert_eq!(
            name_of("baseline/pubmed26n1334.xml.gz"),
            "pubmed26n1334.xml.gz"
        );
        assert_eq!(name_of("manifest.json"), "manifest.json");
        assert_eq!(hex(&[0, 15, 255]), "000fff");
    }

    #[test]
    fn lookups_are_paced_and_wait_what_retry_after_says() {
        let pace = Pace::new(Duration::from_millis(30));
        let started = Instant::now();
        pace.wait();
        pace.wait();
        assert!(started.elapsed() >= Duration::from_millis(30));
        assert_eq!(retry_wait(None), Some(Duration::from_secs(30)));
        assert_eq!(retry_wait(Some(" 7 ")), Some(Duration::from_secs(7)));
        assert_eq!(retry_wait(Some("300")), Some(Duration::from_secs(300)));
        assert_eq!(retry_wait(Some("301")), None, "past the limit: stop");
        assert_eq!(
            retry_wait(Some("Wed, 21 Oct 2026 07:28:00 GMT")),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            with_contact("https://x/works?filter=a", Some("mailto=a%40b")),
            "https://x/works?filter=a&mailto=a%40b"
        );
        assert_eq!(
            with_contact("https://x/works", Some("mailto=a%40b")),
            "https://x/works?mailto=a%40b"
        );
        assert_eq!(with_contact("https://x/works?a", None), "https://x/works?a");
    }
}
