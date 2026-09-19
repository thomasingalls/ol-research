//! The JSON transport over the same fake the gRPC tests use: the routes
//! and the shapes they answer with — the contract a browser client is
//! written against — the cursor across pages and across an epoch, the
//! caps, the refusals, and what a cross-origin request is told.

mod common;

use std::sync::Arc;

use common::{Fake, public_text};
use ol_rht_serve::{Origins, Service, Terms};
use serde_json::Value;

/// The JSON surface on a port of its own, and the base of its URLs.
async fn start(service: Arc<Service<Fake>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let origins = Origins::new(&Origins::DEV).unwrap();
    tokio::spawn(ol_rht_serve::http::serve(listener, service, origins));
    format!("http://{addr}")
}

/// A client that reads a refusal instead of throwing it away: every
/// refusal here carries a body worth asserting on.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .new_agent()
}

/// A GET, as its status and its body.
fn get(agent: &ureq::Agent, url: &str) -> (u16, Value) {
    let mut response = agent.get(url).call().unwrap();
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().unwrap();
    (
        status,
        serde_json::from_str(&body).unwrap_or_else(|e| panic!("{url}: {e}: {body}")),
    )
}

/// The refusal a request was answered with: its status and its code.
fn refused(agent: &ureq::Agent, url: &str) -> (u16, String) {
    let (status, body) = get(agent, url);
    let code = body["code"].as_str().unwrap_or("").to_owned();
    assert!(
        body["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{url}: a refusal says why"
    );
    (status, code)
}

/// An object's field names. `serde_json` holds them sorted, so these
/// assertions are over the set of fields, not the order they are written
/// in, which is the protocol's.
fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn the_routes_answer_the_shapes_a_client_is_written_against() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();

    // Every answer is stamped the same way: the epoch, the batch it is
    // as of, that batch's timestamp, the sources and their attribution.
    let (status, epoch) = get(&agent, &format!("{base}/v1/epoch"));
    assert_eq!(status, 200);
    assert_eq!(
        keys(&epoch),
        ["answered", "arcs", "merged", "nodes", "works"]
    );
    assert_eq!(
        keys(&epoch["answered"]),
        ["at", "attribution", "batch", "epoch", "sources"]
    );
    assert_eq!(epoch["answered"]["epoch"], 1);
    assert_eq!(epoch["answered"]["batch"], 1);
    assert_eq!(epoch["answered"]["at"], "2026-09-16T00:00:01Z");
    assert_eq!(
        epoch["answered"]["attribution"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        (&epoch["nodes"], &epoch["works"], &epoch["arcs"]),
        (&9.into(), &3.into(), &3.into())
    );

    // Resolve: one work, a shared name, an id a split retired — the
    // outcome named, never guessed at.
    let (status, one) = get(&agent, &format!("{base}/v1/resolve?text=doi:10.1000/one"));
    assert_eq!(status, 200);
    assert_eq!(keys(&one), ["answered", "outcome"]);
    assert_eq!(one["outcome"]["public_id"], public_text(1));
    let (_, shared) = get(
        &agent,
        &format!("{base}/v1/resolve?text=doi:10.1000/shared"),
    );
    assert_eq!(
        shared["outcome"]["shared"]["public_ids"],
        serde_json::json!([public_text(2), public_text(3)])
    );
    let (_, split) = get(&agent, &format!("{base}/v1/resolve?text=pmid:9"));
    assert_eq!(split["outcome"]["split"]["public_id"], public_text(9));
    assert_eq!(
        split["outcome"]["split"]["into"],
        serde_json::json!([public_text(2), public_text(3)]),
        "a retired id answers with the works its names went to"
    );
    // A public id resolves like any other name, in any case.
    let (_, folded) = get(
        &agent,
        &format!("{base}/v1/resolve?text={}", public_text(1).to_lowercase()),
    );
    assert_eq!(folded["outcome"]["public_id"], public_text(1));

    // A work: the whole row, and nothing in it is a WorkId.
    let (status, answer) = get(&agent, &format!("{base}/v1/work?work=pmid:1"));
    assert_eq!(status, 200);
    assert_eq!(keys(&answer), ["answered", "work"]);
    let work = &answer["work"];
    assert_eq!(
        keys(work),
        [
            "abstract",
            "authors",
            "citers",
            "concern_by",
            "corrected_by",
            "disputed",
            "form",
            "genre",
            "held_by",
            "language",
            "names",
            "open_access",
            "public_id",
            "published",
            "records",
            "references",
            "retracted_by",
            "stub",
            "title",
            "venue",
        ]
    );
    assert_eq!(work["public_id"], public_text(1));
    assert_eq!(work["title"], "A titled work");
    assert_eq!(work["authors"], serde_json::json!(["Ada Lovelace"]));
    assert_eq!(work["abstract"], "The abstract.");
    assert_eq!(work["held_by"], serde_json::json!(["pubmed"]));
    assert_eq!(
        (&work["references"], &work["citers"]),
        (&2.into(), &1.into())
    );
    assert_eq!(work["stub"], false);
    // A field absent is null, not missing: the shape never varies.
    assert!(work["published"].is_null() && work["open_access"].is_null());
    assert_eq!(
        keys(&work["names"][0]),
        ["by", "namespace", "native", "standing", "value"]
    );
    assert_eq!(work["names"][1]["namespace"], "doi");
    assert_eq!(work["names"][1]["value"], "10.1000/one");
    assert_eq!(
        keys(&work["records"][0]),
        ["native", "promoted", "revised", "source"]
    );
    assert_eq!(
        keys(&work["disputed"][0]),
        ["field", "others", "promoted_from"]
    );
    assert_eq!(work["disputed"][0]["field"], "title");
    assert_eq!(
        work["disputed"][0]["others"],
        serde_json::json!(["crossref: A titled work: with subtitle"])
    );
    assert_eq!(answer["answered"]["sources"], serde_json::json!(["pubmed"]));

    // Search, and one hop out from a seed set.
    let (status, found) = get(&agent, &format!("{base}/v1/search?query=titled"));
    assert_eq!(status, 200);
    assert_eq!(keys(&found), ["answered", "facets", "hits", "total"]);
    assert_eq!(found["total"], 1);
    assert_eq!(keys(&found["hits"][0]), ["public_id", "score", "title"]);
    assert_eq!(found["hits"][0]["public_id"], public_text(1));
    assert_eq!(keys(&found["facets"][0]), ["root", "values"]);
    assert_eq!(found["facets"][0]["root"], "/year");
    assert_eq!(keys(&found["facets"][0]["values"][0]), ["count", "value"]);

    let (status, expanded) = get(
        &agent,
        &format!(
            "{base}/v1/expand?seeds=pmid:1&seeds={}&budget=1&hydrate=true&walk=both",
            public_text(1)
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(keys(&expanded), ["answered", "neighbours", "total"]);
    assert_eq!(expanded["neighbours"].as_array().unwrap().len(), 1);
    assert_eq!(expanded["total"], 2, "one of the two the hop reached");
    assert_eq!(
        keys(&expanded["neighbours"][0]),
        ["degree", "public_id", "score", "summary", "touches"]
    );
    // The raw overlap ties 2 and 3; their degrees do not, and the score
    // is what the budget of one cuts on.
    assert_eq!(expanded["neighbours"][0]["public_id"], public_text(2));
    assert_eq!(expanded["neighbours"][0]["touches"], 1);
    assert_eq!(expanded["neighbours"][0]["degree"], 1);
    assert_eq!(expanded["neighbours"][0]["score"], 1.0);
    // The one summary, the same shape a page's arc carries.
    assert_eq!(
        keys(&expanded["neighbours"][0]["summary"]),
        [
            "authors",
            "names",
            "public_id",
            "published",
            "stub",
            "title",
            "venue"
        ]
    );
    assert!(
        expanded["neighbours"][0]["summary"]["published"].is_null(),
        "null where the merge promoted no date, as on a work"
    );
    assert_eq!(expanded["neighbours"][0]["summary"]["stub"], true);
    assert_eq!(expanded["neighbours"][0]["summary"]["title"], "");
    assert_eq!(
        expanded["neighbours"][0]["summary"]["names"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "a stub is renderable by its names"
    );

    // The shorthand: the same hop with the seeds gathered by the server
    // and the subject left out of its own answer.
    let (status, related) = get(
        &agent,
        &format!("{base}/v1/related?work=pmid:3&by=cited-alongside&hydrate=true"),
    );
    assert_eq!(status, 200);
    assert_eq!(
        keys(&related),
        ["answered", "related", "seeds_total", "seeds_used", "total"]
    );
    assert_eq!(related["related"].as_array().unwrap().len(), 1);
    assert_eq!(related["related"][0]["public_id"], public_text(2));
    assert_eq!(related["seeds_used"], 1);
    assert_eq!(related["seeds_total"], 1);
    assert_eq!(related["total"], 1);
    assert_eq!(
        keys(&related["related"][0]),
        ["degree", "public_id", "score", "summary", "touches"],
        "a neighbour is a neighbour, whichever call sent it"
    );

    // The directions are separate questions, named and not numbered.
    let (status, cited) = get(
        &agent,
        &format!("{base}/v1/expand?seeds=pmid:1&walk=citers"),
    );
    assert_eq!(status, 200);
    assert_eq!(cited["neighbours"].as_array().unwrap().len(), 1);
    assert_eq!(cited["neighbours"][0]["public_id"], public_text(3));

    // A path, and the answer when there is none.
    let (status, walked) = get(
        &agent,
        &format!("{base}/v1/path?from=pmid:3&to=pmid:2&hydrate=true"),
    );
    assert_eq!(status, 200);
    assert_eq!(keys(&walked), ["answered", "outcome"]);
    assert_eq!(walked["outcome"]["found"]["hops"], 2);
    assert_eq!(walked["outcome"]["found"]["total"], 1);
    assert_eq!(
        walked["outcome"]["found"]["paths"][0]["works"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["public_id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        [public_text(3), public_text(1), public_text(2)]
    );
    let (status, nowhere) = get(&agent, &format!("{base}/v1/path?from=pmid:2&to=pmid:3"));
    assert_eq!(status, 200);
    assert_eq!(keys(&nowhere), ["answered", "outcome"]);
    assert_eq!(nowhere["outcome"]["none"]["depth"], 6);
    assert_eq!(nowhere["outcome"]["none"]["forward"], 1);

    // Nothing anywhere carries an internal id.
    let everything = serde_json::to_string(&serde_json::json!([
        epoch, one, answer, found, expanded, related, cited, walked, nowhere
    ]))
    .unwrap();
    assert!(!everything.contains("work_id") && !everything.contains("\"id\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_list_is_one_page_a_request_and_the_cursor_resumes_it() {
    let service = Arc::new(Service::new(Fake::new(1), Terms::stub()));
    let base = start(service.clone()).await;
    let agent = agent();

    // One arc a page: the first page carries the cursor to the second.
    let (status, first) = get(
        &agent,
        &format!(
            "{base}/v1/references?work={}&page=1&hydrate=true",
            public_text(1)
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(keys(&first), ["answered", "arcs", "cursor", "total"]);
    assert_eq!(first["total"], 2);
    assert_eq!(first["arcs"].as_array().unwrap().len(), 1);
    assert_eq!(keys(&first["arcs"][0]), ["by", "public_id", "summary"]);
    assert_eq!(first["arcs"][0]["public_id"], public_text(2));
    assert_eq!(
        first["arcs"][0]["by"],
        serde_json::json!(["pubmed", "crossref"])
    );
    assert_eq!(
        first["arcs"][0]["summary"]["names"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        first["arcs"][0]["summary"]["public_id"], first["arcs"][0]["public_id"],
        "a summary names its own work"
    );
    assert_eq!(first["cursor"], "1:1");

    // Resumed by that cursor, the last page carries none.
    let (_, second) = get(
        &agent,
        &format!("{base}/v1/references?work=pmid:1&page=1&cursor=1:1"),
    );
    assert_eq!(second["arcs"][0]["public_id"], public_text(3));
    assert_eq!(second["cursor"], "");
    // Unhydrated, an arc is its id and its asserters and nothing else:
    // a null summary rather than an empty one, which would read as a
    // work we know nothing about instead of one nobody asked about.
    assert_eq!(keys(&second["arcs"][0]), ["by", "public_id", "summary"]);
    assert!(second["arcs"][0]["summary"].is_null());

    // An empty cursor is the start of the list, as it is in the protocol.
    let (_, from_empty) = get(
        &agent,
        &format!("{base}/v1/references?work=pmid:1&cursor=&page=1"),
    );
    assert_eq!(from_empty["arcs"][0]["public_id"], public_text(2));

    // Citers the other way; an empty list is one empty page.
    let (_, citers) = get(&agent, &format!("{base}/v1/citers?work={}", public_text(2)));
    assert_eq!(citers["arcs"][0]["public_id"], public_text(1));
    let (_, none) = get(
        &agent,
        &format!("{base}/v1/references?work={}", public_text(2)),
    );
    assert_eq!(none["total"], 0);
    assert_eq!(none["arcs"], serde_json::json!([]));
    assert_eq!(none["cursor"], "");

    // The epoch switches under a live cursor: the old one is refused,
    // rather than answered from a list it no longer describes.
    service.swap(Fake::new(2));
    let (status, code) = refused(
        &agent,
        &format!("{base}/v1/citers?work={}&cursor=1:0", public_text(1)),
    );
    assert_eq!((status, code.as_str()), (400, "failed_precondition"));
    let (_, now) = get(&agent, &format!("{base}/v1/epoch"));
    assert_eq!(now["answered"]["epoch"], 2);
}

// One line a refusal; the table is the test.
#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn nothing_is_answered_by_guessing() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    let cases = [
        // A name that is nothing, and one that is not a name at all.
        ("/v1/resolve?text=pmid:404", 404, "not_found"),
        ("/v1/resolve?text=not%20a%20name", 400, "invalid_argument"),
        (
            "/v1/resolve?text=ol:00000000000001",
            400,
            "invalid_argument",
        ),
        // A name that is more than one work, and one a split retired.
        (
            "/v1/work?work=doi:10.1000/shared",
            400,
            "failed_precondition",
        ),
        ("/v1/work?work=pmid:9", 404, "not_found"),
        // A parameter missing, empty, doubled, or of the wrong shape.
        ("/v1/work", 400, "invalid_argument"),
        ("/v1/work?work=", 400, "invalid_argument"),
        ("/v1/work?work=pmid:1&work=pmid:2", 400, "invalid_argument"),
        (
            "/v1/references?work=pmid:1&page=many",
            400,
            "invalid_argument",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=1",
            400,
            "invalid_argument",
        ),
        (
            "/v1/references?work=pmid:1&cursor=nonsense",
            400,
            "invalid_argument",
        ),
        // A parameter the route does not know, rather than ignored.
        (
            "/v1/search?query=x&limit=1&offset=3",
            400,
            "invalid_argument",
        ),
        (
            "/v1/references?work=pmid:1&limit=5",
            400,
            "invalid_argument",
        ),
        ("/v1/epoch?verbose=true", 400, "invalid_argument"),
        // Every cap, refused and not clamped.
        (
            "/v1/references?work=pmid:1&page=101",
            400,
            "invalid_argument",
        ),
        ("/v1/citers?work=pmid:1&page=101", 400, "invalid_argument"),
        ("/v1/search?query=x&limit=101", 400, "invalid_argument"),
        ("/v1/search?query=x&facets=21", 400, "invalid_argument"),
        (
            "/v1/expand?seeds=pmid:1&budget=1001",
            400,
            "invalid_argument",
        ),
        ("/v1/expand?budget=5", 400, "invalid_argument"),
        // A direction that is not one, named rather than silently taken
        // for the default.
        (
            "/v1/expand?seeds=pmid:1&walk=sideways",
            400,
            "invalid_argument",
        ),
        // The two neighbourhoods are opposite questions, so `by` is
        // required and never defaulted, and a limit over the cap is
        // refused there as everywhere.
        ("/v1/related?work=pmid:1", 400, "invalid_argument"),
        (
            "/v1/related?work=pmid:1&by=similar",
            400,
            "invalid_argument",
        ),
        ("/v1/related?by=cites-same", 400, "invalid_argument"),
        (
            "/v1/related?work=pmid:1&by=cites-same&limit=1001",
            400,
            "invalid_argument",
        ),
        (
            "/v1/related?work=pmid:1&by=cites-same&walk=citers",
            400,
            "invalid_argument",
        ),
        ("/v1/related?work=pmid:404&by=cites-same", 404, "not_found"),
        (
            "/v1/path?from=pmid:1&to=pmid:2&depth=11",
            400,
            "invalid_argument",
        ),
        (
            "/v1/path?from=pmid:1&to=pmid:2&paths=11",
            400,
            "invalid_argument",
        ),
        ("/v1/path?from=pmid:1", 400, "invalid_argument"),
        (
            "/v1/path?from=pmid:1&to=pmid:2&walk=citers",
            400,
            "invalid_argument",
        ),
        // A path that is no route.
        ("/v1/works/1", 404, "not_found"),
        ("/", 404, "not_found"),
    ];
    for (path, status, code) in cases {
        assert_eq!(
            refused(&agent, &format!("{base}{path}")),
            (status, code.to_owned()),
            "{path}"
        );
    }
    // A route asked with the wrong method.
    let mut response = agent.options(format!("{base}/v1/epoch")).call().unwrap();
    assert_eq!(response.status().as_u16(), 400);
    let body: Value = serde_json::from_str(&response.body_mut().read_to_string().unwrap()).unwrap();
    assert_eq!(body["code"], "invalid_argument");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_the_listed_origins_may_read_an_answer() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    let dev = "http://localhost:5173";

    // A listed origin is echoed, and the answer says it varies by origin
    // so that no cache hands it to another.
    let response = agent
        .get(format!("{base}/v1/epoch"))
        .header("Origin", dev)
        .call()
        .unwrap();
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        dev
    );
    assert_eq!(response.headers().get("vary").unwrap(), "origin");
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/json"
    );
    assert!(
        response
            .headers()
            .get("access-control-allow-credentials")
            .is_none()
    );

    // A refusal is readable across origins too: a client that cannot
    // read why it was refused is a client that has to guess.
    let refused = agent
        .get(format!("{base}/v1/work?work=pmid:404"))
        .header("Origin", dev)
        .call()
        .unwrap();
    assert_eq!(refused.status().as_u16(), 404);
    assert_eq!(
        refused
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        dev
    );

    // Any other origin is told nothing, which is what stops the browser.
    let other = agent
        .get(format!("{base}/v1/epoch"))
        .header("Origin", "http://evil.example")
        .call()
        .unwrap();
    assert!(other.headers().get("access-control-allow-origin").is_none());
    assert_eq!(other.headers().get("vary").unwrap(), "origin");

    // A preflight is answered without reaching the route.
    let preflight = agent
        .options(format!("{base}/v1/work"))
        .header("Origin", dev)
        .header("Access-Control-Request-Method", "GET")
        .call()
        .unwrap();
    assert_eq!(preflight.status().as_u16(), 204);
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-origin")
            .unwrap(),
        dev
    );
    assert_eq!(
        preflight
            .headers()
            .get("access-control-allow-methods")
            .unwrap(),
        "GET, OPTIONS"
    );
    // And a preflight from elsewhere is told nothing either.
    let refused = agent
        .options(format!("{base}/v1/work"))
        .header("Origin", "http://evil.example")
        .header("Access-Control-Request-Method", "GET")
        .call()
        .unwrap();
    assert_eq!(refused.status().as_u16(), 204);
    assert!(
        refused
            .headers()
            .get("access-control-allow-origin")
            .is_none()
    );
}

#[test]
fn a_wildcard_origin_is_not_a_list() {
    assert!(Origins::new(&["*"]).is_err());
    assert!(Origins::new(&[" * "]).is_err());
    assert!(Origins::new(&["http://\nlocalhost"]).is_err());
    assert_eq!(
        Origins::new(&Origins::DEV).unwrap().listed(),
        vec![
            "http://localhost:5173".to_owned(),
            "http://127.0.0.1:5173".to_owned()
        ]
    );
}
