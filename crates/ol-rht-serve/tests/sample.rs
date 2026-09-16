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
    assert_eq!((work.references, work.citers), (144, 607));
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
        assert_eq!(page.total, 607);
        assert!(page.arcs.iter().all(|a| a.public_id.starts_with("ol:")));
    }
    assert_eq!((pages, arcs), (3, 250));
    let found = client
        .search(pb::SearchRequest {
            query: "prisma".into(),
            limit: 3,
            facets: 3,
        })
        .await
        .unwrap()
        .into_inner();
    assert!(found.total >= 600, "{}", found.total);
    assert_eq!(found.hits.len(), 3);
    let expanded = client
        .expand(pb::ExpandRequest {
            seeds: vec![public_id],
            budget: 5,
            hydrate: true,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(expanded.neighbours.len(), 5);
    eprintln!("first neighbour: {}", expanded.neighbours[0].title);
}
