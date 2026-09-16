# Graph plan

**Status:** draft 1, 2026-09-15 — the citation graph as a serving artifact
over the merged layer. Written from the architecture summary of 2026-09-14
(folded here and retired), the [notes on it](inbox/citation-graph-architecture-notes.md)
of 2026-09-15, the merge plan with MM3 landed (its §6, §8, §9), and the
serving-shape decisions of 2026-09-15, which have no plan doc yet. Agreed
2026-09-16 as a discovery-first shape (§9, G1; §10), the numbering of §4
tentatively, its details to be walked through as they land; where this
draft amends the notes it says so (§10).
**Inputs:** [merge plan](merge-plan.md) §4 (`WorkId`), §6 (edges), §8 (the
layout); the notes; [ambitions](inbox/ambitions-beyond-citations.md) §4
(slices); [candidate management](inbox/candidate-management.md), the
product's side of the funnel.

## 1. What the graph is

The citation graph: one node per work, one arc per distinct edge of the
merged layer's edges set — `A → B` is "A cites B", merge plan §6 — in two
orientations, forward for a work's references and transpose for its
citers, each a compressed sparse row over positions (§4), memory-mapped,
with a label beside every incidence saying who asserted it (§3).

It is a cache, as the merged sets are a cache of the registry: built from
the edges set and the registry fold and nothing else, and a build from
the same inputs is byte-identical — a test in the registry's spirit
(merge plan §4), beside the merge's own. The edges set stays the record
of edges; the graph is the shape that answers in microseconds.

What it is for, in order: `references(work)` and `citers(work)`, bounded
and lazy, with the asserters of each arc; a work's degree without
decoding its list; the expansion step of the funnel, whose management is
the product's ([candidate management](inbox/candidate-management.md));
the quality figures a build computes for free — agreement per pair of
sources, each source's arcs alone, corpus-wide and on every rebuild; and,
later, one graph per *slice* of the ambitions' typed edges (work → person,
work → funder, work → method), each with a label type of its own.

What it is not: not the edges' record (the edges set and the registry
are); not the candidate manager, whose funnel, best-first search and
scoring are the product's plan; not the search index, which answers
facet-shaped questions (ambitions §2); and not a store of positions — a
`WorkId` is what goes in and what comes out.

Vocabulary, adding to the merge plan's §2:

| term                    | meaning                                                                                                                                   |
|-------------------------|-------------------------------------------------------------------------------------------------------------------------------------------|
| **node**                | a work in the graph, held or stub, by `WorkId`                                                                                            |
| **position**            | a node's index in a base: `pos = perm[id − 1]`, `id = inv[pos]`; recomputed at every compaction and never stored outside the base         |
| **arc**, **incidence**  | one edge in one orientation; an edge is two incidences                                                                                    |
| **label**               | what is stored beside an incidence: for citation edges the asserting sources; for a derived slice its process and version                 |
| **base**                | an immutable pair of orientations in position space with their labels, built at compaction                                                |
| **delta**               | an append-only log in `WorkId` space of what changed since the base                                                                       |
| **epoch**               | one base, its permutation and the delta's cut point, published together with the other serving artifacts                                  |
| **compaction**          | a full rebuild of the base from the edges set and the registry fold, cutting the delta                                                    |
| **slice**               | one (from kind, edge kind, to kind); the citation graph is the work → work `cites` slice                                                  |

## 2. Numbers

Planning numbers, from the notes and the ingestion plan's §2. No source
exposes edges at the summary's 2 × 10^10: Semantic Scholar's `citations`
is 2.4 × 10^9 rows, Crossref and OpenAlex are of the same order, and the
June 2026 OpenAlex snapshot holds 510,372,821 works. The POC part file's
50,724 references over 10,873 works is 4.7 a work corpus-wide; the
sample's 24 to 54 a work is what journal articles with deposited lists
look like, not the corpus.

**N = 6 × 10^8** works — OpenAlex's plus stubs, merge plan §11 item 4 —
and **M = 3 × 10^9** distinct edges, the union: on the sample OpenAlex
holds three quarters of Crossref's edges where both hold the citing work
(merge plan §6), and every source adds some of its own. Raw CSR at those
numbers:

| component                                       | per orientation | both      |
|-------------------------------------------------|----------------:|----------:|
| neighbours, `M × u32`                           | 12 GB           | 24 GB     |
| offsets, `(N + 1) × u64`                        | 4.8 GB          | 9.6 GB    |
| **graph**                                       | **17 GB**       | **34 GB** |
| labels, `M × u8` (§3)                           | 3 GB            | 6 GB      |
| permutation and inverse, `N × u32` each (§4)    |                 | 4.8 GB    |

Against the summary's 176 GB. The consequence is a change of emphasis,
not of structure: the storage problem the summary was organised around is
not there; a raw memory-mapped CSR is both the baseline and the first
candidate; compression buys page-cache residency on a machine with less
memory than the graph; and the 5–15 bits per edge range is a measurement
to make, not a budget to plan on — at 8 bits an edge both orientations
are 6 GB before indexes. Offsets stay `u64`: 3 × 10^9 fits a `u32` with
room to spare, and the growth does not. `WorkId` is a `NonZeroU32`,
one-based, so `Option<WorkId>` is four bytes; 6 × 10^8 of 4.29 × 10^9
used.

The box, known 2026-09-16: 128 GB of memory and several TB of SATA SSD,
shared with the search engine, so the graph plans on a share of it and
not the box. Raw at full scale — 45 GB with labels and both numbering
tables — would fit and is greedy; the point of compression (§6) is the
size of the resident set, which G3 measures: at 8 bits an edge both
orientations are 6 GB, and with `perm`, `inv` and an offsets index about
12 GB resident. The transpose builds in one pass (§8): its 12 GB array
fits.

On the sample: 4,039,076 nodes and 5,021,301 edges, 3,956,332 of the
nodes stubs with citers and no references; 20 MB of neighbours and 32 MB
of offsets an orientation, raw. The held-works subgraph — 82,744 nodes,
68,813 edges — is the one whose structure resembles the corpus's.

## 3. Provenance: the label array

The merged layer's edge is `Edge { from, to, by: Sources }`
(`merge::record`), `Sources` a byte with one bit per `SourceId`, three
used, the enum appending. A CSR with no place for `by` loses what the
merge was built to keep. On the sample it is not a detail: of 5,021,301
distinct edges, 2,222,128 are OpenAlex's alone, 1,909,179 Crossref's
alone, 368,316 PubMed's alone, 483,820 PubMed's and Crossref's, 22,248
all three's; between held works one edge in five has a single asserter
(13,722 of 68,813).

What needs it: who asserted this arc; the arcs two or more sources agree
on; one source's arcs alone, corpus-wide and on every rebuild — the 28 %
of Crossref's edges OpenAlex misses on the sample is a quality figure the
report should keep producing at scale, and the graph is where it is
cheap; and later, arcs asserted by a process rather than a source
(ambitions §6), whose asserter is a process and a version.

| option                                                                                            | cost                                                            | what it cannot do                                                                                |
|---------------------------------------------------------------------------------------------------|-----------------------------------------------------------------|--------------------------------------------------------------------------------------------------|
| **a. pure index** — the CSR holds neighbours; `by` stays in the edges set                         | nothing in the graph                                            | filter while traversing; answer "who said" without a second read into the merged sets            |
| **b. parallel label array** — one byte an incidence beside each orientation's neighbour array    | 3 GB an orientation at M = 3 × 10^9, under a fifth of the graph | nothing; a filter is a byte compare, alignment is free                                           |
| **c. one graph per source** — three forward, three transpose                                      | six mappings, and a fourth source multiplies them               | "agreed" is an intersection at query time; a provenance byte is cheaper than any of it           |

**Decided: b**, from the notes. The label type is per slice. With
BVGraph the same byte goes in an arc-label stream — WebGraph has
arc-labelled graphs — or in a raw array indexed by a cumulative-degree
index in Elias–Fano, about `N × 4` bits; either way a byte an incidence,
so the format question (§6) does not change this one.

**What the byte is** — this draft's amendment to the notes (§10): not
`Sources` itself but an index into the build's table of distinct asserter
sets, written in the manifest; seven entries on the sample. A filter is a
lookup in a 256-entry mask computed once a query, which costs nothing,
and the graph's byte depends on how many distinct combinations a build
sees, not on how many sources exist. A smaller win than it sounds — on
the sample all seven possible combinations occur, so eight edge-asserting
sources would show most of their 255 — and its real value is that a
source with no reference lists never counts against the byte, and the
graph's format stops depending on the merge's type. `Sources` is a byte
and `SourceId` appends, so eight sources is the *merged* layer's ceiling,
and the ingestion plan's planned sources and long tail name at least nine
more beyond the three held (Semantic Scholar, arXiv, DataCite, NIH-OCC,
OpenCitations, Europe PMC, CORE, BASE, DBLP): it widens to `u16` before
the ninth source, a merged-layout version bump (merge plan §11, item 19),
and the label array does not follow it. More than 256 distinct asserter
sets in one build stops the build — fail fast — and the day it happens
the byte becomes two. Decided 2026-09-16: the table, because there will
be more than eight sources. And one requirement dropped the same day:
filtering by asserter while traversing is not one; "who asserted this"
is an annotation on what a query returned, which the byte beside each
arc gives for free.

## 4. Ordering: position space

`WorkId`s are minted in arrival order and never change; the merge plan
says so in one line (§4: "they promise stability, not locality"). Between
that and "compression is ordering-dependent" the graph has a **position
space** of its own: `pos = perm[id − 1]`, `id = inv[pos]`, two `u32`
arrays of 2.4 GB each at N = 6 × 10^8, recomputed at compaction and
nowhere else. The delta lives in `WorkId` space (§5), so the permutation
is free to change with every base, and an id minted after the base —
`id − 1 ≥ perm.len()` — has no position and answers from the delta alone.
Nothing outside a base holds a position; the API takes and returns
`WorkId`s.

What the order buys depends on the format:

|                        | raw CSR                                                     | BVGraph                                                          |
|------------------------|-------------------------------------------------------------|------------------------------------------------------------------|
| what ordering affects  | which lists share pages: locality during expansion          | gap sizes and reference coding: bits per edge, and locality      |
| neighbour values       | `WorkId`s, each list sorted by position                     | positions; lists sorted by position                              |
| arrays resident        | `perm`, at the entry of each hop                            | `perm` and `inv`, `inv` on every neighbour returned              |

Candidate orderings, cheapest first:

1. **`WorkId` order** — nothing to compute; `perm` is the identity. G2's
   build and the experiment's control.
2. **Year descending, then venue, then `WorkId`** — from the promoted
   fields. References point backwards in time and cluster by venue and
   field, so consecutive works share references and reference coding has
   something to copy. Descending so that a citers list begins with the
   newest citers, which makes the bounded expansion of a high-degree work
   a prefix read with a defensible default order. Stubs have no year —
   names only — and go last in `WorkId` order; 98 % of the sample's
   works, an artifact of the island, perhaps a sixth of the corpus's
   under §2's numbers.
3. **Topic, then year** — OpenAlex `primary_topic`, or a MeSH tree
   position, as the first key.
4. **A graph-derived order** — layered label propagation or a BFS order,
   WebGraph's own tools, hours over 3 × 10^9 edges on the box. The best
   compression, and the only order that helps cross-venue citing.

G2 builds 1; the experiment (G3) measures 2 and 4 against it on two
figures: bits per edge, and pages touched by a two-hop expansion from a
random held work, which is the locality figure that matters for raw CSR.

The tension: BVGraph needs each list sorted by position, and bounded
expansion needs each list to begin usefully. Order 2 makes those the same
order. Under order 4 the prefix means nothing, and bounded expansion needs
a side table — the top-*k* citers by year of every work over some
degree — which is small and is counted as part of that ordering's cost.

## 5. Updates: base, delta, compaction

"Citation data arrives append-only" is false for this pipeline. PubMed
deletes records; OpenAlex retires ids into others (`merged_ids`); the
registry writes `merge` and `split` lines whose edges follow the
surviving id (merge plan §4); a Crossref re-deposit replaces a reference
list, so edges vanish; the merge plan's incremental merges (§9, after MM5)
are exactly these. A delta that can only add is wrong from the first
snapshot diff.

The delta stays, for the serving-shape reasons of 2026-09-15: incremental
from the start, not an optimisation later; a record on disk to available
in every index under ten minutes, hoped under a second; base + delta +
compaction as the one shape for every serving artifact; one epoch that
all of them publish together; and a test that incremental over batches
equals a full rebuild.

- **Base:** forward and transpose in position space (§4), immutable,
  built at compaction from the edges set and the registry fold, with
  their label arrays, `perm`, `inv` and a manifest.
- **Delta:** an append-only edge log in `WorkId` space, written by the
  incremental merge beside the merged sets it changes (merge plan §11,
  item 20), three line kinds: `add(from, to, by)`; `remove(from, to,
  by)`, a source withdrawing an assertion; `retire(id, into)`, the
  registry's `merge` line echoed so the graph can answer for a retired id
  without the registry — `into` absent for a `split`, whose parts are new
  ids with `add` lines of their own. Folded at start into a map from edge
  to its effective label — the base's label, then the log's adds and
  removes in order; an edge whose label is empty is gone — plus the
  retirements, both ways. The fold is bounded because compaction bounds
  the log.
- **Query:** the base's list for `perm[id − 1]`, each arc's label replaced
  by the fold's where the fold has that edge, the fold's arcs not in the
  base merged in by id, empty labels skipped; a retired id answers as
  its survivor. Results are `WorkId`s, `inv` applied to base hits where
  the format stores positions. Landed 2026-09-16 (G4, `serve::fold`):
  the incremental merge moves a retired id's remaining arcs and its
  citers to the survivor with explicit `remove` and `add` lines rather
  than leaving the union to the reader, so the fold is a map of
  effective labels and nothing else, and the `retire` line only says
  which id answers for which.
- **Compaction:** rebuild the base from the edges set and the registry
  fold, recompute the permutation, cut the log at the epoch, switch the
  mapping and the fold together. On the box a full rebuild is two passes
  over 3 × 10^9 edges: the forward orientation is the edges set's own
  order (§8), the transpose a counting sort by target into a 12 GB
  array — minutes to tens of minutes, cheap enough nightly; on the box's
  SSD the 40 GB of writes alone are a minute and a half. The freshness
  target is met by the delta path, not by rebuild speed; compaction only
  bounds the delta. Asked 2026-09-16 whether a rebuild is slow enough to
  need the delta at all: it is not, and the merge before it is the slow
  part; the delta exists for the ten-minute target, and a daily target
  would drop it (§10).
- **The format and the update design are independent.** A BVGraph base
  is as immutable as a raw one; the delta is identical either way. The
  delta is settled first; it does not wait on the experiment.
- **The test:** the same edges set reached through a base and *n* deltas
  and through one build answers every query identically, and compacts to
  byte-identical files. It is the merge's zero-churn and byte-identical
  tests extended one layer down.

Upstream of the graph, ten minutes means the identity pass and the edge
resolution are incremental too: the names table becomes a live index
rather than a fold at run start, a new record's references resolve
against it, and an unheld name mints a stub as a registry append. That is
the [serving plan](serving-plan.md)'s §5, noted here because the delta
log has to be fed by something.

## 6. Format

Three candidates, measured on the same edges set with the same
permutation (§9, G3):

1. **Raw memory-mapped CSR** — offsets `u64`, neighbours `u32`, labels
   `u8`, one file each per orientation. Simple, predictable, the baseline
   and the first candidate (§2); a random access is one offsets read and
   one contiguous list.
2. **Compressed CSR** — packed successor ids and Elias–Fano degree
   offsets: the middle control that isolates what BVGraph's list
   similarity and reference coding are worth beyond plain packing.
3. **BVGraph** — gaps, intervals, references to earlier lists,
   instantaneous codes; an Elias–Fano offsets index; memory-mapped;
   transposition and permutation as tools. Web-graph compression rates do
   not transfer: citation graphs have structure — temporal direction,
   venue and field clustering, skewed in-degree — but the rate is
   measured, not assumed; 5–15 bits an edge is the range to expect, not a
   budget.

The dependency for 3 is the Rust port from the LAW lab (Vigna and
others): `webgraph`, with `webgraph-algo`, `webgraph-cli`, `sux`,
`dsi-bitstream` and `epserde`. Versions on crates.io, checked 2026-09-15:

| crate           | version | released   |
|-----------------|---------|------------|
| `webgraph`      | 0.6.2   | 2026-05-01 |
| `webgraph-algo` | 0.6.2   | 2026-05-01 |
| `webgraph-cli`  | 0.4.2   | 2026-05-01 |
| `sux`           | 0.14.0  | 2026-05-01 |
| `dsi-bitstream` | 0.10.1  | 2026-07-23 |
| `epserde`       | 0.13.1  | 2026-07-23 |

Still 0.x with a moving API; the Java implementation is the mature one.
Before G3: pin these, confirm the memory-mapped and arc-labelled paths in
the port (not verified — only the versions were), note the build time and
the dependency count. The format is documented and stable, so a reader of
our own is the fallback if the crate stalls.

For the record: one combined list with a direction flag an incidence is
two CSRs interleaved, 1.7 % smaller and slower to read; not proposed.

## 7. Latency and traversal

One work's references and citers are one neighbourhood read in each
orientation. Planning figures: 0.2–2 ms for a cold-ish read from a local
SSD — the box's is SATA, not NVMe, so the upper half of that range; tens
of microseconds warm, under 100 µs as the warm figure; 1 ms a node as the
deliberately bad capacity number. A BVGraph random access
decodes a chain of referenced lists bounded by the window, so one lookup
touches a few lists — still well under a millisecond warm.

Recursive expansion is a different problem: with 20 references and 20
citers a hop returns 40 neighbours, nominally 1,600 second-hop and 64,000
third-hop incidences before overlap, and real neighbourhoods overlap
heavily; it is a batch problem — deduplication, locality, ranking — and
belongs to candidate management. What the graph owes it is its API:
`references(work)` and `citers(work, limit)` bounded and lazy, never an
allocated vector of a methods paper's hundred thousand citers; a work's
degree readable from the offsets without decoding its list; the order
inside a list decided at build (§4) and documented; and the label on
every arc returned.

## 8. Machinery

- **`crates/graph`**, source-agnostic: the build, the base's files and
  manifest, the reader, the delta fold, the report. Depends on `merge`
  (`WorkId`, `Sources`, the edges set through `merge::layout`) and
  `ingest` (the store); on no source crate. The merge plan's §8 rules
  apply: stream, measure, fail fast.
- **The build reads the edges set once per orientation.** The set is
  sorted by `(from, to)` and deduplicated with `by` merged, sharded by
  citing work (merge plan §8), so the forward orientation is its file
  order: degrees counted, offsets written, neighbours and labels
  appended, one pass. The transpose is a counting sort by target: one
  pass for in-degrees, one to place; at scale a 12 GB neighbour array
  written through a mapping, which the box holds (§2), with *k* passes
  over target ranges as the fallback for a busy box — the merge's range
  trick. Under an ordering other
  than 1, positions are applied as the arcs are placed and each list is
  sorted at the end.
- **Layout**, under the store: `graph/v<model>.<merge>.<graph>/` — a
  cache, replaced whole by every build (made beside it as `.part`, then
  renamed over it), the graph having a version of its own that any
  change to its files or their meaning bumps — holding `manifest.json`
  (the three versions, the ordering, N, M, the counts, the asserter
  table of §3, the merged directory's name and shard count, and each
  file's size and SHA-256), `perm.u32`, `inv.u32`, and
  `forward.offsets.u64`, `forward.neighbours.u32`, `forward.labels.u8`
  with their three `transpose.*` twins; later, one epoch directory
  beside the delta. Little-endian, no framing, so a reader is a memory
  map and a slice: it checks the versions and every file's size on open
  and leaves the digests to `check`. The held-works subgraph lands
  beside the graph as `v<…>-held`.
- **Fail fast.** An edge whose end is not a work stops the build (the
  merge's invariant, checked again), as does an edge out of order, more
  than 256 asserter sets (§3), or a second pass that does not agree with
  the first arc for arc; a manifest whose versions are not the binary's
  stops the reader, and so does a file whose size is not what the counts
  imply. A failed build leaves no directory. Invariants `check` verifies:
  the permutation a bijection; offsets from zero, non-decreasing, summing
  to M; every list strictly sorted by position, every neighbour a work,
  every label in the table; every forward arc mirrored in the transpose
  with the same label; every file the digest its manifest says.
- **Commands.** `ol-rht graph build [--store <root>] [--held]` builds both
  orientations from the merged directory of this binary's versions and
  prints the report; `--held` restricts to arcs between held works, the
  sample's 68,815-arc subgraph. `ol-rht graph query <#id|name> [--store
  <root>] [--held] [--limit N]` prints a work's names with their
  standings, then its references and citers with their asserters, fifty
  a list unless `--limit` says otherwise and "… and n more" after; a
  name is the merge plan's text form (`doi:…`, `pmid:…`) found by
  scanning the names sets, an id past the base says so. `ol-rht graph
  check [--store <root>] [--held]` verifies the digests and the
  invariants, rebuilds into a `.check` directory beside the graph and
  compares the eight arrays and the manifest without its timestamp byte
  for byte, then removes the rebuild.
- **The report.** Nodes, works, held, stubs and retired ids; arcs and
  self-citations; per orientation the works with any, p50, p90, p99,
  p99.9 and the maximum with its work; the ten most cited; the works
  with no arc either way; arcs by asserter set; agreement per pair of
  sources twice — corpus-wide, and over arcs whose citing work both
  sources hold and whose target is held, which is the merge plan's §6
  table and must equal it; bytes per file and bits an arc; time and
  allocations on stderr, as every report has.

## 9. Milestones

- **G1** — this plan agreed; merge plan MM4 points here. *Agreed
  2026-09-16 (§10): discovery first, optimisation later; the numbering of
  §4 tentatively; the delta green-lit, with its need questioned and
  answered.*
- **G2** — the raw CSR: `crates/graph`, both orientations in `WorkId`
  order with labels and the asserter table, the manifest, `build`,
  `query` and `check`, the report, the byte-identical test, on the sample
  with and without stubs. This is the merge plan's MM4. *Landed
  2026-09-16: `crates/graph` — `nodes`, `layout`, `build`, `base`,
  `check`, `report` — with `memmap2` as the one new dependency and ten
  tests of its own; `ingest::Store` gained `graph()`. On the sample:
  4,039,078 nodes (4,039,076 works, 2 retired ids), 5,021,301 arcs, 399
  self-citations, seven asserter sets, 140 MiB on disk (91.5 bits an arc
  an orientation, 234 with the permutation), built in 2.5 s at 410 MB
  resident; `check` verifies the digests and every invariant in 0.4 s
  and finds the rebuild byte-identical, 2.9 s in all; the held-works
  build is 68,815 arcs in 1.6 s. The graph's agreement table conditioned
  on both sources holding the citing work reproduces the merge's §6
  figures exactly — 39,462 / 3,259 / 24,530, 23,269 / 9,712 / 15,403,
  36,837 / 14,676 / 1,730 — which is the build's correctness check
  against an independent count. A query by id answers in 35 ms, by name
  in 56 ms when the work is in the first shard. What the report showed:
  the most cited work on the sample is the PRISMA 2020 statement
  (`#33232`, 607 citers); one chemistry review carries 6,442 references;
  a held work with any references has 68 at the median and 181 at p90;
  41,922 works have no arc either way (§11, item 11).*
- **G3** — the experiment: orderings 2 and 4 against 1 (§4), compressed
  CSR and BVGraph against raw (§6), on the sample's two subgraphs for
  tooling and correctness and on the corpus for bits per edge — total
  bytes with indexes, build time, warm and cold one-hop p50/p95 in both
  orientations, batched throughput, deduplicated expansion, pages touched
  by a two-hop expansion, and behaviour on the highest-degree works.
  Decides the format and the ordering.
- **G4** — the delta: the log, the fold, the composed query, compaction,
  the base-plus-deltas test (§5); fed by the merge plan's incremental
  merges, published under the serving plan's epoch (its §5, SP5). Stands
  while the ten-minute freshness target stands; a daily target would
  drop it. *Landed 2026-09-16 with the serving plan's SP5: the edges log
  is `merged/v<…>/delta/edges.jsonl` (`merge::delta`), the fold and the
  merged list are `serve::fold`, and the base-plus-deltas test runs on a
  store built in memory and on the sample with one PubMed file held
  out. The graph crate itself did not change: the base is still the
  base, and a label lookup for the fold is a binary search over a
  work's list.*
- Then: slices for other kinds as their registries land (ambitions §4);
  the side table of §4 if the ordering needs it.

## 10. Decision log

- 2026-09-15, planning numbers — N = 6 × 10^8, M = 3 × 10^9 (from the
  notes); the summary's 10^9 and 2 × 10^10 retired with it. Raw CSR is
  34 GB for both orientations, so the graph is a locality and freshness
  problem, not a storage one; compression is measured, not budgeted.
  Offsets `u64`.
- 2026-09-15, the label array — option b (from the notes): a byte an
  incidence beside each orientation, a label type per slice. Amended in
  this draft: the byte indexes the build's table of asserter sets rather
  than storing `Sources`, so the merged layer's `Sources` can widen to
  `u16` before the ninth source without the graph noticing; more than
  256 sets stops the build.
- 2026-09-15, the delta layer (from the notes) — append-only is false for
  this pipeline, so the delta carries `add`, `remove` and `retire`; base +
  delta + compaction is the serving shape's; the format and the delta are
  independent; base-plus-deltas equals a full build is the test. Refined
  here: `retire` without `into` for a split; the fold is per-edge
  effective labels, an edge going when its label empties; an id past the
  base answers from the delta alone.
- 2026-09-15, position space (from the notes) — `perm` and `inv` are the
  base's, recomputed at compaction; the delta and the API speak `WorkId`;
  four candidate orderings, G2 builds the first, G3 measures the second
  and fourth.
- 2026-09-15, the graph is a cache — of the edges set and the registry
  fold, byte-identical from the same inputs; its own version in the store
  path; replaced whole.
- 2026-09-15, the summary folded — its numbers corrected (§2), its
  direction-bit section reduced to one line (§6), its residue dropped, its
  candidate-management section moved to a note of its own; the file
  retired. A miscount corrected on the way: OpenAlex misses 28 % of
  Crossref's edges on the sample where both hold the citing work, not a
  fifth (merge plan §6, and the notes' §2).
- 2026-09-16, the plan agreed (G1) — as a discovery-first shape: the raw
  build first, optimisation when measurements ask for it; the numbering
  of §4 tentatively, its details to be reported as they land; the graph
  changing between builds expected, since the data is mutable. The delta
  was green-lit with a question: whether a rebuild is slow enough to need
  it. It is not — a rebuild on the box is minutes (§8), and the merge
  before it is the slow part. The delta exists for the ten-minute
  freshness target of the serving-shape decisions, at every layer at
  once; a daily target would drop G4 altogether.
- 2026-09-16, the label table — chosen over the bitmap, because there
  will be more than eight sources. "Who asserted this" is an annotation
  on a query's result, not a filter on the query path, which the byte
  beside each arc serves for free (§3).
- 2026-09-16, the split's `retire` line — accepted: `into` absent when
  there is no survivor.
- 2026-09-16, the box — 128 GB of memory, several TB of SATA SSD, shared
  with the search engine, so the graph plans on a share: compression is
  for the resident set (§2), the transpose builds in one pass (§8), the
  cold-read figure is the upper half of its range (§7).
- 2026-09-16, G2 landed — the transpose is a stable counting sort and
  needs no sort of its own, because the edges set arrives in `(from, to)`
  order; the identity permutation is written all the same so the reader
  has one path for every ordering; the manifest carries the merged
  directory's name and shard count rather than the plan's registry line
  count, since the merged sets are the graph's actual input and the fold
  costs twenty seconds; the manifest's timestamp is the one byte that
  differs between two builds, so `check` compares the arrays and the
  manifest without it. The crate's own tests caught `inv` written as
  positions rather than ids before any run. The one requirement the
  agreement table adds: the graph's conditioned figures must equal the
  merge report's, and they do.

## 11. Open items

1. ~~**The box's memory.**~~ Known 2026-09-16 (§2): 128 GB shared with
   the search engine, SATA SSD. Left open: the graph's share, to set when
   the search engine's own footprint at scale is measured.
2. **`Sources` width.** Merge plan §11, item 19: `u16` before the ninth
   source, a merged-layout version bump; the graph's byte is independent
   of it (§3).
3. ~~**The delta's framing and place.**~~ Decided 2026-09-16 in the
   serving plan (§3, §4, §8) and built the same day: JSON lines under
   the merged directory's `delta/`, one file per merged kind, every line
   stamped with the batch of the merge run that wrote it; the graph
   reads the edges delta and needs no log of its own; the epoch and the
   clock are the serving plan's §4.
4. **The side table** for bounded expansion under a graph-derived
   ordering (§4): the top-*k* citers by year of every work over some
   degree. Only if G3 picks ordering 4.
5. **webgraph-rs.** Pin the versions of §6; confirm the memory-mapped and
   arc-labelled paths; count the dependencies; note the build time.
   Before G3.
6. **Stubs in the served graph.** They have citers and no references:
   `references(stub)` is empty and `citers(stub)` is real. Served, not
   excluded; `--held` is for the experiment and the report.
7. **What is not in the graph.** Edges through shared names alone and
   references with no name (merge plan §6) are unresolved rows, not arcs;
   self-citations are arcs (399 on the sample). The report counts what
   the graph left out.
8. **Other graphs later.** The ambitions (§4 there) want graphs over
   other things than citations between works — authorship, say, or
   works to persons — each its own base with its own kind of label. Open
   only whether such a graph would number its nodes the way this one
   does or its own way. Nothing to do until those things have registries
   of their own.
9. **Agreement at scale.** The 28 % figure and its siblings as report
   lines on every build (§3, §8), so that fraction is never a miscount
   again. Landed in G2: both tables, on every build.
10. **68,815 against 68,813.** The census counted 68,813 edges between
    held works before MM3's reference rules; the held build counts
    68,815, with 19 arcs moved from PubMed-and-Crossref to Crossref alone
    and 13 from all three to Crossref-and-OpenAlex. The rules that moved
    them are the disagreement rule and the stubs of merge plan §6; the
    two extra arcs have not been looked at.
11. **Isolated works.** 41,922 on the sample with no arc either way: the
    14,267 stubs minted for relation targets, which no citation names,
    and 27,655 held works with neither a resolved reference nor a citer —
    a third of the held works, an artifact of the island (part-file works
    that cite nothing the sample holds). The report counts them so a
    corpus build can say whether the number is structural.
12. ~~**Names lookup.**~~ Done: `query work` resolves through the names
    artifact (serving plan SP3); `graph query` keeps its shard scan as a
    diagnostic. The counts kept in these plans are measured ones from
    landed milestones and nothing else, per the note that stood here.

## 12. Sources consulted

The architecture summary of 2026-09-14 (git `5c98db4`, retired) and the
notes of 2026-09-15; the merge report of 2026-09-15 for every sample
number; crates.io on 2026-09-15 for the crate versions of §6; the
WebGraph site (webgraph.di.unimi.it) for the format.
