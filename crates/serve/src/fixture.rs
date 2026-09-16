//! A store built from records held in memory, for the tests of the delta
//! path: a full merge the way the binary does it — identity, registry,
//! the second pass, the merged sets — then every artifact and an epoch,
//! so that an incremental batch can be applied against it and the
//! result compared with a full merge over the same records.

use std::path::PathBuf;

use ingest::Store;
use merge::build::{Builder, Ids, OwnedAssert, OwnedCitation};
use merge::identity::Identity;
use merge::journal::{self, Kind, Line};
use merge::name::{Assertion, field_static};
use merge::public::{Key, Public};
use merge::title::Titles;
use merge::view::{Owned, Text, View};
use merge::{Document, Name, OwnedName, Registry, SourceId, WorkId};

use crate::epoch;
use crate::incremental::{Incoming, Reference};

/// The test key: 32 bytes nobody keeps.
pub(crate) fn key() -> Key {
    Key::from_bytes([7u8; 32])
}

/// A scratch store.
pub(crate) fn store(name: &str) -> Store {
    let root = std::env::temp_dir().join(format!(
        "ol-rht-serve-{name}-{}-{}",
        std::process::id(),
        ingest::time::now().replace([':', '-'], "")
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    Store::new(root)
}

fn owned(view: &View<'_>) -> Owned {
    Owned::of(view)
}

/// A record: its assertions in order, native first, with a title and a
/// view.
pub(crate) fn record(
    source: SourceId,
    names: &[&str],
    title: &str,
    view: View<'_>,
    text: Text,
) -> Incoming {
    let asserts: Vec<OwnedAssert> = names
        .iter()
        .enumerate()
        .map(|(i, n)| OwnedAssert {
            name: OwnedName::parse(n).unwrap(),
            field: if i == 0 { "native" } else { "foreign" }.into(),
            native: i == 0,
        })
        .collect();
    let view = View {
        titles: Titles {
            title: Some(title).filter(|t| !t.is_empty()),
            vernacular: None,
        },
        ..view
    };
    Incoming {
        source,
        native: OwnedName::parse(names[0]).unwrap(),
        asserts,
        view: owned(&view),
        text,
        relations: Vec::new(),
        references: Vec::new(),
    }
}

/// A reference by names alone.
pub(crate) fn cites(record: &mut Incoming, names: &[&str]) {
    record.references.push(Reference {
        names: names.iter().map(|n| OwnedName::parse(n).unwrap()).collect(),
        citation: OwnedCitation::default(),
    });
}

/// The text a work's document carries, by the merge's precedence, from
/// the records in hand.
fn text_of(work: &merge::record::Work<'_>, records: &[Incoming]) -> Text {
    let find = |source: SourceId| -> Option<&Incoming> {
        let promoted = work
            .records
            .iter()
            .find(|r| r.source == source && r.promoted)
            .or_else(|| work.records.iter().find(|r| r.source == source))?;
        records
            .iter()
            .find(|r| r.source == source && r.native.as_name() == promoted.native)
    };
    let mut text = Text::default();
    if let Some(r) = merge::document::abstract_source(work).and_then(find) {
        text.abstract_text.clone_from(&r.text.abstract_text);
    }
    if let Some(r) = work.fields.authors.as_ref().and_then(|f| find(f.from)) {
        text.authors.clone_from(&r.text.authors);
    }
    if let Some(r) = work.fields.venue.as_ref().and_then(|f| find(f.from)) {
        text.venue.clone_from(&r.text.venue);
    }
    text
}

/// A full merge over `records` into `store`, then every artifact and a
/// new epoch, as the binary would do it. Returns the epoch number.
#[allow(clippy::too_many_lines)]
pub(crate) fn full_merge(
    store: &Store,
    records: &[Incoming],
    deletions: &[(SourceId, &str)],
) -> u64 {
    let deleted: Vec<(SourceId, OwnedName)> = deletions
        .iter()
        .map(|(s, n)| (*s, OwnedName::parse(n).unwrap()))
        .collect();
    let alive: Vec<&Incoming> = records
        .iter()
        .filter(|r| {
            !deleted
                .iter()
                .any(|(s, n)| *s == r.source && *n == r.native)
        })
        .collect();
    let mut identity = Identity::new();
    for r in &alive {
        let assertions: Vec<Assertion<'_>> = r
            .asserts
            .iter()
            .map(|a| Assertion {
                name: a.name.as_name(),
                field: field_static(&a.field),
                native: a.native,
            })
            .collect();
        identity
            .push(
                r.source,
                &assertions,
                Titles {
                    title: r.view.title.as_deref(),
                    vernacular: r.view.vernacular.as_deref(),
                },
            )
            .unwrap();
    }
    identity.mark();
    for r in &alive {
        for reference in &r.references {
            let names: Vec<Name<'_>> = reference.names.iter().map(OwnedName::as_name).collect();
            identity.cite(r.source, &names);
        }
        for relation in &r.relations {
            if let Some(t) = &relation.target {
                identity.cite(r.source, &[t.as_name()]);
            }
        }
    }
    let res = identity.resolve(false).unwrap();
    let registry_path = store.registry();
    let mut registry = Registry::read(&registry_path).unwrap();
    let order = res.held_order();
    let stub_works = res.stub_works();
    let all: Vec<Vec<OwnedName>> = order
        .iter()
        .map(|(_, n)| n.clone())
        .chain(stub_works.iter().cloned())
        .collect();
    let at = ingest::time::now();
    let assignment = registry.assign(&all, &at, true);
    Registry::append(&registry_path, &assignment.lines).unwrap();
    let (held_ids, stub_ids) = assignment.ids.split_at(order.len());
    let mut ids = Ids {
        held: vec![WorkId::FIRST; res.works.len()],
        stubs: stub_ids.to_vec(),
    };
    for ((w, _), id) in order.iter().zip(held_ids) {
        ids.held[*w] = *id;
    }
    let mut builder = Builder::new(res, ids).unwrap();
    for r in &alive {
        assert!(builder.view(r.native.as_name(), &r.view.borrow()));
    }
    let from_of = |b: &Builder, native: &OwnedName| -> WorkId {
        let work = b
            .resolution()
            .works
            .iter()
            .position(|w| {
                w.records.iter().any(|rix| {
                    let rec = &b.resolution().records[*rix as usize];
                    b.resolution().arena.get(rec.native) == native
                })
            })
            .unwrap();
        b.ids().held[work]
    };
    for r in &alive {
        let from = from_of(&builder, &r.native);
        for reference in &r.references {
            let names: Vec<Name<'_>> = reference.names.iter().map(OwnedName::as_name).collect();
            builder.reference(from, r.source, &names, &reference.citation.borrow());
        }
        for relation in &r.relations {
            builder.relate(from, r.source, relation);
        }
    }
    let merged = builder.finish().unwrap();
    let merged_dir = merge::layout::dir(store);
    merge::layout::write(&merged_dir, &merged).unwrap();
    // Public ids for every id lacking one, then the journal line.
    let public_path = store.public();
    let mut public = Public::read(&public_path).unwrap();
    let lines = public.mint((1..=registry.minted()).filter_map(WorkId::new), &key(), &at);
    Public::append(&public_path, &lines).unwrap();
    let journal_path = store.journal();
    let last = journal::last(&journal_path).unwrap();
    let batch = last.map_or(1, |l| l.batch + 1);
    let size = |p: PathBuf| merge::delta::len(&p).unwrap();
    journal::append(
        &journal_path,
        &Line {
            batch,
            at: at.clone(),
            kind: Kind::Full,
            registry: registry.lines(),
            public: public.lines(),
            delta: std::collections::BTreeMap::new(),
            bytes: [
                ("registry".to_owned(), size(store.registry())),
                ("public".to_owned(), size(store.public())),
            ]
            .into_iter()
            .collect(),
            inputs: Vec::new(),
            records: alive.len() as u64,
            minted: assignment.minted,
            merged: assignment.merged,
            split: assignment.split,
            dropped: assignment.dropped,
        },
    )
    .unwrap();
    // The artifacts.
    let registry = Registry::read(&registry_path).unwrap();
    let public = Public::read(&public_path).unwrap();
    let names_dir = store.names(model::VERSION, merge::VERSION, crate::VERSION);
    crate::names::build(&names_dir, &merged_dir, &public, &registry, batch).unwrap();
    let owned_records: Vec<Incoming> = alive.iter().map(|r| (*r).clone()).collect();
    let documents: Vec<Document> = merged
        .works
        .iter()
        .filter(|w| !w.stub)
        .map(|w| {
            let borrowed = w.borrow();
            Document::of(&borrowed, Some(&text_of(&borrowed, &owned_records)))
        })
        .collect();
    let works_dir = store.works(model::VERSION, merge::VERSION, crate::VERSION);
    crate::works::build(&works_dir, &merged_dir, &documents, &public, batch).unwrap();
    let graph_dir = graph::layout::dir(store, false);
    let nodes = graph::Nodes::read(&merged_dir).unwrap();
    let edges = merged.edges.clone();
    graph::build::build(
        &graph_dir,
        &nodes,
        graph::layout::Input {
            merged: merge::layout::version(),
            shards: 1,
        },
        graph::build::Options::default(),
        edges,
    )
    .unwrap();
    let search_dir = search::dir(store);
    search::index::build(&search_dir, &documents, batch).unwrap();
    let n = epoch::current(store).unwrap().map_or(1, |c| c + 1);
    let artifact =
        |dir: &std::path::Path, manifest: &str| epoch::artifact(store, dir, manifest).unwrap();
    let manifest = epoch::Manifest {
        epoch: n,
        batch,
        at,
        merged: merge::layout::version(),
        artifacts: [
            ("names", artifact(&names_dir, crate::names::MANIFEST)),
            ("works", artifact(&works_dir, crate::works::MANIFEST)),
            ("graph", artifact(&graph_dir, graph::layout::MANIFEST)),
            ("search", artifact(&search_dir, search::index::MANIFEST)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect(),
    };
    epoch::publish(store, &manifest).unwrap();
    n
}
