# Serving plan

**Status:** draft 1, 2026-09-16, SP5 landed the same day (§9) — the
fourth layer: everything a query touches, built over the merged layer
and the graph. Written from the
serving-shape decisions of 2026-09-15 (§10), the graph plan with G2
landed (its §5 and §11 leave the epoch, the delta's place and the names
lookup to this plan), and the ambitions' split between facet-shaped and
hop-shaped questions. Kept short on purpose: it fixes the shape — the
artifacts, their one form, the clock, the public id, the query core — and
leaves each artifact's internals to its milestone. Agreed 2026-09-16
(SP1, §10), the order of SP3–SP6 tentatively.
**Inputs:** [merge plan](merge-plan.md) §4 (the registry), §8 (the merged
sets), §9 (incremental merges); [graph plan](graph-plan.md) §5 (the delta);
[ingestion plan](ingestion-plan.md) §3.5 (the store), §9 (M9, the search
sinks); [ambitions](inbox/ambitions-beyond-citations.md) §2, §5, §8;
[candidate management](inbox/candidate-management.md); the data terms of
the sources, noted 2026-09-15.

## 1. What the serving layer is

The layer a question reaches. Five questions, in the order the product
asks them: *resolve* — which work is this name, or this public id; *work*
— the merged record of a work, its names, its promoted fields, its
records; *references* and *citers* — the graph, bounded, with who
asserted each arc; *search* — works matching a query, with facets over
their promoted fields; and, for the product, *expand* — a bounded
neighbourhood of a seed set, which is the funnel's first step and
otherwise the product's business.

Every artifact here is a cache, as the merged sets are of the registry
and the graph of the edges set: rebuildable from the merged layer, the
registry, the public-id log and one key (§6), versioned, byte-identical
from the same inputs. Nothing under it is of record. It is not the
product's ranking, which lives in candidate management; not full text,
which is never served (ambitions §6); not authentication, which is
deferred (§7); and not a place where a `WorkId` is visible — a public id
or a name goes in, and public ids and names come out.

Vocabulary, adding to the merge plan's §2 and the graph plan's §1:

| term          | meaning                                                                                                                                 |
|---------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| **artifact**  | one served structure — the names lookup, the works index, the graph, the lexical index — each a sink over the merged layer              |
| **base**      | an artifact's immutable build, from the merged sets as of one batch                                                                     |
| **delta**     | what the merged layer appended since that batch, folded by the artifact's reader                                                        |
| **batch**     | one merge run, full or incremental, numbered in the journal (§4): the one clock every artifact and every answer is positioned by        |
| **epoch**     | a set of bases built from the same batch, published together by one manifest; readers open an epoch and tail the deltas past it         |
| **watermark** | the batch a reader has folded up to; an answer says which                                                                               |
| **public id** | the id the world sees for a work: minted beside the `WorkId`, opaque, stable, redirecting on merge (§6)                                  |
| **origin**    | where new records come from between snapshots: a `watch` cursor per source, later publisher feeds (§5)                                  |

## 2. Numbers

The freshness target, decided 2026-09-15: a record on disk to available
in every artifact under ten minutes, hoped under a second — freshness the
way a newspaper treats breaking news. Two clocks are outside our control
and bound what the target can mean: the sources' own lag — Crossref
indexes a deposit about an hour after it is created, PubMed and OpenAlex
update daily — and, later, publisher feeds, which are where "under a
second from publication" would have to come from. The target is measured
from raw on our disk.

Where the time goes on the delta path (§5), at scale, per batch of new
records: identity against a live names table, microseconds a record;
appends to four delta logs, milliseconds; the graph's and the names'
readers fold an appended line as they read it; the lexical index commits
a batch in tens of milliseconds to a second. The floor is the lexical
commit cadence, about a second; ten minutes is met with an order of
magnitude to spare. What the target costs is not latency but the
incremental merge itself — the largest piece of work in this plan (§5,
SP5) — and until it lands the layer serves from a nightly epoch, which
is a day's freshness and already what the sources give for two of
three.

The resident set on the box — 128 GB, shared with the search engine, so
the layer plans on a share — at N = 6 × 10^8 works and 10^9 names, order
of magnitude, to be measured on the sample and extrapolated at SP2 and
SP3: the graph's 12 GB compressed (graph plan §2); the names lookup as a
finite-state transducer, 5–15 GB, DOIs sharing prefixes; the works index
page-cached on demand from 150–250 GB on disk; the lexical index's hot
parts — term dictionaries and the facet columns — 20–40 GB of an index
several hundred GB on disk. The layer is disk-heavy and memory-modest,
and every figure here is a guess until an artifact exists.

## 3. The artifacts and their one shape

Base + delta + compaction is the one shape, decided 2026-09-15. Each
artifact's base is built from the merged sets at a batch, its delta is
whatever the merged layer appended since, and compaction is a new base
from a later batch, published as a new epoch. What differs per artifact
is only what the base is and what its delta reads.

| artifact   | keyed by                       | base                                                                                                          | delta                                                                                        | answers                        |
|------------|--------------------------------|---------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------|--------------------------------|
| **names**  | a name's text form; a public id | a memory-mapped finite-state transducer from the names set and the public log: text → `WorkId`               | the registry's and the public log's own lines past the epoch — no log of its own              | resolve                        |
| **works**  | `WorkId`                       | the works set re-laid for random access: an offsets array over blocks of records, zstd with one dictionary     | the works delta: whole merged records, latest wins                                            | work                           |
| **graph**  | `WorkId`                       | the graph plan's base: both orientations, labels, the permutation                                             | the edges delta: `add`, `remove`, `retire` (graph plan §5)                                    | references, citers, expand     |
| **lexical**| `WorkId`                       | the search engine's index over the works set: title, abstract, authors, venue, year, kinds, language, status  | the engine's own incremental updates per batch, committed with the batch number               | search, facets                 |

Two artifacts have no delta file because their deltas already exist: the
names lookup tails the registry, whose `mint`, `name`, `drop`, `merge`
and `split` lines are exactly the events a lookup needs, and the public
log beside it. The graph tails the edges delta the graph plan specified.
The works index needs the one new delta, and the lexical index absorbs
the same works delta through its engine. So an incremental merge appends
to the registry, the public log, and one delta per merged kind — works,
edges, relations, unresolved — and nothing else; the artifacts read.

The lexical artifact is the odd one: its base is not rebuilt at every
epoch, because a full reindex of the corpus is hours and the engine
maintains its own segments; its epoch entry records the batch it has
absorbed, and a rebuild happens when the mapping changes. Which engine is
SP2's question, Tantivy first because it is embedded Rust and testable
under cargo, Meilisearch second as the server for comparison, with the
scale caveat of §11 item 4.

## 4. The clock, the journal, the epoch

The registry's line count was the graph plan's candidate for a clock and
is not enough: a re-deposit that replaces a reference list changes edges
and writes no registry line. The clock is the **batch**: every merge run,
full or incremental, appends one line to `merged/journal.jsonl` —

```text
{"batch": 4127, "at": "…", "kind": "incremental", "origin": {"crossref": "<cursor>", …},
 "registry": <lines after>, "public": <lines after>,
 "delta": {"works": <bytes after>, "edges": …, "relations": …, "unresolved": …},
 "records": <normalized records merged>, "minted": …, "merged": …, "split": …}
```

— and every delta line carries its batch number. A full merge writes a
`full` line: the merged directory replaced whole, its delta files new and
empty. A reader positions itself by the batch, never by a byte offset it
computed itself.

An **epoch** is one manifest, `epochs/<n>.json`, naming a batch and the
bases built from it: the merged directory's name, and for each artifact
its directory, version and digest, or for the lexical index the commit
that holds its batch. `epochs/current` names the latest complete epoch,
and writing it is the switch: readers open the epoch it names, open its
bases, and tail every delta from its batch forward, advancing one
watermark only when every artifact has folded a batch, so an answer
never joins a search hit at batch *n* + 3 to a works index at *n* + 2.

Compaction is the nightly path: a full merge, then every base rebuilt,
then a new epoch. It bounds the deltas at a day's arrivals; the freshness
target is met by the deltas, not by how fast compaction runs. Old epochs
stay until no reader holds them, then go.

## 5. The freshness path, end to end

One new record, from a source to an answer:

1. **Origin.** A `watch` listing kind per source, a cursor each — Crossref
   by index date with its deep-paging cursor, PubMed's daily update files
   and, inside a day, Entrez's date fields, OpenAlex by update date; each
   polled at the pace the provider publishes, one request in flight,
   receipts as every fetch has (ingestion plan §3.5, §8 item 8). Publisher
   feeds come after a first-seen measurement — how far ahead of Crossref
   a feed actually is, over a day — and as the lowest-precedence source
   kind (§11, item 2). The ingestion plan's M9 owns the origins; this plan
   owns what happens after normalization.
2. **Raw and normalized.** A watch page lands under `raw/<source>/watch/`
   with a receipt and normalizes like any page.
3. **Incremental merge** (`ol-rht merge --batch <sets>…`, landed
   2026-09-16 as `serve::incremental`). The batch is a set of normalized
   sets — works, references, deletions, any source. Each record's names
   are looked up in the live names table — the names base plus the
   registry's tail, the same structure the names artifact serves from —
   and the merge plan's rules are re-run on the *component* the batch
   cuts out of the live state: every record of every work a batch
   record's names touch, closed over shared names, the batch's copies
   replacing the rows' and its deletions removing them. That is possible
   without the normalized layer because every record in a merged row
   carries its evidence — its assertions and its view (merge plan §5,
   version 2). The registry's answer — reuse, mint, merge — is a line
   each, and a mint a public-id line (§6). References resolve through the
   batch's own resolution first and the live table second; a reference
   naming no held work joins the stubs it names with the names it does
   not, exactly as the full merge groups them, so an existing stub gains
   an alias or a citing source, two stubs one reference names merge, and
   a name nothing holds mints one. The previous list of each batch
   record's source, read from the graph's own forward list, is diffed
   against the new: `add` and `remove` lines; a retired id's remaining
   arcs and its citers move to the survivor; relations and unresolved
   rows are put whole per work and source, and moved on a retirement.
   What it appends, in this order: the registry lines and the public
   lines (of record), the four delta logs, the journal line, and then
   the batch into the lexical index with the batch number as the
   engine's commit payload. One lock under `merged/` keeps runs, full or
   incremental, from overlapping.
4. **The artifacts fold.** The names reader tails the registry and the
   public log from the byte offsets the journal records, and the works
   delta for the shared names it makes; the graph reader folds the edges
   delta into a map of effective labels over its base, both orientations;
   the works reader indexes the delta's latest line per id and reads a
   row on demand, the log held open so a compaction cannot pull it away;
   the lexical reader reloads and reads its commit payload.
5. **The watermark advances** when all four have the batch — a batch is
   folded only once the lexical index carries it, so an answer never mixes
   two — and the record answers. Every answer carries the watermark and
   its timestamp.

Five cases the incremental rules leave to a full merge, each stopping the
batch before it writes anything and saying why: a work's names splitting
between works; a work losing its last record while something cites it,
because the stub it would become takes its names from the citing
references; a duplicate record of one source deleted or moved while its
twin stays; two ids retired in one batch with an arc between them; and a
text — an abstract, author names, a venue name — that a record not in the
batch and not promoted before would have to supply, because rows keep the
text of their promoted records only. Two things a batch does that a full
merge then tidies, both reported: a work with a duplicate record outside
the batch has arcs added and none removed, and a stub left with no citing
arc keeps its names until a compaction drops them.

Two tests, both the merge's own tests extended a layer: after *n*
incremental batches, a full merge appends nothing to the registry and its
merged sets equal the previous base with the deltas folded; and a query
core over an epoch plus *n* deltas answers every question identically to
one over the epoch built after them, and the compacted bases are
byte-identical. Both run on a store built in memory (`serve`'s tests: new
records, a re-deposit, a bridging merge, stubs cited again, deletions, the
refusals) and on the sample with one PubMed file held out
(`crates/ol-rht/tests/incremental.rs`, when `OL_RHT_STORE` names the
store), where `serve::compare` walks every id, every key of either base
and the fixed queries.

Compaction is the nightly full merge, then `epoch publish`, which
refuses at an incremental batch: an epoch is a set of bases at a full
merge. A reader that sees a full line past its epoch stops folding there
and keeps answering at its last watermark until the new epoch is
published. A batch may not run between a full merge and its epoch; the
replay of batches that a long compaction would delay (§11, item 16) is
not built.

## 6. The public id

Decided 2026-09-15: a work has a public id, minted as a second append-only
log beside the registry, and the `WorkId` still never leaves. This draft
proposes what the id is:

- **Minted with the `WorkId`.** `merged/public.jsonl`, one `mint` line per
  id — `{"id": <WorkId>, "public": "<text>", "at": …}` — written by the
  merge in the run that mints the id, stubs included, so every work the
  graph can reach has one and the names artifact answers for it. Never
  reused, never changed.
- **A keyed function of the `WorkId`**, not a random draw: 64 bits of a
  keyed hash (SipHash-2-4, or BLAKE3 keyed) of the id under a key held
  outside the store with the other secrets, hashed again with a counter
  on the rare collision, so that the log is a pure function of the
  registry and the key and a registry rebuilt from scratch reproduces
  it — the merge's byte-identical test survives. Opaque without the key;
  derived from our own stable id, not from anything the world can
  correct. A lost key stops consistent minting, not lookup: the log is
  what is of record, and a new key mints new ids only.
- **Text.** Crockford base32 of the 64 bits, 13 characters, then
  Crockford's check character, case-folded on input, under the prefix
  `ol:` — a namespace of its own in the names vocabulary, so that a
  public id is a name like any other to `resolve`. Decided 2026-09-16.
- **Redirects.** A merged work's public id resolves to its survivor
  through the registry, forever; a split work's public id answers with
  both parts, an ambiguity the API says out loud rather than guessing.

## 7. The query core and its transports

**The core** is a library, synchronous, over one open epoch and its
deltas, speaking `WorkId` inside and nothing else (landed 2026-09-16 as
`serve::Serving`, with `query work | search | expand` in the binary): `resolve(name) →
Option<WorkId>` for names and public ids alike; `work(id) → Record`, the
merged record with its names and standings, promoted fields with their
disagreements, and its source records; `references(id, limit)` and
`citers(id, limit)` as the graph gives them, lazy, each arc with its
asserters; `search(query, filters, facets, limit) → hits and facet
counts` over the promoted fields — year, form, genre, language, status,
venue, the sources holding the work, and later MeSH, topics and entity
ids as the ambitions ask (§8 there); and `expand(seeds, budget)`, one
hop out from a seed set, deduplicated and bounded, the primitive the
funnel starts from and no more. Every answer carries the epoch and the
watermark it was answered at.

**The transport** is a second binary: gRPC first, decided 2026-09-15 on
`tonic` 0.14 over `hyper` 1, with server streaming for a hub's citers
and deadlines on every call; a JSON facade over the same core later if
wanted, on `axum` 0.8. Landed 2026-09-16 as `ol-rht-serve` (SP4). The transport is where `WorkId` becomes public id
and back: nothing serialized outward carries a `WorkId`, and the core's
types never reach the wire unmapped.

**Terms.** NLM asks attribution and currency; abstracts are the risk;
the rest is free (terms noted 2026-09-15). Every answer therefore names
the sources it drew on and the epoch's date, which is currency, and a
`terms` module in the transport decides per source what a response may
carry — the abstract's presence by source is already a merged field.
First-class in code eventually, as the user asked; a stub with the
attribution lines now (§11, item 3).

**Auth and quotas** are deferred until hardware and traffic are known.
Their place is the transport, a principal resolved per request from
state kept outside the store, and a limit per principal on calls and on
`citers` fan-out; the core never sees them.

## 8. Machinery

- **`crates/serve`**, source-agnostic: the assembled document (landed
  2026-09-16: the merged record's promoted fields joined with the text
  its promoted source records carry, one document per held work); the
  epoch manifest and the journal; the names artifact (the transducer and
  its registry-tail fold); the works artifact (blocks, offsets,
  dictionary, the delta index); the query core over those two and
  `graph`; the tests of §5. Depends on `merge`, `graph` and `ingest`; on
  no source crate.
- **`crates/search`**, the lexical index behind one shape the core
  calls — build from documents, open, search with facets, and at SP5
  absorb a batch and commit with its number — with Tantivy as a module
  until a second engine exists (`search-meili` as a client of a server
  the user runs, when wanted). The abstract text functions (merge plan
  §11, item 7) live in `merge::view`, since the sinks are not the only
  readers, and every `Contributor` answers `text`: the abstract, the
  author names and the venue name of a record as plain text.
- **`crates/ol-rht-serve`**, the transport binary, and in `ol-rht` the
  build commands.
- **Layout**, under the store: `merged/journal.jsonl` and
  `merged/public.jsonl`, of record beside the registry;
  `merged/v<model>.<merge>/delta/{works,edges,relations,unresolved}.jsonl`,
  a cache like the sets beside them, JSON lines stamped with their batch
  (`merge::delta`), the binary framing when volume asks (the registry's
  path, ingestion plan M9); `merged/merge.lock` while a merge runs;
  `names/v<…>/`, `works/v<…>/`, `search/<engine>/v<…>/` and `graph/v<…>/`
  as bases, each versioned by model, merge and its own version;
  `epochs/<n>.json` and `epochs/current`.
- **Commands.** `ol-rht names build`, `ol-rht works build`, `ol-rht search
  build --engine <name>`, each from the merged directory with a report;
  `ol-rht epoch publish`, which builds what is missing for the current
  batch and writes the manifest and `current`; `ol-rht epoch check`,
  which verifies every base's digest and the byte-identical rebuilds;
  `ol-rht merge --batch <path>… [--public-key <file>]`, the incremental
  merge over the normalized sets at the paths (set files, prefixes or
  directories), with a report of what it minted, merged, dropped, added
  and removed; `ol-rht-serve --store <root> --listen <addr> [--poll
  <seconds>]`, which folds the journal every poll, one second by
  default.
- **Fail fast.** A delta line whose batch is not the journal's stops the
  reader; a journal with a gap stops it; an epoch naming a base whose
  digest differs stops the open; a batch against an epoch older than the
  last full merge is refused, as is a second merge while one runs; delta
  bytes a batch that never reached the journal left behind are cut at
  the next batch's start and reported; a public id that does not fold to
  a live work is an answer, not an error; every build prints its report.

## 9. Milestones

- **SP1** — this plan agreed; the merge plan's incremental milestone and
  the graph plan's G4 point here. *Agreed 2026-09-16: the batch clock,
  the deltas' places and the keyed public id as proposed; batch shape
  first, Tantivy for the proof of concept; SP3–SP6 in this order
  tentatively.*
- **SP2** — the lexical artifact, batch shape: `crates/search` and the
  Tantivy adapter over the works set with the abstract text functions
  and the facet fields, a `search` report on the sample (index size,
  build time, hits for a fixed query set). Tantivy alone for the proof
  of concept (2026-09-16); the Meilisearch comparison when a second
  engine is wanted. *Landed 2026-09-16, the mechanics without the
  policy: `crates/serve` with the assembled document; `text` on every
  `Contributor` and the three text functions in `merge::view` (markup
  stripped, PubMed's sections joined under their labels, OpenAlex's
  inverted index unwound); `crates/search` with Tantivy 0.26 behind one
  shape, a manifest of its own, a `.part` directory replaced whole;
  `ol-rht search build` and `search query`. The mapping is one
  provisional function that every report prints as such; the decisions
  it stands in for are §11, item 11. On the sample: 82,744 documents,
  one per held work — 72,051 with an abstract (PubMed 45,347, OpenAlex
  26,422, Crossref 282), 82,256 with authors, 78,080 with a venue —
  assembled in 6.4 s and indexed in 3.5 s, 56 MiB in one segment,
  610 MB resident; a query answers in under a millisecond with facet
  counts over its matches; the six fixed queries put the PRISMA
  statements, the trial designs and the CRISPR papers first, which is
  what plain BM25 over titles and abstracts should do.*
- **SP3** — the names and works artifacts, the epoch manifest, the public
  log minted by the merge, the query core over names, works and graph,
  `epoch publish` and `epoch check` on the sample. *Landed 2026-09-16:
  `merge::journal` and `merge::public` (the journal line every complete
  merge run now appends; the public log, minted when `merge` is given
  `--public-key <file>` and backfilled for every id lacking one);
  `merge::document`, the assembled work, moved from `serve` so that
  `search` and `serve` both read it; `crates/serve` with `names` (the
  transducer, the shared side list, the retirements), `works` (blocks of
  256 rows, an offsets array, one row everything the merge knows plus the
  text), `epoch` and `core`; `ol-rht names build`, `works build`, `epoch
  publish`, `epoch check`, and `query work | search | expand` through the
  core — the artifact-level `graph query` and `search query` stay as
  diagnostics rather than routing through it. On the sample: batch 1 in
  the journal; the names lookup 36 MiB for 4,704,915 keys (4 shared, 2
  retired ids), 6 s; the works index 4,039,076 rows in 15,778 blocks,
  365 MiB raw to 90 MiB, 10 s at 460 MB resident; `epoch publish` 13 s
  including the graph and the lexical index rebuilt for the batch;
  `epoch check` 20 s — every digest, both rebuilds byte-identical, every
  promoted name resolving to its row, the graph and the index agreeing
  on 4,039,078 nodes, the lexical index holding the 82,744 held works —
  at 1.2 GB resident, most of it the registry fold. A `query work` by
  DOI answers with names, records, title, authors, venue, the promoted
  fields with their disagreements, the abstract, references and citers;
  `query expand` over three spellings of one work answers in 0.1 ms.
  Public ids are minted on the next merge run with the key, which the
  user makes; until then every row has none and `ol:` resolves nothing.*
- **SP4** — the transport: `ol-rht-serve` with gRPC, the public-id
  boundary, streaming citers, the terms stub; a client in the test suite.
  *Landed 2026-09-16: `crates/ol-rht-serve` — the protocol under
  `proto/olrht/v1/serving.proto` (seven calls: epoch, resolve, get work,
  references and citers as paged streams, search, expand; every answer
  stamped with its epoch, batch, timestamp, sources and attribution
  lines), compiled by `protox` in the build script so nothing outside
  cargo is installed, with reflection and health beside it; the server
  generic over an `Answers` trait the query core implements, the epoch
  swappable underneath it, a poller switching to a new `current` every
  five seconds with requests in flight finishing on the old; the caps
  (a page 100, a search 100, facets 20, an expansion 1,000, seeds 100)
  refused rather than clamped; a cursor `epoch:offset` refused from
  another epoch; the terms stub with a line per source and an abstract
  switch per source, all on. The names lookup gained `public.u64`, the
  `WorkId` to public-id array the boundary maps out through, which
  bumped the layer's version to 2. Tests: three over a fake — the id
  boundary in every spelling, shared and split names, paging and
  resumption and limits, an epoch switch under a live cursor, the caps,
  the errors — and one over the real store when `OL_RHT_STORE` names
  it, which the sample passes: epoch 3 over 4,039,078 nodes and
  5,021,301 arcs, the PRISMA statement resolved to its `ol:` id, its
  607 citers paged three times to a limit of 250, hydrated, search and
  expansion answering. The user minted the public ids the same afternoon
  with their key and published epoch 2; epoch 3 is the layer's version
  bump. `grpcurl` is not installed; the binary serves reflection for
  it whenever it is.*
- **SP5** — the delta path: the journal, the incremental merge over a
  batch of normalized records against the live names table, the four
  delta logs, every reader folding, the graph plan's G4, the lexical
  incremental commit, the two tests of §5, and the freshness measured
  from raw on disk to answered. *Landed 2026-09-16 (§5, §10):
  `merge::delta` and `merge::layout::Lock`; `serve::fold`,
  `serve::incremental` and `serve::compare`; `search::index::absorb`
  with the batch as the commit payload; `ol-rht merge --batch`; the
  server polling the journal every second and stamping every answer
  with the watermark. The merged layout went to version 2 and the
  serving layer to 3 for the evidence the rows now carry. On the sample,
  the last PubMed baseline file held out and merged as a batch of 4,989
  records against an epoch over the rest: 4,988 components, 12 held
  works and 609 stubs minted, 941 stubs merged into older ones — the
  records claimed names two references had given separately — 18,889
  names attached, 22,519 arcs added, 1,297 removed, 1,291 moved with a
  retired id, 4,988 documents put into the lexical index; the batch
  itself took 2.7 s in a release build (§11, item 17), and a reader
  polling once a second answered at the new batch 1.3 s after it ended,
  the fold itself 0.26 s. The real server on the same store logged its
  fold within the second the batch ended, having
  first reported the lexical index one batch behind — the watermark
  rule doing its job. The synthetic-store tests pass every case of §5
  and §11 item 8, the full merge after them appending nothing and the
  comparison finding no difference. The sample harness found three
  divergences in the batch's handling of stubs whose names a record
  takes in part, and after the third fix its full merge appended
  nothing to the registry and its comparison of 4,040,017 ids found one
  difference, in the evidence and not in any answer: one stub name's
  list of citing works lacked a work whose reference named the stub
  beside a held work, the registry's order having chosen the stub for
  the arc (§11, item 21). Fixed the same night, the harness passes on
  the sample: nothing appended to the registry, and no difference in
  4,040,017 ids.*
- **SP6** — origins: the `watch` kind with the ingestion plan's M9, the
  cursor per source, the first-seen measurement, and then, if the
  measurement earns it, publisher feeds.

## 10. Decision log

- 2026-09-15, the serving shape (the user's decisions, logged here for
  want of this document) — a public id, minted as a second append-only
  log beside the registry; incremental from the start rather than an
  optimisation later, a record on disk to available everywhere under
  ten minutes and hoped under a second; gRPC first, `tonic` on `hyper` 1
  having been checked healthy; auth and quotas deferred until hardware
  and traffic are known. Proposed the same day and taken into this
  draft: base + delta + compaction as every artifact's one shape, one
  epoch published together, and the test that incremental over batches
  equals a full rebuild.
- 2026-09-15, publisher feeds — wanted eventually as a freshness origin,
  a long tail with more code; six feeds probed read-only: all exist,
  three carry DOIs, two abstracts, ScienceDirect a PII and IEEE a
  document number only; Crossref's `created` within the hour of a feed's
  own stamp on three works. Decided then: a `watch` origin with Crossref
  first, PubMed second, feeds third and lowest in precedence, after a
  day-long first-seen measurement. Data terms first class in code
  eventually, not now.
- 2026-09-16, this draft — the batch as the clock, in a journal, because
  the registry's line count misses edge-only changes; two artifacts with
  no delta file of their own, the names lookup reading the registry and
  the graph reading the edges delta; the works delta the one new log;
  the lexical index absorbing batches rather than rebuilding per epoch;
  the public id a keyed function of the `WorkId` so the log is
  reproducible, minted with the `WorkId`, stubs included, redirecting on
  merge and answering both parts on split; the query core synchronous
  and `WorkId`-speaking with the transport as the public-id boundary;
  batch shape first (SP2–SP4), the delta path after (SP5), origins last
  (SP6), since a nightly epoch is already the sources' own freshness for
  two of three.
- 2026-09-16, the plan agreed (SP1) — the four proposals above taken as
  they stand; batch shape first with Tantivy alone for the proof of
  concept, the Meilisearch comparison deferred; the order of SP3–SP6 a
  tentative yes, to be revisited as each lands.
- 2026-09-16, SP2 landed without the policy — the user has opinions on
  indexing and the build stops short of them: one provisional mapping,
  printed by every report, and the questions of §11 item 11. What the
  build settled that is not policy: the assembled document is the
  serving layer's unit, made once for the lexical index and, at SP3, the
  works index; the text functions live in `merge::view` and every
  `Contributor` answers `text`; Tantivy is a module of `search`, not a
  crate of its own, until a second engine exists; the index is not
  byte-identical between builds — segment ids are random — so its check
  is document counts and the fixed queries, as §3 foresaw.
- 2026-09-16, SP4 landed — as shaped. Two things settled in the
  building: every request field that names a work takes a public id or
  any name in the text form, since the lookup is the same one; and the
  `WorkId`-to-public-id array lives in the names lookup, so the serving
  process never holds the key. Not settled: per-request logging, which
  a tower layer would give and the binary does not yet have (§11, item
  14).
- 2026-09-16, SP5 landed — the delta path as §5 shaped it, with what the
  building settled: the merged record carries each source record's
  evidence (its assertions and its view) and each stub name's citing
  works, because the incremental rules re-run on a component cut from
  the rows and nothing else, and the sample showed the first run where
  that was not enough — a record arriving for some of a stub's names,
  where the citers of the rest decide whether they become a stub or go;
  the lexical index is written in place by the merge batch, one writer,
  the server only reloading; the journal line lands before the lexical
  commit, and a reader folds a batch only once the index carries it,
  which is the watermark rule made concrete; a batch stops and leaves
  to a full merge the five cases §5 lists rather than guess; one lock
  under `merged/`; an epoch is published only at a full merge, and a
  batch only against an epoch at the last one; the full merge itself
  gained PubMed's deletions and the latest revision of a record
  delivered twice, without which nightly compaction would undo update
  files. Not built: the replay of batches a long compaction delays
  (§11, item 16).
- 2026-09-16, the batch's speed — asked by the user how long the runs
  take. Every timing behind SP5's entry came from a debug build: a
  sample harness run took 31.5 minutes there and 5.9 in release. A
  profile of the release batch then put over nine tenths of its 102 s in
  one loop: for every group of stubs, the stub step copied the table of
  every stub name the whole batch cited and looked each copied name up
  in the group, a cost that grows with the square of the batch; row
  reads were 2 %. SP5's entry had blamed a contended store and item 17
  row reads, and both are corrected. Decided with the user: the loop
  looks up each group's own names, and the scans of the same shape
  became lookups too — a record's text found by walking the batch, the
  re-deposit check, the sources that left each retired id, the
  deletions copied for every component, the uncited-stub check over
  every changed arc, and a stub name's citers searched as a list. On
  the sample the batch went from 102 s to 2.7 s with the same report,
  the full merge after it appended nothing, and the comparison found
  the same one difference (§11, item 21); the harness runs in
  3.9 minutes in release.
- 2026-09-16, the missing citer — fixed on the user's word, so that the
  harness passes (§11, item 21): the batch takes a stub name's citers
  from where its arcs land, as the full merge does, rather than from
  which references named no held work, and writes stub rows once the
  arcs are known, the stubs it makes of names a record left included.
  The synthetic store gained a reference naming a stub beside a held
  work for each shape; on the sample the harness passes with no
  difference in 4,040,017 ids, and the batch still takes 2.7 s. Two
  probes on the synthetic store found what the sample cannot show. Not
  settled: the full merge reads every copy's references of a record
  delivered twice where the batch replaces the list (§11, item 22), and
  the batch keeps a deleted record's work among a stub's citers (§11,
  item 23).
- 2026-09-17, copies and withdrawals — decided with the user after the
  probes of §11 items 22 and 23: the newest copy of a record speaks for
  all of it, names, fields and references, in both merges, because the
  latest version is the likeliest to be right and a later copy's new
  name must not split a work's id; the batch removes a record's citers
  from the stubs its arcs left, whether the record was withdrawn or its
  list changed; and the rules for copies and withdrawals live in one
  place the binary and the test fixture both call, the probes become
  synthetic cases, and a second sample harness holds out PubMed update
  files. The user's condition on dropping stale citers: a path that was
  once true is not authoritative, but finding things by it should stay
  as possible as it can be (§11, item 24). Not settled: whether a stub
  name's sources are derived from its citers, and whether a withdrawn
  record's references still make stubs in the full merge's first pass.
  Nothing built yet.
- 2026-09-16, SP3 landed — as shaped, with three small departures: the
  query core got commands of its own (`query work`, `query search`,
  `query expand`) and the artifact-level `graph query` and `search query`
  stay as diagnostics, so each layer can be asked alone; the assembled
  document lives in `merge::document`, not `serve`, because `search`
  reads it too and a crate cycle is the alternative; and `epoch publish`
  rebuilds the graph and the lexical index when their manifests predate
  the batch, which is the only clock they carry. Measured: the works
  index compresses four to one in blocks of 256 without a dictionary,
  so the dictionary of §11 item 5 waits for a reason; the names build
  folds the whole registry for two retirements, which is the wrong
  cost at scale (§11, item 12).
- 2026-09-16, SP3 shaped — decided with the user: the names lookup is a
  finite state transducer (`fst`), public ids in the same map, shared
  names through a side list, the registry's tail as its delta; the
  public id's hash is `blake3` keyed with 32 bytes from a file the user
  makes and passes by flag, minted in `merge` and backfilled on its next
  run; the merge writes the journal line, `epoch publish` builds what is
  missing for the batch and writes the manifest and `current`, `epoch
  check` verifies digests, byte-identical rebuilds and the artifacts
  against each other; the query core is a synchronous library over the
  current epoch, `WorkId` inside, and the two existing query commands
  route through it. Settled the same day: the merged shards stay as the
  merge writes them and the works index is a read index over them — one
  row per work holding everything the merge knows (names with standings,
  the source records, the promoted fields with their disagreements) and
  the display text SP2 assembles, stubs with their names only; the
  public id reads `ol:` plus thirteen base32 characters and a check
  character.

## 11. Open items

1. ~~**The public id's text.**~~ Decided 2026-09-16 (§6): `ol:`, thirteen
   Crockford base32 characters and a check character, from 64 bits of a
   keyed `blake3` hash. Persons will need one too when they are served
   (ambitions §8).
2. **Feeds.** The first-seen measurement over a day, then a feed registry
   only if feeds lead Crossref by enough to matter; abstracts from feeds
   are a terms question of their own.
3. **Terms in code.** Which fields a response may carry per source,
   attribution lines, currency; a stub at SP4, the rules when the
   product's use is decided.
4. **The engine at scale.** Meilisearch is built for application search
   and is uncomfortable past tens of millions of documents; at 6 × 10^8
   works the candidates are Tantivy, Quickwit — Tantivy sharded over an
   object store, which is where the store is going — or OpenSearch. SP2
   compares on the sample what the sample can show: relevance, typo
   tolerance, facets, the sink's shape; scale is measured on the box.
5. **The works index's block size and dictionary**, and whether a record
   is served whole or by field; SP3's measurement.
6. **The names transducer's update cost.** An FST is immutable; the
   registry tail is folded into a side map and merged at compaction
   (built 2026-09-16: `serve::fold::NamesTail`, plus the shared names
   the works delta makes and the public log's tail). How large the tail
   grows in a day at scale decides whether a day's compaction is enough
   for it.
7. **`expand`'s budget** and what it dedupes on; the product's, once
   candidate management is a plan.
8. ~~**Incremental identity's edge cases.**~~ Spelled out 2026-09-16
   with fixtures (`serve`'s tests over a store built in memory): a
   record bridging two held works merges the younger into the older,
   its arcs and citers moving; a name moving between works is a split,
   which stops the batch for a full merge; a deleted record leaves its
   work's other records to re-promote, drops the work when it was the
   last and nothing cites it, and stops the batch when it was the last
   and something does. Two more the sample taught: a record arriving for
   some of a stub's names — the rest go to a stub of their own where a
   reference cites them alone, which the stub row's per-name citers
   say, and are dropped otherwise; and a text only the normalized layer
   has, which stops the batch (§5).
9. **Consistency across artifacts** is by watermark: a batch is folded
   only once the lexical index has absorbed it, so the older watermark
   is shown and never a mix (built 2026-09-16). Open: how long a lag is
   tolerated before it is an alarm; the server logs it once.
10. **Old epochs.** How many to keep, and the reader handoff on switch.
11. **Indexing policy.** The user's, deliberately undecided; the
    provisional mapping stands in for it. What it leaves open: which
    fields are searched and how they are weighted against each other;
    tokenization — stemming, stop words, diacritics, an analyzer per
    language (the sample holds `fra`, `spa`, `por`, `deu`, `rus` and
    `zho` works), n-grams for typo tolerance, the engine's 40-byte term
    cap that drops long chemical names; which abstract when several
    sources have one (now PubMed, OpenAlex, Crossref in that order) and
    whether PubMed's section labels are indexed with the text; the form
    of author names and their variants; which facets and at what grain,
    venue among them once it has an id; what the index stores against
    what the works index serves (now the id and the title); alternate,
    vernacular and sub-titles; keywords, MeSH terms, chemicals and topics
    as text or as facets; which sources' abstracts the terms allow in an
    index at all; and one document per work against one per record.
12. **The registry fold in `names build` and `epoch check`.** Most of a
    gigabyte on the sample to learn two retirements; at scale, tens of
    gigabytes. The retirements should be readable without folding the
    names — a side file the registry keeps, or the journal carrying
    them — before the corpus. The readers do not fold: they tail from
    the byte offsets the journal records since SP5, or count lines to
    find them for a journal line written before.
13. **`work(id)` memory.** A lookup decompresses one 256-row block, tens
    of kilobytes; `rows()` for the cross-check decodes every block in
    turn. Both fine on the sample; the corpus wants the check sampled or
    parallel.
14. **Request logging and metrics.** The binary logs its start, its
    switches and its internal errors to stderr and nothing per request;
    a tower layer for method, status, latency and epoch is the shape,
    when there is traffic to look at.
15. **Old artifact directories.** A version bump leaves the previous
    `names/` and `works/` directories and the epochs that named them in
    the store; item 10's answer decides when they go.
16. **Replay after a compaction.** A full merge at corpus scale takes
    hours, and no batch may run between it and its epoch, so freshness
    pauses for the compaction. The journal records each batch's inputs
    so the batches a compaction delays can be merged again onto the new
    base; the replay itself is not built.
17. **The batch's own speed.** 2.7 s on the sample's batch of 4,989
    records in a release build. Row reads are three fifths of it —
    nearly all of that decompressing a block of 256 rows to read one —
    then the lexical commit and the fold. A block cache beyond the last
    one, and stubness and public ids from the lookup rather than the row,
    are the first two things to try before the corpus. The shape to keep
    out of the batch is a walk over something the size of the batch for
    each thing in it: that was over nine tenths of a 102-second batch
    until 2026-09-16 (§10).
18. **What a batch cannot know.** The cases of §5 that stop a batch or
    wait for a compaction all want one normalized record by native id;
    the merge plan's item 22 is the index that would give it.
19. **The journal is read whole on every poll.** Small for years of
    batches a minute apart; a reader that remembers its byte offset is
    the fix when it is not.
20. **Paging across a fold.** A list's offsets shift by the arcs a batch
    adds or removes before it in id order, so a page fetched after a
    fold may skip or repeat an arc; the cursor says nothing about it.
    Pinning a cursor to its watermark would make every list stale the
    moment a batch lands; carrying the last arc's id instead of an
    offset is the better cursor, when paging matters.
21. ~~**A stub cited beside a held name.**~~ Fixed 2026-09-16. A
    reference naming a stub and a held work at once follows the
    registry's order (merge plan §6), so the arc can land on the stub;
    the full merge counts that reference among the stub name's citers,
    and the batch did not, since it counted citers only where no held
    name was named. The batch now takes a stub's citers from its own
    arc resolution and writes the stub's row after the arcs are known,
    a stub it made of names a record left included. The synthetic store
    has the shapes — a stub the batch also settles, one it reads only
    for the arc, and a stub made of leftover names cited alone and
    beside a held name in the same batch — and the sample harness
    passes.
22. **A record delivered twice, its references.** The full merge takes
    such a record's names from its first copy and its view from the
    copy with the latest revision stamp, but reads the references of
    every copy in both passes: its arcs are the union of the lists, its
    stubs the union of the names they cite. The batch replaces a
    re-deposited record's list. A probe on the synthetic store — B citing
    E, then delivered again citing A alone — left the batch without the
    arc from B to E and the full merge with it. The merge plan's §8 says
    the copy with the latest revision stamp contributes; PubMed's update
    files deliver records again every day, so which side changes is to
    settle before they arrive, and the sample harness cannot see it. A
    second probe found worse in the full merge: it takes a record's names
    from its first copy, so a name only a later copy carries — a DOI
    PubMed added — is minted as a work of its own, and once a batch had
    attached that name to the work, the next full merge split the work's
    id in two. The sample has no such work yet. *Decided 2026-09-17: the
    newest copy speaks for all of the record, in both merges (§10). Not
    built.*
23. **A deleted record's citers.** The batch removes a deleted record's
    arcs but leaves its work among the citers of the stub names it
    cited; the full merge does not count it. A probe on the synthetic
    store — two records citing a stub, one deleted while its work stays
    held — found that one entry in the stub's evidence and nothing else,
    but it does not stay evidence: in a second probe a later record took
    over the stub, and the batch used the stale citer to mint a stub for
    the name the record left and draw an arc to it from the withdrawn
    record's work, where the full merge dropped the name. A re-deposit
    whose new list drops a stub leaves the same stale citer. Invisible
    to the sample harness, which has no deletions or copies. Related:
    the full merge's first pass still reads a withdrawn record's
    references when it decides which names are stubs, and the test
    fixture skips them. *Decided 2026-09-17: the batch removes stale
    citers (§10). Not built; the first-pass question is open.*
24. **Finding things by paths that were once true.** The user's
    condition on removing stale citers (§10, 2026-09-17): a citation from
    a withdrawn record, a name a later copy no longer carries, a name the
    registry dropped — each was out in the world, and someone may look
    for a work by it. None is authoritative, and none may build a work
    or an arc; but the layer should keep as many of these paths findable
    as it can, answering by them as what was once so rather than not at
    all. The registry already keeps every dropped name and when; stale
    citers and superseded copies keep nothing yet.

## 12. Sources consulted

The serving-shape and feeds conversations of 2026-09-15 and the notes
kept of them; the graph plan's §5 and §11; crates.io on 2026-09-15 for
`tonic` 0.14, `hyper` 1 and `axum` 0.8; the six publisher feeds probed
read-only the same day; the data terms of Crossref, NLM, OpenAlex,
Semantic Scholar and arXiv as noted 2026-09-15.
