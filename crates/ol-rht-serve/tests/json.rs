//! The JSON transport over the same fake the gRPC tests use: the routes
//! and the shapes they answer with — the contract a browser client is
//! written against — the cursor across pages and across an epoch, the
//! caps, the refusals, and what a cross-origin request is told.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
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
    assert_eq!(work["retracted_by"], serde_json::json!(["crossref"]));
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
    assert_eq!(
        keys(&found["hits"][0]),
        ["public_id", "score", "summary", "title"]
    );
    assert_eq!(found["hits"][0]["public_id"], public_text(1));
    assert_eq!(found["hits"][0]["title"], "A titled work");
    assert!(
        found["hits"][0]["summary"].is_null(),
        "hydrate was not asked for, and the index's title is all there is"
    );
    // Hydrated, a hit is a row as a list's is, and says what the work
    // itself says about its standing.
    let (status, hydrated) = get(
        &agent,
        &format!("{base}/v1/search?query=titled&hydrate=true"),
    );
    assert_eq!(status, 200);
    let summary = &hydrated["hits"][0]["summary"];
    assert_eq!(summary["public_id"], public_text(1));
    assert_eq!(summary["title"], work["title"]);
    assert_eq!(summary["authors"], work["authors"]);
    assert_eq!(summary["venue"], "Journal of Tests");
    assert_eq!(summary["retracted_by"], work["retracted_by"]);
    assert_eq!(summary["corrected_by"], serde_json::json!([]));
    assert_eq!(summary["concern_by"], serde_json::json!([]));
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
            "concern_by",
            "corrected_by",
            "names",
            "public_id",
            "published",
            "retracted_by",
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
            "{base}/v1/references?work={}&page_size=1&hydrate=true",
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
        &format!("{base}/v1/references?work=pmid:1&page_size=1&cursor=1:1"),
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
        &format!("{base}/v1/references?work=pmid:1&cursor=&page_size=1"),
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
            "/v1/references?work=pmid:1&page_size=many",
            400,
            "invalid_argument",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=1",
            400,
            "invalid_argument",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=",
            400,
            "invalid_argument",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=yes",
            400,
            "invalid_argument",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=TRUE",
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
            "/v1/references?work=pmid:1&page_size=101",
            400,
            "invalid_argument",
        ),
        (
            "/v1/citers?work=pmid:1&page_size=101",
            400,
            "invalid_argument",
        ),
        ("/v1/search?query=x&limit=101", 400, "invalid_argument"),
        (
            "/v1/search?query=x&facet_values=21",
            400,
            "invalid_argument",
        ),
        // The protocol's spelling is not the JSON one: `facets` reads as
        // a count of roots and never was (the user, 2026-09-19).
        ("/v1/search?query=x&facets=5", 400, "invalid_argument"),
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

/// A refusal names the parameter and says what was wrong with it, in the
/// words it said them in before a route's parameters were a struct.
///
/// Asserted to the character, because a derived refusal is the easy
/// thing to let drift into serde's own prose — "unknown field `page`,
/// expected one of `work`, `page_size`, ..." — which is a sentence about
/// a Rust struct and not about this route. The message is what a person
/// reads and the only thing that says how to fix the request, so it is
/// as much the contract as the status code is.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_names_the_parameter_and_says_why() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    let cases = [
        // A name the route does not read, and the route that reads none.
        (
            "/v1/references?work=pmid:1&page=100",
            "\"page\" is not a parameter of this route; it takes work, page_size, cursor, hydrate",
        ),
        (
            "/v1/epoch?verbose=true",
            "\"verbose\" is not a parameter of this route; it takes none",
        ),
        // Missing, empty, doubled.
        ("/v1/work", "work is required"),
        ("/v1/work?work=", "work is empty"),
        (
            "/v1/work?work=pmid:1&work=pmid:2",
            "work was given more than once",
        ),
        ("/v1/related?work=pmid:1", "by is required"),
        // Not the shape the parameter is.
        (
            "/v1/references?work=pmid:1&page_size=many",
            "page_size=\"many\" is not a count",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=1",
            "hydrate=\"1\"; true or false expected",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=",
            "hydrate=\"\"; true or false expected",
        ),
        (
            "/v1/references?work=pmid:1&hydrate=yes",
            "hydrate=\"yes\"; true or false expected",
        ),
        // A name the enum does not have, which lists the ones it does.
        (
            "/v1/expand?seeds=pmid:1&walk=sideways",
            "walk=\"sideways\"; both, references, citers expected",
        ),
        (
            "/v1/related?work=pmid:1&by=similar",
            "by=\"similar\"; cites-same, cited-alongside expected",
        ),
        // An empty one says the same, which is a change from `by is
        // empty`: it was the accident of a required name going through
        // `one()` and a walk through `at_most_one()`, and the two now
        // answer alike, each naming what it would have taken.
        (
            "/v1/related?work=pmid:1&by=",
            "by=\"\"; cites-same, cited-alongside expected",
        ),
        // The caps, which the struct does not hold and `service::bounded`
        // still does — each labelled with the name this transport spells
        // the parameter by, `facet_values` where the protocol says
        // `facets`.
        (
            "/v1/references?work=pmid:1&page_size=101",
            "page_size 101 is over the cap of 100",
        ),
        (
            "/v1/search?query=x&limit=101",
            "limit 101 is over the cap of 100",
        ),
        (
            "/v1/search?query=x&facet_values=21",
            "facet_values 21 is over the cap of 20",
        ),
        (
            "/v1/path?from=pmid:1&to=pmid:2&depth=11",
            "depth 11 is over the cap of 10",
        ),
        // A repeated parameter given none of itself: the route says so
        // in its own terms, which is more than "seeds is required".
        ("/v1/expand?budget=5", "0 seeds; one to 100 expected"),
    ];
    for (path, message) in cases {
        let (status, body) = get(&agent, &format!("{base}{path}"));
        assert_eq!((status, &body["message"]), (400, &message.into()), "{path}");
    }
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

// --- the generated TypeScript ---------------------------------------------

/// The committed bindings describe the bytes this surface actually
/// sends.
///
/// `web/src/generated/serving.ts` is generated from the Rust types
/// (`crate::ts`), and the generator agreeing with itself proves nothing
/// — the question is whether what it wrote describes a response. So the
/// declarations are read back here and checked against real bodies, off
/// a real socket: the field names one for one, `null` where the merge
/// promoted nothing, and every count a JSON number and not a string,
/// which is the one thing a proto3-JSON generator would have got wrong
/// for every `uint64` on this surface.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_generated_typescript_describes_these_bodies() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    let ts = Bindings::committed();

    let bodies = [
        ("EpochInfo", "/v1/epoch"),
        ("ResolveResponse", "/v1/resolve?text=doi:10.1000/one"),
        ("ResolveResponse", "/v1/resolve?text=doi:10.1000/shared"),
        ("ResolveResponse", "/v1/resolve?text=pmid:9"),
        // No namespace: every name the text can be read as.
        ("ResolveResponse", "/v1/resolve?text=77"),
        // A citation naming two works, one held and one a stub; and one
        // naming nothing, which is the reason's half.
        (
            "LookupCitationResponse",
            "/v1/citation?text=doi:10.1000/one.%20PMID:%202",
        ),
        ("LookupCitationResponse", "/v1/citation?text=PMID:%20404"),
        // Found by its words, beside a work a PMID names: the check, and
        // the two differing; and a near miss.
        (
            "LookupCitationResponse",
            "/v1/citation?text=Lovelace%20A.%20A%20titled%20work.%20PMID:%202",
        ),
        (
            "LookupCitationResponse",
            "/v1/citation?text=Babbage%20C.%20An%20untitled%20essay.",
        ),
        ("GetWorkResponse", "/v1/work?work=pmid:1"),
        (
            "WorkPage",
            "/v1/references?work=pmid:1&page_size=1&hydrate=true",
        ),
        // Unhydrated: `summary` is null, which is the shape's other half.
        ("WorkPage", "/v1/references?work=pmid:1"),
        ("WorkPage", "/v1/citers?work=pmid:2"),
        ("SearchResponse", "/v1/search?query=titled"),
        ("SearchResponse", "/v1/search?query=titled&hydrate=true"),
        (
            "ExpandResponse",
            "/v1/expand?seeds=pmid:1&hydrate=true&walk=both",
        ),
        (
            "RelatedResponse",
            "/v1/related?work=pmid:3&by=cited-alongside&hydrate=true",
        ),
        (
            "PathResponse",
            "/v1/path?from=pmid:3&to=pmid:2&hydrate=true",
        ),
        // The other two outcomes of a path.
        ("PathResponse", "/v1/path?from=pmid:2&to=pmid:3"),
    ];
    for (declared, path) in bodies {
        let (status, body) = get(&agent, &format!("{base}{path}"));
        assert_eq!(status, 200, "{path}");
        ts.describes(declared, &body, path);
    }

    // The bodies above are not the whole of it unless they reach every
    // declaration. Three they cannot: `Disputed` needs a work two
    // sources disagree about, `PathStopped` a search that runs out of
    // budget, and `PubDate` a promoted date — the fake promotes none,
    // which is what makes `published: null` testable here instead.
    //
    // The request declarations are not among them: no response body
    // reaches one, and they are checked the other way round, against the
    // routes themselves, by
    // [`the_generated_typescript_names_parameters_the_routes_take`].
    let mut expected = ts.request_side();
    expected.extend(["Disputed", "PathStopped", "PubDate"].map(str::to_owned));
    let reached = ts.reached(&expected.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(
        reached.is_empty(),
        "never checked against a body: {reached:?}"
    );
}

/// The parameters the committed TypeScript declares are the parameters
/// the routes read.
///
/// The other half of the same contract, and the half that went wrong:
/// the bodies above prove what comes back, this proves what may be sent.
/// Every name a client can write is asked of the route that declares it
/// and answered, and a name no route declares is refused by name — so a
/// client that compiles is a client this server will answer, which it
/// was not on the morning of 2026-09-19.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_generated_typescript_names_parameters_the_routes_take() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    let ts = Bindings::committed();

    // A value each parameter takes, by the name it is declared under.
    // The fake holds pmid:1 through pmid:3 (`common::Fake`).
    let value = |name: &str| match name {
        "work" | "from" | "to" | "text" | "seeds" => "pmid:1",
        "query" => "x",
        "by" => "cites-same",
        "walk" => "both",
        // The start of a list, which is what the protocol means by an
        // empty cursor and the one empty value this surface accepts.
        "cursor" => "",
        "hydrate" => "true",
        _ => "1",
    };

    let routes = ts.routes();
    assert_eq!(routes.len(), 10, "the map is every route");
    for (path, params, _) in routes {
        let written = ts.types[params]
            .iter()
            // `optional_fields` writes an optional parameter `name?`;
            // the name on the wire is the same either way.
            .map(|(name, _)| name.trim_end_matches('?'))
            .map(|name| format!("{name}={}", value(name)))
            .collect::<Vec<_>>()
            .join("&");
        let url = format!("{base}{path}?{written}");
        let (status, body) = get(&agent, &url);
        assert_eq!(status, 200, "{url}: {body}");

        // And a name no route declares, refused by name.
        let (status, refusal) = get(&agent, &format!("{url}&page=1"));
        assert_eq!(
            (status, &refusal["code"]),
            (400, &"invalid_argument".into())
        );
        assert!(
            refusal["message"]
                .as_str()
                .unwrap_or_default()
                .starts_with("\"page\" is not a parameter of this route; it takes "),
            "{path}: {refusal}"
        );
    }
}

/// The declarations in `web/src/generated/serving.ts`, as the field
/// names and TypeScript types of each `export type`.
struct Bindings {
    types: BTreeMap<String, Vec<(String, String)>>,
    seen: std::cell::RefCell<BTreeSet<String>>,
}

impl Bindings {
    /// Reads the committed file. Deliberately the committed one and not
    /// a freshly generated one: it is the file the frontend compiles
    /// against, and the one a stale checkout would hold.
    fn committed() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/generated/serving.ts");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e}; run `just ts-types`", path.display()));
        let mut types = BTreeMap::new();
        for decl in text.split("export type ").skip(1) {
            let (name, body) = decl.split_once(" =").expect("`export type X = …`");
            let body = body.split_once("\n};").map_or(body, |(b, _)| b);
            let mut fields = Vec::new();
            for line in body.lines() {
                let line = line.trim();
                // The doc comments are prose; the members are `name: type,`.
                if line.starts_with(['/', '*', '{', '}']) || line.is_empty() {
                    continue;
                }
                if let Some(arm) = line.strip_prefix("| { ") {
                    // A oneof arm: `| { "public_id": string }`.
                    let arm = arm.trim_end_matches([';', '}', ' ']);
                    let (key, ty) = arm.split_once(": ").expect("a union arm");
                    fields.push((key.trim_matches('"').to_owned(), ty.trim().to_owned()));
                } else if let Some((key, ty)) = line.split_once(": ") {
                    fields.push((
                        key.trim_matches('"').to_owned(),
                        ty.trim_end_matches(',').to_owned(),
                    ));
                }
            }
            types.insert(name.trim().to_owned(), fields);
        }
        assert!(types.len() > 20, "only {} declarations parsed", types.len());
        Self {
            types,
            seen: std::cell::RefCell::new(BTreeSet::new()),
        }
    }

    /// The route map as it was generated: each path, the declaration of
    /// what may be sent to it, and the declaration of what comes back.
    fn routes(&self) -> Vec<(&str, &str, &str)> {
        self.types["Routes"]
            .iter()
            .map(|(path, both)| {
                let (params, response) = both
                    .trim_matches(['{', '}', ' '])
                    .split_once(", response: ")
                    .expect("`{ params: X, response: Y }`");
                (
                    path.as_str(),
                    params.trim().trim_start_matches("params: "),
                    response.trim(),
                )
            })
            .collect()
    }

    /// Every declaration only a request reaches: `Routes` itself, the
    /// `params` of each route, and whatever those name.
    fn request_side(&self) -> Vec<String> {
        let mut out = vec!["Routes".to_owned()];
        let mut queue: Vec<String> = self
            .routes()
            .iter()
            .map(|(_, params, _)| (*params).to_owned())
            .collect();
        while let Some(name) = queue.pop() {
            if out.contains(&name) {
                continue;
            }
            for (_, ty) in self.types.get(&name).into_iter().flatten() {
                let ty = ty.trim_start_matches("Array<").trim_end_matches('>');
                if self.types.contains_key(ty) {
                    queue.push(ty.to_owned());
                }
            }
            out.push(name);
        }
        out
    }

    /// Declarations no body reached, less the ones named as expected.
    fn reached(&self, expected_to_miss: &[&str]) -> Vec<String> {
        let seen = self.seen.borrow();
        self.types
            .keys()
            .filter(|name| !seen.contains(*name) && !expected_to_miss.contains(&name.as_str()))
            .cloned()
            .collect()
    }

    /// `value` is exactly what the declaration `name` says it is.
    fn describes(&self, name: &str, value: &Value, at: &str) {
        self.seen.borrow_mut().insert(name.to_owned());
        let fields = self
            .types
            .get(name)
            .unwrap_or_else(|| panic!("{at}: no declaration named {name}"));
        let object = value
            .as_object()
            .unwrap_or_else(|| panic!("{at}: {name} is declared an object; got {value}"));

        // A oneof is a union of one-key objects: exactly one arm, and
        // its key is the one the body carries.
        if name.ends_with("Outcome") {
            assert_eq!(object.len(), 1, "{at}: {name} is one arm; got {value}");
            let (key, arm) = object.iter().next().unwrap();
            let (_, ty) = fields
                .iter()
                .find(|(k, _)| k == key)
                .unwrap_or_else(|| panic!("{at}: {name} has no arm {key:?}"));
            self.holds(ty, arm, &format!("{at}: {name}.{key}"));
            return;
        }

        let declared: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        let sent: Vec<&str> = object.keys().map(String::as_str).collect();
        // `serde_json` sorts its keys and the protocol does not, so this
        // is over the set of names and not the order they were written.
        let mut declared_sorted = declared.clone();
        declared_sorted.sort_unstable();
        assert_eq!(declared_sorted, sent, "{at}: {name}'s fields");

        for (key, ty) in fields {
            self.holds(ty, &object[key], &format!("{at}: {name}.{key}"));
        }
    }

    /// `value` is what the TypeScript type `ty` says it is.
    fn holds(&self, ty: &str, value: &Value, at: &str) {
        if let Some(inner) = ty.strip_suffix(" | null") {
            // prost makes a message field optional and serde writes the
            // absence as null, so this is `| null` and never `?`.
            if value.is_null() {
                return;
            }
            return self.holds(inner, value, at);
        }
        assert!(!value.is_null(), "{at}: null, and {ty} does not allow it");
        if let Some(inner) = ty.strip_prefix("Array<").and_then(|t| t.strip_suffix('>')) {
            let items = value
                .as_array()
                .unwrap_or_else(|| panic!("{at}: {ty}; got {value}"));
            for (n, item) in items.iter().enumerate() {
                self.holds(inner, item, &format!("{at}[{n}]"));
            }
            return;
        }
        match ty {
            "string" => assert!(value.is_string(), "{at}: string; got {value}"),
            // The whole of the u64 question: a count arrives as a JSON
            // number, never as a string, so `number` is what describes
            // it and `bigint` would not.
            "number" => assert!(
                value.is_number(),
                "{at}: number; got {value}{}",
                if value.is_string() {
                    " (a string!)"
                } else {
                    ""
                }
            ),
            "boolean" => assert!(value.as_bool().is_some(), "{at}: boolean; got {value}"),
            named => self.describes(named, value, at),
        }
    }
}

/// The JSON surface of a server that has no epoch it can read yet: `503`
/// with the code `unavailable`, on every route, until one is swapped in
/// (2026-09-24).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_server_with_no_epoch_answers_unavailable_until_it_has_one() {
    let service = Arc::new(Service::<Fake>::waiting(Terms::stub()));
    let base = start(service.clone()).await;
    let agent = agent();
    for route in ["/v1/epoch", "/v1/resolve?text=doi:10.1000/one"] {
        let (status, code) = refused(&agent, &format!("{base}{route}"));
        assert_eq!((status, code.as_str()), (503, "unavailable"), "{route}");
    }
    service.swap(Fake::new(1));
    let (status, body) = get(&agent, &format!("{base}/v1/epoch"));
    assert_eq!(status, 200, "{body}");
}

/// A search, its text written into the URL as a browser writes it.
fn search(agent: &ureq::Agent, base: &str, text: &str) -> (u16, Value) {
    let mut response = agent
        .get(format!("{base}/v1/search"))
        .query("query", text)
        .call()
        .unwrap();
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().unwrap();
    (status, serde_json::from_str(&body).unwrap())
}

/// Any text is a query (2026-09-25): what Tantivy's parser refused or
/// took for a command is answered; empty is the query that finds
/// nothing, where a missing `query` is still a URL built wrong; and the
/// one refusal is size, named.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn any_text_is_a_query_and_only_size_is_refused() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    for text in [
        "\"",
        "(((((((((((((((((((((((((((((((",
        "5'-UTR",
        "id:[1 TO 5000]",
        "aspirin^99999",
        "-mice",
        "title:",
        "\u{1b}[2J \u{202e} \n",
        "",
        "   ",
    ] {
        let (status, body) = search(&agent, &base, text);
        assert_eq!(status, 200, "{text:?}: {body}");
        assert_eq!(body["total"], 0, "{text:?}");
    }
    let (status, body) = search(&agent, &base, "\"titled\"");
    assert_eq!((status, body["total"].as_u64()), (200, Some(1)));
    let (status, body) = get(&agent, &format!("{base}/v1/search"));
    assert_eq!(
        (status, body["message"].as_str()),
        (400, Some("query is required"))
    );

    // A thousand and twenty-four words of ten letters each: past the
    // eight kilobytes many proxies allow a URL, and answered here.
    let words = |n: usize| vec!["abcdefghij"; n].join(" ");
    let (status, _) = search(&agent, &base, &words(query_parser::MAX_WORDS));
    assert_eq!(status, 200);
    let (status, body) = search(&agent, &base, &words(query_parser::MAX_WORDS + 1));
    assert_eq!(
        (status, body["code"].as_str(), body["message"].as_str()),
        (
            400,
            Some("invalid_argument"),
            Some("the query is 1025 words; the limit is 1024")
        )
    );
    // The byte cap is out of a URL's reach: the `http` crate refuses a
    // URI over 65,534 bytes before any route sees it, so a GET cannot
    // carry a query that long, and a text of any other script than
    // ASCII's, three bytes of URL to each of its own, not a third of it
    // (the gRPC test reaches the cap). What the server answers a URL
    // that long with:
    let long = format!("/v1/search?query={}", "a".repeat(70_000));
    assert_eq!(raw_status(&base, &long), 414);
}

/// The status a request line is answered with, written by hand, since
/// no client here will write a URL the `http` crate refuses.
fn raw_status(base: &str, target: &str) -> u16 {
    use std::io::{Read, Write};

    let addr = base.trim_start_matches("http://");
    let mut stream = std::net::TcpStream::connect(addr).unwrap();
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).unwrap();
    std::str::from_utf8(&head[9..12]).unwrap().parse().unwrap()
}

/// Searches run on threads of their own within a time limit (2026-09-25):
/// a search past it is refused `deadline_exceeded`; one that finds every
/// search thread busy for the whole limit, `resource_exhausted`; and all
/// the while every other call answers — on the one request thread this
/// server has, which a search running on it would have held.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_slow_search_holds_no_request_thread() {
    use std::time::{Duration, Instant};

    let service =
        Service::new(Fake::new(1), Terms::stub()).searching(1, Duration::from_millis(300));
    let base = start(Arc::new(service)).await;
    let first = std::thread::spawn({
        let base = base.clone();
        move || search(&agent(), &base, "slow")
    });
    // The first search holds the one search thread for 600 ms, answering
    // its caller at 300.
    std::thread::sleep(Duration::from_millis(100));
    let agent = agent();
    let asked = Instant::now();
    let (status, _) = get(&agent, &format!("{base}/v1/epoch"));
    assert_eq!(status, 200);
    assert!(
        asked.elapsed() < Duration::from_millis(250),
        "{:?}",
        asked.elapsed()
    );
    let (status, body) = search(&agent, &base, "slow");
    assert_eq!(
        (status, body["code"].as_str()),
        (429, Some("resource_exhausted"))
    );
    let (status, body) = first.join().unwrap();
    assert_eq!(
        (status, body["code"].as_str()),
        (504, Some("deadline_exceeded"))
    );
}

/// A citation looked up over JSON, as its status and body.
fn lookup(agent: &ureq::Agent, base: &str, text: &str) -> (u16, Value) {
    let mut response = agent
        .get(format!("{base}/v1/citation"))
        .query("text", text)
        .call()
        .unwrap();
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().unwrap();
    (status, serde_json::from_str(&body).unwrap())
}

/// A citation is looked up by what it names (citation lookup plan, CL1):
/// each identifier written in it, as written and as read, with what it
/// names; and the works they lead to, each with the names that led there
/// — two works when the citation disagrees with itself, listed both, and
/// none with the reason when nothing leads anywhere.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn a_citation_is_looked_up_by_what_it_names() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();

    // A DOI in capitals ending the citation's last sentence: read without
    // the full stop, which is what the names table holds.
    let (status, body) = lookup(
        &agent,
        &base,
        "Lovelace A. A titled work. J Tests. 2026. doi:10.1000/ONE.",
    );
    assert_eq!(status, 200);
    assert_eq!(
        keys(&body),
        [
            "answered",
            "could_not",
            "differ",
            "identifiers",
            "matches",
            "near_misses",
            "only_names"
        ]
    );
    assert_eq!(body["only_names"], false, "the citation has words");
    let doi = &body["identifiers"][0];
    assert_eq!(doi["written"], "10.1000/ONE");
    assert_eq!(doi["name"], "doi:10.1000/one");
    assert_eq!(doi["resolved"], "one");
    assert_eq!(doi["public_ids"], serde_json::json!([public_text(1)]));
    assert_eq!(body["matches"].as_array().unwrap().len(), 1);
    assert_eq!(body["matches"][0]["public_id"], public_text(1));
    assert_eq!(
        body["matches"][0]["named_by"],
        serde_json::json!(["doi:10.1000/one"])
    );
    assert_eq!(body["matches"][0]["summary"]["title"], "A titled work");
    assert_eq!(body["could_not"], "");

    // Two identifiers naming one work: one match, named by both.
    let (_, both) = lookup(&agent, &base, "doi:10.1000/one. PMID: 1");
    assert_eq!(both["identifiers"].as_array().unwrap().len(), 2);
    assert_eq!(both["matches"].as_array().unwrap().len(), 1);
    assert_eq!(
        both["matches"][0]["named_by"],
        serde_json::json!(["doi:10.1000/one", "pmid:1"])
    );

    // Two naming different works: both listed, each with its name — the
    // citation disagrees with itself, and that is the answer.
    let (_, differ) = lookup(&agent, &base, "doi:10.1000/one. PMID: 2");
    let matches = differ["matches"].as_array().unwrap();
    assert_eq!(
        matches
            .iter()
            .map(|m| (m["public_id"].clone(), m["named_by"].clone()))
            .collect::<Vec<_>>(),
        [
            (
                public_text(1).into(),
                serde_json::json!(["doi:10.1000/one"])
            ),
            (public_text(2).into(), serde_json::json!(["pmid:2"])),
        ]
    );
    assert_eq!(
        matches[1]["summary"]["stub"], true,
        "a stub is an answer too"
    );

    // A shared name and a split id lead to every work they lead to.
    let (_, shared) = lookup(&agent, &base, "doi:10.1000/shared");
    assert_eq!(shared["identifiers"][0]["resolved"], "shared");
    let (_, split) = lookup(&agent, &base, "PMID: 9");
    assert_eq!(split["identifiers"][0]["resolved"], "split");
    for body in [&shared, &split] {
        assert_eq!(
            body["matches"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["public_id"].clone())
                .collect::<Vec<_>>(),
            [public_text(2), public_text(3)]
        );
    }

    // A name nothing answers to is reported, and no work is listed.
    let (_, unknown) = lookup(&agent, &base, "Some citation. PMID: 404");
    assert_eq!(unknown["identifiers"][0]["written"], "PMID: 404");
    assert_eq!(unknown["identifiers"][0]["resolved"], "none");
    assert_eq!(
        unknown["identifiers"][0]["public_ids"],
        serde_json::json!([])
    );
    assert_eq!(unknown["matches"], serde_json::json!([]));
    assert_eq!(unknown["could_not"], "nothing-came-up");

    // Nothing to go on: no identifier, and no word to search.
    let (_, blank) = lookup(&agent, &base, "   ");
    assert_eq!(blank["could_not"], "nothing-to-go-on");

    // Refused: no text, empty text, and more than a citation's worth.
    for url in [
        format!("{base}/v1/citation"),
        format!("{base}/v1/citation?text="),
    ] {
        assert_eq!(refused(&agent, &url), (400, "invalid_argument".into()));
    }
    let at_cap = "x".repeat(ol_rht_serve::service::CITATION_MAX_BYTES);
    assert_eq!(lookup(&agent, &base, &at_cap).0, 200);
    let (status, over) = lookup(&agent, &base, &format!("{at_cap}x"));
    assert_eq!((status, &over["code"]), (400, &"invalid_argument".into()));
}

/// A citation's words are searched, and every work they turn up is checked
/// against it (citation lookup plan, CL3): a work that passes is listed, one
/// that fails is a near miss with what failed, and a work an identifier
/// names is listed whatever the check says, with the check beside it. When
/// the identifier names one work and the words pass another, the answer
/// says they differ. The fake's search turns up work 1 — titled "A titled
/// work", by Ada Lovelace, with no date — for any word with `titled` in it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn a_citation_s_words_turn_up_works_and_each_is_checked() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();

    // Found by its words alone, and passing on the two fields it has.
    let (_, words) = lookup(&agent, &base, "Lovelace A. A titled work. J Tests. 2026.");
    assert_eq!(words["identifiers"], serde_json::json!([]));
    assert_eq!(words["could_not"], "");
    assert_eq!(words["differ"], false);
    let one = &words["matches"][0];
    assert_eq!(one["public_id"], public_text(1));
    assert_eq!(one["named_by"], serde_json::json!([]));
    assert_eq!(one["by_text"], true);
    let verdict = &one["verdict"];
    assert_eq!(
        (
            &verdict["title"],
            &verdict["first_author"],
            &verdict["year"],
            &verdict["passes"]
        ),
        (
            &"agrees".into(),
            &"agrees".into(),
            &"work-does-not-say".into(),
            &true.into()
        )
    );
    // The row has no year, so the citation's 2026 is nothing's.
    assert_eq!(verdict["unaccounted"], serde_json::json!(["2026"]));

    assert_eq!(verdict["rule"], "every-field");

    // A title typed alone passes on the title, and says so.
    let (_, alone) = lookup(&agent, &base, "A titled work.");
    assert_eq!(alone["matches"][0]["public_id"], public_text(1));
    assert_eq!(alone["matches"][0]["verdict"]["rule"], "title-alone");
    assert_eq!(
        alone["matches"][0]["verdict"]["first_author"],
        "not-in-citation"
    );

    // Turned up and failing: a near miss, with what failed.
    let (_, near) = lookup(&agent, &base, "Babbage C. An untitled essay.");
    assert_eq!(near["matches"], serde_json::json!([]));
    assert_eq!(near["could_not"], "nothing-agreed");
    let missed = &near["near_misses"][0];
    assert_eq!(missed["public_id"], public_text(1));
    assert_eq!(missed["verdict"]["title"], "not-in-citation");
    assert_eq!(missed["verdict"]["first_author"], "not-in-citation");
    assert_eq!(missed["verdict"]["passes"], false);

    // An identifier and the words on the same work: one match, found both
    // ways, and nothing differs.
    let (_, same) = lookup(&agent, &base, "Lovelace A. A titled work. doi:10.1000/one");
    assert_eq!(same["matches"].as_array().unwrap().len(), 1);
    assert_eq!(
        same["matches"][0]["named_by"],
        serde_json::json!(["doi:10.1000/one"])
    );
    assert_eq!(same["matches"][0]["by_text"], true);
    assert_eq!(same["differ"], false);

    // An identifier naming one work and the words passing another: both
    // listed, the passing one first, and the answer says they differ.
    let (_, differ) = lookup(&agent, &base, "Lovelace A. A titled work. PMID: 2");
    assert_eq!(
        differ["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| (m["public_id"].clone(), m["named_by"].clone()))
            .collect::<Vec<_>>(),
        [
            (public_text(1).into(), serde_json::json!([])),
            (public_text(2).into(), serde_json::json!(["pmid:2"])),
        ]
    );
    assert_eq!(differ["differ"], true);

    // A DOI the words do not bear out: still listed, since the citation
    // names it, with the check saying what the words do not.
    let (_, named) = lookup(
        &agent,
        &base,
        "Babbage C. An untitled essay. doi:10.1000/one",
    );
    let listed = &named["matches"][0];
    assert_eq!(listed["named_by"], serde_json::json!(["doi:10.1000/one"]));
    assert_eq!(listed["verdict"]["title"], "not-in-citation");
    assert_eq!(listed["verdict"]["passes"], false);
    assert_eq!(named["near_misses"], serde_json::json!([]));
    assert_eq!(named["could_not"], "");
}

/// Find by ID takes what a person has, with no namespace (2026-09-28): a
/// name as it stands answers as it always has, and anything else is read
/// every way it can be — the identifiers in it, or the whole of it as one
/// token in every namespace — and every name that names something is
/// listed with what it names. The fake holds `pmid:77` for work 1 and
/// `pmc:PMC77` for work 3, and `reg1016:S0000000000000001` for work 1.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines)]
async fn find_by_id_needs_no_namespace() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    let found = |text: &str| {
        let mut response = agent
            .get(format!("{base}/v1/resolve"))
            .query("text", text)
            .call()
            .unwrap();
        let status = response.status().as_u16();
        let body: Value =
            serde_json::from_str(&response.body_mut().read_to_string().unwrap()).unwrap();
        (status, body)
    };
    let names = |body: &Value| {
        body["outcome"]["found"]["names"]
            .as_array()
            .unwrap_or_else(|| panic!("no list: {body}"))
            .iter()
            .map(|n| (n["name"].clone(), n["public_ids"].clone()))
            .collect::<Vec<_>>()
    };

    // A number that is a PMID of one work and a PMC id of another: both.
    let (status, body) = found("77");
    assert_eq!(status, 200);
    assert_eq!(
        names(&body),
        [
            ("pmid:77".into(), serde_json::json!([public_text(1)])),
            ("pmc:PMC77".into(), serde_json::json!([public_text(3)])),
        ]
    );
    // A publisher's own id, in whichever publisher's namespace holds it.
    let (_, body) = found("S0000000000000001");
    assert_eq!(
        names(&body),
        [(
            "reg1016:S0000000000000001".into(),
            serde_json::json!([public_text(1)])
        )]
    );
    // A DOI as a link and in capitals, a labelled PMID, and a public id
    // without its `ol:`, each read as the name it is.
    for (typed, name) in [
        ("https://doi.org/10.1000/ONE", "doi:10.1000/one"),
        ("PMID: 1", "pmid:1"),
        (
            public_text(1).trim_start_matches("ol:"),
            public_text(1).as_str(),
        ),
    ] {
        let (status, body) = found(typed);
        assert_eq!(status, 200, "{typed}: {body}");
        assert_eq!(
            names(&body),
            [(name.into(), serde_json::json!([public_text(1)]))],
            "{typed}"
        );
    }
    // A name as it stands still answers as one.
    let (_, body) = found("pmid:77");
    assert_eq!(body["outcome"]["public_id"], public_text(1));

    // A token nothing holds, in any namespace; and words with no
    // identifier among them.
    let (status, body) = found("40975433");
    assert_eq!((status, &body["code"]), (404, &"not_found".into()));
    let (status, body) = found("not a name");
    assert_eq!((status, &body["code"]), (400, &"invalid_argument".into()));
}

/// A citation that is one token is read as Find by ID reads it, with no
/// namespace needed; a bare number inside a longer citation is not tried,
/// its volume of 193 being PMID 193, another paper.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_citation_of_one_token_needs_no_namespace() {
    let base = start(Arc::new(Service::new(Fake::new(1), Terms::stub()))).await;
    let agent = agent();
    let (_, number) = lookup(&agent, &base, "77");
    assert_eq!(
        number["identifiers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["name"].clone())
            .collect::<Vec<_>>(),
        ["pmid:77", "pmc:PMC77"]
    );
    assert_eq!(number["matches"].as_array().unwrap().len(), 2);
    let (_, publisher) = lookup(&agent, &base, "S0000000000000001");
    assert_eq!(
        publisher["matches"][0]["named_by"],
        serde_json::json!(["reg1016:S0000000000000001"])
    );
    let (_, inside) = lookup(&agent, &base, "Babbage C. An essay. 1951;77:12.");
    assert_eq!(inside["identifiers"], serde_json::json!([]));
    assert_eq!(inside["only_names"], false);

    // Identifiers and nothing more — bare, as a link, labelled, or several
    // — say so, and are not searched as words: there are none.
    for text in [
        "77",
        "https://doi.org/10.1000/ONE",
        "doi:10.1000/one. PMID: 1",
        "PubMed PMID: 1",
    ] {
        let (_, body) = lookup(&agent, &base, text);
        assert_eq!(body["only_names"], true, "{text}");
        assert_eq!(body["near_misses"], serde_json::json!([]), "{text}");
    }
    let (_, with_words) = lookup(&agent, &base, "A titled work. doi:10.1000/one");
    assert_eq!(with_words["only_names"], false);
    assert_eq!(with_words["matches"][0]["by_text"], true);
}

/// A lookup searches, so it keeps to the search's time limit (plan, D7):
/// one past it is refused `deadline_exceeded`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slow_lookup_is_refused_at_the_time_limit() {
    use std::time::Duration;

    let service =
        Service::new(Fake::new(1), Terms::stub()).searching(1, Duration::from_millis(300));
    let base = start(Arc::new(service)).await;
    let (status, body) = lookup(&agent(), &base, "Lovelace A. A slow work.");
    assert_eq!(
        (status, body["code"].as_str()),
        (504, Some("deadline_exceeded"))
    );
}
