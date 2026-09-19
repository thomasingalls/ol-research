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

// Every route is a thin call into the synchronous query core, which the
// gRPC methods call the same way; there is nothing to await inside one.
#![allow(clippy::unused_async)]

use std::fmt;
use std::sync::Arc;

use axum::extract::{FromRequestParts, Query, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router, middleware};
use tonic::{Code, Status};

use crate::answers::{Answers, Direction};
use crate::proto::v1 as pb;
use crate::service::{self, Service};

/// The routes, under `/v1`: one per call in the protocol, every one a
/// GET, since every one is a question. A work is named in a parameter
/// and never in the path — a name is `doi:10.1000/x`, with a slash in
/// it, and a path segment would have to carry it twice-encoded.
pub fn router<A: Answers>(service: Arc<Service<A>>, origins: Origins) -> Router {
    Router::new()
        .route("/v1/epoch", get(epoch::<A>))
        .route("/v1/resolve", get(resolve::<A>))
        .route("/v1/work", get(work::<A>))
        .route("/v1/references", get(references::<A>))
        .route("/v1/citers", get(citers::<A>))
        .route("/v1/search", get(search::<A>))
        .route("/v1/expand", get(expand::<A>))
        .route("/v1/related", get(related::<A>))
        .route("/v1/path", get(path::<A>))
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

/// `GET /v1/epoch` — which epoch and batch the server answers from, its
/// counts, and the terms.
async fn epoch<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::EpochInfo>, Refusal> {
    params.only(&[])?;
    let a = service.current();
    Ok(Json(service::epoch_info(&*a, service.terms())))
}

/// `GET /v1/resolve?text=` — which work a name or public id is.
async fn resolve<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::ResolveResponse>, Refusal> {
    params.only(&["text"])?;
    let a = service.current();
    Ok(Json(service::resolved(
        &*a,
        service.terms(),
        params.one("text")?,
    )?))
}

/// `GET /v1/work?work=` — everything the merge knows about one work.
async fn work<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::GetWorkResponse>, Refusal> {
    params.only(&["work"])?;
    let a = service.current();
    Ok(Json(service::one_work(
        &*a,
        service.terms(),
        params.one("work")?,
    )?))
}

/// `GET /v1/references?work=&page=&cursor=&hydrate=` — one page of what
/// a work cites.
async fn references<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::WorkPage>, Refusal> {
    list(&service, &params, Direction::References)
}

/// `GET /v1/citers?work=&page=&cursor=&hydrate=` — one page of what
/// cites a work.
async fn citers<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::WorkPage>, Refusal> {
    list(&service, &params, Direction::Citers)
}

/// One page of a list. The stream sends page after page; a request here
/// takes one, and the `cursor` the page carries asks for the next —
/// empty when the list is done. `page` is the page's size, not its
/// number, as it is in the protocol.
fn list<A: Answers>(
    service: &Service<A>,
    params: &Params,
    direction: Direction,
) -> Result<Json<pb::WorkPage>, Refusal> {
    params.only(&["work", "page", "cursor", "hydrate"])?;
    let a = service.current();
    let id = service::work_id(&*a, params.one("work")?)?;
    let page = service::bounded(
        params.count("page")?,
        service::PAGE_DEFAULT,
        service::PAGE_MAX,
        "page",
    )?;
    // Empty is the start of the list, which is what the protocol's empty
    // cursor means; anything else must be this epoch's.
    let offset = match params.at_most_one("cursor")?.filter(|c| !c.is_empty()) {
        Some(text) => service::cursor(text, a.epoch().epoch)?,
        None => 0,
    };
    Ok(Json(service::one_page(
        &*a,
        service.terms(),
        id,
        direction,
        offset,
        page,
        params.flag("hydrate")?,
    )?))
}

/// `GET /v1/search?query=&limit=&facets=` — works matching a query, with
/// facet counts over the matches.
async fn search<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::SearchResponse>, Refusal> {
    params.only(&["query", "limit", "facets"])?;
    let a = service.current();
    Ok(Json(service::searched(
        &*a,
        service.terms(),
        params.one("query")?,
        params.count("limit")?,
        params.count("facets")?,
    )?))
}

/// `GET /v1/expand?seeds=&seeds=&walk=&budget=&hydrate=` — one hop out
/// from a seed set. `seeds` is repeated, once per seed, since a name may
/// hold any punctuation a separator could use. `walk` is `references`,
/// `citers` or `both`, named rather than numbered: the protocol's enum
/// has no text form a query string could carry, and a number here would
/// be a client writing `2` and meaning nothing it could read back.
async fn expand<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::ExpandResponse>, Refusal> {
    params.only(&["seeds", "walk", "budget", "hydrate"])?;
    let a = service.current();
    let seeds = params.every("seeds");
    Ok(Json(service::expanded(
        &*a,
        service.terms(),
        &seeds,
        service::walk_named(params.at_most_one("walk")?)?,
        params.count("budget")?,
        params.flag("hydrate")?,
    )?))
}

/// `GET /v1/related?work=&by=&limit=&hydrate=` — what else cites what a
/// work cites (`by=cites-same`), or what else is cited by what cites it
/// (`by=cited-alongside`), in one call. The shorthand for the two
/// questions the reader asks; `/v1/expand` is the primitive under it and
/// stays as it is, for a seed set that is nobody's list.
///
/// `by` is named and required: the two are opposite questions, and a
/// default would answer one of them to a client that meant the other.
async fn related<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::RelatedResponse>, Refusal> {
    params.only(&["work", "by", "limit", "hydrate"])?;
    let a = service.current();
    Ok(Json(service::related(
        &*a,
        service.terms(),
        params.one("work")?,
        service::by_named(params.one("by")?)?,
        params.count("limit")?,
        params.flag("hydrate")?,
    )?))
}

/// `GET /v1/path?from=&to=&depth=&paths=&hydrate=` — the shortest
/// directed paths from one work to another. No path within the bound is
/// its own outcome in the answer and not an empty list; see
/// [`service::pathed`].
async fn path<A: Answers>(
    State(service): State<Arc<Service<A>>>,
    params: Params,
) -> Result<Json<pb::PathResponse>, Refusal> {
    params.only(&["from", "to", "depth", "paths", "hydrate"])?;
    let a = service.current();
    Ok(Json(service::pathed(
        &*a,
        service.terms(),
        params.one("from")?,
        params.one("to")?,
        params.count("depth")?,
        params.count("paths")?,
        params.flag("hydrate")?,
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

/// The routes, for the refusal that names them.
const ROUTES: [&str; 9] = [
    "/v1/epoch",
    "/v1/resolve",
    "/v1/work",
    "/v1/references",
    "/v1/citers",
    "/v1/search",
    "/v1/expand",
    "/v1/related",
    "/v1/path",
];

// --- the query string -----------------------------------------------------

/// A request's query string, as the pairs it was written with: a
/// repeated key is kept, since an expansion names its seeds one `seeds`
/// at a time, and a key the route does not read is refused rather than
/// ignored — a misspelled parameter is otherwise a wrong answer given
/// confidently.
#[derive(Debug)]
struct Params(Vec<(String, String)>);

impl<S: Send + Sync> FromRequestParts<S> for Params {
    type Rejection = Refusal;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<Vec<(String, String)>>::from_request_parts(parts, state)
            .await
            .map(|Query(pairs)| Self(pairs))
            .map_err(|e| Refusal(Status::invalid_argument(e.body_text())))
    }
}

impl Params {
    /// Refuses any key the route does not read. Every route calls this
    /// first, so a misspelling is named before whatever it left missing.
    fn only(&self, known: &[&str]) -> Result<(), Refusal> {
        for (key, _) in &self.0 {
            if !known.contains(&key.as_str()) {
                return Err(invalid(&format!(
                    "{key:?} is not a parameter of this route; it takes {}",
                    if known.is_empty() {
                        "none".to_owned()
                    } else {
                        known.join(", ")
                    }
                )));
            }
        }
        Ok(())
    }

    /// The one value of a key the route requires.
    fn one(&self, key: &str) -> Result<&str, Refusal> {
        match self.at_most_one(key)? {
            Some(value) if !value.is_empty() => Ok(value),
            Some(_) => Err(invalid(&format!("{key} is empty"))),
            None => Err(invalid(&format!("{key} is required"))),
        }
    }

    /// The value of a key given at most once.
    fn at_most_one(&self, key: &str) -> Result<Option<&str>, Refusal> {
        let mut found = self.0.iter().filter(|(k, _)| k == key);
        let first = found.next().map(|(_, v)| v.as_str());
        if found.next().is_some() {
            return Err(invalid(&format!("{key} was given more than once")));
        }
        Ok(first)
    }

    /// Every value of a repeated key, in the order they were written.
    fn every(&self, key: &str) -> Vec<String> {
        self.0
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// A count, zero when absent — the protocol's spelling for "the
    /// default", which the shared cap then applies.
    fn count(&self, key: &str) -> Result<u32, Refusal> {
        match self.at_most_one(key)? {
            None => Ok(0),
            Some(text) => text
                .parse()
                .map_err(|_| invalid(&format!("{key}={text:?} is not a count"))),
        }
    }

    /// A flag, false when absent; `true` and `false` and nothing else.
    fn flag(&self, key: &str) -> Result<bool, Refusal> {
        match self.at_most_one(key)? {
            None | Some("false") => Ok(false),
            Some("true") => Ok(true),
            Some(text) => Err(invalid(&format!("{key}={text:?}; true or false expected"))),
        }
    }
}

fn invalid(why: &str) -> Refusal {
    Refusal(Status::invalid_argument(why.to_owned()))
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
