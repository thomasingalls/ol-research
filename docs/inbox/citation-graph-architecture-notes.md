# Citation-graph architecture: notes on the summary

**Status:** notes, 2026-09-15 — written against the
[architecture summary](citation-graph-architecture-summary.md), the merge
plan with MM3 landed (§6, §9, §10), the ingestion plan's long-tail
consequences (its §2), and the serving-shape decisions of 2026-09-15,
which have no plan doc yet. What this is: the corrections and decisions
the summary needs before MM4 builds against it. Left out on purpose: the
experiment's design; it serves more purposes than bits per edge.

## 1. The numbers this note uses

The summary budgets 10^9 works and 2 × 10^10 edges. The ingestion plan
already records that no source exposes edges at that order: Semantic
Scholar's `citations` is 2.4 × 10^9 rows, Crossref and OpenAlex are of the
same order, and the June 2026 OpenAlex snapshot holds 510,372,821 works.
The POC part file's 50,724 references over 10,873 works is 4.7 a work
corpus-wide; the sample's 24 to 54 a work is what journal articles with
deposited lists look like, not the corpus.

Planning numbers here: **N = 6 × 10^8** (OpenAlex's works plus stubs,
merge plan §11 item 4) and **M = 3 × 10^9** (the union: Crossref carries a
fifth of its edges that OpenAlex lacks, §6). Raw CSR at those numbers:

| component | per orientation | both |
|---|---:|---:|
| neighbours, `M × u32` | 12 GB | 24 GB |
| offsets, `(N + 1) × u64` | 4.8 GB | 9.6 GB |
| **graph** | **17 GB** | **34 GB** |
| labels, `M × u8` (§2) | 3 GB | 6 GB |
| permutation and inverse, `N × u32` each (§4) | | 4.8 GB |

Against the summary's 176 GB. The consequence is a change of emphasis,
not of structure: the storage problem the summary is organised around is
not there; a raw memory-mapped CSR is both the baseline and the first
candidate; compression buys page-cache residency on a machine with less
memory than the graph, and the 5–15 bits per edge range is a measurement
to make, not a budget to plan on. Offsets stay `u64`: 3 × 10^9 fits a
`u32`, the growth does not.

## 2. Edge provenance

The merged layer's edge is `Edge { from, to, by: Sources }`
(`merge::record`), `Sources` a byte with one bit per `SourceId`, three
used, the enum appending. The summary's CSR has no place for `by`. On the
sample it is not a detail: of 5,021,301 distinct edges, 2,222,128 are
OpenAlex's alone, 1,909,179 Crossref's alone, 368,316 PubMed's alone,
483,820 PubMed's and Crossref's, 22,248 all three's; between held works,
one edge in five has a single asserter (13,722 of 68,813).

What needs it: who asserted this edge; the edges at least two sources
agree on; one source's edges alone, corpus-wide and on every rebuild — the
fifth OpenAlex misses is a quality figure the report should keep producing
at scale, and the graph is where it is cheap; and later, edges asserted by
a process rather than a source ([ambitions](ambitions-beyond-citations.md)
§6), whose asserter is a process and a version.

| option | cost | what it cannot do |
|---|---|---|
| **a. pure index** — the CSR holds neighbours; `by` stays in the edges set, sharded by citing work | nothing in the graph | filter while traversing; answer "who said" without a second read into the merged sets |
| **b. parallel label array** — one byte an incidence beside each orientation's neighbour array, written by the same sorted build | 3 GB an orientation at M = 3 × 10^9, under a fifth of the graph | nothing; a filter is a byte compare, alignment is free |
| **c. one graph per source** — three forward, three transpose | six mappings, and a fourth source multiplies them | "agreed" is an intersection at query time; a provenance byte is cheaper than any of it |

**Proposed: b.** The label type is per slice: citation edges carry
`Sources`; a derived slice carries its process id. With BVGraph the same
byte goes in an arc-label stream — WebGraph has arc-labelled graphs — or
in a raw array indexed by a cumulative-degree index in Elias–Fano, about
`N × 4` bits; either way a byte an incidence, so the format question does
not change this one. Two consequences for the merged layer:

- `Sources` is a byte and `SourceId` appends. Eight sources is the
  ceiling, and the ingestion plan's long tail names more than that
  (Semantic Scholar, arXiv, DataCite, NIH-OCC, OpenCitations, Europe PMC,
  CORE, BASE, DBLP). Widen to `u16` before the ninth source; it is written
  into merged sets, so that is a merged-layout version bump, and the label
  array follows it.
- The graph is a cache of the edges set, as the merged sets are a cache
  of the registry: a build from the same edges set is byte-identical, and
  that is a test in the registry's spirit, beside the ones the merge has.

## 3. Updates: the delta layer

The summary's premise — "citation data arrives append-only" — is false
for this pipeline. PubMed deletes records; OpenAlex retires ids into
others (`merged_ids`); the registry writes `merge` and `split` lines whose
edges follow the surviving id (merge plan §4); a Crossref re-deposit
replaces a reference list, so edges vanish; the plan's incremental merges
(§9, after MM5) are exactly these. A delta that can only add is wrong
from the first snapshot diff.

The delta stays, for a different reason than the summary gives: the
serving-shape decisions of 2026-09-15. Incremental from the start, not an
optimisation later; a record on disk to available in every index under
ten minutes, hoped under a second; base + delta + compaction as the one
shape for every serving artifact; one epoch that all of them publish
together; and a test that incremental over batches equals a full rebuild.
So the shape is the summary's, with removals and redirects added:

- **Base:** forward and transpose CSR in position space (§4), immutable,
  built at compaction from the edges set and the registry fold, with its
  label arrays.
- **Delta:** an append-only edge log in `WorkId` space under `merged/`,
  three line kinds proposed — `add(from, to, by)`; `remove(from, to, by)`,
  a source withdrawing an assertion, the edge going when `by` empties;
  `retire(id, into)`, the registry's `merge` line echoed so the graph can
  answer for a retired id without the registry. Folded at start into a
  small in-memory forward and transpose plus a tombstone set; the fold is
  bounded because compaction bounds the log.
- **Query:** `base(perm[id]) ∪ delta(id) − tombstones`, deduplicated on
  `WorkId`; a retired id answers as its survivor, whose lists are the union
  of both ids' base and delta entries. Results are always `WorkId`s, the
  inverse permutation applied to base hits.
- **Compaction:** rebuild base from the edges set and the registry fold,
  recompute the permutation, cut the log at the epoch, switch the mapping
  and the fold together. On the box a full rebuild is a sort of 3 × 10^9
  `(u32, u32)` pairs, 24 GB, minutes to tens of minutes with a radix sort
  — cheap enough nightly. The freshness target is met by the delta path,
  not by rebuild speed; compaction only bounds the delta.
- **The format and the update design are independent.** A BVGraph base is
  as immutable as a raw one; the delta is identical either way. Settle
  the delta first; it does not wait on the experiment.
- **The test:** the same edges set reached through a base and n deltas
  and through one build answers every query identically, and compacts to
  byte-identical files. It is the merge's zero-churn and byte-identical
  tests extended one layer down.

Upstream of the graph, ten minutes means the identity pass and the edge
resolution are incremental too: the names table becomes a live index
rather than a fold at run start, a new record's references resolve
against it, and an unheld name mints a stub as a registry append. That is
the serving-shape plan's problem, noted here because the delta log has
to be fed by something.

## 4. Ordering

`WorkId`s are minted in arrival order and never change; the merge plan
says it in one line — "they promise stability, not locality, and the
graph experiment's ordering strategies relabel as they see fit" (§4).
The summary says compression is ordering-dependent and stops there. What
is missing between the two: the graph has a **position space** of its
own, `pos = perm[id]` and `id = inv[pos]`, two `u32` arrays of 2.4 GB each
at N = 6 × 10^8, recomputed at compaction and nowhere else. The delta
lives in `WorkId` space (§3), so the permutation is free to change with
every base. (`WorkId` is one-based; positions are not; `perm` is indexed
by `id − 1`.)

What the order buys depends on the format:

| | raw CSR | BVGraph |
|---|---|---|
| what ordering affects | which lists share pages: locality during expansion | gap sizes and reference coding: bits per edge, and locality |
| neighbour values | can stay `WorkId`s | positions; lists sorted by position |
| arrays needed | `perm`, at the entry of each hop | `perm` and `inv`, `inv` on every neighbour returned, so resident |

Candidate orderings, cheapest first:

1. **`WorkId` order** — nothing to compute. MM4's first build ("keyed by
   `WorkId`") and the control.
2. **Year descending, then venue, then `WorkId`** — from the promoted
   fields. References point backwards in time and cluster by venue and
   field, so consecutive works share references and reference coding has
   something to copy. Descending so that a citers list begins with the
   newest citers, which makes the bounded expansion of a high-degree work
   a prefix read with a defensible default order. Stubs have no year —
   names only — and go last in `WorkId` order; on the sample they are 98 %
   of works, an artifact of the island, and at scale their share is item
   4 of §11.
3. **Topic, then year** — OpenAlex `primary_topic` (or a MeSH tree
   position) as the first key.
4. **A graph-derived order** — layered label propagation or a BFS order,
   WebGraph's own tools, hours over 3 × 10^9 edges on the box. The best
   compression, and the only order that helps cross-venue citing.

MM4 builds 1. The experiment measures 2 and 4 against it on two figures:
bits per edge, and pages touched by a two-hop expansion from a random
held work, which is the locality figure that matters for raw CSR.

The tension the summary leaves open: BVGraph needs each list sorted by
position, and bounded expansion needs each list to begin usefully. Order
2 makes those the same order. Under order 4 the prefix means nothing, and
bounded expansion needs a side table — the top-k citers by year of every
work over some degree — which is small and should be counted as part of
that ordering's cost.

## 5. Smaller points

- **Candidate management.** The funnel, best-first search and the scoring
  signals are the product's plan, not the graph's; move that section to
  its own note. What stays in the graph's plan is its API consequence:
  `references(work)` and `citers(work, limit)` are bounded and lazy, a
  work's degree is readable without decoding its list (the offsets give
  it), and the order inside a list is decided at build (§4) and documented.
- **WebGraph.** The dependency is the Rust port: the `webgraph` crate from
  the LAW lab (Vigna and others), with `sux`, `dsi-bitstream` and
  `epserde` — BVGraph read and write, memory mapping, transposition,
  permutations, LLP. As of last look it is 0.x with a moving API; the Java
  implementation is the mature one. Before the experiment: pin a version,
  confirm the memory-mapped and arc-labelled paths, note the build time
  and the dependency count. The format is documented and stable, so a
  reader of our own is the fallback if the crate stalls. Not verified in
  this session.
- **The direction bit.** The section rebuts a layout nobody proposes now.
  Keep one line for the record — a direction-flagged combined list is two
  CSRs interleaved, 1.7 % smaller and slower to read — and cut the rest.
- **Residue.** The summary reads as the summary of a conversation:
  "Current Summary"; "earlier assumed ingestion APIs should not be
  treated as reliable" (nothing in this repository assumed any); "the
  architecture has shifted from…"; an assessment of crates evaluated
  elsewhere. Rewrite as statements, or fold what survives into merge plan
  MM4 and retire the file.
- **Ids.** The summary's `WorkId(u32)` is the code's
  `WorkId(NonZeroU32)`: one-based, `Option<WorkId>` four bytes, 6 × 10^8
  of 4.29 × 10^9 used. Its "separate mapping layer" for DOIs and the rest
  is the names table and the registry (merge plan §3, §4) — nothing new.
  The world reaches a work through its names and, by the serving-shape
  decision, through a public id minted in a second log; the graph never
  sees either.
- **Latency.** The figures stand. One addition: a BVGraph random access
  decodes a chain of referenced lists, bounded by the window, so one
  lookup touches a few lists — still well under a millisecond warm.

## 6. Where each of these goes

- Merge plan MM4: the label array and the `Sources` width (§2); the delta
  log's line kinds and the test (§3); the position space and the ordering
  list (§4); the API lines (§5).
- The summary: the numbers (§1); cut the direction bit; clean the residue.
- The serving-shape plan, when it is written: the epoch, the compaction
  cadence, the live names table (§3).
