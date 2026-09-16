//! The build (graph plan §8): two passes over the edges set. The first
//! counts every work's degrees in both orientations and collects the
//! asserter sets; the second writes the forward orientation in the set's
//! own `(from, to)` order and places the transpose by counting sort —
//! stable, so each citers list comes out sorted — with a label beside
//! every arc. The permutation is the identity: `WorkId` order is the one
//! ordering this version knows (plan §4). Files land in a `.part`
//! directory beside the target and replace it whole once the manifest is
//! written.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use merge::Sources;
use merge::record::Edge;

use crate::error::Error;
use crate::layout::{
    self, FORWARD_LABELS, FORWARD_NEIGHBOURS, FORWARD_OFFSETS, FileInfo, INV, Input, Manifest,
    PERM, Sink, TRANSPOSE_LABELS, TRANSPOSE_NEIGHBOURS, TRANSPOSE_OFFSETS,
};
use crate::nodes::{self, Nodes};

/// How to build.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// Arcs between held works only: the subgraph whose structure
    /// resembles the corpus's (plan §2).
    pub held: bool,
}

/// What a build left on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    #[allow(missing_docs)]
    pub dir: PathBuf,
    #[allow(missing_docs)]
    pub manifest: Manifest,
}

/// The edges set, read once per call: a build calls it twice and expects
/// the same edges in the same order both times.
pub trait Edges {
    /// Feeds every edge to `sink`, in the set's `(from, to)` order.
    ///
    /// # Errors
    ///
    /// The set could not be read, or `sink` stopped.
    fn each(&mut self, sink: &mut dyn FnMut(Edge) -> Result<(), Error>) -> Result<(), Error>;
}

impl<F> Edges for F
where
    F: FnMut(&mut dyn FnMut(Edge) -> Result<(), Error>) -> Result<(), Error>,
{
    fn each(&mut self, sink: &mut dyn FnMut(Edge) -> Result<(), Error>) -> Result<(), Error> {
        self(sink)
    }
}

impl Edges for Vec<Edge> {
    fn each(&mut self, sink: &mut dyn FnMut(Edge) -> Result<(), Error>) -> Result<(), Error> {
        self.iter().copied().try_for_each(sink)
    }
}

/// What the first pass counted.
struct Counted {
    out_degree: Vec<u32>,
    in_degree: Vec<u32>,
    arcs: u64,
    self_loops: u64,
    dropped: u64,
    /// Every distinct asserter set, sorted: a label is an index into it.
    table: Vec<Sources>,
}

struct Offsets {
    forward: Vec<u64>,
    transpose: Vec<u64>,
}

type Files = BTreeMap<&'static str, FileInfo>;

/// Builds both orientations of the graph over `nodes` from `edges` into
/// `dir`, replacing what was there.
///
/// # Errors
///
/// I/O; an edge out of order or with an end that is not a work; more
/// asserter sets than a byte indexes; an edges set that was not the same
/// in the second pass.
pub fn build<E: Edges>(
    dir: &Path,
    nodes: &Nodes,
    input: Input,
    options: Options,
    mut edges: E,
) -> Result<Built, Error> {
    let counted = count(nodes, options, &mut edges)?;
    let part = part_of(dir);
    if part.exists() {
        fs::remove_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    }
    fs::create_dir_all(&part).map_err(|e| Error::io(&part, e))?;
    let offsets = Offsets {
        forward: offsets_of(&counted.out_degree),
        transpose: offsets_of(&counted.in_degree),
    };
    let mut files = Files::new();
    files.insert(
        FORWARD_OFFSETS,
        write_u64s(&part, FORWARD_OFFSETS, &offsets.forward)?,
    );
    files.insert(
        TRANSPOSE_OFFSETS,
        write_u64s(&part, TRANSPOSE_OFFSETS, &offsets.transpose)?,
    );
    place(
        &part, nodes, options, &counted, &offsets, &mut edges, &mut files,
    )?;
    files.insert(PERM, write_u32s(&part, PERM, 0..nodes.len())?);
    files.insert(INV, write_u32s(&part, INV, 1..=nodes.len())?);
    let manifest = Manifest {
        graph: crate::VERSION,
        model: model::VERSION,
        merge: merge::VERSION,
        built_at: ingest::time::now(),
        ordering: layout::ORDERING.to_owned(),
        neighbours: layout::NEIGHBOURS.to_owned(),
        held_only: options.held,
        nodes: nodes.len(),
        works: nodes.works(),
        held: nodes.held(),
        stubs: nodes.stubs(),
        arcs: counted.arcs,
        self_loops: counted.self_loops,
        dropped: counted.dropped,
        asserters: layout::names_of(&counted.table),
        input,
        files: files
            .into_iter()
            .map(|(name, info)| (name.to_owned(), info))
            .collect(),
    };
    manifest.write(&part)?;
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::rename(&part, dir).map_err(|e| Error::io(dir, e))?;
    Ok(Built {
        dir: dir.to_path_buf(),
        manifest,
    })
}

/// Pass one: degrees, the asserter table, the counts.
fn count<E: Edges>(nodes: &Nodes, options: Options, edges: &mut E) -> Result<Counted, Error> {
    let n = usize::try_from(nodes.len()).expect("a u32 fits");
    let (mut out_degree, mut in_degree) = (vec![0u32; n], vec![0u32; n]);
    let (mut arcs, mut self_loops, mut dropped) = (0u64, 0u64, 0u64);
    let mut asserters = BTreeSet::new();
    let mut previous = None;
    edges.each(&mut |edge| {
        if !admit(nodes, options, &mut previous, edge)? {
            dropped += 1;
            return Ok(());
        }
        out_degree[nodes::index(edge.from)] += 1;
        in_degree[nodes::index(edge.to)] += 1;
        arcs += 1;
        if edge.from == edge.to {
            self_loops += 1;
        }
        asserters.insert(edge.by);
        Ok(())
    })?;
    if asserters.len() > 256 {
        return Err(Error::TooManyAsserters {
            count: asserters.len(),
        });
    }
    Ok(Counted {
        out_degree,
        in_degree,
        arcs,
        self_loops,
        dropped,
        table: asserters.into_iter().collect(),
    })
}

/// Checks order and ends; `Ok(false)` for an edge `--held` leaves out.
fn admit(
    nodes: &Nodes,
    options: Options,
    previous: &mut Option<Edge>,
    edge: Edge,
) -> Result<bool, Error> {
    if let Some(prev) = *previous
        && (edge.from, edge.to) <= (prev.from, prev.to)
    {
        return Err(Error::Unsorted {
            previous: prev,
            edge,
        });
    }
    *previous = Some(edge);
    if !nodes.present(edge.from) || !nodes.present(edge.to) {
        return Err(Error::UnknownEnd { edge });
    }
    Ok(!options.held || (nodes.is_held(edge.from) && nodes.is_held(edge.to)))
}

/// Pass two: the forward arrays streamed in file order, the transpose
/// placed by counting sort, every arc checked against the first pass.
fn place<E: Edges>(
    part: &Path,
    nodes: &Nodes,
    options: Options,
    counted: &Counted,
    offsets: &Offsets,
    edges: &mut E,
    files: &mut Files,
) -> Result<(), Error> {
    let mut neighbours = Sink::create(&part.join(FORWARD_NEIGHBOURS))?;
    let mut labels = Sink::create(&part.join(FORWARD_LABELS))?;
    let arcs = usize::try_from(counted.arcs).expect("arcs fit in memory");
    let (mut placed, mut placed_labels) = (vec![0u32; arcs], vec![0u8; arcs]);
    let mut cursor = offsets.transpose.clone();
    let (mut written, mut previous) = (0u64, None);
    let changed = |what: String| Error::Changed { what };
    edges.each(&mut |edge| {
        if !admit(nodes, options, &mut previous, edge)? {
            return Ok(());
        }
        let f = nodes::index(edge.from);
        if written < offsets.forward[f] || written >= offsets.forward[f + 1] {
            return Err(changed(format!(
                "{} has more or fewer references than counted",
                edge.from
            )));
        }
        let label = counted.table.binary_search(&edge.by).map_err(|_| {
            changed(format!(
                "{} -> {} asserted by {}, which the first pass never saw",
                edge.from, edge.to, edge.by
            ))
        })?;
        let label = u8::try_from(label).expect("at most 256 asserter sets");
        neighbours.put(&edge.to.get().to_le_bytes())?;
        labels.put(&[label])?;
        let t = nodes::index(edge.to);
        let at = cursor[t];
        if at >= offsets.transpose[t + 1] {
            return Err(changed(format!("{} has more citers than counted", edge.to)));
        }
        let ix = usize::try_from(at).expect("arcs fit in memory");
        placed[ix] = edge.from.get();
        placed_labels[ix] = label;
        cursor[t] += 1;
        written += 1;
        Ok(())
    })?;
    if written != counted.arcs {
        return Err(changed(format!(
            "{written} arcs in the second pass, {} in the first",
            counted.arcs
        )));
    }
    files.insert(FORWARD_NEIGHBOURS, neighbours.finish()?);
    files.insert(FORWARD_LABELS, labels.finish()?);
    files.insert(
        TRANSPOSE_NEIGHBOURS,
        write_u32s(part, TRANSPOSE_NEIGHBOURS, placed.iter().copied())?,
    );
    files.insert(
        TRANSPOSE_LABELS,
        write_bytes(part, TRANSPOSE_LABELS, &placed_labels)?,
    );
    Ok(())
}

/// Prefix sums: `degrees.len() + 1` offsets, the last the total.
fn offsets_of(degrees: &[u32]) -> Vec<u64> {
    let mut offsets = Vec::with_capacity(degrees.len() + 1);
    let mut total = 0u64;
    offsets.push(0);
    for d in degrees {
        total += u64::from(*d);
        offsets.push(total);
    }
    offsets
}

fn write_u64s(part: &Path, name: &str, values: &[u64]) -> Result<FileInfo, Error> {
    let mut sink = Sink::create(&part.join(name))?;
    for v in values {
        sink.put(&v.to_le_bytes())?;
    }
    sink.finish()
}

fn write_u32s(
    part: &Path,
    name: &str,
    values: impl Iterator<Item = u32>,
) -> Result<FileInfo, Error> {
    let mut sink = Sink::create(&part.join(name))?;
    for v in values {
        sink.put(&v.to_le_bytes())?;
    }
    sink.finish()
}

fn write_bytes(part: &Path, name: &str, bytes: &[u8]) -> Result<FileInfo, Error> {
    let mut sink = Sink::create(&part.join(name))?;
    sink.put(bytes)?;
    sink.finish()
}

/// The directory a build lands in before it replaces `dir`.
fn part_of(dir: &Path) -> PathBuf {
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("graph");
    dir.with_file_name(format!("{name}.part"))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use merge::record::Edge;
    use merge::{SourceId, Sources, WorkId};

    use super::{Options, build};
    use crate::base::Base;
    use crate::error::Error;
    use crate::layout::{self, Input};
    use crate::nodes::Nodes;

    pub(crate) fn id(n: u32) -> WorkId {
        WorkId::new(n).unwrap()
    }

    pub(crate) fn by(sources: &[SourceId]) -> Sources {
        sources.iter().copied().collect()
    }

    fn edge(from: u32, to: u32, sources: &[SourceId]) -> Edge {
        Edge {
            from: id(from),
            to: id(to),
            by: by(sources),
        }
    }

    /// Five works — 1 and 2 held by everyone, 3 held by PubMed, 4 a stub,
    /// 5 retired — and six edges in `(from, to)` order.
    pub(crate) fn sample() -> (Nodes, Vec<Edge>) {
        use SourceId::{Crossref, OpenAlex, Pubmed};
        let mut nodes = Nodes::new();
        nodes
            .add(id(1), false, by(&[Pubmed, Crossref, OpenAlex]))
            .unwrap();
        nodes
            .add(id(2), false, by(&[Pubmed, Crossref, OpenAlex]))
            .unwrap();
        nodes.add(id(3), false, by(&[Pubmed])).unwrap();
        nodes.add(id(4), true, Sources::NONE).unwrap();
        nodes.add(id(6), true, Sources::NONE).unwrap();
        let edges = vec![
            edge(1, 2, &[Pubmed, Crossref, OpenAlex]),
            edge(1, 4, &[Crossref]),
            edge(2, 1, &[OpenAlex]),
            edge(2, 3, &[Pubmed, Crossref]),
            edge(2, 4, &[Crossref]),
            edge(3, 3, &[Pubmed]),
            edge(3, 6, &[Pubmed]),
        ];
        (nodes, edges)
    }

    pub(crate) fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ol-rht-graph-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn input() -> Input {
        Input {
            merged: "v1.1".into(),
            shards: 1,
        }
    }

    #[test]
    fn both_orientations_and_their_labels() {
        let (nodes, edges) = sample();
        let root = scratch("build");
        let dir = root.join(layout::name(false));
        let built = build(&dir, &nodes, input(), Options::default(), edges).unwrap();
        assert_eq!(built.manifest.nodes, 6);
        assert_eq!(
            (
                built.manifest.works,
                built.manifest.held,
                built.manifest.stubs
            ),
            (5, 3, 2)
        );
        assert_eq!((built.manifest.arcs, built.manifest.self_loops), (7, 1));
        // The table is sorted by the merge's bit order.
        assert_eq!(
            built.manifest.asserters,
            vec![
                vec!["pubmed".to_owned()],
                vec!["crossref".to_owned()],
                vec!["pubmed".to_owned(), "crossref".to_owned()],
                vec!["openalex".to_owned()],
                vec![
                    "pubmed".to_owned(),
                    "crossref".to_owned(),
                    "openalex".to_owned()
                ],
            ]
        );
        let base = Base::open(&dir).unwrap();
        let refs = |n| {
            base.references(id(n))
                .map(|a| (a.work.get(), a.by.to_string()))
                .collect::<Vec<_>>()
        };
        let cites = |n| {
            base.citers(id(n))
                .map(|a| (a.work.get(), a.by.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            refs(1),
            vec![
                (2, "pubmed+crossref+openalex".to_owned()),
                (4, "crossref".to_owned())
            ]
        );
        assert_eq!(refs(4), vec![]);
        assert_eq!(refs(5), vec![]);
        assert_eq!(refs(7), vec![], "past the base");
        assert_eq!(
            cites(4),
            vec![(1, "crossref".to_owned()), (2, "crossref".to_owned())]
        );
        assert_eq!(
            cites(3),
            vec![(2, "pubmed+crossref".to_owned()), (3, "pubmed".to_owned())]
        );
        assert_eq!((base.out_degree(id(2)), base.in_degree(id(2))), (3, 1));
        assert_eq!(base.position(id(6)), Some(5));
        assert_eq!(base.work_at(5), Some(id(6)));
        assert_eq!(base.position(id(7)), None);
        crate::check::invariants(&base).unwrap();
        assert_eq!(crate::check::digests(&base).unwrap(), 8);
        // Held only: arcs between 1, 2 and 3.
        let held = root.join(layout::name(true));
        let built = build(&held, &nodes, input(), Options { held: true }, sample().1).unwrap();
        assert_eq!((built.manifest.arcs, built.manifest.dropped), (4, 3));
        let base = Base::open(&held).unwrap();
        assert_eq!(base.in_degree(id(4)), 0);
        assert_eq!(base.out_degree(id(2)), 2);
        crate::check::invariants(&base).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_rebuild_is_byte_identical() {
        let (nodes, edges) = sample();
        let root = scratch("twice");
        let a = root.join(layout::name(false));
        let b = root.join(format!("{}.again", layout::name(false)));
        build(&a, &nodes, input(), Options::default(), edges.clone()).unwrap();
        build(&b, &nodes, input(), Options::default(), edges).unwrap();
        assert_eq!(
            crate::check::differing(&a, &b).unwrap(),
            Vec::<String>::new()
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_build_refuses_bad_input() {
        let (nodes, edges) = sample();
        let root = scratch("refuse");
        let dir = root.join(layout::name(false));
        let mut unsorted = edges.clone();
        unsorted.swap(0, 1);
        assert!(matches!(
            build(&dir, &nodes, input(), Options::default(), unsorted),
            Err(Error::Unsorted { .. })
        ));
        let mut unknown = edges.clone();
        unknown.push(edge(6, 7, &[SourceId::Pubmed]));
        assert!(matches!(
            build(&dir, &nodes, input(), Options::default(), unknown),
            Err(Error::UnknownEnd { .. })
        ));
        // A set that shrinks between the passes.
        let mut calls = 0;
        let flaky = |sink: &mut dyn FnMut(Edge) -> Result<(), Error>| {
            calls += 1;
            let take = if calls == 1 {
                edges.len()
            } else {
                edges.len() - 1
            };
            edges[..take].iter().copied().try_for_each(sink)
        };
        assert!(matches!(
            build(&dir, &nodes, input(), Options::default(), flaky),
            Err(Error::Changed { .. })
        ));
        assert!(!dir.exists(), "a failed build leaves no directory");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
