//! The server: the generated service over anything that [`Answers`], the
//! epoch it serves swappable underneath it, every request bounded by the
//! caps here and every answer stamped with the batch it is as of — the
//! watermark, which the poller advances — and its terms.

use std::sync::{Arc, RwLock};

use merge::{Sources, WorkId};
use serve::Resolved;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::answers::{Answers, Direction};
use crate::convert;
use crate::proto::v1 as pb;
use crate::proto::v1::serving_server::Serving;
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
/// Seeds an expansion may take.
pub const SEEDS_MAX: usize = 100;

/// The server.
pub struct Service<A: Answers> {
    answers: RwLock<Arc<A>>,
    terms: Arc<Terms>,
}

impl<A: Answers> std::fmt::Debug for Service<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service").finish_non_exhaustive()
    }
}

impl<A: Answers> Service<A> {
    /// Serves `answers` under `terms`.
    pub fn new(answers: A, terms: Terms) -> Self {
        Self {
            answers: RwLock::new(Arc::new(answers)),
            terms: Arc::new(terms),
        }
    }

    /// Switches to a new epoch; requests in flight finish on the old one.
    pub fn swap(&self, answers: A) {
        let mut guard = self
            .answers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Arc::new(answers);
    }

    /// The epoch answering now.
    #[must_use]
    pub fn current(&self) -> Arc<A> {
        self.answers
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// A count from the request, defaulted and capped.
fn bounded(value: u32, default: u32, max: u32, what: &str) -> Result<usize, Status> {
    let v = if value == 0 { default } else { value };
    if v > max {
        return Err(Status::invalid_argument(format!(
            "{what} {v} is over the cap of {max}"
        )));
    }
    Ok(usize::try_from(v).unwrap_or(usize::MAX))
}

fn status(e: &serve::Error) -> Status {
    match e {
        serve::Error::NotAName(what) => Status::invalid_argument(what.clone()),
        serve::Error::NoEpoch => Status::failed_precondition(e.to_string()),
        other => {
            eprintln!("ol-rht-serve: {other}");
            Status::internal(other.to_string())
        }
    }
}

/// The one work a request names.
fn work_id<A: Answers>(a: &A, text: &str) -> Result<WorkId, Status> {
    match a.resolve(text).map_err(|e| status(&e))? {
        Some(Resolved::One(id)) => Ok(id),
        Some(Resolved::Shared(ids)) => Err(Status::failed_precondition(format!(
            "{text} is a shared name, accepted into {} works; resolve it and pick one",
            ids.len()
        ))),
        Some(Resolved::Split(_)) => Err(Status::not_found(format!(
            "{text} was retired by a split; no survivor answers for it"
        ))),
        None => Err(Status::not_found(format!("{text}: no such work"))),
    }
}

fn answered<A: Answers>(a: &A, terms: &Terms, sources: Sources) -> pb::Answered {
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
fn cursor(text: &str, epoch: u64) -> Result<u64, Status> {
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

type PageStream = ReceiverStream<Result<pb::WorkPage, Status>>;

impl<A: Answers> Service<A> {
    fn list(
        &self,
        req: &pb::ListRequest,
        direction: Direction,
    ) -> Result<Response<PageStream>, Status> {
        let a = self.current();
        let terms = self.terms.clone();
        let id = work_id(&*a, &req.work)?;
        let page = bounded(req.page, PAGE_DEFAULT, PAGE_MAX, "page")?;
        let epoch = a.epoch().epoch;
        let mut offset = if req.cursor.is_empty() {
            0
        } else {
            cursor(&req.cursor, epoch)?
        };
        let limit = if req.limit == 0 { u64::MAX } else { req.limit };
        let hydrate = req.hydrate;
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            let mut sent = 0u64;
            loop {
                let n = page.min(usize::try_from(limit - sent).unwrap_or(usize::MAX));
                let p = a.page(id, direction, offset, n);
                let arcs = match convert::arcs(&*a, &p.arcs, hydrate) {
                    Ok(arcs) => arcs,
                    Err(status) => {
                        let _ = tx.send(Err(status)).await;
                        return;
                    }
                };
                let got = p.arcs.len() as u64;
                offset += got;
                sent += got;
                let done = got == 0 || offset >= p.total || sent >= limit;
                let message = pb::WorkPage {
                    answered: Some(answered(&*a, &terms, p.by)),
                    arcs,
                    cursor: if done {
                        String::new()
                    } else {
                        format!("{epoch}:{offset}")
                    },
                    total: p.total,
                };
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
        let a = self.current();
        let counts = a.counts();
        Ok(Response::new(pb::EpochInfo {
            answered: Some(answered(&*a, &self.terms, Terms::all())),
            merged: counts.merged,
            nodes: counts.nodes,
            works: counts.works,
            arcs: counts.arcs,
        }))
    }

    async fn resolve(
        &self,
        req: Request<pb::ResolveRequest>,
    ) -> Result<Response<pb::ResolveResponse>, Status> {
        let a = self.current();
        let text = req.into_inner().text;
        let outcome = match a.resolve(&text).map_err(|e| status(&e))? {
            Some(Resolved::One(id)) => {
                pb::resolve_response::Outcome::PublicId(convert::public(&*a, id)?)
            }
            Some(Resolved::Shared(ids)) => pb::resolve_response::Outcome::Shared(pb::Shared {
                public_ids: ids
                    .into_iter()
                    .map(|id| convert::public(&*a, id))
                    .collect::<Result<_, _>>()?,
            }),
            Some(Resolved::Split(id)) => pb::resolve_response::Outcome::Split(pb::Split {
                public_id: convert::public(&*a, id)?,
            }),
            None => return Err(Status::not_found(format!("{text}: no such work"))),
        };
        Ok(Response::new(pb::ResolveResponse {
            answered: Some(answered(&*a, &self.terms, Sources::NONE)),
            outcome: Some(outcome),
        }))
    }

    async fn get_work(
        &self,
        req: Request<pb::GetWorkRequest>,
    ) -> Result<Response<pb::GetWorkResponse>, Status> {
        let a = self.current();
        let text = req.into_inner().work;
        let id = work_id(&*a, &text)?;
        let row = a
            .work(id)
            .map_err(|e| status(&e))?
            .ok_or_else(|| Status::not_found(format!("{text}: retired, or past the epoch")))?;
        let sources = convert::held_by(&row);
        Ok(Response::new(pb::GetWorkResponse {
            answered: Some(answered(&*a, &self.terms, sources)),
            work: Some(convert::work(&*a, &row, &self.terms)?),
        }))
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
        let a = self.current();
        let req = req.into_inner();
        let limit = bounded(req.limit, SEARCH_DEFAULT, SEARCH_MAX, "limit")?;
        let facets = bounded(req.facets, FACETS_DEFAULT, FACETS_MAX, "facets")?;
        let hits = a
            .search(&req.query, limit, facets)
            .map_err(|e| status(&e))?;
        Ok(Response::new(pb::SearchResponse {
            answered: Some(answered(&*a, &self.terms, Terms::all())),
            total: hits.total as u64,
            hits: hits
                .hits
                .into_iter()
                .map(|h| {
                    Ok(pb::Hit {
                        public_id: convert::public(&*a, h.id)?,
                        score: h.score,
                        title: h.title.unwrap_or_default(),
                    })
                })
                .collect::<Result<_, Status>>()?,
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
        }))
    }

    async fn expand(
        &self,
        req: Request<pb::ExpandRequest>,
    ) -> Result<Response<pb::ExpandResponse>, Status> {
        let a = self.current();
        let req = req.into_inner();
        if req.seeds.is_empty() || req.seeds.len() > SEEDS_MAX {
            return Err(Status::invalid_argument(format!(
                "{} seeds; one to {SEEDS_MAX} expected",
                req.seeds.len()
            )));
        }
        let budget = bounded(req.budget, EXPAND_DEFAULT, EXPAND_MAX, "budget")?;
        let seeds = req
            .seeds
            .iter()
            .map(|s| work_id(&*a, s))
            .collect::<Result<Vec<_>, _>>()?;
        let neighbours = a
            .expand(&seeds, budget)
            .into_iter()
            .map(|(id, touches)| convert::neighbour(&*a, id, touches, req.hydrate))
            .collect::<Result<_, _>>()?;
        Ok(Response::new(pb::ExpandResponse {
            answered: Some(answered(&*a, &self.terms, Terms::all())),
            neighbours,
        }))
    }
}
