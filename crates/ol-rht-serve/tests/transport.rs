//! The gRPC transport over a fake: the id boundary, paging and cursors,
//! the caps, the errors, the terms, and an epoch switch under a live
//! cursor. The fake itself is [`common`], which the JSON tests share.

mod common;

use std::sync::Arc;

use common::{Fake, public_text};
use ol_rht_serve::proto::v1::serving_client::ServingClient;
use ol_rht_serve::proto::v1::serving_server::ServingServer;
use ol_rht_serve::proto::v1::{self as pb, resolve_response::Outcome};
use ol_rht_serve::{Service, Terms};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::Code;
use tonic::transport::Server;

async fn start(service: Arc<Service<Fake>>) -> ServingClient<tonic::transport::Channel> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(
        Server::builder()
            .add_service(ServingServer::from_arc(service))
            .serve_with_incoming(TcpListenerStream::new(listener)),
    );
    for _ in 0..50 {
        if let Ok(client) = ServingClient::connect(format!("http://{addr}")).await {
            return client;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the server did not come up");
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn the_boundary_speaks_public_ids_only() {
    let service = Arc::new(Service::new(Fake::new(1), Terms::stub()));
    let mut client = start(service).await;
    let info = client
        .epoch(pb::EpochRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!((info.nodes, info.works, info.arcs), (9, 3, 3));
    let answered = info.answered.unwrap();
    assert_eq!((answered.epoch, answered.batch), (1, 1));
    assert_eq!(answered.attribution.len(), 3);

    // Resolve by name, by public id in any case, shared, split, unknown, garbage.
    let by_name = client
        .resolve(pb::ResolveRequest {
            text: "doi:10.1000/one".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(by_name.outcome, Some(Outcome::PublicId(public_text(1))));
    let lower = public_text(1).to_lowercase();
    let by_public = client
        .resolve(pb::ResolveRequest { text: lower })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(by_public.outcome, Some(Outcome::PublicId(public_text(1))));
    let shared = client
        .resolve(pb::ResolveRequest {
            text: "doi:10.1000/shared".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        shared.outcome,
        Some(Outcome::Shared(pb::Shared {
            public_ids: vec![public_text(2), public_text(3)]
        }))
    );
    let split = client
        .resolve(pb::ResolveRequest {
            text: "pmid:9".into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        split.outcome,
        Some(Outcome::Split(pb::Split {
            public_id: public_text(9),
            into: vec![public_text(2), public_text(3)],
        }))
    );
    let unknown = client
        .resolve(pb::ResolveRequest {
            text: "pmid:404".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), Code::NotFound);
    let garbage = client
        .resolve(pb::ResolveRequest {
            text: "not a name".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(garbage.code(), Code::InvalidArgument);
    let bad_check = client
        .resolve(pb::ResolveRequest {
            text: "ol:00000000000001".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(bad_check.code(), Code::InvalidArgument);

    // A work, by name: the row on the wire, the abstract under the stub's terms.
    let work = client
        .get_work(pb::GetWorkRequest {
            work: "pmid:1".into(),
        })
        .await
        .unwrap()
        .into_inner();
    let w = work.work.unwrap();
    assert_eq!(w.public_id, public_text(1));
    assert_eq!(w.title, "A titled work");
    assert_eq!(w.authors, vec!["Ada Lovelace".to_owned()]);
    assert_eq!(w.r#abstract, "The abstract.");
    assert_eq!((w.references, w.citers), (2, 1));
    assert_eq!(w.held_by, vec!["pubmed".to_owned()]);
    assert_eq!(w.names[1].namespace, "doi");
    assert_eq!(w.names[1].value, "10.1000/one");
    assert_eq!(w.disputed.len(), 1);
    assert_eq!(w.disputed[0].field, "title");
    assert_eq!(
        w.disputed[0].others,
        vec!["crossref: A titled work: with subtitle".to_owned()]
    );
    assert_eq!(work.answered.unwrap().sources, vec!["pubmed".to_owned()]);
    let shared_work = client
        .get_work(pb::GetWorkRequest {
            work: "doi:10.1000/shared".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(shared_work.code(), Code::FailedPrecondition);
}

#[tokio::test]
async fn lists_page_and_cursors_survive_only_their_epoch() {
    let service = Arc::new(Service::new(Fake::new(1), Terms::stub()));
    let mut client = start(service.clone()).await;
    // One arc a page: two pages, the first with a cursor.
    let mut stream = client
        .references(pb::ListRequest {
            work: public_text(1),
            page: 1,
            cursor: String::new(),
            hydrate: true,
            limit: 0,
        })
        .await
        .unwrap()
        .into_inner();
    let first = stream.message().await.unwrap().unwrap();
    assert_eq!(first.total, 2);
    assert_eq!(first.arcs.len(), 1);
    assert_eq!(first.arcs[0].public_id, public_text(2));
    assert_eq!(
        first.arcs[0].by,
        vec!["pubmed".to_owned(), "crossref".to_owned()]
    );
    let summary = first.arcs[0].summary.as_ref().expect("hydrated");
    assert_eq!(summary.public_id, first.arcs[0].public_id);
    assert_eq!(summary.names.len(), 2, "hydrated with promoted names");
    // Work 2 is a stub: no title, and its names are what identifies it.
    assert!(summary.stub && summary.title.is_empty());
    assert_eq!(first.cursor, "1:1");
    let second = stream.message().await.unwrap().unwrap();
    assert_eq!(second.arcs[0].public_id, public_text(3));
    assert_eq!(second.cursor, "");
    assert!(stream.message().await.unwrap().is_none());
    // Resume from the cursor; a limit stops early.
    let mut resumed = client
        .references(pb::ListRequest {
            work: "pmid:1".into(),
            page: 50,
            cursor: "1:1".into(),
            hydrate: false,
            limit: 1,
        })
        .await
        .unwrap()
        .into_inner();
    let page = resumed.message().await.unwrap().unwrap();
    assert_eq!(page.arcs.len(), 1);
    assert_eq!(page.arcs[0].public_id, public_text(3));
    assert!(page.arcs[0].summary.is_none(), "hydrate was not asked for");
    assert_eq!(page.cursor, "");
    // An empty list answers one empty page with its total.
    let mut none = client
        .references(pb::ListRequest {
            work: public_text(2),
            page: 0,
            cursor: String::new(),
            hydrate: false,
            limit: 0,
        })
        .await
        .unwrap()
        .into_inner();
    let page = none.message().await.unwrap().unwrap();
    assert!(page.arcs.is_empty() && page.total == 0 && page.cursor.is_empty());
    // Over the cap.
    let over = client
        .citers(pb::ListRequest {
            work: public_text(1),
            page: 101,
            cursor: String::new(),
            hydrate: false,
            limit: 0,
        })
        .await
        .unwrap_err();
    assert_eq!(over.code(), Code::InvalidArgument);
    // The epoch switches: the old cursor is refused, the new epoch answers.
    service.swap(Fake::new(2));
    let stale = client
        .citers(pb::ListRequest {
            work: public_text(1),
            page: 1,
            cursor: "1:0".into(),
            hydrate: false,
            limit: 0,
        })
        .await
        .unwrap_err();
    assert_eq!(stale.code(), Code::FailedPrecondition);
    let info = client
        .epoch(pb::EpochRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.answered.unwrap().epoch, 2);
}

#[tokio::test]
async fn search_and_expand_are_bounded() {
    let service = Arc::new(Service::new(Fake::new(1), Terms::stub()));
    let mut client = start(service).await;
    let found = client
        .search(pb::SearchRequest {
            query: "titled".into(),
            limit: 0,
            facets: 0,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(found.total, 1);
    assert_eq!(found.hits[0].public_id, public_text(1));
    assert_eq!(found.facets[0].root, "/year");
    let over = client
        .search(pb::SearchRequest {
            query: "x".into(),
            limit: 101,
            facets: 0,
        })
        .await
        .unwrap_err();
    assert_eq!(over.code(), Code::InvalidArgument);
    // Two spellings of one work are one seed, so the cosine divides by
    // one and not two. Work 1 cites 2 and 3 and 3 cites it back, so
    // walking both ways reaches 2 (degree 1, score 1) and 3 (degree 2,
    // score 0.707); the budget of one keeps 2, where the raw overlap
    // alone would have tied them and kept whichever came first.
    let expanded = client
        .expand(pb::ExpandRequest {
            seeds: vec!["pmid:1".into(), public_text(1)],
            budget: 1,
            hydrate: true,
            walk: pb::Walk::Both.into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(expanded.neighbours.len(), 1);
    assert_eq!(expanded.total, 2, "one of the two the hop reached");
    let first = &expanded.neighbours[0];
    assert_eq!(first.public_id, public_text(2));
    assert_eq!((first.touches, first.degree), (1, 1));
    assert!((first.score - 1.0).abs() < 1e-9, "{}", first.score);
    // The same summary an arc carries, and a stub is renderable from it.
    let summary = first.summary.as_ref().expect("hydrated");
    assert!(summary.stub && !summary.names.is_empty());
    // The directions are separate questions: what work 1 cites is 2 and
    // 3, what cites it is 3 alone.
    let cited = client
        .expand(pb::ExpandRequest {
            seeds: vec!["pmid:1".into()],
            budget: 0,
            hydrate: false,
            walk: pb::Walk::References.into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        cited
            .neighbours
            .iter()
            .map(|n| n.public_id.clone())
            .collect::<Vec<_>>(),
        [public_text(2), public_text(3)]
    );
    let citing = client
        .expand(pb::ExpandRequest {
            seeds: vec!["pmid:1".into()],
            budget: 0,
            hydrate: false,
            walk: pb::Walk::Citers.into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        citing
            .neighbours
            .iter()
            .map(|n| n.public_id.clone())
            .collect::<Vec<_>>(),
        [public_text(3)]
    );
    let no_seeds = client
        .expand(pb::ExpandRequest {
            seeds: vec![],
            budget: 0,
            hydrate: false,
            walk: pb::Walk::Both.into(),
        })
        .await
        .unwrap_err();
    assert_eq!(no_seeds.code(), Code::InvalidArgument);
    // A direction the protocol does not have is refused, not defaulted.
    let unknown = client
        .expand(pb::ExpandRequest {
            seeds: vec!["pmid:1".into()],
            budget: 0,
            hydrate: false,
            walk: 7,
        })
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), Code::InvalidArgument);
}

/// The shorthand for the two questions: the seeds are the server's, and
/// the subject is not in its own answer.
#[tokio::test]
async fn related_gathers_its_own_seeds_and_leaves_the_subject_out() {
    let service = Arc::new(Service::new(Fake::new(1), Terms::stub()));
    let mut client = start(service).await;
    let ask = |work: String, by: pb::By| {
        let mut client = client.clone();
        async move {
            client
                .related(pb::RelatedRequest {
                    work,
                    by: by.into(),
                    limit: 0,
                    hydrate: true,
                })
                .await
        }
    };
    // Work 3 cites work 1 and is cited by it. Co-citation seeds with its
    // citers — work 1 — and walks their references, which are 2 and 3:
    // 3 is the subject and is left out, so the answer is 2 alone.
    let alongside = ask(public_text(3), pb::By::CitedAlongside)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        alongside
            .related
            .iter()
            .map(|n| n.public_id.clone())
            .collect::<Vec<_>>(),
        [public_text(2)],
        "the subject heads its own list unless it is dropped"
    );
    assert_eq!((alongside.seeds_used, alongside.seeds_total), (1, 1));
    assert_eq!(alongside.total, 1);
    assert!(alongside.related[0].summary.is_some(), "hydrated");
    // Bibliographic coupling seeds with what work 1 cites — 2 and 3 —
    // and walks their citers, which are work 1 itself: the subject
    // again, and so an empty answer rather than a list headed by it.
    let same = ask("pmid:1".into(), pb::By::CitesSame)
        .await
        .unwrap()
        .into_inner();
    assert!(same.related.is_empty(), "{:?}", same.related);
    assert_eq!((same.seeds_used, same.seeds_total, same.total), (2, 2, 0));
    // A stub cites nothing we know of, so its coupling has no seeds at
    // all: an empty answer saying `0 of 0`, which is a work with nothing
    // to couple on and not a failure.
    let nothing = ask(public_text(2), pb::By::CitesSame)
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        (
            nothing.related.len(),
            nothing.seeds_used,
            nothing.seeds_total,
            nothing.total
        ),
        (0, 0, 0, 0)
    );
    // A neighbourhood the protocol does not have is refused, never
    // defaulted to the other question.
    let unknown = ask("pmid:1".into(), pb::By::CitesSame).await;
    assert!(unknown.is_ok());
    let refused = client
        .related(pb::RelatedRequest {
            work: "pmid:1".into(),
            by: 7,
            limit: 0,
            hydrate: false,
        })
        .await
        .unwrap_err();
    assert_eq!(refused.code(), Code::InvalidArgument);
    // And a limit over the cap is refused, never clamped.
    let over = client
        .related(pb::RelatedRequest {
            work: "pmid:1".into(),
            by: pb::By::CitesSame.into(),
            limit: 1001,
            hydrate: false,
        })
        .await
        .unwrap_err();
    assert_eq!(over.code(), Code::InvalidArgument);
}

#[tokio::test]
async fn a_path_is_directed_bounded_and_says_so_when_there_is_none() {
    let service = Arc::new(Service::new(Fake::new(1), Terms::stub()));
    let mut client = start(service).await;
    let ask = |from: String, to: String, depth: u32| {
        let mut client = client.clone();
        async move {
            client
                .path(pb::PathRequest {
                    from,
                    to,
                    depth,
                    paths: 0,
                    hydrate: true,
                })
                .await
                .unwrap()
                .into_inner()
                .outcome
                .unwrap()
        }
    };
    // 3 -> 1 -> 2, and the arc that reached each work beside it.
    let pb::path_response::Outcome::Found(found) = ask("pmid:3".into(), "pmid:2".into(), 0).await
    else {
        panic!("no path 3 -> 2")
    };
    assert_eq!((found.hops, found.total, found.paths.len()), (2, 1, 1));
    assert_eq!(
        found.paths[0]
            .works
            .iter()
            .map(|w| w.public_id.clone())
            .collect::<Vec<_>>(),
        [public_text(3), public_text(1), public_text(2)]
    );
    assert!(found.paths[0].works[0].by.is_empty());
    assert_eq!(found.paths[0].works[1].by, ["pubmed", "crossref"]);
    // Hydrated with the same summary a page carries: work 1 is held and
    // titled, work 2 is a stub with names and no title.
    let held = found.paths[0].works[1].summary.as_ref().unwrap();
    assert_eq!((held.title.as_str(), held.stub), ("A titled work", false));
    assert_eq!(held.authors, ["Ada Lovelace"]);
    let stub = found.paths[0].works[2].summary.as_ref().unwrap();
    assert!(stub.stub && stub.title.is_empty() && !stub.names.is_empty());
    // The other direction is the other question: 2 cites nothing.
    let pb::path_response::Outcome::None(none) = ask("pmid:2".into(), "pmid:3".into(), 0).await
    else {
        panic!("2 -> 3 should have no path")
    };
    assert_eq!((none.depth, none.forward), (6, 1));
    assert!(none.backward >= 1);
    // A bound too short is no path and never a wrong one.
    let pb::path_response::Outcome::None(none) = ask("pmid:3".into(), "pmid:2".into(), 1).await
    else {
        panic!("3 -> 2 is two hops, not one")
    };
    assert_eq!(none.depth, 1);
    // A work to itself.
    let pb::path_response::Outcome::Found(found) = ask("pmid:1".into(), public_text(1), 0).await
    else {
        panic!("a work is not itself")
    };
    assert_eq!((found.hops, found.paths[0].works.len()), (0, 1));
    // Both caps refused, not clamped.
    for (depth, paths) in [(11, 0), (0, 11)] {
        let over = client
            .path(pb::PathRequest {
                from: "pmid:3".into(),
                to: "pmid:2".into(),
                depth,
                paths,
                hydrate: false,
            })
            .await
            .unwrap_err();
        assert_eq!(over.code(), Code::InvalidArgument);
    }
}

/// A server with no epoch it can read — one built for a version the
/// store has not been rebuilt for yet — refuses every call `UNAVAILABLE`
/// rather than exiting, and answers the moment an epoch is swapped in
/// (2026-09-24).
#[tokio::test]
async fn a_server_with_no_epoch_waits_and_then_answers() {
    let service = Arc::new(Service::<Fake>::waiting(Terms::stub()));
    let mut client = start(service.clone()).await;
    let status = client.epoch(pb::EpochRequest {}).await.unwrap_err();
    assert_eq!(status.code(), Code::Unavailable, "{status:?}");
    let status = client
        .resolve(pb::ResolveRequest {
            text: "doi:10.1000/one".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), Code::Unavailable, "{status:?}");

    service.swap(Fake::new(2));
    let info = client
        .epoch(pb::EpochRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.answered.unwrap().epoch, 2);
}
