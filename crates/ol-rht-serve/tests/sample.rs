//! The real server over a real store, when `OL_RHT_STORE` names one with
//! a published epoch; skipped otherwise. Before public ids are minted the
//! boundary refuses with a failed precondition, and that is what this
//! test then asserts; once they exist it walks the whole surface.

use std::sync::Arc;

use ingest::Store;
use ol_rht_serve::proto::v1::serving_client::ServingClient;
use ol_rht_serve::proto::v1::serving_server::ServingServer;
use ol_rht_serve::proto::v1::{self as pb, resolve_response::Outcome};
use ol_rht_serve::{Service, Terms};
use serve::Serving;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::Code;
use tonic::transport::Server;

/// A summary as a line: the title a held work has, or the names a stub
/// has instead, which is what makes a stub showable at all.
fn shown(summary: Option<&pb::Summary>) -> String {
    let Some(s) = summary else {
        return "(not hydrated)".to_owned();
    };
    if !s.title.is_empty() {
        return s.title.clone();
    }
    let names: Vec<String> = s
        .names
        .iter()
        .map(|n| format!("{}:{}", n.namespace, n.value))
        .collect();
    format!(
        "({}) {}",
        if s.stub { "stub" } else { "no title" },
        names.join(", ")
    )
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn the_sample_store_answers() {
    let Ok(root) = std::env::var("OL_RHT_STORE") else {
        eprintln!("OL_RHT_STORE not set; skipping");
        return;
    };
    let serving = Serving::open(&Store::new(root)).expect("an epoch");
    let epoch = serving.epoch().epoch;
    let service = Arc::new(Service::new(serving, Terms::stub()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(
        Server::builder()
            .add_service(ServingServer::from_arc(service))
            .serve_with_incoming(TcpListenerStream::new(listener)),
    );
    let mut client = loop {
        if let Ok(c) = ServingClient::connect(format!("http://{addr}")).await {
            break c;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    let info = client
        .epoch(pb::EpochRequest {})
        .await
        .unwrap()
        .into_inner();
    eprintln!(
        "epoch {epoch}: {} nodes, {} works, {} arcs of {}",
        info.nodes, info.works, info.arcs, info.merged
    );
    assert_eq!(info.answered.unwrap().epoch, epoch);
    let resolved = client
        .resolve(pb::ResolveRequest {
            text: "doi:10.1136/bmj.n71".into(),
        })
        .await;
    let public_id = match resolved {
        Err(status) if status.code() == Code::FailedPrecondition => {
            eprintln!("no public ids yet: {}", status.message());
            assert!(status.message().contains("no public id"));
            return;
        }
        Err(status) => panic!("{status}"),
        Ok(r) => match r.into_inner().outcome {
            Some(Outcome::PublicId(p)) => p,
            other => panic!("{other:?}"),
        },
    };
    eprintln!("the PRISMA statement is {public_id}");
    let work = client
        .get_work(pb::GetWorkRequest {
            work: public_id.clone(),
        })
        .await
        .unwrap()
        .into_inner()
        .work
        .unwrap();
    assert!(work.title.contains("PRISMA 2020"), "{}", work.title);
    // The sample's epoch 7 of 2026-09-19; it is also the most cited work
    // the graph holds.
    assert_eq!((work.references, work.citers), (143, 882));
    let mut citers = client
        .citers(pb::ListRequest {
            work: public_id.clone(),
            page: 100,
            cursor: String::new(),
            hydrate: true,
            limit: 250,
        })
        .await
        .unwrap()
        .into_inner();
    let mut pages = 0;
    let mut arcs = 0;
    while let Some(page) = citers.message().await.unwrap() {
        pages += 1;
        arcs += page.arcs.len();
        assert_eq!(page.total, 882);
        assert!(page.arcs.iter().all(|a| a.public_id.starts_with("ol:")));
    }
    assert_eq!((pages, arcs), (3, 250));
    let found = client
        .search(pb::SearchRequest {
            query: "prisma".into(),
            limit: 3,
            facets: 3,
            hydrate: true,
        })
        .await
        .unwrap()
        .into_inner();
    assert!(found.total >= 600, "{}", found.total);
    assert_eq!(found.hits.len(), 3);
    // A hit is found by its text, so there is a record behind it.
    assert!(found.hits.iter().all(|h| {
        h.summary
            .as_ref()
            .is_some_and(|s| !s.stub && !s.title.is_empty())
    }));
    // Co-citation: seed with the work's citers and walk their references.
    let seeds: Vec<String> = client
        .citers(pb::ListRequest {
            work: public_id.clone(),
            page: 100,
            cursor: String::new(),
            hydrate: false,
            limit: 100,
        })
        .await
        .unwrap()
        .into_inner()
        .message()
        .await
        .unwrap()
        .unwrap()
        .arcs
        .into_iter()
        .map(|a| a.public_id)
        .collect();
    assert_eq!(seeds.len(), 100);
    let expanded = client
        .expand(pb::ExpandRequest {
            seeds,
            budget: 5,
            hydrate: true,
            walk: pb::Walk::References.into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(expanded.neighbours.len(), 5);
    assert!(expanded.total > 5, "{} reached", expanded.total);
    for n in &expanded.neighbours {
        eprintln!(
            "co-cited {:.4} ({} of {}): {}",
            n.score,
            n.touches,
            n.degree,
            shown(n.summary.as_ref())
        );
        assert!(n.touches > 0 && u64::from(n.touches) <= n.degree);
        assert!(n.score > 0.0 && n.score <= 1.0);
    }
    // The subject is reached by every one of its own citers, so it heads
    // its own co-citation list wherever the question is asked with a
    // bare seed set; `related` is the call that knows the subject.
    assert_eq!(expanded.neighbours[0].public_id, public_id);
    // Ranked by the score and not by the raw overlap.
    assert!(
        expanded
            .neighbours
            .windows(2)
            .all(|w| w[0].score >= w[1].score)
    );
    let both = client
        .expand(pb::ExpandRequest {
            seeds: vec![public_id.clone()],
            budget: 5,
            hydrate: true,
            walk: pb::Walk::Both.into(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(both.neighbours.len(), 5);

    // The same question in one call, with the seeds the server's own.
    for (by, what) in [
        (pb::By::CitedAlongside, "cited alongside"),
        (pb::By::CitesSame, "cites the same"),
    ] {
        let related = client
            .related(pb::RelatedRequest {
                work: public_id.clone(),
                by: by.into(),
                limit: 20,
                hydrate: true,
            })
            .await
            .unwrap()
            .into_inner();
        eprintln!(
            "{what}: {} of {} shown, {} seeds used of {}",
            related.related.len(),
            related.total,
            related.seeds_used,
            related.seeds_total
        );
        assert!(!related.related.is_empty());
        assert!(related.related.len() <= 20);
        assert!(related.total >= related.related.len() as u64);
        // Bounded and honest: the seeds walked are the whole list or
        // what the arc budget allowed of it, and the answer says which.
        assert!(related.seeds_used > 0 && related.seeds_used <= related.seeds_total);
        // The subject is never in its own answer, which is the whole
        // reason the call exists (it scores 0.735 against itself).
        assert!(
            related.related.iter().all(|n| n.public_id != public_id),
            "the subject heads its own list unless it is dropped"
        );
        assert!(related.related.windows(2).all(|w| w[0].score >= w[1].score));
        // Every row is renderable, stub or not: a title, or names.
        for n in &related.related {
            let summary = n.summary.as_ref().expect("hydrated");
            assert_eq!(summary.public_id, n.public_id);
            assert!(
                !summary.title.is_empty() || !summary.names.is_empty(),
                "{} has neither a title nor a name",
                n.public_id
            );
            assert!(summary.title.is_empty() || !summary.stub);
        }
        eprintln!("  first: {}", shown(related.related[0].summary.as_ref()));
    }
    // Co-citation seeds with the work's citers, of which this one has
    // 882 — the most any work we hold has, and still well inside the
    // budget of arcs the seeding may walk, so the answer here is the
    // whole question and says so by using every seed there is.
    let whole = client
        .related(pb::RelatedRequest {
            work: public_id.clone(),
            by: pb::By::CitedAlongside.into(),
            limit: 0,
            hydrate: false,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!((whole.seeds_used, whole.seeds_total), (882, 882));
    assert_eq!(whole.related.len(), 20, "the default limit");

    // A path. On a sample this sparse most pairs have none, and that is
    // an outcome of its own rather than an empty list; whichever comes
    // back, it says how far the search looked.
    let answer = client
        .path(pb::PathRequest {
            from: public_id.clone(),
            to: public_id.clone(),
            depth: 0,
            paths: 0,
            hydrate: true,
        })
        .await
        .unwrap()
        .into_inner();
    // A work to itself is zero hops, one path, that work.
    match answer.outcome.unwrap() {
        pb::path_response::Outcome::Found(found) => {
            assert_eq!((found.hops, found.paths.len()), (0, 1));
            assert_eq!(found.paths[0].works[0].public_id, public_id);
        }
        other => panic!("{other:?}"),
    }
    // One of its own references is one hop away, and the arc's asserters
    // travel with it.
    let onward = client
        .references(pb::ListRequest {
            work: public_id.clone(),
            page: 1,
            cursor: String::new(),
            hydrate: false,
            limit: 1,
        })
        .await
        .unwrap()
        .into_inner()
        .message()
        .await
        .unwrap()
        .unwrap()
        .arcs[0]
        .public_id
        .clone();
    let answer = client
        .path(pb::PathRequest {
            from: public_id.clone(),
            to: onward.clone(),
            depth: 4,
            paths: 3,
            hydrate: false,
        })
        .await
        .unwrap()
        .into_inner();
    let pb::path_response::Outcome::Found(found) = answer.outcome.unwrap() else {
        panic!("a work it cites is one hop away")
    };
    assert_eq!(found.hops, 1);
    for path in &found.paths {
        assert_eq!(u32::try_from(path.works.len()).unwrap(), found.hops + 1);
        assert_eq!(path.works[0].public_id, public_id);
        assert_eq!(path.works[path.works.len() - 1].public_id, onward);
        assert!(path.works[0].by.is_empty());
        assert!(path.works[1..].iter().all(|w| !w.by.is_empty()));
    }
    // The other direction is the other question, and on a corpus this
    // sparse it is ordinarily answered by no path at all — which is an
    // outcome of its own, saying how far the search looked.
    let answer = client
        .path(pb::PathRequest {
            from: onward.clone(),
            to: public_id.clone(),
            depth: 6,
            paths: 0,
            hydrate: false,
        })
        .await
        .unwrap()
        .into_inner();
    match answer.outcome.unwrap() {
        pb::path_response::Outcome::Found(found) => {
            eprintln!("{onward} -> {public_id}: {} hops back", found.hops);
        }
        pb::path_response::Outcome::None(none) => {
            eprintln!(
                "{onward} -> {public_id}: none within {} hops ({} forward, {} backward)",
                none.depth, none.forward, none.backward
            );
            assert_eq!(none.depth, 6);
            assert!(none.forward >= 1 && none.backward > 1);
        }
        pb::path_response::Outcome::Stopped(stopped) => {
            panic!("the budget ran out at {} hops", stopped.proved)
        }
    }
    // Every cap is refused, never clamped.
    for bad in [
        pb::PathRequest {
            from: public_id.clone(),
            to: onward.clone(),
            depth: 11,
            paths: 0,
            hydrate: false,
        },
        pb::PathRequest {
            from: public_id.clone(),
            to: onward,
            depth: 0,
            paths: 11,
            hydrate: false,
        },
    ] {
        assert_eq!(
            client.path(bad).await.unwrap_err().code(),
            Code::InvalidArgument
        );
    }
}
