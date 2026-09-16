//! The transport over a fake: the id boundary, paging and cursors, the
//! caps, the errors, the terms, and an epoch switch under a live cursor.

use std::collections::BTreeMap;
use std::sync::Arc;

use merge::identity::Standing;
use merge::record::{Field, Flags, StatusBy};
use merge::{OwnedName, SourceId, Sources, WorkId};
use ol_rht_serve::answers::{Answers, Counts, Direction, Page};
use ol_rht_serve::proto::v1::serving_client::ServingClient;
use ol_rht_serve::proto::v1::serving_server::ServingServer;
use ol_rht_serve::proto::v1::{self as pb, resolve_response::Outcome};
use ol_rht_serve::{Service, Terms};
use search::Hits;
use serve::epoch::Manifest;
use serve::row::{Fields, Named, Record, Text};
use serve::{Resolved, Row};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::Code;
use tonic::transport::Server;

fn id(n: u32) -> WorkId {
    WorkId::new(n).unwrap()
}

fn public_text(n: u32) -> String {
    merge::public::encode(u64::from(n))
}

/// Works 1 (held, cites 2 and 3, cited by 3) and 2 and 3 (stubs); a
/// shared DOI on 2 and 3; id 9 retired by a split.
struct Fake {
    epoch: Manifest,
    rows: BTreeMap<WorkId, Row>,
    names: BTreeMap<String, Resolved>,
}

fn row(n: u32, names: &[&str], stub: bool) -> Row {
    Row {
        id: id(n),
        stub,
        public: Some(public_text(n)),
        names: names
            .iter()
            .map(|t| Named {
                name: OwnedName::parse(t).unwrap(),
                standing: Standing::Promoted,
                by: [SourceId::Pubmed].into_iter().collect(),
                native: !stub,
                cited_by: smallvec::SmallVec::new(),
            })
            .collect(),
        records: if stub {
            Vec::new()
        } else {
            vec![Record {
                source: SourceId::Pubmed,
                native: OwnedName::parse(names[0]).unwrap(),
                promoted: true,
                asserts: smallvec::smallvec![serve::row::Assert {
                    name: OwnedName::parse(names[0]).unwrap(),
                    field: "pmid".into(),
                    native: true,
                }],
                view: merge::view::Owned::default(),
            }]
        },
        rejected: Vec::new(),
        fields: Fields {
            title: (!stub).then(|| Field {
                value: "A <i>titled</i> work".into(),
                from: SourceId::Pubmed,
                others: smallvec::smallvec![merge::record::Other {
                    source: SourceId::Crossref,
                    value: "A titled work: with subtitle".into()
                }],
            }),
            abstract_in: if stub {
                Sources::NONE
            } else {
                [SourceId::Pubmed].into_iter().collect()
            },
            status: StatusBy::default(),
            ..Fields::default()
        },
        flags: Flags::default(),
        text: (!stub).then(|| Text {
            title: Some("A titled work".to_owned()),
            abstract_text: Some("The abstract.".to_owned()),
            authors: vec!["Ada Lovelace".to_owned()],
            venue: Some("Journal of Tests".to_owned()),
        }),
    }
}

impl Fake {
    fn new(epoch: u64) -> Self {
        let mut names = BTreeMap::new();
        names.insert("pmid:1".to_owned(), Resolved::One(id(1)));
        names.insert("doi:10.1000/one".to_owned(), Resolved::One(id(1)));
        names.insert("pmid:2".to_owned(), Resolved::One(id(2)));
        names.insert("pmid:3".to_owned(), Resolved::One(id(3)));
        names.insert(
            "doi:10.1000/shared".to_owned(),
            Resolved::Shared(vec![id(2), id(3)]),
        );
        names.insert("pmid:9".to_owned(), Resolved::Split(id(9)));
        for n in [1, 2, 3, 9] {
            names.insert(public_text(n), Resolved::One(id(n)));
        }
        let mut rows = BTreeMap::new();
        rows.insert(id(1), row(1, &["pmid:1", "doi:10.1000/one"], false));
        rows.insert(id(2), row(2, &["pmid:2", "doi:10.1000/shared"], true));
        rows.insert(id(3), row(3, &["pmid:3", "doi:10.1000/shared"], true));
        Self {
            epoch: Manifest {
                epoch,
                batch: epoch,
                at: format!("2026-09-16T00:00:0{epoch}Z"),
                merged: "v1.1".into(),
                artifacts: BTreeMap::new(),
            },
            rows,
            names,
        }
    }
}

impl Answers for Fake {
    fn epoch(&self) -> &Manifest {
        &self.epoch
    }

    fn watermark(&self) -> serve::Watermark {
        serve::Watermark {
            batch: self.epoch.batch,
            at: self.epoch.at.clone(),
        }
    }

    fn counts(&self) -> Counts {
        Counts {
            merged: "v1.1".into(),
            nodes: 9,
            works: 3,
            arcs: 3,
        }
    }

    fn resolve(&self, text: &str) -> Result<Option<Resolved>, serve::Error> {
        if text.starts_with("ol:") {
            let canonical = merge::public::canonical(text)
                .map_err(|e| serve::Error::NotAName(e.to_string()))?;
            return Ok(self.names.get(&canonical).cloned());
        }
        merge::Name::parse(text).map_err(|e| serve::Error::NotAName(e.to_string()))?;
        Ok(self.names.get(text).cloned())
    }

    fn work(&self, id: WorkId) -> Result<Option<Row>, serve::Error> {
        Ok(self.rows.get(&id).cloned())
    }

    fn public_of(&self, id: WorkId) -> Option<String> {
        (id.get() <= 9).then(|| public_text(id.get()))
    }

    fn page(&self, work: WorkId, direction: Direction, offset: u64, n: usize) -> Page {
        let by: Sources = [SourceId::Pubmed, SourceId::Crossref].into_iter().collect();
        let all: Vec<graph::Arc> = match (work.get(), direction) {
            (1, Direction::References) => vec![
                graph::Arc { work: id(2), by },
                graph::Arc { work: id(3), by },
            ],
            (1, Direction::Citers) => vec![graph::Arc { work: id(3), by }],
            (2 | 3, Direction::Citers) => vec![graph::Arc { work: id(1), by }],
            _ => Vec::new(),
        };
        let total = all.len() as u64;
        let arcs: Vec<graph::Arc> = all
            .into_iter()
            .skip(usize::try_from(offset).unwrap())
            .take(n)
            .collect();
        Page {
            by: if arcs.is_empty() { Sources::NONE } else { by },
            arcs,
            total,
        }
    }

    fn search(&self, text: &str, limit: usize, _facets: usize) -> Result<Hits, serve::Error> {
        let hits = if text.contains("titled") {
            vec![search::Hit {
                score: 1.5,
                id: id(1),
                title: Some("A titled work".into()),
            }]
        } else {
            Vec::new()
        };
        Ok(Hits {
            total: hits.len(),
            hits: hits.into_iter().take(limit).collect(),
            facets: vec![("/year".into(), vec![("2026".into(), 1)])],
        })
    }

    fn expand(&self, seeds: &[WorkId], budget: usize) -> Vec<(WorkId, u32)> {
        let mut out = Vec::new();
        if seeds.contains(&id(1)) {
            out.push((id(3), 1));
            out.push((id(2), 1));
        }
        out.truncate(budget);
        out
    }
}

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
            public_id: public_text(9)
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
    assert_eq!(first.arcs[0].names.len(), 2, "hydrated with promoted names");
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
    let expanded = client
        .expand(pb::ExpandRequest {
            seeds: vec!["pmid:1".into(), public_text(1)],
            budget: 1,
            hydrate: true,
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(expanded.neighbours.len(), 1);
    assert_eq!(expanded.neighbours[0].public_id, public_text(3));
    let no_seeds = client
        .expand(pb::ExpandRequest {
            seeds: vec![],
            budget: 0,
            hydrate: false,
        })
        .await
        .unwrap_err();
    assert_eq!(no_seeds.code(), Code::InvalidArgument);
}
