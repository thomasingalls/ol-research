//! The JSON surface (serving plan §7: "a JSON facade over the same core
//! later if wanted, on `axum` 0.8"): the query core again, for a browser,
//! which cannot speak gRPC over `fetch`. Nothing is shaped twice — every
//! route calls the same [`crate::service`] function its gRPC method
//! calls and answers with the same message, serialized as JSON instead
//! of protobuf, so the two transports cannot drift apart.
//!
//! Nine routes under `/v1`, one per call the protocol has. The two
//! streaming calls answer one page a request instead, resumed by the
//! cursor the page carries — `epoch:offset`, the cursor the stream
//! sends, refused the same way from another epoch (§10, item 20, on what
//! an offset means across a fold).
//!
//! Fail fast: a parameter a route does not know, a count over its cap, a
//! flag that is neither `true` nor `false`, a path that is not a route —
//! each is a refusal naming what was wrong, never a default quietly
//! taken. A refusal carries the gRPC code by name and the reason, under
//! the HTTP status that code maps to, so both surfaces say the same
//! thing, each in its own vocabulary.
//!
//! Each route declares what it takes, as a struct [`crate::query`] reads
//! the query string into. That declaration is the only list of the
//! names: what is refused is what it does not have, and the TypeScript
//! the browser client builds its URLs from is generated from it
//! ([`crate::ts`]), so a client that compiles is a client this surface
//! will answer. It was written out three times until 2026-09-19, and the
//! three fell out of step.

// Every route is a thin call into the synchronous query core, which the
// gRPC methods call the same way; there is nothing to await inside one.
#![allow(clippy::unused_async)]

use std::fmt;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router, middleware};
use serde::Deserialize;
use tonic::{Code, Status};

use crate::answers::{Answers, Direction};
use crate::proto::v1 as pb;
use crate::query::Params;
use crate::service::{self, Service};

/// The one list of the routes, read three ways: by `axum` as the router
/// ([`routed`]), by the refusal that names them ([`ROUTES`]), and by the
/// TypeScript as the map a client indexes by (`each_route`, walked from
/// [`crate::ts`]). It was three lists until 2026-09-19, and a route in
/// one and not another was a misleading 404 or a path no client could
/// write.
///
/// A row is the path, the handler, what the handler reads the query
/// string into and the message it answers with. The two types are not
/// taken on trust: [`held`] fails to compile when they are not the
/// handler's own.
macro_rules! routes {
    ($($path:literal => $handler:ident($params:ty) -> $response:ty,)+) => {
        fn routed<A: Answers>() -> Router<Arc<Service<A>>> {
            Router::new()
                $(.route($path, get(held::<A, $params, $response, _, _, _>($handler::<A>))))+
        }

        /// The paths, for the refusal that names them.
        pub(crate) const ROUTES: &[&str] = &[$($path),+];

        /// Every route's path and two types, in the router's order.
        #[cfg(test)]
        pub(crate) fn each_route(visit: &mut impl crate::ts::EachRoute) {
            $(visit.route::<$params, $response>($path);)+
        }
    };
}

// Under `/v1`: one per call in the protocol, every one a GET, since
// every one is a question. A work is named in a parameter and never in
// the path — a name is `doi:10.1000/x`, with a slash in it, and a path
// segment would have to carry it twice-encoded.
routes! {
    "/v1/epoch" => epoch(EpochParams) -> pb::EpochInfo,
    "/v1/resolve" => resolve(ResolveParams) -> pb::ResolveResponse,
    "/v1/work" => work(WorkParams) -> pb::GetWorkResponse,
    "/v1/references" => references(ListParams) -> pb::WorkPage,
    "/v1/citers" => citers(ListParams) -> pb::WorkPage,
    "/v1/search" => search(SearchParams) -> pb::SearchResponse,
    "/v1/expand" => expand(ExpandParams) -> pb::ExpandResponse,
    "/v1/related" => related(RelatedParams) -> pb::RelatedResponse,
    "/v1/path" => path(PathParams) -> pb::PathResponse,
}

/// What a handler may answer with, for a route whose message is `R`.
trait Body<R> {}
impl<R> Body<R> for Json<R> {}
impl<R> Body<R> for Result<Json<R>, Refusal> {}

/// The handler, unchanged — having been held to the row it is listed in:
/// it reads `P` and answers with `R`, or this does not compile.
fn held<A, P, R, H, F, O>(handler: H) -> H
where
    A: Answers,
    H: Fn(State<Arc<Service<A>>>, Params<P>) -> F,
    F: Future<Output = O>,
    O: Body<R>,
{
    handler
}

/// The routes, with what is not one refused by name and the origins
/// allowed to ask.
pub fn router<A: Answers>(service: Arc<Service<A>>, origins: Origins) -> Router {
    routed::<A>()
        .fallback(no_route)
        .method_not_allowed_fallback(no_method)
        .layer(middleware::from_fn_with_state(origins, cors))
        .with_state(service)
}

/// Serves the routes on `listener` until the task is dropped.
///
/// # Errors
///
/// The listener failing.
pub async fn serve<A: Answers>(
    listener: tokio::net::TcpListener,
    service: Arc<Service<A>>,
    origins: Origins,
) -> std::io::Result<()> {
    axum::serve(listener, router(service, origins)).await
}

// --- the routes -----------------------------------------------------------
//
// Each route declares the parameters it takes as a struct, which is the
// whole of what it takes: `crate::query` refuses anything else by that
// declaration, and `crate::ts` generates the TypeScript the browser
// client builds its URLs from out of the same struct, so the names are
// written once. What a struct cannot say is a range, so the caps stay in
// `service::bounded`, called here with the name this transport spells
// the parameter by.

/// A count the request left out, in the spelling [`service::bounded`]
/// reads as "the default": zero. The protocol's own spelling too —
/// prost writes an unset `uint32` as zero — so both transports hand the
/// shared cap the same thing.
fn count(given: Option<u32>) -> u32 {
    given.unwrap_or(0)
}

/// What `/v1/epoch` takes: nothing. Declared all the same, so that a
/// parameter sent to it is refused by the rule every other route is
/// refused by, rather than by this one remembering to ask.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(::ts_rs::TS), ts(export_to = "serving.ts"))]
pub(crate) struct EpochParams {}

/// `GET /v1/epoch` — which epoch and batch the server answers from, its
/// counts, and the terms.
async fn epoch<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    _: Params<EpochParams>,
) -> Result<Json<pb::EpochInfo>, Refusal> {
    let a = service.current()?;
    Ok(Json(service::epoch_info(&*a, service.terms())))
}

/// What `/v1/resolve` takes.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(::ts_rs::TS), ts(export_to = "serving.ts"))]
pub(crate) struct ResolveParams {
    /// A name — `doi:10.1000/x`, `pmid:12345678` — or a public id.
    text: String,
}

/// `GET /v1/resolve?text=` — which work a name or public id is.
async fn resolve<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<ResolveParams>,
) -> Result<Json<pb::ResolveResponse>, Refusal> {
    let a = service.current()?;
    Ok(Json(service::resolved(&*a, service.terms(), &params.text)?))
}

/// What `/v1/work` takes.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(::ts_rs::TS), ts(export_to = "serving.ts"))]
pub(crate) struct WorkParams {
    /// The work, by public id or by any name that points at it.
    work: String,
}

/// `GET /v1/work?work=` — everything the merge knows about one work.
async fn work<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<WorkParams>,
) -> Result<Json<pb::GetWorkResponse>, Refusal> {
    let a = service.current()?;
    Ok(Json(service::one_work(&*a, service.terms(), &params.work)?))
}

/// What `/v1/references` and `/v1/citers` take: the same page of the
/// same shape, walked in opposite directions.
#[derive(Debug, Deserialize)]
#[cfg_attr(
    test,
    derive(::ts_rs::TS),
    ts(export_to = "serving.ts", optional_fields)
)]
pub(crate) struct ListParams {
    /// The work whose list this is.
    work: String,
    /// Arcs a page carries, to a cap of 100; 50 when it is not asked.
    page_size: Option<u32>,
    /// Where to resume, from the previous page's `cursor`. Empty, or
    /// absent, is the start of the list.
    cursor: Option<String>,
    /// Whether each arc carries the summary of the work it points at.
    /// Off unless asked, in either literal `true` or `false`.
    hydrate: Option<bool>,
}

/// `GET /v1/references?work=&page_size=&cursor=&hydrate=` — one page of
/// what a work cites.
async fn references<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<ListParams>,
) -> Result<Json<pb::WorkPage>, Refusal> {
    list(&service, &params, Direction::References)
}

/// `GET /v1/citers?work=&page_size=&cursor=&hydrate=` — one page of what
/// cites a work.
async fn citers<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<ListParams>,
) -> Result<Json<pb::WorkPage>, Refusal> {
    list(&service, &params, Direction::Citers)
}

/// One page of a list. The stream sends page after page; a request here
/// takes one, and the `cursor` the page carries asks for the next —
/// empty when the list is done.
///
/// `page_size` is the arcs a page carries, which is what it says. There
/// is no page number to confuse it with — a list is walked by cursor —
/// but the parameter was called `page` until 2026-09-19, and a name that
/// reads as a number is a wrong answer given confidently, which is the
/// thing this surface is built not to do. The protocol still spells the
/// field `page` (`ListRequest.page`), so the two transports differ in
/// this one name: renaming it there would change nothing on the wire,
/// where fields are numbered, but it would break every client that names
/// them — the reflection descriptor this server registers among them —
/// and the protocol is the user's to rename.
fn list<A: Answers>(
    service: &Service<A>,
    params: &ListParams,
    direction: Direction,
) -> Result<Json<pb::WorkPage>, Refusal> {
    let a = service.current()?;
    let id = service::work_id(&*a, &params.work)?;
    let page_size = service::bounded(
        count(params.page_size),
        service::PAGE_DEFAULT,
        service::PAGE_MAX,
        "page_size",
    )?;
    // Empty is the start of the list, which is what the protocol's empty
    // cursor means; anything else must be this epoch's.
    let offset = match params.cursor.as_deref().filter(|c| !c.is_empty()) {
        Some(text) => service::cursor(text, a.epoch().epoch)?,
        None => 0,
    };
    Ok(Json(service::one_page(
        &*a,
        service.terms(),
        id,
        direction,
        offset,
        page_size,
        params.hydrate.unwrap_or(false),
    )?))
}

/// What `/v1/search` takes.
///
/// `facet_values` is the values kept per facet root, not a number of
/// roots: the mapping's seven roots are all always answered, and this
/// says how deep each one goes. The protocol calls the same field
/// `facets`, which reads as a count of roots and is not one; renaming it
/// there would break every client keying on the name, so the JSON
/// spelling is the corrected one (the user, 2026-09-19).
#[derive(Debug, Deserialize)]
#[cfg_attr(
    test,
    derive(::ts_rs::TS),
    ts(export_to = "serving.ts", optional_fields)
)]
pub(crate) struct SearchParams {
    /// The query, as typed.
    query: String,
    /// Hits returned, to a cap of 100; 10 when it is not asked.
    limit: Option<u32>,
    /// Values kept per facet root, to a cap of 20; 5 when it is not
    /// asked.
    facet_values: Option<u32>,
}

/// `GET /v1/search?query=&limit=&facet_values=` — works matching a
/// query, with facet counts over the matches.
async fn search<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<SearchParams>,
) -> Result<Json<pb::SearchResponse>, Refusal> {
    let a = service.current()?;
    let limit = service::bounded(
        count(params.limit),
        service::SEARCH_DEFAULT,
        service::SEARCH_MAX,
        "limit",
    )?;
    let facet_values = service::bounded(
        count(params.facet_values),
        service::FACETS_DEFAULT,
        service::FACETS_MAX,
        "facet_values",
    )?;
    Ok(Json(service::searched(
        &*a,
        service.terms(),
        &params.query,
        limit,
        facet_values,
    )?))
}

/// Which way an expansion walks. Named and not numbered: the protocol's
/// enum has no text form a query string could carry, and a number here
/// would be a client writing `2` and meaning nothing it could read back.
/// Absent is `both`, which is what an expansion was before they were
/// split.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(::ts_rs::TS), ts(export_to = "serving.ts"))]
pub(crate) enum Walk {
    /// Both directions at once.
    Both,
    /// What the seeds cite.
    References,
    /// What cites the seeds.
    Citers,
}

impl From<Walk> for serve::Walk {
    fn from(walk: Walk) -> Self {
        match walk {
            Walk::Both => Self::Both,
            Walk::References => Self::References,
            Walk::Citers => Self::Citers,
        }
    }
}

/// What `/v1/expand` takes.
#[derive(Debug, Deserialize)]
#[cfg_attr(
    test,
    derive(::ts_rs::TS),
    ts(export_to = "serving.ts", optional_fields)
)]
pub(crate) struct ExpandParams {
    /// The seed set, written one `seeds=` at a time: a name may hold any
    /// punctuation a separator could use, so there is no separator. One
    /// to 100 of them.
    seeds: Vec<String>,
    /// Which way to walk; `both` when it is not asked.
    walk: Option<Walk>,
    /// Neighbours returned, to a cap of 1000; 20 when it is not asked.
    budget: Option<u32>,
    /// Whether each neighbour carries a summary. Off unless asked.
    hydrate: Option<bool>,
}

/// `GET /v1/expand?seeds=&seeds=&walk=&budget=&hydrate=` — one hop out
/// from a seed set.
async fn expand<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<ExpandParams>,
) -> Result<Json<pb::ExpandResponse>, Refusal> {
    let a = service.current()?;
    Ok(Json(service::expanded(
        &*a,
        service.terms(),
        &params.seeds,
        params.walk.unwrap_or(Walk::Both).into(),
        count(params.budget),
        params.hydrate.unwrap_or(false),
    )?))
}

/// Which neighbourhood `/v1/related` answers. Required and never
/// defaulted: the two are opposite questions, and a default would answer
/// one of them to a client that meant the other.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(test, derive(::ts_rs::TS), ts(export_to = "serving.ts"))]
pub(crate) enum RelatedBy {
    /// Seeded with the work's references, walking their citers: papers
    /// that cite the same sources as this one.
    CitesSame,
    /// Seeded with the work's citers, walking their references: papers
    /// cited alongside this one.
    CitedAlongside,
}

impl From<RelatedBy> for serve::By {
    fn from(by: RelatedBy) -> Self {
        match by {
            RelatedBy::CitesSame => Self::CitesSame,
            RelatedBy::CitedAlongside => Self::CitedAlongside,
        }
    }
}

/// What `/v1/related` takes.
#[derive(Debug, Deserialize)]
#[cfg_attr(
    test,
    derive(::ts_rs::TS),
    ts(export_to = "serving.ts", optional_fields)
)]
pub(crate) struct RelatedParams {
    /// The work the neighbourhood is measured around.
    work: String,
    /// Which of the two questions to answer.
    by: RelatedBy,
    /// Neighbours returned, to a cap of 1000; 20 when it is not asked.
    limit: Option<u32>,
    /// Whether each neighbour carries a summary. Off unless asked.
    hydrate: Option<bool>,
}

/// `GET /v1/related?work=&by=&limit=&hydrate=` — what else cites what a
/// work cites (`by=cites-same`), or what else is cited by what cites it
/// (`by=cited-alongside`), in one call. The shorthand for the two
/// questions the reader asks; `/v1/expand` is the primitive under it and
/// stays as it is, for a seed set that is nobody's list.
async fn related<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<RelatedParams>,
) -> Result<Json<pb::RelatedResponse>, Refusal> {
    let a = service.current()?;
    Ok(Json(service::related(
        &*a,
        service.terms(),
        &params.work,
        params.by.into(),
        count(params.limit),
        params.hydrate.unwrap_or(false),
    )?))
}

/// What `/v1/path` takes.
#[derive(Debug, Deserialize)]
#[cfg_attr(
    test,
    derive(::ts_rs::TS),
    ts(export_to = "serving.ts", optional_fields)
)]
pub(crate) struct PathParams {
    /// The work the path starts at.
    from: String,
    /// The work it is looked for at.
    to: String,
    /// Arcs a path may be long, to a cap of 10; 6 when it is not asked.
    depth: Option<u32>,
    /// Paths one answer may carry, each through a different middle, to a
    /// cap of 10; 3 when it is not asked.
    paths: Option<u32>,
    /// Whether each work along a path carries a summary. Off unless
    /// asked.
    hydrate: Option<bool>,
}

/// `GET /v1/path?from=&to=&depth=&paths=&hydrate=` — the shortest
/// directed paths from one work to another. No path within the bound is
/// its own outcome in the answer and not an empty list; see
/// [`service::pathed`].
async fn path<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    Params(params): Params<PathParams>,
) -> Result<Json<pb::PathResponse>, Refusal> {
    let a = service.current()?;
    Ok(Json(service::pathed(
        &*a,
        service.terms(),
        &params.from,
        &params.to,
        count(params.depth),
        count(params.paths),
        params.hydrate.unwrap_or(false),
    )?))
}

/// A path that is no route: named, so that a misspelling in a client is
/// not a bare 404.
async fn no_route(uri: Uri) -> Refusal {
    Refusal(Status::not_found(format!(
        "{}: no such route; the routes are {}",
        uri.path(),
        ROUTES.join(", ")
    )))
}

/// A route asked with the wrong method: every one is a GET.
async fn no_method(method: Method, uri: Uri) -> Refusal {
    Refusal(Status::invalid_argument(format!(
        "{method} {}: every route is a GET",
        uri.path()
    )))
}

// --- refusals -------------------------------------------------------------

/// A refusal on its way out. The shared code raises a [`Status`], which
/// is not this crate's type, so it travels in a wrapper that can answer.
#[derive(Debug)]
pub struct Refusal(Status);

impl From<Status> for Refusal {
    fn from(status: Status) -> Self {
        Self(status)
    }
}

/// What a refusal looks like on the wire: the gRPC code by name, so a
/// client can tell a shared name from a misspelled parameter without
/// reading prose, and the reason. Never mistakable for an answer, which
/// always carries `answered`.
#[derive(Debug, serde::Serialize)]
struct Refused<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        let (status, code) = refused(self.0.code());
        (
            status,
            Json(Refused {
                code,
                message: self.0.message(),
            }),
        )
            .into_response()
    }
}

/// A gRPC code as an HTTP status and a name, the mapping Google's own
/// `google.rpc.Code` publishes. Only four of these arise here — a bad
/// parameter, a name that resolves to nothing, a state the request came
/// too late or too early for, and a failure — but the code is what the
/// gRPC surface would send, and the whole of it is written out so the
/// two never disagree.
fn refused(code: Code) -> (StatusCode, &'static str) {
    match code {
        Code::Ok => (StatusCode::OK, "ok"),
        Code::Cancelled => (StatusCode::INTERNAL_SERVER_ERROR, "cancelled"),
        Code::Unknown => (StatusCode::INTERNAL_SERVER_ERROR, "unknown"),
        Code::InvalidArgument => (StatusCode::BAD_REQUEST, "invalid_argument"),
        Code::DeadlineExceeded => (StatusCode::GATEWAY_TIMEOUT, "deadline_exceeded"),
        Code::NotFound => (StatusCode::NOT_FOUND, "not_found"),
        Code::AlreadyExists => (StatusCode::CONFLICT, "already_exists"),
        Code::PermissionDenied => (StatusCode::FORBIDDEN, "permission_denied"),
        Code::ResourceExhausted => (StatusCode::TOO_MANY_REQUESTS, "resource_exhausted"),
        Code::FailedPrecondition => (StatusCode::BAD_REQUEST, "failed_precondition"),
        Code::Aborted => (StatusCode::CONFLICT, "aborted"),
        Code::OutOfRange => (StatusCode::BAD_REQUEST, "out_of_range"),
        Code::Unimplemented => (StatusCode::NOT_IMPLEMENTED, "unimplemented"),
        Code::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        Code::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
        Code::DataLoss => (StatusCode::INTERNAL_SERVER_ERROR, "data_loss"),
        Code::Unauthenticated => (StatusCode::UNAUTHORIZED, "unauthenticated"),
    }
}

// --- the other origin -----------------------------------------------------

/// The origins a browser may read an answer from. Exact, and no
/// wildcard: the frontend runs on a dev server at another port, which is
/// the whole of the problem, and `*` would hand the surface to every
/// page the same browser has open.
#[derive(Debug, Clone)]
pub struct Origins(Arc<Vec<HeaderValue>>);

/// An origin the list cannot take.
#[derive(Debug)]
pub enum OriginError {
    /// `*`: every page in the browser, which this does not open to. Name
    /// the origins instead.
    Wildcard,
    /// Bytes no header can carry.
    NotAHeaderValue(String),
}

impl fmt::Display for OriginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wildcard => write!(
                f,
                "\"*\" is not an origin; name the origins the browser may read from"
            ),
            Self::NotAHeaderValue(text) => write!(f, "{text:?} is not an origin"),
        }
    }
}

impl std::error::Error for OriginError {}

impl Origins {
    /// Where a Vite dev server answers, in both spellings of loopback: a
    /// browser holds `localhost` and `127.0.0.1` to be different
    /// origins, and which one the developer typed is theirs to choose.
    pub const DEV: [&str; 2] = ["http://localhost:5173", "http://127.0.0.1:5173"];

    /// The origins named, each an exact scheme, host and port, spelled
    /// as the browser sends them.
    ///
    /// # Errors
    ///
    /// `*`, or text no header can carry.
    pub fn new<S: AsRef<str>>(origins: &[S]) -> Result<Self, OriginError> {
        origins
            .iter()
            .map(|origin| {
                let origin = origin.as_ref();
                if origin.trim() == "*" {
                    return Err(OriginError::Wildcard);
                }
                HeaderValue::from_str(origin)
                    .map_err(|_| OriginError::NotAHeaderValue(origin.to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|origins| Self(Arc::new(origins)))
    }

    /// The listed origin equal to `origin`, which is what the answer
    /// echoes — the spelling the server was configured with, not the
    /// one the request carried.
    fn allows(&self, origin: &HeaderValue) -> Option<&HeaderValue> {
        self.0.iter().find(|allowed| *allowed == origin)
    }

    /// How the startup line says what it allows.
    #[must_use]
    pub fn listed(&self) -> Vec<String> {
        self.0
            .iter()
            .map(|o| String::from_utf8_lossy(o.as_bytes()).into_owned())
            .collect()
    }
}

/// Cross-origin access, by hand and as small as it goes. Every route is
/// a GET carrying no header of its own, which a browser sends with no
/// preflight at all, so the whole of it is `Access-Control-Allow-Origin`
/// on a listed origin and `Vary: Origin` always, so that no cache ever
/// hands one origin's answer to another. A preflight is answered anyway,
/// for the request some later client makes. No credentials are allowed:
/// nothing here reads a cookie, and the surface holds no principal yet
/// (serving plan §7, auth deferred).
async fn cors(State(origins): State<Origins>, request: Request, next: Next) -> Response {
    let allowed = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|origin| origins.allows(origin))
        .cloned();
    let preflight = request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD);
    // A preflight asks about the route; it is not a call of it.
    let mut response = if preflight {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(header::VARY, HeaderValue::from_static("origin"));
    if let Some(origin) = allowed {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        if preflight {
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, OPTIONS"),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("content-type"),
            );
            headers.insert(
                header::ACCESS_CONTROL_MAX_AGE,
                HeaderValue::from_static("600"),
            );
        }
    }
    response
}
