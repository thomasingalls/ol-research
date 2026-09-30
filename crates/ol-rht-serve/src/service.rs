//! The server: the generated service over anything that [`Answers`], the
//! epoch it serves swappable underneath it, every request bounded by the
//! caps here and every answer stamped with the batch it is as of — the
//! watermark, which the poller advances — and its terms.
//!
//! The answers themselves are shaped by the free functions below, one
//! per call, and the gRPC methods are the thinnest wrappers over them.
//! The JSON surface ([`crate::http`], serving plan §7) calls the same
//! functions and sends the same messages, so a change lands on both
//! transports at once and neither can drift from the other.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use merge::{Sources, WorkId};
use search::Hits;
use serve::{By, Resolved, Walk};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::answers::{Answers, Direction};
use crate::convert;
use crate::lookup;
use crate::proto::v1 as pb;
use crate::proto::v1::serving_server::Serving;
use crate::querylog::{self, Looked, Outcome};
use crate::terms::Terms;

/// Arcs a page may carry.
pub const PAGE_MAX: u32 = 100;
/// Arcs a page carries when the request says zero.
pub const PAGE_DEFAULT: u32 = 50;
/// Hits a search may return.
pub const SEARCH_MAX: u32 = 100;
#[allow(missing_docs)]
pub const SEARCH_DEFAULT: u32 = 10;
/// Facet values per root a search may return.
pub const FACETS_MAX: u32 = 20;
#[allow(missing_docs)]
pub const FACETS_DEFAULT: u32 = 5;
/// Neighbours an expansion may return.
pub const EXPAND_MAX: u32 = 1000;
#[allow(missing_docs)]
pub const EXPAND_DEFAULT: u32 = 20;
/// Seeds an expansion may take: a bound on the request, not on the
/// work, which is a different thing from the bound [`related`] puts on
/// the seeds it gathers for itself (`serve::core::RELATED_ARCS`, on the
/// arcs walked).
pub const SEEDS_MAX: usize = 100;
/// Arcs a path may be long. Ten is already far past what the corpus
/// holds — the held-to-held subgraph's mean out-degree is under one
/// (graph plan §7) — and the cost of a hop is the cost of a ball.
pub const DEPTH_MAX: u32 = 10;
/// The library's own default, so the transport and a direct caller ask
/// the same question of the same store.
pub const DEPTH_DEFAULT: u32 = graph::Bounds::DEFAULT.depth;
/// Paths one answer may carry, each through a different middle.
pub const PATHS_MAX: u32 = 10;
#[allow(missing_docs)]
pub const PATHS_DEFAULT: u32 = 3;
/// Works the two halves of a path search may expand between them before
/// giving up. Not the caller's: a resource guard, not a question.
pub const PATH_VISITS: u64 = graph::Bounds::DEFAULT.visits;
/// How long a search may take, from its request's arrival to its answer,
/// when the server is not told otherwise (`--search-deadline`).
pub const SEARCH_DEADLINE: Duration = Duration::from_secs(10);
/// Bytes a citation may hold. A line of a reference list is a few hundred;
/// a page pasted by mistake is refused rather than read (citation lookup
/// plan, D7).
pub const CITATION_MAX_BYTES: usize = 4 * 1024;

/// Searches that may run at once when the server is not told otherwise
/// (`--search-threads`): half the cores, so that the other half is always
/// free for every other call, the health checks among them, and for the
/// poll that folds a batch.
#[must_use]
pub fn search_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1))
}

/// The server.
pub struct Service<A: Answers> {
    /// The epoch answering, or none while the server waits for an epoch
    /// this build can read (2026-09-24): a server built for a new model
    /// or merge version starts before the store has been rebuilt for it,
    /// and waits, not ready, while the one it replaces keeps answering.
    answers: RwLock<Option<Arc<A>>>,
    terms: Arc<Terms>,
    searches: Searches,
}

/// Where searches run (2026-09-25): never on the threads that answer
/// requests, where until then a search that took minutes held one of
/// them for minutes and a few such held them all, health checks and all;
/// on a thread of their own instead, at most `permits` at once, each
/// within `deadline`.
struct Searches {
    permits: Arc<Semaphore>,
    deadline: Duration,
}

impl Searches {
    fn new(threads: usize, deadline: Duration) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(threads.max(1))),
            deadline,
        }
    }
}

impl<A: Answers> std::fmt::Debug for Service<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service").finish_non_exhaustive()
    }
}

impl<A: Answers> Service<A> {
    /// Serves `answers` under `terms`, searching with the defaults
    /// ([`search_threads`], [`SEARCH_DEADLINE`]).
    pub fn new(answers: A, terms: Terms) -> Self {
        Self {
            answers: RwLock::new(Some(Arc::new(answers))),
            terms: Arc::new(terms),
            searches: Searches::new(search_threads(), SEARCH_DEADLINE),
        }
    }

    /// A server with no epoch yet: every call is refused `UNAVAILABLE`
    /// until [`swap`](Self::swap) gives it one.
    #[must_use]
    pub fn waiting(terms: Terms) -> Self {
        Self {
            answers: RwLock::new(None),
            terms: Arc::new(terms),
            searches: Searches::new(search_threads(), SEARCH_DEADLINE),
        }
    }

    /// Runs at most `threads` searches at once, at least one, each within
    /// `deadline`.
    #[must_use]
    pub fn searching(mut self, threads: usize, deadline: Duration) -> Self {
        self.searches = Searches::new(threads, deadline);
        self
    }

    /// Switches to a new epoch; requests in flight finish on the old one.
    pub fn swap(&self, answers: A) {
        let mut guard = self
            .answers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(Arc::new(answers));
    }

    /// The terms every answer it sends is stamped with.
    pub(crate) fn terms(&self) -> &Terms {
        &self.terms
    }

    /// The epoch answering now, or `UNAVAILABLE` while there is none.
    ///
    /// # Errors
    ///
    /// `UNAVAILABLE` while the server waits for an epoch it can read.
    pub fn current(&self) -> Result<Arc<A>, Status> {
        self.answers
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or_else(|| {
                Status::unavailable(
                    "no epoch this server can read is published yet: the store is being \
                     rebuilt for the version it runs",
                )
            })
    }
}

/// A count from the request, defaulted and capped. Both transports call
/// it, so the caps are one rule.
pub(crate) fn bounded(value: u32, default: u32, max: u32, what: &str) -> Result<usize, Status> {
    let v = if value == 0 { default } else { value };
    if v > max {
        return Err(Status::invalid_argument(format!(
            "{what} {v} is over the cap of {max}"
        )));
    }
    Ok(usize::try_from(v).unwrap_or(usize::MAX))
}

pub(crate) fn status(e: &serve::Error) -> Status {
    match e {
        serve::Error::NotAName(what) => Status::invalid_argument(what.clone()),
        serve::Error::NoEpoch => Status::failed_precondition(e.to_string()),
        // The query log's, not the fault log's.
        serve::Error::Search(search::Error::Deadline) => Status::deadline_exceeded(e.to_string()),
        other => {
            eprintln!("ol-rht-serve: {other}");
            Status::internal(other.to_string())
        }
    }
}

/// The one work a request names.
pub(crate) fn work_id<A: Answers>(a: &A, text: &str) -> Result<WorkId, Status> {
    match a.resolve(text).map_err(|e| status(&e))? {
        Some(Resolved::One(id)) => Ok(id),
        Some(Resolved::Shared(ids)) => Err(Status::failed_precondition(format!(
            "{text} is a shared name, accepted into {} works; resolve it and pick one",
            ids.len()
        ))),
        Some(Resolved::Split { into, .. }) => Err(Status::not_found(format!(
            "{text} was retired by a split; no one work answers for it. It was split into {}",
            into.iter()
                .map(|id| convert::public(a, *id))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ")
        ))),
        None => Err(Status::not_found(format!("{text}: no such work"))),
    }
}

pub(crate) fn answered<A: Answers>(a: &A, terms: &Terms, sources: Sources) -> pb::Answered {
    let watermark = a.watermark();
    pb::Answered {
        epoch: a.epoch().epoch,
        batch: watermark.batch,
        at: watermark.at,
        sources: sources.iter().map(|s| s.name().to_owned()).collect(),
        attribution: terms.attribution(sources),
    }
}

/// `epoch:offset`; a cursor from another epoch is refused.
pub(crate) fn cursor(text: &str, epoch: u64) -> Result<u64, Status> {
    let bad = || Status::invalid_argument(format!("{text:?} is not a cursor"));
    let (e, offset) = text.split_once(':').ok_or_else(bad)?;
    let e: u64 = e.parse().map_err(|_| bad())?;
    if e != epoch {
        return Err(Status::failed_precondition(format!(
            "the cursor is from epoch {e}; the server is at {epoch}; start the list over"
        )));
    }
    offset.parse().map_err(|_| bad())
}

/// Which epoch and batch answers, its counts, and the terms.
pub(crate) fn epoch_info<A: Answers>(a: &A, terms: &Terms) -> pb::EpochInfo {
    let counts = a.counts();
    pb::EpochInfo {
        answered: Some(answered(a, terms, Terms::all())),
        merged: counts.merged,
        nodes: counts.nodes,
        works: counts.works,
        arcs: counts.arcs,
    }
}

/// Which work a name or public id is: one, the several a shared name was
/// accepted into, or the id a split retired with the works its names went
/// to — a path that was once true, answered as what was once so rather
/// than not at all (serving plan §11, item 24). Text that is no name as it
/// stands is read every way it can be ([`found`]).
///
/// # Errors
///
/// Text that is not a name, or names nothing.
pub(crate) fn resolved<A: Answers>(
    a: &A,
    terms: &Terms,
    text: &str,
) -> Result<pb::ResolveResponse, Status> {
    let outcome = match a.resolve(text) {
        Ok(Some(Resolved::One(id))) => {
            pb::resolve_response::Outcome::PublicId(convert::public(a, id)?)
        }
        Ok(Some(Resolved::Shared(ids))) => pb::resolve_response::Outcome::Shared(pb::Shared {
            public_ids: ids
                .into_iter()
                .map(|id| convert::public(a, id))
                .collect::<Result<_, _>>()?,
        }),
        Ok(Some(Resolved::Split { id, into })) => pb::resolve_response::Outcome::Split(pb::Split {
            public_id: convert::public(a, id)?,
            into: into
                .into_iter()
                .map(|part| convert::public(a, part))
                .collect::<Result<_, _>>()?,
        }),
        Ok(None) => return Err(Status::not_found(format!("{text}: no such work"))),
        Err(serve::Error::NotAName(why)) => found(a, text, &why)?,
        Err(e) => return Err(status(&e)),
    };
    Ok(pb::ResolveResponse {
        // No sources, and not for want of asking: this is the one answer
        // the layer cannot attribute today (2026-09-19; §11, item 33).
        // A resolve is answered out of the names artifact, a transducer
        // from a name's text to a `WorkId` and nothing besides (§3), and
        // the registry tail it folds past the epoch is no better — a
        // `mint` or `name` line carries the id, the name and the
        // timestamp, and no source at all. The merged
        // layer does know: `merge::record::NameRow::by` is exactly which
        // sources asserted a name for a work, and `serve::names::build`
        // reads that column and does not keep it.
        //
        // Nothing adjacent stands in for it. The work's `held_by` is who
        // holds records of the work, which is a different claim from who
        // said this name is that work; a shared name has no one work to
        // ask; a split's retired id has no row left to ask at all; and a
        // public id was minted here, so no source asserted it in the
        // first place. So the stamp says what is true — this answer
        // names no source it can stand behind — rather than a plausible
        // set of sources that would read as provenance and not be any.
        answered: Some(answered(a, terms, Sources::NONE)),
        outcome: Some(outcome),
    })
}

/// Which works a citation may be, and why (citation lookup plan, §3): the
/// text capped, its words read by the query parser, and the lookup itself
/// ([`crate::lookup`]) run on a search thread within the search's time
/// limit — half of it is a search, and a search holds no request thread
/// (2026-09-25). Written to the query log whatever became of it.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for text over [`CITATION_MAX_BYTES`] or over the
/// parser's caps; `RESOURCE_EXHAUSTED` when no search thread comes free
/// within the time limit; `DEADLINE_EXCEEDED` for a lookup that runs past
/// it; and the engine's failures, a row that does not decode, a work with
/// no public id.
pub(crate) async fn looked_up<A: Answers>(
    service: &Service<A>,
    a: Arc<A>,
    text: &str,
) -> Result<pb::LookupCitationResponse, Status> {
    let started = Instant::now();
    let log = |outcome, looked: Option<&Looked>| {
        querylog::looked_up(outcome, text, looked, started.elapsed());
    };
    if text.len() > CITATION_MAX_BYTES {
        log(Outcome::TooLong, None);
        return Err(Status::invalid_argument(format!(
            "the citation is {} bytes, over the cap of {CITATION_MAX_BYTES}: one citation at a time",
            text.len()
        )));
    }
    let asked = match query_parser::parse(&citation_match::check::searchable(text)) {
        Ok(asked) => asked,
        Err(e) => {
            log(Outcome::TooLong, None);
            return Err(Status::invalid_argument(e.to_string()));
        }
    };
    let deadline = service.searches.deadline;
    let until = started + deadline;
    let permit = match search_thread(service, until).await {
        Ok(permit) => permit,
        Err((outcome, refusal)) => {
            log(outcome, None);
            return Err(refusal);
        }
    };
    let terms = Arc::clone(&service.terms);
    let owned = text.to_owned();
    // The permit goes with the lookup, as with a search.
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        lookup::lookup(&*a, &terms, &owned, &asked, until)
    });
    match tokio::time::timeout_at(until.into(), task).await {
        Ok(Ok(Ok((response, looked)))) => {
            log(Outcome::Answered, Some(&looked));
            Ok(response)
        }
        Err(_) | Ok(Ok(Err(Stopped::Search(serve::Error::Search(search::Error::Deadline))))) => {
            log(Outcome::TimedOut, None);
            Err(Status::deadline_exceeded(format!(
                "the lookup ran past its time limit of {} s",
                deadline.as_secs()
            )))
        }
        Ok(Ok(Err(Stopped::Search(e)))) => {
            log(Outcome::Failed, None);
            Err(status(&e))
        }
        Ok(Ok(Err(Stopped::Answer(refusal)))) => {
            log(Outcome::Failed, None);
            Err(refusal)
        }
        Ok(Err(stopped)) => {
            eprintln!("ol-rht-serve: a lookup stopped: {stopped}");
            log(Outcome::Failed, None);
            Err(Status::internal(format!("the lookup stopped: {stopped}")))
        }
    }
}

/// A search thread, waited for until `until`; or why there is none, as the
/// log's outcome and the refusal.
async fn search_thread<A: Answers>(
    service: &Service<A>,
    until: Instant,
) -> Result<OwnedSemaphorePermit, (Outcome, Status)> {
    let permits = Arc::clone(&service.searches.permits);
    match tokio::time::timeout_at(until.into(), permits.acquire_owned()).await {
        Ok(Ok(permit)) => Ok(permit),
        Ok(Err(closed)) => {
            eprintln!("ol-rht-serve: search threads: {closed}");
            Err((Outcome::Failed, Status::internal(closed.to_string())))
        }
        Err(_) => {
            eprintln!(
                "ol-rht-serve: every search thread was busy for the whole {} s time limit",
                service.searches.deadline.as_secs()
            );
            Err((
                Outcome::Busy,
                Status::resource_exhausted("every search thread is busy; try again shortly"),
            ))
        }
    }
}

/// Text that is no name as it stands, read every way it can be (the user,
/// 2026-09-28: "'find by id' should not require namespace"): the names the
/// citation reader finds in it, or the whole of it as one token in every
/// namespace ([`lookup::read_names`]), each that names something listed
/// with what it names. A public id written wrong is still refused as
/// written: `ol:` says what it was meant to be, and nothing else is tried.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for a malformed public id, or text of more than one
/// word with no identifier in it; `NOT_FOUND` for a token no namespace
/// holds anything by.
fn found<A: Answers>(
    a: &A,
    text: &str,
    why: &str,
) -> Result<pb::resolve_response::Outcome, Status> {
    let token = text.trim();
    if token
        .get(..3)
        .is_some_and(|p| p.eq_ignore_ascii_case(merge::public::PREFIX))
    {
        return Err(Status::invalid_argument(why.to_owned()));
    }
    let names = lookup::read_names(a, text)?
        .into_iter()
        .filter(lookup::Read::names_something)
        .map(|read| lookup::cited(a, text, read).map(|(name, _)| name))
        .collect::<Result<Vec<_>, _>>()?;
    if !names.is_empty() {
        return Ok(pb::resolve_response::Outcome::Found(pb::Found { names }));
    }
    if token.is_empty() || token.contains(char::is_whitespace) {
        return Err(Status::invalid_argument(format!(
            "{text:?} is no name, and there is no identifier in it: {why}"
        )));
    }
    Err(Status::not_found(format!(
        "{token}: nothing we hold by that, in any namespace"
    )))
}

/// Everything the merge knows about the one work `text` names.
///
/// # Errors
///
/// A name that is no one work, or a row retired or past the epoch.
pub(crate) fn one_work<A: Answers>(
    a: &A,
    terms: &Terms,
    text: &str,
) -> Result<pb::GetWorkResponse, Status> {
    let id = work_id(a, text)?;
    let row = a
        .work(id)
        .map_err(|e| status(&e))?
        .ok_or_else(|| Status::not_found(format!("{text}: retired, or past the epoch")))?;
    let sources = convert::held_by(&row);
    Ok(pb::GetWorkResponse {
        answered: Some(answered(a, terms, sources)),
        work: Some(convert::work(a, &row, terms)?),
    })
}

/// One page of a list, with the cursor that resumes it: `epoch:offset`,
/// empty when the list is done. The streaming transport calls this once
/// a page, the JSON one once a request.
///
/// # Errors
///
/// A work on the page with no public id, or a row that does not decode.
pub(crate) fn one_page<A: Answers>(
    a: &A,
    terms: &Terms,
    id: WorkId,
    direction: Direction,
    offset: u64,
    n: usize,
    hydrate: bool,
) -> Result<pb::WorkPage, Status> {
    let page = a.page(id, direction, offset, n);
    let arcs = convert::arcs(a, &page.arcs, hydrate)?;
    let next = offset + page.arcs.len() as u64;
    let done = page.arcs.is_empty() || next >= page.total;
    Ok(pb::WorkPage {
        answered: Some(answered(a, terms, page.by)),
        arcs,
        cursor: if done {
            String::new()
        } else {
            format!("{}:{next}", a.epoch().epoch)
        },
        total: page.total,
    })
}

/// Works matching a query, with facet counts over the matches: the text
/// read by the query parser, searched on a search thread within the time
/// limit, and written to the query log whatever became of it. Any text
/// within the parser's caps is a query, and whitespace alone is the query
/// that finds nothing.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for text over the parser's caps,
/// `RESOURCE_EXHAUSTED` when no search thread comes free within the time
/// limit, `DEADLINE_EXCEEDED` for a search that runs past it, and the
/// engine's failures and a row that does not decode. The two counts
/// arrive already bounded, because the transports spell them
/// differently: gRPC's `facets` field is JSON's `facet_values`, which is
/// what the number is (the user, 2026-09-19).
pub(crate) async fn searched<A: Answers>(
    service: &Service<A>,
    a: Arc<A>,
    text: &str,
    limit: usize,
    facet_values: usize,
    hydrate: bool,
) -> Result<pb::SearchResponse, Status> {
    let started = Instant::now();
    let deadline = service.searches.deadline;
    let until = started + deadline;
    let log = |outcome, words, hits| querylog::log(outcome, text, words, hits, started.elapsed());
    let asked = match query_parser::parse(text) {
        Ok(asked) => asked,
        Err(e) => {
            log(Outcome::TooLong, None, None);
            return Err(Status::invalid_argument(e.to_string()));
        }
    };
    let words = Some(asked.words().len());
    let permit = match search_thread(service, until).await {
        Ok(permit) => permit,
        Err((outcome, refusal)) => {
            log(outcome, words, None);
            return Err(refusal);
        }
    };
    let terms = Arc::clone(&service.terms);
    // The permit goes with the search and is let go when the search
    // stops, not when its caller stops waiting: a search past its time
    // limit holds its thread until the engine next looks at the clock.
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let hits = a
            .search(&asked, limit, facet_values, until)
            .map_err(Stopped::Search)?;
        answer(&*a, &terms, hits, hydrate).map_err(Stopped::Answer)
    });
    let late = || {
        Status::deadline_exceeded(format!(
            "the search ran past its time limit of {} s",
            deadline.as_secs()
        ))
    };
    match tokio::time::timeout_at(until.into(), task).await {
        Ok(Ok(Ok(response))) => {
            log(Outcome::Answered, words, Some(response.total));
            Ok(response)
        }
        Err(_) | Ok(Ok(Err(Stopped::Search(serve::Error::Search(search::Error::Deadline))))) => {
            log(Outcome::TimedOut, words, None);
            Err(late())
        }
        Ok(Ok(Err(Stopped::Search(e)))) => {
            log(Outcome::Failed, words, None);
            Err(status(&e))
        }
        Ok(Ok(Err(Stopped::Answer(refusal)))) => {
            log(Outcome::Failed, words, None);
            Err(refusal)
        }
        Ok(Err(stopped)) => {
            eprintln!("ol-rht-serve: a search stopped: {stopped}");
            log(Outcome::Failed, words, None);
            Err(Status::internal(format!("the search stopped: {stopped}")))
        }
    }
}

/// Why a search or a lookup on its thread gave no answer: the engine, or
/// the answer made from what it found.
pub(crate) enum Stopped {
    Search(serve::Error),
    Answer(Status),
}

/// The response for `hits`.
fn answer<A: Answers>(
    a: &A,
    terms: &Terms,
    hits: Hits,
    hydrate: bool,
) -> Result<pb::SearchResponse, Status> {
    Ok(pb::SearchResponse {
        answered: Some(answered(a, terms, Terms::all())),
        total: hits.total as u64,
        hits: convert::hits(a, &hits.hits, hydrate)?,
        facets: hits
            .facets
            .into_iter()
            .map(|(root, values)| pb::Facet {
                root,
                values: values
                    .into_iter()
                    .map(|(value, count)| pb::FacetValue { value, count })
                    .collect(),
            })
            .collect(),
    })
}

/// The directions by the names a transport carrying text spells them
/// with, which is the JSON surface's [`crate::http::Walk`] and the words
/// the refusal below lists.
pub(crate) const WALKS: [&str; 3] = ["both", "references", "citers"];

/// Which arcs a hop follows, from the protocol's enum. Both transports
/// name it the same way; an enum value the protocol does not have is
/// refused, never quietly taken for the default.
pub(crate) fn walk(code: i32) -> Result<Walk, Status> {
    match pb::Walk::try_from(code) {
        Ok(pb::Walk::Both) => Ok(Walk::Both),
        Ok(pb::Walk::References) => Ok(Walk::References),
        Ok(pb::Walk::Citers) => Ok(Walk::Citers),
        Err(_) => Err(Status::invalid_argument(format!(
            "walk {code} is not a direction; {} expected",
            WALKS.join(", ")
        ))),
    }
}

/// One hop out from a seed set along `walk`, best-scored first.
///
/// # Errors
///
/// No seeds or more than the cap, a budget over its cap, a direction the
/// protocol does not have, or a seed that names no one work.
pub(crate) fn expanded<A: Answers>(
    a: &A,
    terms: &Terms,
    seeds: &[String],
    walk: Walk,
    budget: u32,
    hydrate: bool,
) -> Result<pb::ExpandResponse, Status> {
    if seeds.is_empty() || seeds.len() > SEEDS_MAX {
        return Err(Status::invalid_argument(format!(
            "{} seeds; one to {SEEDS_MAX} expected",
            seeds.len()
        )));
    }
    let budget = bounded(budget, EXPAND_DEFAULT, EXPAND_MAX, "budget")?;
    let seeds = seeds
        .iter()
        .map(|s| work_id(a, s))
        .collect::<Result<Vec<_>, _>>()?;
    let found = a.expand(&seeds, walk, budget, None);
    Ok(pb::ExpandResponse {
        answered: Some(answered(a, terms, Terms::all())),
        neighbours: convert::neighbours(a, &found.neighbours, hydrate)?,
        total: found.total,
    })
}

/// The two questions by the names a transport carrying text spells them
/// with, which is the JSON surface's [`crate::http::RelatedBy`] and the
/// words the refusal below lists.
pub(crate) const BYS: [&str; 2] = ["cites-same", "cited-alongside"];

/// Which neighbourhood, from the protocol's enum; an enum value the
/// protocol does not have is refused rather than quietly taken for the
/// default, as a walk is.
pub(crate) fn by(code: i32) -> Result<By, Status> {
    match pb::By::try_from(code) {
        Ok(pb::By::CitesSame) => Ok(By::CitesSame),
        Ok(pb::By::CitedAlongside) => Ok(By::CitedAlongside),
        Err(_) => Err(Status::invalid_argument(format!(
            "by {code} is not a neighbourhood; {} expected",
            BYS.join(", ")
        ))),
    }
}

/// What else cites what a work cites (`cites-same`), or what else is
/// cited by what cites it (`cited-alongside`): one call where the client
/// otherwise makes two — a page of a list, then that page handed back as
/// seeds.
///
/// Two calls is not merely chatty, it is a different question. The seeds
/// are then whichever page the client happened to be holding, so a work
/// with 882 citers is answered about the fifty of them the client had
/// fetched, and a client that paged twice gets a different answer than
/// one that paged once. Here the seeds are the work's own list, gathered
/// by the server and bounded by the arcs walking them would read, with
/// [`pb::RelatedResponse::seeds_used`] and `seeds_total` on the answer
/// saying whether the bound bit ([`serve::Serving::related`] for what is
/// dropped when it does, and why those).
///
/// # Errors
///
/// A limit over its cap, a neighbourhood the protocol does not have, or
/// a work that is no one work.
pub(crate) fn related<A: Answers>(
    a: &A,
    terms: &Terms,
    work: &str,
    by: By,
    limit: u32,
    hydrate: bool,
) -> Result<pb::RelatedResponse, Status> {
    let budget = bounded(limit, EXPAND_DEFAULT, EXPAND_MAX, "limit")?;
    let id = work_id(a, work)?;
    let found = a.related(id, by, budget);
    Ok(pb::RelatedResponse {
        answered: Some(answered(a, terms, Terms::all())),
        related: convert::neighbours(a, &found.neighbours, hydrate)?,
        seeds_used: found.seeds_used,
        total: found.total,
        seeds_total: found.seeds_total,
    })
}

/// The shortest directed paths from one work to another: "does `from`
/// cite something that cites … `to`". The outcome is a `oneof` and not a
/// list, because no path within the bound is the ordinary answer here
/// and has to read as an answer — the held-to-held subgraph's mean
/// out-degree is 0.63 on the epoch of 2026-09-19 — and because a search
/// that ran out of budget is a third thing again, which an empty list
/// could not say.
///
/// # Errors
///
/// A depth or a path count over its cap, or an end that names no one
/// work.
pub(crate) fn pathed<A: Answers>(
    a: &A,
    terms: &Terms,
    from: &str,
    to: &str,
    depth: u32,
    paths: u32,
    hydrate: bool,
) -> Result<pb::PathResponse, Status> {
    let bounds = graph::Bounds {
        depth: u32::try_from(bounded(depth, DEPTH_DEFAULT, DEPTH_MAX, "depth")?)
            .unwrap_or(DEPTH_MAX),
        paths: bounded(paths, PATHS_DEFAULT, PATHS_MAX, "paths")?,
        visits: PATH_VISITS,
    };
    let (start, end) = (work_id(a, from)?, work_id(a, to)?);
    let mut by = Sources::NONE;
    let outcome = match a.path(start, end, bounds) {
        graph::Found::Paths {
            hops,
            paths,
            meetings,
            ..
        } => {
            let mut wire = Vec::with_capacity(paths.len());
            for path in &paths {
                for step in path {
                    for source in step.by.iter() {
                        by.insert(source);
                    }
                }
                wire.push(pb::Path {
                    works: convert::steps(a, path, hydrate)?,
                });
            }
            pb::path_response::Outcome::Found(pb::Paths {
                hops,
                paths: wire,
                total: meetings,
            })
        }
        graph::Found::None { depth, reached } => pb::path_response::Outcome::None(pb::NoPath {
            depth,
            forward: reached.forward,
            backward: reached.backward,
        }),
        graph::Found::Stopped {
            depth,
            proved,
            visits,
            reached,
        } => pb::path_response::Outcome::Stopped(pb::PathStopped {
            depth,
            proved,
            visits,
            forward: reached.forward,
            backward: reached.backward,
        }),
    };
    Ok(pb::PathResponse {
        answered: Some(answered(a, terms, by)),
        outcome: Some(outcome),
    })
}

type PageStream = ReceiverStream<Result<pb::WorkPage, Status>>;

impl<A: Answers> Service<A> {
    /// A list as a stream of pages: [`one_page`] called until the list
    /// runs out or the request's own limit does. A limit reached ends
    /// the stream with no cursor — what was asked for was sent, and
    /// there is nothing to resume.
    fn list(
        &self,
        req: &pb::ListRequest,
        direction: Direction,
    ) -> Result<Response<PageStream>, Status> {
        let a = self.current()?;
        let terms = self.terms.clone();
        let id = work_id(&*a, &req.work)?;
        let page = bounded(req.page, PAGE_DEFAULT, PAGE_MAX, "page")?;
        let mut offset = if req.cursor.is_empty() {
            0
        } else {
            cursor(&req.cursor, a.epoch().epoch)?
        };
        let limit = if req.limit == 0 { u64::MAX } else { req.limit };
        let hydrate = req.hydrate;
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            let mut sent = 0u64;
            loop {
                let n = page.min(usize::try_from(limit - sent).unwrap_or(usize::MAX));
                let mut message = match one_page(&*a, &terms, id, direction, offset, n, hydrate) {
                    Ok(message) => message,
                    Err(status) => {
                        let _ = tx.send(Err(status)).await;
                        return;
                    }
                };
                let got = message.arcs.len() as u64;
                offset += got;
                sent += got;
                let done = message.cursor.is_empty() || sent >= limit;
                if done {
                    message.cursor.clear();
                }
                if tx.send(Ok(message)).await.is_err() || done {
                    return;
                }
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tonic::async_trait]
impl<A: Answers> Serving for Service<A> {
    async fn epoch(&self, _: Request<pb::EpochRequest>) -> Result<Response<pb::EpochInfo>, Status> {
        let a = self.current()?;
        Ok(Response::new(epoch_info(&*a, &self.terms)))
    }

    async fn resolve(
        &self,
        req: Request<pb::ResolveRequest>,
    ) -> Result<Response<pb::ResolveResponse>, Status> {
        let a = self.current()?;
        Ok(Response::new(resolved(
            &*a,
            &self.terms,
            &req.into_inner().text,
        )?))
    }

    async fn lookup_citation(
        &self,
        req: Request<pb::LookupCitationRequest>,
    ) -> Result<Response<pb::LookupCitationResponse>, Status> {
        let a = self.current()?;
        Ok(Response::new(
            looked_up(self, a, &req.into_inner().text).await?,
        ))
    }

    async fn get_work(
        &self,
        req: Request<pb::GetWorkRequest>,
    ) -> Result<Response<pb::GetWorkResponse>, Status> {
        let a = self.current()?;
        Ok(Response::new(one_work(
            &*a,
            &self.terms,
            &req.into_inner().work,
        )?))
    }

    type ReferencesStream = PageStream;

    async fn references(
        &self,
        req: Request<pb::ListRequest>,
    ) -> Result<Response<PageStream>, Status> {
        self.list(&req.into_inner(), Direction::References)
    }

    type CitersStream = PageStream;

    async fn citers(&self, req: Request<pb::ListRequest>) -> Result<Response<PageStream>, Status> {
        self.list(&req.into_inner(), Direction::Citers)
    }

    async fn search(
        &self,
        req: Request<pb::SearchRequest>,
    ) -> Result<Response<pb::SearchResponse>, Status> {
        let a = self.current()?;
        let req = req.into_inner();
        let limit = bounded(req.limit, SEARCH_DEFAULT, SEARCH_MAX, "limit")?;
        let facets = bounded(req.facets, FACETS_DEFAULT, FACETS_MAX, "facets")?;
        Ok(Response::new(
            searched(self, a, &req.query, limit, facets, req.hydrate).await?,
        ))
    }

    async fn expand(
        &self,
        req: Request<pb::ExpandRequest>,
    ) -> Result<Response<pb::ExpandResponse>, Status> {
        let a = self.current()?;
        let req = req.into_inner();
        Ok(Response::new(expanded(
            &*a,
            &self.terms,
            &req.seeds,
            walk(req.walk)?,
            req.budget,
            req.hydrate,
        )?))
    }

    async fn related(
        &self,
        req: Request<pb::RelatedRequest>,
    ) -> Result<Response<pb::RelatedResponse>, Status> {
        let a = self.current()?;
        let req = req.into_inner();
        Ok(Response::new(related(
            &*a,
            &self.terms,
            &req.work,
            by(req.by)?,
            req.limit,
            req.hydrate,
        )?))
    }

    async fn path(
        &self,
        req: Request<pb::PathRequest>,
    ) -> Result<Response<pb::PathResponse>, Status> {
        let a = self.current()?;
        let req = req.into_inner();
        Ok(Response::new(pathed(
            &*a,
            &self.terms,
            &req.from,
            &req.to,
            req.depth,
            req.paths,
            req.hydrate,
        )?))
    }
}
