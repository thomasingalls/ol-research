# Serving plan

**Status:** draft 1, 2026-09-16 — the fourth layer: everything a query
touches, built over the merged layer and the graph. Written from the
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
3. **Incremental merge.** The identity pass over one record: its names
   looked up in the live names table — the names base plus the registry's
   tail, the same structure the names artifact serves from — and the
   merge plan's rules applied to the component the record touches: reuse,
   mint, merge, split, each a registry line and, for a mint, a public-id
   line (§6). Then the second pass for that record alone: its view, its
   relations, its references resolved through the same table, an unheld
   name minting a stub. What it appends: the works delta (the merged
   record, whole), the edges delta (`add` for each resolved reference;
   `remove` for each edge of that work's previous version, read from the
   graph's own forward list, that the new list lacks; `retire` echoing a
   registry `merge`), the relations and unresolved deltas likewise, and
   the journal line last.
4. **The artifacts fold.** The names reader has the registry tail; the
   graph reader folds the edges delta into its map of effective labels;
   the works reader indexes the delta's latest record per id; the lexical
   engine adds or updates the document and commits with the batch number.
5. **The watermark advances** when all four have the batch, and the
   record answers.

Two tests, both the merge's own tests extended a layer: after *n*
incremental batches, a full merge appends nothing to the registry and its
merged sets equal the previous base with the deltas folded; and a query
core over an epoch plus *n* deltas answers every question identically to
one over the epoch built after them, and the compacted bases are
byte-identical.

Until SP5 lands, steps 3 and 4 are the nightly compaction, and the layer
is a day fresh. That is already what PubMed and OpenAlex give, and the
plan builds the batch shape first because it is a serving system on its
own, then the delta path, then the origins.

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
wanted, on `axum` 0.8. The transport is where `WorkId` becomes public id
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
  a cache like the sets beside them, JSON lines first and the binary
  framing when volume asks (the registry's path, ingestion plan M9);
  `names/v<…>/`, `works/v<…>/`, `search/<engine>/v<…>/` and `graph/v<…>/`
  as bases, each versioned by model, merge and its own version;
  `epochs/<n>.json` and `epochs/current`.
- **Commands.** `ol-rht names build`, `ol-rht works build`, `ol-rht search
  build --engine <name>`, each from the merged directory with a report;
  `ol-rht epoch publish`, which builds what is missing for the current
  batch and writes the manifest and `current`; `ol-rht epoch check`,
  which verifies every base's digest and runs the byte-identical and the
  base-plus-deltas tests; `ol-rht-serve --store <root> --listen <addr>`.
  The incremental merge is `ol-rht merge --batch <listing>` when SP5
  lands.
- **Fail fast.** A delta line whose batch is not the journal's next stops
  the reader; an epoch naming a base whose digest differs stops the
  open; a public id that does not fold to a live work is an answer, not
  an error; every build prints its report.

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
- **SP5** — the delta path: the journal, the incremental merge over a
  batch of normalized records against the live names table, the four
  delta logs, every reader folding, the graph plan's G4, the lexical
  incremental commit, the two tests of §5, and the freshness measured
  from raw on disk to answered.
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
   registry tail is folded into a side map and merged at compaction. How
   large the tail grows in a day at scale decides whether a day's
   compaction is enough for it.
7. **`expand`'s budget** and what it dedupes on; the product's, once
   candidate management is a plan.
8. **Incremental identity's edge cases.** A record bridging two held works
   (a merge), a name moving between works (a split), and a deleted PubMed
   record — each a registry event whose serving consequence is spelled
   out at SP5 with a fixture.
9. **Consistency across artifacts** is by watermark; what a reader shows
   while one artifact lags — the older watermark, never a mix — and how
   long a lag is tolerated before it is an alarm.
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
12. **The registry fold in `names build` and `epoch check`.** Twenty
    seconds and most of a gigabyte on the sample to learn two
    retirements; at scale, minutes and tens of gigabytes. The
    retirements should be readable without folding the names — a side
    file the registry keeps, or the journal carrying them — before the
    corpus.
13. **`work(id)` memory.** A lookup decompresses one 256-row block, tens
    of kilobytes; `rows()` for the cross-check decodes every block in
    turn. Both fine on the sample; the corpus wants the check sampled or
    parallel.

## 12. Sources consulted

The serving-shape and feeds conversations of 2026-09-15 and the notes
kept of them; the graph plan's §5 and §11; crates.io on 2026-09-15 for
`tonic` 0.14, `hyper` 1 and `axum` 0.8; the six publisher feeds probed
read-only the same day; the data terms of Crossref, NLM, OpenAlex,
Semantic Scholar and arXiv as noted 2026-09-15.
