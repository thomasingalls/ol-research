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

The cited-as lookup, measured on the sample 2026-09-18 (§11, item 28):
59,079 names and 228,938 rows — 3.5 MB, of which 0.9 MB is the transducer
and 2.7 MB the rows at 12 bytes each — against a 241 MB works index and a
227 MB merged layer, because only 188,027 of the sample's references land
on a work it holds. At the corpus nearly every reference lands on one, so
the rows are one per name a reference gives: the artifact grows with the
edge count rather than the work count, and is measured again when the
corpus is merged.

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
| **cited-as**| a name's text form              | a transducer over the `cited_as` set: every name a reference gave, with the work it landed on, the citing work and its source (2026-09-18) | the `cited_as` delta, whole per citing work and source | nothing the world asks: who cited a name, and what an incremental merge reads to resolve a reference again |

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
   replacing the rows' and its deletions removing them. Which copy speaks
   is the full merge's rule (merge plan §8, 2026-09-17): of a record's
   copies in the batch and the one its row holds, the newest by stamp,
   version and set — a row's record names its set and version for this —
   and a deletion, the batch's own or one of the store's deletions sets,
   withdraws the copies in the sets before it, the newest deletion
   counting, which is an accepted loss (merge plan §10). That is possible
   without the normalized layer because every record in a merged row
   carries its evidence — its assertions and its view (merge plan §5,
   version 2 and 3). The registry's answer — reuse, mint, merge — is a line
   each, and a mint a public-id line (§6). References resolve through the
   batch's own resolution first and the live table second; a reference
   naming no held work joins the stubs it names with the names it does
   not, exactly as the full merge groups them, so an existing stub gains
   an alias or a citing source, two stubs one reference names merge, and
   a name nothing holds mints one. The previous list of each batch
   record's source, read from the graph's own forward list, is diffed
   against the new: `add` and `remove` lines; a retired id's remaining
   arcs and its citers move to the survivor; relations and unresolved
   rows are put whole per work and source, and moved on a retirement. A
   stub's citers follow the arcs: a withdrawn record's citations come off
   every stub it cited before, a retired citing work's are restated under
   its survivor, and then the new landings go on, a stub's row written
   once they are known and only if it changed. A re-delivered record's
   citations stand, its earlier copies' included (2026-09-18), and so do
   its names: absence is not an assertion (merge plan §8, 2026-09-18), so
   a batch's copy joins the names the row's copy gave and fills its own
   empty fields from the row's, and a copy older than the row's — a
   baseline file merged after the update files — joins its names and
   values to the row rather than only its citations. A text the batch
   needs and no row kept is read from the normalized layer: the record's
   view names the set its speaking copy came from, so one set is opened
   and scanned (`serve::incremental::Texts`, answered by the binary, which
   knows which crate reads a source's sets).
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

Two cases the incremental rules leave to a full merge, each stopping the
batch before it writes anything and saying why: two ids retired in one
batch with an arc between them; and, since 2026-09-17, a stub the batch
made of a record's leftover names cited together with another stub, and
a work whose citation moves to such a stub retired in the same batch.

A third is gone since 2026-09-19: **a work's names splitting between
works**, which the batch now performs (merge plan §11, item 3). Dividing
a row is one problem whether one part keeps its id or none does, so it
is one path: the names each part took, the names no part took going to a
stub of their own where a reference cites them without a name a part
took, and every reference that landed on the row settled again by §6 —
the citation text before the order — over the row's own evidence. Where
no part continues the row, the id is retired with no successor, its row
and its arcs go, and its parts answer for it (§11, item 24). Three
refusals hold the path to what the row can say (item 28): a reference
whose text repeats a DOI on more than one side, one that gave a single
namespace on more than one side, and one whose every name here is
dropped. A fourth is the split's own, and a guard rather than a shape
seen: a row that splits while keeping a record the batch does not carry,
whose arcs, relations and unresolved rows only the normalized layer
could divide between the parts.

A fourth is gone since 2026-09-18: a work losing its last record while
something cites it keeps its id and becomes the stub its citers name,
name by name, which the cited-as evidence says (item 28). Three more are
gone the same day, all to one reader: a text — an abstract, author
names, a venue name — that a record outside the batch and not promoted
before would have to supply is read from that record's own normalized
set, and so is the record a work keeps of a source whose twin the batch
withdrew or moved, whose arcs, relations, unresolved rows and cited-as
evidence it restates in place of the pair's. Two things a batch does
that a full merge then tidies, both reported: a work with a duplicate
record outside the batch has arcs added and none removed, and a stub
left with no citing arc keeps its names until a compaction drops them.

Two tests, both the merge's own tests extended a layer: after *n*
incremental batches, a full merge appends nothing to the registry and its
merged sets equal the previous base with the deltas folded; and a query
core over an epoch plus *n* deltas answers every question identically to
one over the epoch built after them, and the compacted bases are
byte-identical. Both run on a store built in memory (`serve`'s tests: new
records, a re-deposit, a bridging merge, stubs cited again, deletions, the
refusals, and since 2026-09-17 records delivered again with other names
and shorter lists, withdrawn citers, and a batch older than a deletions
set the store holds) and on the sample: one PubMed baseline file held out
(`crates/ol-rht/tests/incremental.rs`), and PubMed's update files held out
and merged one batch each (`crates/ol-rht/tests/update_files.rs`), both
when `OL_RHT_STORE` names the store, where `serve::compare` walks every
id, every key of either base and the fixed queries.

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
  every part, an ambiguity the API says out loud rather than guessing.
  Built 2026-09-19 (§11, item 24): the `split` outcome carries the
  parts, followed to live works.

## 7. The query core and its transports

**The core** is a library, synchronous, over one open epoch and its
deltas, speaking `WorkId` inside and nothing else (landed 2026-09-16 as
`serve::Serving`, with `query work | search | expand` in the binary): `resolve(name) →
Option<WorkId>` for names and public ids alike; `work(id) → Record`, the
merged record with its names and standings, promoted fields with their
disagreements, and its source records, with `rows(ids) → map` beside it,
which is the same answer for a whole page in one walk of the index
(2026-09-19, §10; §11 items 13 and 31); `references(id, limit)` and
`citers(id, limit)` as the graph gives them, lazy, each arc with its
asserters; `search(query, filters, facets, limit) → hits and facet
counts` over the promoted fields — year, form, genre, language, status,
venue, the sources holding the work, and later MeSH, topics and entity
ids as the ambitions ask (§8 there); `expand(seeds, walk, budget)`, one
hop out from a seed set, deduplicated, scored and bounded, the primitive
the funnel starts from; and `path(from, to, bounds)`, the bounded
directed search of the graph plan's §7; and `related(work, by, budget)`,
which is `expand` over a seed set the work itself owns — the two
questions a reader actually asks, gathered here rather than by the
client (2026-09-19, §10). Every answer carries the epoch and the
watermark it was answered at.

**The two traversal primitives**, since they are where the funnel starts
(both 2026-09-19, §10). `expand` takes the direction as an argument —
`references`, `citers` or both — because the two features that use it go
opposite ways on both axes: co-citation seeds with a work's *citers* and
walks *references*, bibliographic coupling seeds with its *references*
and walks *citers*. It returns, per neighbour, the raw overlap, the
degree that overlap is out of, and **Salton's cosine** over the two,
`|S ∩ N(v)| / sqrt(|S| · |N(v)|)`, which is what it ranks and truncates
by. `N(v)` is the orientation *opposite* the one walked — the arc that
reached `v` from a seed is an in-arc of `v` — and its size is two
offsets, so the normalisation costs no second walk. `path` answers with
the shortest paths there are, or with *no path within this bound and
this is how far I looked*, or with *my budget ran out and here is the
smaller bound I did prove*: three outcomes, never an empty list, since on
this corpus the second is the ordinary one (graph plan §7).

**The two questions over them** (2026-09-19, §10). `related(work, by,
budget)` is one call where a client otherwise made two — a page of a
list, then that page handed back as seeds — and the second call is not
merely chatty but a different question, since the seeds are then
whichever page the client was holding. `by` is `cites-same`
(bibliographic coupling: seeds the work's references, walks citers) or
`cited-alongside` (co-citation: seeds its citers, walks references), and
neither is a default. The subject is left out of its own answer, which it
must be: every seed reaches it, so its `touches` is the whole seed set
and the seed set is the orientation its degree counts — it scores exactly
one against itself and heads every list it is not dropped from. The
seeding is bounded by the **arcs** walking it would read and not by a
count of seeds, since a seed's own list may be any length, and both
numbers come back so a bounded answer reads as one.

**The search box** (2026-09-25, §10). What a person types is read by
our own parser (`crates/query-parser`) into a query tree that every
engine is asked from, and never handed to an engine as its own query
language: Tantivy's parser, which read the box until then, took `:`,
quotes, brackets, `^`, `~`, `*`, `+`, `-` and `AND`/`OR`/`NOT` for
commands, refused `5'-UTR` and `(2R,3S)-dihydroxy`, let `id:[1 TO
100]` walk the internal numbering, and took a second to read twenty
nested parentheses, each one more doubling it. The language's first
pass is plain text — every character is text, a word is what lies
between whitespace — and its one refusal is size: 1,024 words, 64 KiB.
Each engine's translation of the tree lives beside the engine
(`search::translate` for Tantivy); the tree keeps each word as typed,
with its place in the text, for the engines to split as they indexed
and for a highlighter to mirror. A search runs on a thread of its own,
never on the threads that answer requests, at most `--search-threads`
at once and each within `--search-deadline`, and writes one line to the
query log on standard output, apart from the fault log on standard
error. Its second pass is §11, item 36; what comes next, item 37.

**One hydrated row shape** across the three lists that carry works —
an arc of a page, a neighbour of an expansion, a step on a path — since
they had drifted into three (2026-09-19, §10): `public_id`, `stub`,
`title`, `names`, `authors`, `venue`, `published`, present when the
request asked to hydrate and the work has a row, absent when it did not
or has none. A stub is what it is shaped for: 98.55 % of reference
targets are stubs, which have no title and never will until something
holds a record of them, and their promoted names are what a reader can
be shown instead.

**The transport** is a second binary: gRPC first, decided 2026-09-15 on
`tonic` 0.14 over `hyper` 1, with server streaming for a hub's citers
and deadlines on every call; a JSON facade over the same core later if
wanted, on `axum` 0.8. Landed 2026-09-16 as `ol-rht-serve` (SP4). It was
wanted: the facade landed 2026-09-19 as `ol_rht_serve::http`, nine GET
routes under `/v1` on a listener of its own, each calling the same
`service` function its gRPC method calls and answering the same message
serialized as JSON — a browser cannot speak gRPC over `fetch`, and
nothing is shaped twice, so the two transports cannot drift. The
protocol's two streams answer one page a request there, resumed by the
cursor the page carries; the origins a browser may read from are an
exact list and never `*`. The transport is where `WorkId` becomes public id
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
- **`crates/query-parser`**, the search box's text read into the query
  tree (2026-09-25): no engine in it, so that every engine's translation
  reads the same tree, and so that it can be compiled for a browser if
  the highlighter that mirrors it is built that way.
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
  and removed; `ol-rht-serve --store <root> [--listen <addr>] [--http
  <addr>|off] [--allow-origin <origin>]… [--poll <seconds>]
  [--search-threads <n>] [--search-deadline <seconds>]`, which folds
  the journal every poll, one second by default, runs at most
  `--search-threads` searches at once (half the cores unless told), each
  within `--search-deadline` (ten seconds unless told), and serves §7's
  JSON facade on a second listener — `127.0.0.1:50052` unless told
  otherwise, `off` to leave the port unbound, and the origins named
  exactly, the Vite dev server's two in both spellings of loopback when
  none is given. Both listeners are bound before anything else starts,
  so a port already taken stops the server rather than leaving half of
  it up.
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
  the arc (§11, item 21). Fixed the same night, the harness passed on
  the sample: nothing appended to the registry, and no difference in
  4,040,017 ids — until the store held PubMed's update files, the next
  day (§10, §11 items 25 to 27).*
- **SP6** — origins: the `watch` kind with the ingestion plan's M9, the
  cursor per source, the first-seen measurement, and then, if the
  measurement earns it, publisher feeds.

## 10. Decision log

- 2026-09-19, one read for a page, one row shape for every list, and
  `related` — three changes with one reason between them: a browser
  asking one question should make one request and get back rows it can
  render (§7; §11 items 7, 13, 31 and 32). Written against the frontend
  as it was being written, so the shapes here are a contract.

  **The bulk read.** `Serving::rows(&[WorkId]) → HashMap<WorkId, Row>`
  beside `work(id)`: the ids deduplicated and sorted, the works delta's
  overrides applied exactly as `work` applies them — the same row either
  way, tested over a store with a batch folded over it — and the base
  walked once per block rather than once per id. A map and not the
  caller's order, because every caller already holds that order (its
  page, its neighbours, its path) and wants the row *of* a work while
  walking it; an id with no row is not a key, which is `work` answering
  `None`.

  Measured on epoch 7, the index now counting the blocks and the bytes
  it decompresses:

  | a page of | one row at a time | in one call |
  |---|---|---|
  | 100 citers (arcs, id order) | 39 blocks, 25.6 MiB, 19.5 ms | 39 blocks, 25.6 MiB, 19.4 ms |
  | 20 neighbours (ranked) | 20 blocks, 3.3 MiB, 2.6 ms | 17 blocks, 2.3 MiB, 1.9 ms |
  | 100 neighbours (ranked) | 100 blocks, 7.7 MiB, 6.4 ms | 85 blocks, 5.6 MiB, 4.8 ms |
  | 100 search hits (ranked) | 93 blocks, 54.9 MiB, 41.3 ms | 59 blocks, 34.5 MiB, 26.1 ms |

  The honest reading: the win is small, and the cost it made visible is
  not. A page of arcs comes out of the graph in id order and the index
  already keeps the last block decompressed, so the run through a block
  was already collapsing — nothing to win there, which is the premise the
  work started from and it was wrong. The win is where the ids arrive
  *ranked*, which is every list a score orders: 1.2× to 1.6× fewer blocks
  and the same in time, and it cannot lose.

  What the byte counter says is the thing to act on, and it is item 31:
  hydrating a hundred held rows decompresses **25.6 MiB to keep 349 KB**.
  A block of held works is 687 KB raw where the average block is 38 KB,
  because stubs are 97 % of the index and nearly free while held rows are
  large and cluster together. End to end, `/v1/citers?page_size=100` is
  **0.5 ms** and the same page with `hydrate=true` is **20 ms** — forty
  times, all of it block decompression for rows thrown away.

  **One row shape.** The three converters had drifted into three: an arc
  and a path's step carried a title and promoted names, a neighbour
  carried a title alone and had no names field at all. A stub has no
  title, so an expansion landing on one — 98.55 % of reference targets
  are stubs — had neither a title nor an identifier and could not be
  shown. Now all three carry one `Summary` message, in its own field, so
  the drift cannot recur and a client renders any of them with one
  function: `public_id`, `stub`, `title`, `names`, `authors`, `venue`,
  `published`, and since 2026-09-24 `retracted_by`, `corrected_by` and
  `concern_by` (§11, item 34). Present when the request asked to hydrate
  and the work has a row; absent (null) when it did not or has none,
  rather than an empty summary, which would read as a work we know
  nothing about. `Arc.title` and `Arc.names` and `Neighbour.title` are
  gone and their field numbers reserved. Every field is read exactly where `Work` reads it, so a
  summary cannot disagree with the work it summarises.

  **`related`.** `GET /v1/related?work=&by=&limit=&hydrate=` and the
  `Related` RPC, both through the same function in `service.rs`;
  `/v1/expand` stays as it is, the general primitive under it. `by` is
  `cites-same` or `cited-alongside`, named and required, because the two
  are opposite questions and a default would answer one to a client that
  meant the other. The server gathers the seeds, which is the point: two
  calls meant the seeds were whichever page the client happened to be
  holding, so a work with 882 citers was answered about the fifty the
  client had fetched, and a client that paged twice got a different
  answer from one that paged once.

  The subject is dropped from its own answer, inside `expand` and before
  the truncate. It has to be, and the reason is arithmetic rather than
  taste: every seed reaches the subject, so its `touches` is the whole
  seed set, and the seed set *is* the orientation its degree counts —
  `|S| / sqrt(|S| · |S|)` is exactly one. It heads every list it is not
  dropped from, and nothing else can reach it: on epoch 7 the PRISMA
  statement's best real co-citation neighbour scores 0.259 and its best
  coupling neighbour 0.983.

  **The seeding is bounded by arcs, not by seeds**, which is the one
  decision here that a measurement reversed. A cap on the seed count
  bounds no work at all, since one seed's list may be any length; the
  cap is therefore 250,000 arcs — about a tenth of a second at the 2,444
  arcs a millisecond measured here, and the same order as the path
  search's 250,000-work visit guard. Over it, the seeds kept are those of
  **smallest degree in the orientation about to be walked**. That was
  first argued from cost and specificity alone; measured against the
  unbounded answer — PRISMA's co-citation over all 882 of its citers —
  it is also simply the better answer at equal cost:

  | budget | seeds, smallest first | top-20 kept | seeds, list order | top-20 kept |
  |---|---|---|---|---|
  | 10,000 arcs | 256 | 8 of 20 | 87 | 1 of 20 |
  | 25,000 arcs | 463 | 13 of 20 | 234 | 7 of 20 |
  | 50,000 arcs | 680 | 17 of 20 | 409 | 10 of 20 |

  A budget buys three times the seeds when it spends them on short lists,
  and a work citing three hundred others puts the subject in almost no
  company — the same reason the cosine discounts a hub, applied a step
  earlier. The earlier reading of this, at a fixed *count* of 100 seeds,
  said the opposite; it was measuring four times the arcs for the list
  order and calling it fidelity.

  Nothing in this corpus reaches the bound — the most-cited work walks
  94,098 arcs for its co-citation, 3,147 for its coupling — so every
  answer today uses every seed there is and says so. `seeds_used` against
  `seeds_total` is what says it; `total` is what the hop reached before
  `limit` truncated, as a list's `total` is to its page.

  End to end on epoch 7, warm: `related?by=cited-alongside&limit=20`
  hydrated on the most-cited work we hold is **26 ms** and 11.7 KB,
  `by=cites-same` **9.4 ms** and 19.3 KB. The answers are what they
  should be — the co-citation of the PRISMA statement is RoB 2, ROBINS-I
  and Rayyan, and its coupling is its own two sibling republications
  ahead of everything else.

  `ol-rht query related <work> --by <cites-same|cited-alongside>`
  follows, and `query expand` reads its titles through the bulk read.
  No version bump: nothing on disk changed, so the user's epoch 7 serves
  all of this as it stands.
- 2026-09-19, the traversal primitives — `expand` gains a direction and
  a degree normalisation, and `path` is built (§7, §11 items 7 and 29;
  graph plan §7, §10). Two decisions taken with the user, and one
  measurement taken before either.

  **The measurement first**, because it decides whether path is worth
  anything on this corpus: the subgraph induced on held works alone —
  which is the only subgraph a directed path moves through, a stub
  having no references and so ending any forward walk — holds **93,007
  distinct arcs over 148,210 works**, a mean out-degree of **0.628**.
  That is an order below percolation, and the components agree: 107,584
  weak ones, the largest holding a quarter of the held works and the
  next 44; 148,122 strong ones, the largest of size 3. Of 200,000 random
  held pairs, 4 have a directed path within six hops. The user's estimate
  was mean out-degree ~1 from the merge report's 185,328 assertions
  landing on held works; distinct arcs are half that, since a held→held
  arc carries 1.95 asserters on average. So the claim is verified, and
  understated. The full table is the graph plan's §7, and two report
  lines print it on every `graph build` from now on.

  **Direction.** The walk is `references`, `citers` or both, and both is
  only the default because it is what `expand` was. The reason is not
  symmetry but that the two features that want it are mirrors of each
  other on both axes at once: co-citation seeds with a work's citers and
  walks their references; bibliographic coupling seeds with its
  references and walks their citers. Neither is served by a walk that
  does both.

  **Normalisation, inside `expand` and before the truncate** — which is
  the whole of the point, since normalising a page already chosen by raw
  overlap would normalise a set the hubs had picked. **Salton's cosine**,
  `|S ∩ N(v)| / sqrt(|S| · |N(v)|)`, over Jaccard: it is the co-citation
  literature's own measure, and its square-root denominator *discounts* a
  hub where Jaccard's linear `|S ∪ N(v)|` all but deletes any work with a
  long citer list whatever it shares with the seeds. The two agree on
  15.5 of the top 20 in any case; the choice that matters is normalising
  at all. Measured over 500 co-citation expansions on epoch 7: the top
  twenty by raw overlap and by cosine share **8.5 works of 20**, and the
  median citer count of the top hit falls from **14 to 4**. The deeper
  reason is that the raw count is nearly no signal at this scale — those
  runs threw up 911,071 candidates touched by exactly one seed against
  39,951 touched by two, so an unnormalised ranking is a tie over almost
  the whole answer, broken by id order.

  The raw overlap and the degree travel beside the score, because "7 of
  its 9 citers also cite this" is a sentence a reader can weigh and a
  cosine is not. `N(v)` is the orientation opposite the one walked, and
  its size is two offsets on the base — no second walk — with the folded
  list counted instead for the few works a recent batch touched.

  **`path` is directed.** A → B means "A cites something that cites … B";
  an undirected walk wanders up and down the arcs and means nothing, so
  the other direction is the other question, asked by swapping the ends.
  Bidirectional breadth-first, bounded by depth, by how many paths to
  return and by a visit budget; the two half-searches cover the bound
  exactly, so an empty meeting is a *proof* of absence rather than a
  failure to find. Hence three outcomes in the protocol as a `oneof` —
  paths, none within the bound with how far it looked, or stopped with
  the smaller bound it did prove — and never an empty list, which on this
  corpus would be the answer to almost every question and would read as
  an error.

  Both are on both transports through the same free functions in
  `service.rs`, so neither surface can drift: `Expand` gains a `Walk`
  enum and `Neighbour` gains `degree` and `score`; `Path` is a new RPC
  and `/v1/path` a new route, where the direction is named
  (`walk=references`) and not numbered. Every cap is refused and never
  clamped, in that layer's existing style, and a `walk` value the
  protocol does not have is refused rather than taken for the default.
  `ol-rht query expand --walk` and `ol-rht query path` follow.

  No version bump: nothing on disk changed. `graph::path` is a reader
  over neighbourhoods a caller supplies, so the served path folds the
  edges delta like every other answer — tested by applying a batch and
  walking over the arc it added.

  One wart the sample turned up, left to the caller: co-citation seeds
  with a work's citers, and every one of them cites that work, so the
  work heads its own co-citation list at a score nothing else reaches.
  `expand` is given seeds and never the subject, so it cannot know to
  drop it.
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
  as possible as it can be (§11, item 24). Settled the same evening, on
  the user's word for both recommendations: a stub name's sources are its
  citers', and a withdrawn record's references make no stubs.
- 2026-09-19, the batch performs a split, and a split id answers — the
  split rule of merge plan §11, item 3 reaching this layer, taken with
  the user after measuring the corpus's fourteen splits. The batch had
  stopped on any split since 2026-09-16, "since the citing references
  would have to be resolved anew to follow the parts". They are — by the
  path built that morning for a record taking over part of a stub (§11,
  item 28), which is the same problem: a row's names divided, each
  citing reference settled again by §6 over the row's own evidence. So
  the two became one path rather than two, and dividing a row now asks
  the same questions whether one part keeps the id or none does.

  What the split added to it: destinations that are *held works* and not
  only the stubs leftover names become; a retirement with no successor,
  which the batch's retirement machinery did not model — it walked a
  single chain of `(id, into)` — and now carries parts beside it; and
  one refusal of its own, a row that splits while keeping a record the
  batch does not carry, whose arcs, relations and unresolved rows only
  the normalized layer could divide — a guard, since every row the
  sample splits is a stub or a work whose records the batch carries. The
  three refusals of item 28 carry over unchanged, and the proof they
  rest on holds unchanged with them: it never named one side as the
  row's continuation.

  Measured on PubMed's five update files, held out of the sample and
  merged one batch each in path order (`update_files`): all five now
  apply, `1339` included, where the run stopped at the third (`1336`)
  before. `1336` performs the only two splits in the files, both the
  fallback — a stub of three names two of its own records divide between
  them (§11, item 25) — and the fourteen the corpus holds are all the
  first clause's shape, one held part and one bare stub. Both halves
  were needed. One line of churn is left over the whole run, and it is
  the leftover stub's grouping rather than the split (item 25).

  The three hold-outs that guard against regression, each a full merge
  without one file and then that file as a batch, are unchanged at **0
  differences**: `pubmed26n1334` 0 of 5,228,675 ids, `pubmed26n1335` 0
  of 5,237,827 and `pubmed26n1337` 0 of 5,227,709. Of the four refusals
  none fires on the sample. The fourth is a guard rather than a
  measured shape: no row the sample splits holds a record at all — all
  fourteen of the corpus's and both of `1336`'s are stubs or works whose
  records the batch carries — so it stands against a shape no fixture
  could be made to produce either, the identity pass joining a record to
  the work whose name it claims wherever the sample was pushed to part
  them.

  And a split id now **answers** (§11, item 24). The registry has always
  written the parts in its `split` line and the fold has always thrown
  them away; it keeps them now, the names base gains a `split.u32`
  beside `retired.u32`, and `resolve`'s `split` outcome — which carried
  back only the id the reader had just sent — carries the works its
  names went to. `serve` version 5; the merged layer is untouched, and
  so is the registry's format, since a keeper reuses its id or merges it
  into an older one it also claims, both of which the log already says.

- 2026-09-19, the batch runs §6 on a split instead of imitating it —
  asked for by the user, who saw that the affected names are a finite,
  knowable set and asked whether the data on disk was already enough.
  It was, once a stub's row carried two marks per citer (merge plan's
  entry the same day). Where a record arrives for part of a stub, the
  batch no longer asks *did this citer also give a name the record
  kept?* — a guess that agreed with §6 only while the order alone
  decided — but rebuilds each citer's reference as the row's names
  divide it and settles it the way `resolve_reference` does.

  Two questions had been answered by that one test, and separating them
  was most of the work. **Membership** — is a leftover name a name of its
  own? — is the merge plan's §11 item 14: a reference naming a held work
  makes no stub, its other names being assertions about that work. **The
  arc** is §6's. They run in that order, since §6 can only weigh names
  that still exist. Both halves were caught by the harness rather than by
  reading: the baseline hold-out's full merge appended two registry lines
  the batch had not (`drop pmid:41560844`, `drop pmid:41567733`) because
  the batch minted stubs for names given only beside a held one; and a
  fixture then showed a reference can name a held work *and* land on the
  stub by its text, which the row had no way to know — hence the second
  mark.

  Three shapes are refused rather than guessed, and all three are §6
  reaching past the row: a reference whose text repeats a DOI on each
  side of the division; one that gave a single namespace on each side,
  which §6 settles by the order the reference wrote its names in; and one
  that named a held work and has every name it gave here dropped, whose
  arc then leaves for a work the row cannot name. None fires on the
  sample.

  Measured over the three hold-outs, each a full merge without the file
  and then the file as a batch: **`pubmed26n1334` 0 differences of
  5,228,675 ids** (6 before, all of them this path), **`pubmed26n1335` 0
  of 5,237,827** (16 before) and **`pubmed26n1337` 0 of 5,227,709**. Two
  bugs on the way were worth their own note: the batch had been keeping
  the first citer of a record and skipping its other references, where
  the full merge joins them — which loses a later text mark and a later
  held-beside mark, the two directions the diff showed; and the arcs were
  right in every hold-out before either fix, the differences being only
  the new marks. What this does not touch: `pubmed26n1336` still stops on
  a work's names splitting between works (§11, item 3), as before.
- 2026-09-18, absence is not an assertion, on the batch — the merge
  plan's §8 rule (its §10, decided with the user the same day) reaches
  the incremental path in three places: a batch copy that speaks over the
  row's joins the row's names and fills its own empty fields from the
  row's view; a batch copy the row's speaks over — a baseline file merged
  after the update files that superseded it — no longer gives its
  citations alone but joins its names and values to the row's record, its
  work pulled into the component for it; and the fixture's full merge
  joins standing copies the same way, so both merges answer alike. One
  stop goes with it: a record that drops a name no longer parts its work
  between two.
- 2026-09-18, a twin's arcs and the ids they are in — the `pubmed26n1335`
  hold-out diverged from a full merge by nine arcs and citer lists, of
  which four had one cause: when a batch withdraws one of a work's two
  records of a source, the surviving twin's arcs are resolved against the
  ids *after* the batch, while the graph's existing arcs are in the ids
  *before* it. An arc whose target the same batch retired is the same arc
  under a new id, and the diff took it for one the twin no longer gives:
  it was removed and never restated. The deletions loop now follows the
  retirement for those (the retirement itself leaves them alone, the
  pair's record having been withdrawn) and adds any arc the twin gives
  that the graph has not. Found by hand on the sample: work #94114 held
  two PubMed records, 1335 withdrew one, and stub #3128522 merged into
  #882716 in the same batch.
- 2026-09-18, who cited a name (§11, item 28) — the larger citers list
  the user chose ("let's do this version, i think"), in the shape SP7
  already had rather than a new artifact: the `cited_as` set keeps a row
  for every name a reference gave beside the work it landed on, not only
  for the names nothing holds. The delta is written per citing work and
  source, as it already was, so a citation costs one line on the citer's
  side and never rewrites the cited work's row — which is why the list
  lives here and not on the work's names, where every citation would
  rewrite a row. It answers the two cases a batch could not: a name
  leaving a work that references still cite becomes a stub carrying those
  citers, their arcs following it; and a work losing its last record
  keeps its id, becomes a stub, and its names take the citers the
  evidence names. Relations are in the set too, flagged: they make no arc
  and no citer, but a name a relation names is a work of its own once it
  leaves, which is what the sample's own case turned on.
- 2026-09-18, records from the normalized layer — a batch stopped when a
  work's promoted abstract, authors or venue came from a record outside
  the batch whose text no row kept (§11, item 25: PubMed's update file
  `1335`), and again, on the same file, when a work held two PubMed
  records and one of them was withdrawn: the graph labels an arc by its
  source and not by which record of that source gave it, so the batch
  could not tell the twin's citations from the dead record's. Both are
  answered the same way. The row already names, per record, the set its
  speaking copy came from, so the batch opens that one prefix and scans
  it, rather than wanting a record index (item 18) or every record's text
  in its row. What it reads is a whole record — names, view, text,
  relations, references — so the twin that stays can restate the pair's
  arcs, relations, unresolved rows and cited-as evidence, and only the
  arcs no twin justifies come off. The reader is a trait the binary
  answers, `serve` knowing no source; without one the batch stops as
  before, which is what `serve`'s own fixtures still test.
- 2026-09-18, the cited-as evidence, built (SP7) — the divergence of
  §11 item 26 closed, on the user's word to keep "who cited this id" for
  every id and to start where correctness needs it. What the merge writes:
  a `cited_as` set beside the others, one row per name a reference gave
  beside the held work it landed on — the citing work, the source, the
  landing, the namespace that landed it, and whether that reference was
  the pair's only one landing there. 849 references on the sample give
  such a name, 9 KB of rows. What the serving layer adds: a `cited`
  artifact, a transducer from the name to its rows, published with the
  epoch; a fifth delta log, `cited_as.jsonl`, put whole per citing work
  and source; and a fold that indexes the tail by name. What the batch
  does with it: for every name it gives to a work or a stub it resolves
  those references again, moving the arc when the new name's namespace
  outranks the one that landed it, and restating the stub's citers. Both
  sample harnesses are green with it, the first time since the update
  files landed.
- 2026-09-18, the ingest fixes and the reference union, built — the
  user's four calls of the day, in order. (1) Ingest keeps every id: both
  of a PubMed record's DOIs, a list of publisher item ids, a `pmc` slot
  that holds something else as a malformed claim, and a reference's ids of
  any other kind; the checked-in exclusion list is empty again and the
  sample is normalized under `normalized/v2/` (model version 2, ingestion
  plan §10). (2) A record's citations are the union over its standing
  copies (merge plan §10, version 4): both passes of the full merge and
  the test fixture read every copy a deletion has not withdrawn; the
  incremental merge adds a re-delivered record's arcs and removes none,
  takes citations back only from a withdrawn record, and carries the
  copies a row's newer copy supersedes in `Batch.cited_only` so their
  references still make stubs and arcs. (3) A field's candidates order by
  native name rather than by the run's record order, so two duplicate
  records of one source land the same way in both merges. Measured on the
  sample: the baseline harness's full merge appends nothing and the
  comparison is down to the four rows of item 26; holding out
  `pubmed26n1337`, the 45,789 registry lines of item 27 are gone. (4)
  Publisher ids are stored but not yet names: the user's design is our own
  publisher key mapped to DOI registrants, and the measurement that decides
  which may be identity is in merge plan §3.1.
- 2026-09-17, copies and withdrawals built — `merge::copies` ranks every
  copy (stamp, version, set in path order, place in its set) and reads the
  deletions sets before the identity pass; both passes, the search
  documents' text and the test fixture ask it which copy speaks; a merged
  row's records name their set and version (merge version 3, serving
  version 4), and the batch ranks its copies against the row's and
  against the store's own deletions sets, which a batch of sets older
  than them must see. The batch takes a replaced or withdrawn record's
  citations back, restates a retired citing work's, and writes a stub's
  row only if it changed; two new cases stop it (§5). The synthetic store
  keeps the probes as tests. On the sample, with PubMed's first five
  update files normalized into the store (ingestion plan §10): the
  baseline harness, which now holds out a file older than the update
  files around it, no longer differs in text or registry, and differs by
  one reference whose names another work took (§11, item 26); the update
  files, tried one batch each, stop three times in five (item 25); and
  file 1337 alone, which applies, is followed by a full merge that drops
  24,081 names and splits 7,234 stubs the batch kept (item 27). Nothing
  was re-merged in the user's store: its registry would take the update
  files' ids for good, which is the user's call.
- 2026-09-17, hosting — the user's: two or three machines. One builds
  the search index, one serves, and possibly a third is swapped in for
  the live server once a build is complete. From the user's experience,
  updating a search index under load is not pretty, so unless
  incremental updates keep taking seconds, applying batches to the
  index the live server reads is probably not an option; and periodic
  full runs are operationally wise in any case. Only the machines and
  their roles are recorded here (§11, item 28).
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
- 2026-09-24, a server waits for an epoch it can read — the user's
  "yes, build both", after the image automation moved `ol-rht-serve` onto
  builds of model versions 4 and 5 and each exited on a store built under
  3, leaving the site answering nothing. `ol-rht-serve` no longer exits
  when it cannot open the store. When `current` names an epoch built from
  another merged layer (another model or merge version) it answers from
  the newest epoch built from its own; when there is none it listens,
  refuses every call `UNAVAILABLE` (503 on JSON), reports
  `olrht.v1.Serving` `NOT_SERVING` to health checks, and switches the
  moment an epoch it can read is published. Only the version decides the
  fallback: an epoch of the server's own version that does not open is
  broken, not foreign, and is not papered over with an older one. The
  poll now opens an epoch once, when `current` moves to it, rather than
  every second while it cannot. `Serving::open_epoch` opens one by
  number. The deployment half — a rolling handover gated on that health —
  is the deployment plan's (D5, 2026-09-24).
- 2026-09-24, the artifacts built without holding the corpus — the
  user's "start on the serving memory fixes", after a measurement put the
  corpus's `epoch publish` at 135–185 GB (`docs/inbox/serving-memory.md`).
  Three changes, every artifact byte for byte what it was:
  - The documents are assembled by id range. `merge::Assembler` spills
    each held work's document without its text, and each copy's text as
    it is read, to disk by the range its id falls in, keeping in memory
    only a compact entry per source record; an `Assembly` gives the
    documents back a range at a time, text filled in the order it was
    read, which is the order the copy rule needs. The works index asks for
    documents by increasing id (`merge::Documents`) and the lexical index
    takes them a range at a time (`search::index::Build`), so
    `epoch publish` builds both in one pass and lets the documents go
    before the cited-as lookup and the graph.
  - The cited-as lookup and the names lookup sort their pairs on disk
    (`merge::runs`) and write their transducers as the pairs come back.
  - The names lookup reads the registry for its ids alone
    (`registry::IdFold`) and the public log as a stream.
  On the sample the four artifacts and the lexical index's answers are the
  same, `epoch check` passes, and the whole publish's peak went from 998 MB
  to 808 MB; the sample's held works all fall in one range, so the saving
  there is small, and the corpus's figure is its first run. The works
  index still reads the public log into its two maps (about 4 GB on the
  corpus, E).
- 2026-09-24, the corpus's first epoch. Epoch 5, from the merge of record
  of the whole PubMed 2026 baseline (batch 5, 39,994,988 records, merged
  v8.13), published in 39 minutes and checked in 29
  (`ol-rht-publish-corpus-v1`): 41,990,151 works and 405,847,886 arcs
  served, every rebuild byte for byte, at 18 GB resident for the publish
  and 22 GB for the check (`docs/inbox/serving-memory.md`). The server's
  pod limit went from 8 to 64 GiB — the user: nothing else on the box
  matters as much right now — so that most of its 44 GB of mapped
  artifacts can stay in the page cache; the handover took the site from
  epoch 4 to 5 without a gap.
- 2026-09-25, the 18 GB. The works index reads the public log into a
  slot per id (`merge::public::ById`) instead of `Public`'s two maps
  (about 5 GB of the corpus's publish, D), and a range of documents goes
  before the next is read. The publish and the check print each step's
  heap. The check's 22 GB was the mapped pages of the indexes its
  cross-check reads, not heap (`docs/inbox/serving-memory.md`).
- 2026-09-25, serve version 7, with merge version 14 (merge plan §11,
  item 24). The names base keeps every name a row refuses with the works
  refusing it (`refused.fst`, `refused.u32`), and the fold keeps the same
  for the works delta's rows; `Serving::refusing` answers from both. A
  batch closes each component over a name's refusers as well as its
  holders, and gives each row's refused claims back to the record that
  made them, so the identity pass decides them with every claimant in it.
- 2026-09-25, the search box reads any text — the user's "stray
  characters can bring down the server! we need to be able to support
  arbitrary queries, including special characters", with PubMed whole in
  the index. A probe of about 120 inputs against Tantivy's own parser on
  a three-document index found:
  - **Nested parentheses doubling its time per level**, valid or not:
    twenty deep a second to read, twenty-two four seconds, twenty-three
    past the probe's five-second cap (release build). The search ran on the threads
    that answer every request, so a few such queries held them all,
    health checks among them.
  - **Ordinary scientific text refused as syntax**: `5'-UTR`,
    `(2R,3S)-dihydroxy`, `doi:10.1000/x`, `http://…`, an unmatched quote,
    `aspirin AND`, `-aspirin`. Every refusal went out as `INTERNAL` (500),
    whole query and all into the fault log, and the browser client
    retried it once, as it retries any `internal`.
  - **The engine's commands reaching past the text**: `id:[1 TO 100]`
    searched by internal `WorkId` and answered with public ids, the
    mapping the keyed public id keeps opaque (§6); `*` matched every
    work; `^` boosts made every score infinite or zero; no size limit.

  The user's calls, one at a time:
  - **Our own parser** on the server, `crates/query-parser`, and later a
    syntax highlighter in the client that mirrors it, designed later.
    The parser reads into a query tree with no engine in it, since the
    user wants the reader "adaptable to several query backends"; each
    engine's translation of the tree sits beside the engine
    (`search::translate`). The tree keeps each word as typed with its
    byte span: splitting a word is the engine's, as it split the
    documents, and the spans are what the highlighter needs. A vector
    engine reads the same tree: its text embedded, its filters and
    exclusions applied as filters.
  - **Plain text first**, every character text, a word what lies between
    whitespace, so the first pass is about the interface being right;
    PubMed's shape — phrases, exclusions, fields, `AND`/`OR`/`NOT`,
    parentheses — in a later pass (§11, item 36). Whatever is built
    should be easy to change, the first pass being temporary.
  - **Words split at punctuation stay Tantivy's**, the pieces of a word
    side by side as a phrase (`IL-6` is `il` then `6`), though the
    tokenizer is wrong for this dataset and is its own project (§11,
    item 11). Where Tantivy's parser read plain words at all, the
    translation finds the same documents at the same scores, down to two
    things it did quietly: a word typed twice counts once, and every
    word in every field is one flat list of alternatives. A property
    test holds the two to it.
  - **A size cap**, 1,024 words as typed — "i'd start much longer";
    words as the unit is tentative — and 64 KiB, checked first, so one
    enormous word cannot slip past. Over either is `INVALID_ARGUMENT`
    (400) naming the cap: `the query is 1025 words; the limit is 1024`.
    The user wants long queries dealt with now; over GET a URI is capped
    at 65,534 bytes by the `http` crate and answered `414` before any
    route sees it, so the byte cap is only reached over gRPC, and a
    query in any script but ASCII, three bytes of URL to each of its
    own, gets a third of it. Whether search becomes a POST, and keeps a
    little state per client, is open (§11, item 36).
  - **An empty query finds nothing**, on both transports. proto3 has no
    required field and cannot tell an empty string from an absent one;
    the JSON surface can, and keeps refusing a missing `query` as a URL
    built wrong, while `?query=` is now the query that finds nothing
    (`query::MaybeEmpty`, the one parameter declared so).
  - **Searches on threads of their own** — "oh was that not happening
    already?" It was not. At most `--search-threads` at once, half the
    cores unless told, each within `--search-deadline`, ten seconds
    unless told, from the request's arrival. No thread free within the
    limit is `RESOURCE_EXHAUSTED` (429), the machines' trouble and in the
    fault log; a search past it is `DEADLINE_EXCEEDED` (504). Tantivy
    cannot cancel a search, but a collector may walk a segment's matches
    itself, and `search::until` does, looking at the clock every 4,096
    documents, so a search past its time gives its thread back. Hits,
    the count and the facet counts are now one pass over the matches,
    where they were two.
  - **Failed queries out of the fault log, into a log of their own** —
    "isn't the fault log how we figured out that there was a problem in
    the first place?" One line per search on standard output, whatever
    became of it (`answered`, `too-long`, `busy`, `timed-out`,
    `failed`), with its bytes, words, hits and milliseconds, and the
    query last, whole up to the byte cap and "escaped/encoded so it's
    readable but not dangerous": every invisible or controlling
    character — escape sequences, right-to-left overrides, zero-width
    characters, line separators, combining marks — written as a
    `\u{…}` escape, a backslash as `\\`, everything visible in any
    script as typed. Plain text, not JSON, because a JSON reader undoes
    its escapes on the way out and these are undone by nothing.
  - **Tests generated, not listed**: the user's "*strong* preference for
    hypothesis-style property-based testing, for this and in general";
    `proptest` (and `fast-check` for the client when the highlighter
    comes). Properties: any text reads into exactly its own words; the
    word cap counts words as typed; plain words find what Tantivy's
    parser found, at its scores; any word of a title that the index
    split into pieces finds that title; the log's line is visible
    character by character and undoes to the whole query. The probe's
    inputs are a fixed list beside them, to grow.
- 2026-09-26, measurement before any search improvement — the user's
  "before getting lost down a rabbit hole of potential search
  improvements, i want a very reliable way to measure 'goodness' of the
  search i have", and "we have no users yet", so item 37's first step, a
  query log, has nothing to read. The measurement is its own plan,
  [`docs/evaluation-plan.md`](evaluation-plan.md): precision, recall and F
  with an interval on every number, suites built from the corpus leading
  and published ones checking them. Items 11 and 36 wait on it. Later the
  same day the user scrapped the suites built from the corpus; the
  published sets lead (evaluation plan §9).

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

   The user, 2026-09-25: what is thin is mature, feature-complete
   libraries like Lucene, which is why everyone uses Lucene; Tantivy
   claims to do what Lucene does and clearly does not, and by the
   search-benchmark-game (tantivy-search.github.io/bench) recent Lucene
   (10.3.0) now leads it on every measure — Lucene's only cost is Java.
   Vespa may have strengths. No change of engine now; a fork of Tantivy
   may be interesting later. The query parser's tree (§7) keeps the
   engine a translation away either way.
5. **The works index's block size and dictionary**, and whether a record
   is served whole or by field; SP3's measurement.
6. **The names transducer's update cost.** An FST is immutable; the
   registry tail is folded into a side map and merged at compaction
   (built 2026-09-16: `serve::fold::NamesTail`, plus the shared names
   the works delta makes and the public log's tail). How large the tail
   grows in a day at scale decides whether a day's compaction is enough
   for it.
7. **`expand`'s budget** and what it dedupes on; the product's, once
   candidate management is a plan. Two thirds of this were settled
   2026-09-19 (§7, §10): the **direction** is the caller's —
   `references`, `citers` or both — and the ranking is **Salton's
   cosine** over the neighbour's degree in the orientation opposite the
   walk, computed before the truncate, with the raw overlap and the
   degree returned beside it. It dedupes on the work: a seed that
   reaches a neighbour along both orientations is one seed, and two
   spellings of one seed are one seed, so the score cannot exceed one.

   A third was settled 2026-09-19 with `related` (§10, item 32): `expand`
   takes the work the question is *about*, and leaves it out of its own
   answer, because the truncate happens inside — a caller dropping it
   afterwards would be one short of the budget it asked for and could
   not take it out of the count of what the hop reached. It also returns
   that count now, so a page of neighbours says what it is a page of.

   What is still open is the budget itself — 20 by default and 1,000 at
   the cap, both guesses — and everything past one hop. A second hop
   wants a different shape: seeds weighted by the first hop's scores,
   and a stopping rule, which is candidate management and not this.
   Also open: whether the score should carry the year or the arc's
   asserters, which the measurement above cannot answer because the
   sample has no ground truth to score against — the retrieval
   benchmarks of the inbox are what would.
8. ~~**Incremental identity's edge cases.**~~ Spelled out 2026-09-16
   with fixtures (`serve`'s tests over a store built in memory): a
   record bridging two held works merges the younger into the older,
   its arcs and citers moving; a name moving between works is a split,
   which the batch performs since 2026-09-19 (merge plan §11, item 3),
   with a fixture for each half: the part that holds the records keeps
   the id, and a stub two arriving records divide between them is
   retired with no successor, both parts minted and every citer settled
   again by §6; a deleted record leaves its
   work's other records to re-promote, drops the work when it was the
   last and nothing cites it, and stops the batch when it was the last
   and something does. Two more the sample taught: a record arriving for
   some of a stub's names — the rest go to a stub of their own where a
   reference cites them alone, which the stub row's per-name citers
   say, and are dropped otherwise; and a text only the normalized layer
   has, which the batch reads from there since 2026-09-18 (item 25).
   Three refusals were added 2026-09-19 with the split rule of item 28,
   each one §6 reaching past the row it is settling: a reference whose
   citation text repeats a DOI on more than one side of the division;
   one that gave a single namespace on more than one side, which §6
   settles by the order the reference wrote its names in and no row
   keeps; and one whose every name here is dropped, whose arc then
   leaves for a work the row cannot name. They carried over unchanged
   when the same path took the split of merge plan §11, item 3, later
   the same day — one division, whether a part keeps the id or none
   does — and a fourth joined them, the split's own: a row that splits
   while keeping a record the batch does not carry, whose arcs,
   relations and unresolved rows only the normalized layer could divide
   between the parts — a guard, no row the sample splits holding a record
   at all. None fires on the sample.
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

    The tokenizer is wrong for this dataset and is a project of its own,
    top to bottom (the user, 2026-09-25). Its thesis, what the current
    mapping does to scientific text, the literature and the ideas so far
    are `docs/inbox/text-analysis.md`; its plan, draft 1 of 2026-09-29,
    is `docs/tokenizer-plan.md`. Until the project lands, a query
    word is split as the index split the documents — by hand now, in
    `search::translate`, rather than through Tantivy's parser, which is
    what that note's idea 6 needs before query-time analysis can differ
    from the index's. What the search box showed that day:
    - `of` matched every document it was in, so `trial of aspirin`
      counts nearly everything. Whether that is a stop list's to answer
      is the project's: the note argues against one, since IDF already
      discounts `of` in the ranking and Tantivy's English list turns
      `vitamin A` into `vitamin`; what it costs is the count and the
      facets, which is the next point's.
    - Whether every word must match or any may (any, today) is a
      product choice the user has not settled — any until users want
      every match enumerated, which is how researchers tend to work.
    - An `é` typed as `e` and a combining accent found nothing, the
      index holding the other spelling.
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

    Measured 2026-09-19 and the "tens of kilobytes" is wrong in the
    direction that matters: 38 KB is the *average* block raw, and a block
    of held works is 687 KB, because stubs are 97 % of the index and cost
    almost nothing while held rows are large and land together. A block
    is now decompressed into an `Arc` and shared rather than copied per
    lookup — no measurable change in wall time at this block size, and
    it removes 68 MB of copying from a hydrated page of a hundred — and
    the index counts the blocks and bytes it decompresses, which is what
    the measurements of item 31 are made with.
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
22. ~~**A record delivered twice, its references.**~~ Built 2026-09-17
    (§10, merge plan §8 step 0). The full merge took such a record's names
    from its first copy and its view from the copy with the latest
    revision stamp, but read the references of every copy in both passes,
    so its arcs were the union of the lists; the batch replaced a
    re-deposited record's list. A probe on the synthetic store — B citing
    E, then delivered again citing A alone — left the batch without the
    arc from B to E and the full merge with it. A second probe found worse
    in the full merge: a name only a later copy carried — a DOI PubMed
    added — was minted as a work of its own, and once a batch had
    attached that name to the work, the next full merge split the work's
    id in two. The sample had no such work. Now the newest copy speaks
    for all of the record in both merges, and the search documents take
    their text from it too.
23. ~~**A deleted record's citers.**~~ Built 2026-09-17 (§10). The batch
    removed a deleted record's arcs but left its work among the citers of
    the stub names it cited, where the full merge did not count it; and
    that did not stay evidence: in a probe where a later record took the
    stub over, the batch used the stale citer to mint a stub for the name
    the record left and drew an arc to it from the withdrawn record's
    work, where the full merge dropped the name. A re-deposit whose new
    list dropped a stub left the same stale citer. The full merge's first
    pass, meanwhile, still read a withdrawn record's references when it
    decided which names are stubs, which the test fixture did not. Now
    the batch takes a replaced or withdrawn record's citations back,
    restates a retired citing work's under its survivor, and a stub
    name's sources are its citers' in both merges; the first pass reads
    no reference of a copy that does not speak.
24. **Finding things by paths that were once true.** The user's
    condition on removing stale citers (§10, 2026-09-17): a citation from
    a withdrawn record, a name a later copy no longer carries, a name the
    registry dropped — each was out in the world, and someone may look
    for a work by it. None is authoritative, and none may build a work
    or an arc; but the layer should keep as many of these paths findable
    as it can, answering by them as what was once so rather than not at
    all. The registry already keeps every dropped name and when; stale
    citers and superseded copies keep nothing yet.

    One such path was answering nothing and now answers, 2026-09-19:
    **an id a split retired**. Rarer since the split rule (merge plan
    §11, item 3) gives the id to the part that holds the records, but
    not gone — the corpus's fourteen are retired for good, and the
    fallback keeps making more — and each has a public id that was out
    in the world. It resolves to `split`, which now carries the works
    its names went to: *this was one work; it is these now*. It builds
    no work and no arc, as the condition asks; it is a redirect to
    several. Before today the `split` outcome carried only the retired
    id back, which told a reader nothing it had not just sent.

    The parts come from the registry, which has always written them in
    the `split` line and never kept them past the fold; the names base
    gains a `split.u32` beside `retired.u32`, one `(id, part)` pair
    each, and the batch's tail folds the line as the base does — a
    `serve` version bump (5) and no merged-layer change, so `names
    build` and `epoch publish` are what it asks for and not a merge. A
    part merged since is followed to its survivor, and a part a later
    split retired answers with its own parts, so the answer is always
    live works.

    The file and its manifest count are **optional to a reader**: a base
    written before they existed opens and answers with no parts, which
    is what the layer answered anyway. The version tuple is then the one
    thing that refuses such a base, and it refuses it by saying which
    version it is — which is the point. A manifest is parsed before its
    version is read, so a required field there turns a version mismatch
    into a parse error, and a reader learns nothing from it (found
    2026-09-19, when the field was added without a default and the
    published epoch stopped opening with a serde error instead of the
    version message it should have given).
25. **Update files the batch cannot take.** PubMed's first five update
    files, tried as one batch each against the rest of the sample and
    compacted whenever one stopped (2026-09-17): `pubmed26n1335` stopped on
    a text only the normalized layer has — PubMed deleted a duplicate PMID
    whose record supplied a work's abstract, and OpenAlex's, not kept in
    the row, would have to take over; `1336` on a split; `1339` on a stub
    made of a record's leftover names cited together with another stub;
    `1337` and `1338` applied. With other update files in the base,
    `1338` stopped on a split too. Two of those three causes are settled
    since 2026-09-18: the text is read from the record's own normalized
    set, which the row already names, rather than wanting the record index
    of item 18 or every record's text in its row; and a split caused by a
    record dropping a name cannot happen now that the name stays (merge
    plan §8). `1335` then stopped on a third cause the same file holds —
    the work of the deleted PMID keeps another PubMed record — which the
    same reader answers: the twin is read from its own set and restates
    the pair. The leftover stub's merge still wants a rule of its own.

    Where the files stood at the end of 2026-09-18: `1335` **applies**,
    and `1336` stopped on a split — the one rule left (merge plan §11,
    item 3). It has one, and only one: a **stub** of three names,
    `{doi:10.1038/s41420-025-02366-3, pmid:41469416, pmc:PMC12848308}`,
    which two of the file's own records divide between them —
    `pmid:41390757`, which claims the PMC id, and `pmid:41469416`. The
    old work is a stub and both parts hold records, so it is the
    fallback: no part continues it, the id is retired with no successor
    and both parts are minted. Worth noting for what it says about the
    shapes: the fourteen splits the corpus holds are all a held work and
    a bare stub, which the rule's first clause settles by keeping the id,
    and the only splits the update files make are the fallback. The rule
    needed both halves to get past `1336`.

    Where the files stand at the end of 2026-09-19, with the split rule
    built: **all five apply**, in path order against the rest of the
    sample, `1339` included — `1336` performs two splits (both the
    fallback) and no other batch performs any. What the run does not yet
    do is agree entirely with a full merge: over the five batches the
    following full merge appends **one** registry line of 5,327,963,
    `merge #4967125 into #4967124` — two stubs `1339` minted separately
    for `doi:10.1007/s40519-019-00640-9` and `pmid:30715680`, names one
    work held together until `1339` re-delivered the PubMed record whose
    wrong link they were, and which the full merge makes one stub. That
    is the leftover stub's own grouping (merge plan §11, item 23) and
    not the split: the row that lost them keeps its id, so it is the
    division's first clause, whose code path is unchanged. It was not
    measurable before, `1336` having stopped the run three batches
    earlier. Not yet traced.
26. ~~**A reference whose names another work takes.**~~ Built 2026-09-18.
    A reference names a held work's DOI beside a PMID and a PMC id nothing
    holds, and lands on the held work; a batch then gives the PMID to a
    stub, and the full merge, ranking a PMID over a DOI, moves the
    reference there while the batch left the arc where it was. The merged
    layer now keeps what item 14 of the merge plan only counted: the
    `cited_as` set, one row per name a reference gave beside the work it
    landed on, with the citing work, the source, the landing and the
    namespace that landed it, and whether that reference was the pair's
    only one landing there. A fifth delta log carries a batch's own rows,
    whole per citing work and source; the `cited` artifact is a
    transducer over the set, read by name; and a batch, for every name it
    gives to a work or a stub, resolves those references again and moves
    the arc when the new name's namespace outranks the one that landed it.
    The same round found a second cause of the divergence and fixed it: a
    re-delivered record's arcs must follow a retirement like any other
    citer's, since the union no longer restates them. On the sample both
    harnesses now differ in nothing — the baseline's four rows and the
    nineteen with `pubmed26n1337` held out are gone.

27. ~~**Compaction churn from changed reference lists.**~~ Traced and
    cured 2026-09-18 (merge plan §10). Every one of the 45,789 registry
    lines a full merge wrote after `pubmed26n1337`'s batch was a stub the
    base had minted: 7,234 splits of the shape {DOI, PMID} into {DOI} and
    {PMID}, the 14,474 ids those splits minted, and 24,081 names dropped.
    The cause was PubMed replacing a publisher's reference list with PMC's
    when an article lands there, and our rule taking only the newest copy's
    list. With a record's citations the union over its standing copies, the
    same run appends nothing. What the batch must also do, and now does:
    read the references of a copy the row's newer copy supersedes
    (`Batch.cited_only`), or the full merge finds co-citations it does not.
28. ~~**Who cited a name.**~~ Built 2026-09-18, the larger half of the
    cited-as evidence the user chose: the `cited_as` set keeps a row for
    *every* name a reference gave beside the work it landed on, not only
    the names nothing holds, so the layer can answer "who cited this
    name" for any name. Two flags keep the uses apart: a row marked
    *beside* is one whose name named no work when the reference was read,
    and only such a row can take an arc with it (item 26); two marks say
    what the reference's own citation text backs — *by text*, that the
    text repeats a DOI of the work it landed on, and *in text*, that it
    repeats this row's name — so that a batch ranks the text before the
    order exactly as a full merge does (merge plan §6), a name the text
    repeats taking the arc even though the order puts a DOI last, and a
    landing the text backs moving for no order at all; a row marked
    *relation* comes from a typed relation, not a reference, and so says
    the name is still named while making neither arc nor citer — which is
    how a name the identity pass would leave as a stub of its own is
    recognised (the sample's case: an OpenAlex record whose location names
    a PubMed landing page by a PMID PubMed then withdrew). Two cases that stopped a batch are answered by
    it, both seen on the sample the same day: a name leaving a work while
    references still cite it — PubMed withdrew a duplicate PMID, the
    work kept its twin record, and the batch dropped the name a full
    merge then minted a stub for (three registry lines on the
    `pubmed26n1335` hold-out) — and a work losing its last record while
    something cites it, which now keeps its id and becomes the stub its
    citers name. A name with no citer is dropped as before.

    Extended 2026-09-19, and the reason is worth stating plainly. When a
    record arrives for some of a stub's names, the batch used to ask one
    question of each citer — *did it also give a name the record kept?* —
    and leave the arc where it was if so. That is not §6; it is a guess
    that happened to agree with §6 while the order alone decided, and it
    stopped agreeing the moment the citation text could beat the order
    (merge plan §6, version 6): the names left behind are by construction
    the ones nothing holds, and the text, measured, backs the DOI in 449
    of 492 checkable disagreements and the PMID in none — so the rule and
    the guess were aimed at each other. The batch now **runs the rule**:
    the row's names as that citer's reference divides them, each with the
    text mark the stub's row carries since merge version 7, settled the
    way `resolve_reference` settles one.

    That the row's own names are the whole contest is the claim the path
    rests on, and it has a proof. §6 picks a *name* — a DOI the text
    repeats, else the lowest namespace — and then follows it to a work.
    A split changes no name's namespace and no name's text mark; it
    changes only which work a name names. So the name §6 picked before
    the split is the name it picks after, and that name was this row's,
    since the reference landed here. A name of some other work lost
    before and cannot win now. Two shapes escape the proof, and both are
    refused rather than guessed (§5): a reference whose text repeats a
    DOI on *each* side of the split, where §6 gives the text up and falls
    back to the order over names the row never held — the case the merge
    plan's 2026-09-18 entry noted as open and left unruled — and one that
    gave the same namespace on each side, which §6 settles by the order
    the reference wrote its names in, and no row keeps that.

    One thing the path does not do, noted 2026-09-19 rather than
    measured: when a reference's arc moves — item 26's move, a leftover
    name's, or a split part's — and it lands on a *held* work, no
    cited-as row is restated for it. The rows are written whole per
    citing work and source from that record's own references, and the
    batch does not hold a record it is not carrying. So the arc is
    right and the evidence behind it goes stale: the old row still names
    the work the reference landed on before, and `citers_of` will not
    find it under the new one. It costs nothing today — the arcs and the
    rows are what a reader answers from — and it costs a later batch the
    evidence to divide that work again. `serve::compare` does not walk
    the cited-as set, so no hold-out would show it. The record index of
    item 18 is one answer; restating the rows from the store is another.

    What is still open: a stub minted for leftover names cited together
    with another stub. The split rule (merge plan §11, item 3) was ruled
    later the same day, and this path took it whole: dividing a row is
    one problem whether a part keeps the id or none does, and the proof
    above holds either way, since it never rested on one side being the
    row's continuation. What the split added is a destination that is a
    held work rather than a stub, and one refusal of its own (item 8).

29. **A path's bounds are the transport's guesses.** Six hops by default
    and ten at the cap; three paths and ten; 250,000 works expanded,
    which is not the caller's at all — a resource guard rather than a
    question. Nothing on the sample comes near any of them: the widest
    probe expanded 3,606 works at a depth of eleven, and the deepest
    real answer was one hop. They are placeholders for numbers a corpus
    and some traffic will set, and the shape they are placeholders for is
    right: a cap refused, never clamped, and a search that hits the guard
    saying what smaller bound it did prove instead of pretending to have
    finished.

30. **A path over a sample that has almost none.** Built 2026-09-19 and
    correct, and on this corpus it answers "no path" to nearly every
    question that is not a work citing another directly — 4 of 200,000
    random held pairs have one within six hops (graph plan §7). The
    ratio that moves is the 1.45 % of arcs landing on a work we hold,
    and full ingestion is what moves it. Two things to do when it does:
    re-measure the table of the graph plan's §7 on the corpus, and
    revisit the fixed half-and-half search, which is what makes the
    bound provable but expands a hub in full on whichever side holds one
    (graph plan §11, item 13).

31. **A hydrated page decompresses 75 times what it keeps.** Built and
    measured 2026-09-19 (§10). `Serving::rows(ids)` answers a whole
    page's rows in one walk of the works index, deduplicated, sorted, the
    works delta's overrides applied exactly as `work(id)` applies them,
    and the three hydrating converters call it once a page instead of
    once a row. What that saves is real but small — nothing at all on a
    page of arcs, which arrive in id order and so were already running
    through the index's one-block cache; 1.2× on an expansion's
    neighbours and 1.6× on a hundred search hits, both of which arrive
    ranked and so scattered.

    The number to act on is the one it made visible. Hydrating a hundred
    held rows decompresses **25.6 MiB to keep 349 KB**, and end to end a
    hydrated page of 100 citers is **20 ms** against **0.5 ms** for the
    same page unhydrated. Three levers, in the order they are worth
    trying, none taken yet because each costs an artifact rebuild and an
    epoch the user publishes: a **smaller block** for the same dictionary
    (item 5's open question, now with a number behind it); **stopping the
    decompression early**, since a block's slot offsets are at its head
    and the rows wanted are usually in its first half — zstd cannot seek
    but it can stop; and a **summary sidecar**, the seven fields of §7's
    hydrated row in their own array, which is what every list actually
    reads and is perhaps a fiftieth of a row. The third is the real
    answer if lists stay the product's spine.

32. **`related`'s seeding, and what it drops.** Built 2026-09-19 (§7,
    §10). The bound is on the **arcs** the walk will read — 250,000,
    about a tenth of a second at the 2,444 arcs a millisecond measured on
    epoch 7 — and not on a count of seeds, which bounds nothing when one
    seed's list may be any length. Over the bound, the seeds kept are
    those of smallest degree in the orientation about to be walked; one
    seed is always walked however long its list; and `seeds_used` against
    `seeds_total` on every answer says whether the bound bit.

    Nothing in this corpus reaches it: the most-cited work we hold walks
    94,098 arcs for its co-citation and 3,147 for its coupling, so every
    answer today is the whole question. What is open is the number
    itself, for the corpus where it will bite, and whether smallest-first
    stays the drop rule there — it is the right one at equal cost by a
    wide margin on this corpus (the measurements are in §10), but that
    was measured against an unbounded answer this corpus is small enough
    to compute, which is exactly what a corpus that needs the bound will
    not be. The retrieval benchmarks of the inbox are what would settle
    it.

33. **`resolve` cannot be attributed, and why.** Every other answer
    names the sources it drew on: a work names who holds records of it, a
    page of arcs names who asserted them, a path names the asserters
    along it. A resolve names none — `Sources::NONE` — and that is the
    honest stamp and not an oversight (2026-09-19). The names artifact
    is text → `WorkId` and nothing besides (§3), and the
    registry tail it folds past the epoch is no better: a `mint` or
    `name` line carries an id, a name and a timestamp, and no source.
    The merged layer does know — `NameRow::by` is exactly which sources
    asserted a name for a work — and `names::build` reads that column
    and drops it.

    The work's own `held_by` is not a stand-in, and a stub shows why: on
    epoch 7, `doi:10.1371/journal.pmed.1000097` resolves to a stub whose
    `held_by` is empty and whose every name is marked asserted by PubMed
    and Crossref. Who holds records of a work and who said a name is that
    work are two different claims, and on 97 % of the index the first is
    nobody.

    What it would take, in the two halves the artifact has. **The base**
    would carry a `Sources` per key, which is a `u8` over the three
    sources today; the transducer's value is already a `u64` with the
    `WorkId` in its low 32 bits and the shared flag at bit 63, so the
    three bits ride in the value's spare middle rather than in a file of
    their own, and the shared side list gains a byte per member. On the
    epoch of 2026-09-19 that is 11,534,123 keys over a 145 MB
    transducer; the values there are already one per key and so share
    nothing, which is the case where widening them is cheapest, but the
    number is a build away and not a calculation. Of those keys 5,238,452
    are public ids, which no source asserted and which would carry the
    empty set, truthfully. **The delta** needs no new format: the fold
    already reads whole rows out of the works delta and already sees
    `names[].by` on them (`NamesTail::apply_row`), so the tail can key a
    name to its asserters without the registry changing — which matters,
    because the registry is the store of record and adding a field to
    the 5,350,333 lines it already holds is a different and much larger
    decision. What wants checking first is whether every name event past
    an epoch does arrive with a works-delta row for its work; where one
    does not, the tail would have to answer with no sources, and an
    answer whose attribution depends on how recently the name was
    asserted is worse than one that never claims any.

    Then a names base rebuilt and an epoch published, both the user's.
    Until then the stamp stays empty, and the reason is written where
    the empty stamp is set and not only here.

34. **Search hits hydrate, and a summary carries a work's standing.**
    Built 2026-09-24. `SearchRequest.hydrate` and `hydrate=` on
    `/v1/search`, off unless asked like every other list's, put the one
    `Summary` on each `Hit` through the same `Serving::rows` walk (item
    31), so a result renders as a row of any list does. The summary gains
    `retracted_by`, `corrected_by` and `concern_by`, read where `Work`
    reads them, so a row can warn of a retraction before anyone opens the
    work (the user chose the flags, 2026-09-24). A work's reference and
    citer counts were left off it on purpose: `out_degree` is two offsets
    on the base, but that is not yet shown to hold on the full collection
    (the user, 2026-09-24).

    Measured on epoch 8 of the local store (model v2), best of five: a
    fifty-hit search is about 1 ms unhydrated and 14–23 ms hydrated, and
    the body grows from about 10 KB to about 50 KB, most of it names.
    Hits arrive ranked and so scattered, which is item 31's worst case,
    and a summary sidecar would answer this list as it would the others.
    An older server refuses `hydrate` on search as an unknown parameter,
    so the server that reads it is deployed before a client that sends
    it.

35. **The batch's evidence, and the list of events.** Found 2026-09-25,
    once merge plan §11 item 24 let the update-files harness past the
    registry check to the comparison (before, it stopped one `drop` apart
    and never compared), and built the same day on the user's choices:
    stored decisions kept up to date by a named list of events, not
    facts worked out on demand; a reference two copies of a record carry
    is one reference; a record's earlier copies read back from the
    normalized layer. On the sample store at model 9, the comparison now
    also checks every cited-as row and every unresolved row, which it
    never did — the reason stale evidence surfaced batches after it went
    stale.

    Measured (M), `1335`–`1339` held out: the committed code (4eef069)
    gave 38 differences before the comparison could see evidence; with
    it, 5,018; now 1. The `1334` hold-out: 85, now 0.

    What each part fixed:

    - **A reference counted once** (merge version 15): the full merge
      counted a reference once a copy, so an arc two copies repeat was
      no one reference's (`sole` false) and its unresolved row was
      written twice. Now by fingerprint — the names and the citation,
      whole, the citation read as text so the same reference in two sets
      prints alike.
    - **A record's references are every copy's**: a batch rewrote a
      re-delivered record's cited-as and unresolved rows from its newest
      copy alone. A record's view names the sets of the copies its
      speaking copy supersedes (merge version 15), and the batch reads
      them back, each set once (`earlier_copies`), for its own records and
      for the older copies it carries for their citations — whose pair it
      restates with the row's relations too.
    - **The list of events** (`Engine::moved_names`): a citing record the
      batch does not carry keeps rows drawn from where its references'
      names pointed. Every event that makes them stale is a name's target
      changing — a stub becoming a held work (the case of #3202, #46038,
      #1313865) or a held work a stub, a name given to a work or a stub,
      a name dropped, a work merged or split, a name becoming shared or
      ceasing to be. The batch asks every name it touched where it
      pointed before and where after; for each that changed, the records
      that gave it — by their cited-as rows, or as citers of the stub
      that held it — are read back from the normalized layer and their
      cited-as rows, unresolved rows and relations restated through the
      final table. A relation keeps its cited-as row on a stub too (merge
      version 15), since it leaves no citer there.
    - **A pair is every record of its source**: a work holding two
      records of one source (twins) had its lists rewritten from the
      re-delivered one alone; the twin is read back too, and each pair's
      lists are gathered from every path that restates it and written
      once.
    - **Smaller**: a relation's row with no reference beside it is `sole`,
      as the full merge counts it; a stub landing counts toward `sole`.

    **One citer's mark on a divided stub**, built the same day (the user:
    "go ahead with the divided-stub case"). Stub #1293222: a record's two
    copies cite it differently — one by its DOI beside a PMID that no
    record held yet, one by another PMID alone. Both references land on
    the stub, whose row keeps one citer of the record per name, and when a
    later batch makes the first PMID a held work the stub divides: the
    batch could not tell which reference gave which name, settled the two
    as one, and marked the second PMID's citer as naming a held work
    beside it, where the full merge does not. Now a stub that divides —
    a name left over, or names going to different parts — has its citing
    records' references read back (`Engine::stub_citations`: the batch's
    own copy where it carries the record, else every copy the normalized
    layer has), and each reference is settled by §6 on its own, with its
    own marks; the landing key gains which reference it was. A stub taken
    over whole by one part reads nothing back, since every reference
    lands there whatever it gave. The test
    `a_records_copies_citing_one_stub_differently_mark_each_name_by_its_own_reference`
    reproduces it and passes.

    **Both harnesses now pass** (M, the sample store at model 9): the
    update files with 0 differences, cited-as and unresolved rows
    included, where the committed code of the morning gave 38 before the
    comparison could see the evidence and 5,018 once it could; the `1334`
    hold-out with 0.

    **What it costs, and what is next.** The batches on the update files
    took 5.5, 19.8, 11.5, 13.9 and 23.2 s before this item and 9.2, 28.4,
    15.0, 21.1 and 38.7 s once it was built (M, the laptop). Every read
    back scans a whole normalized set to find its records.

    Step 1, the same day (the user: "go ahead with step 1"): the division
    reads set by set too. Which stub divides is known only as its
    component resolves, so the citing records of every stub the
    components hold are read ahead, one scan a set, into a cache the
    division and the list of events read from (`Engine::read_ahead`,
    `Engine::read_copies`); a stub taken over whole reads for nothing,
    but the cost is the sets, not the records. The batch's report counts
    the sets it scanned. Now 8.0, 27.1, 15.0, 20.9 and 30.4 s, scanning
    7, 11, 13, 15 and 18 sets (M): the sample has seven, and a batch reads
    them in up to three passes — the earlier copies and the read-ahead
    before the components resolve, the list of events after, since it
    needs the final table. One scan is about half a second (M, as a
    dump).

    The last two steps, the same day (the user: "let's do the remaining
    two steps"). The two passes before the rules run are one: a stub a
    record arriving can divide is one its names reach, so its citers are
    known right after the copy rule, and they are read with the earlier
    copies into one cache (`ReadBack`); the engine's read-ahead reads only
    what that could not foresee. Then 7.3, 25.1, 13.5, 18.3 and 28.9 s,
    scanning 5, 8, 9, 10 and 12 sets (M). And a normalized set can be
    sought (ingestion plan §10, 2026-09-25): its works and references
    files are written in frames of 256 KiB with an index of each frame
    and each work's native name, so a reader finds a record by a seek and
    one frame, and scans only a set normalized before. On the sample,
    normalized again, both harnesses still pass with 0 differences; the
    update-file batches are within noise of before (7.4, 27.0, 12.6, 20.6
    and 32.7 s) and the `1334` hold-out's from 6.2 to 3.9 s (M). The
    sample's batches want thousands of records from each of its seven
    sets, where a scan is nearly as cheap as seeking; a batch on the
    corpus wants a few from each of hundreds, which is what the index is
    for (E). The corpus is normalized again for model 9 anyway, and its
    sets will carry the index then.

36. **The query language's second pass.** The first (2026-09-25, §7)
    is plain text, so that the interface is right first; the second is
    PubMed's shape — quoted phrases, `-` to exclude, fields under names
    of our own, `AND`/`OR`/`NOT` and parentheses, nesting capped — read
    in one pass as the first is, so that nothing typed can cost more to
    read than its length. Waiting on it:
    - *What an exclusion alone means.* `-mice` with nothing to find is
      "everything but mice" in the user's reading; over the whole corpus
      that is every work at one score, so it wants a filter or a sort to
      go with it.
    - *Identifiers typed into the box* — a DOI, a PMID, an `ol:` id —
      found as the work they name before any text search.
    - *Year, genre and the other facets as filters*, partly in the text
      and partly beside it; the user partly agrees.
    - *A word typed twice counts once*, as Tantivy's parser counted it
      and the first pass keeps; whether it should is the product's.
    - *Long queries.* Words as the cap's unit is tentative. A GET's URI
      stops at 65,534 bytes, a third of that in any script but ASCII,
      so search may become a POST, and may keep a little state per
      client "to keep track of their experience"; both undecided.
    - *The highlighter* in the client, mirroring the parser: the tree's
      spans are bytes of UTF-8 and a browser counts UTF-16, so the
      design says which side converts, and whether the client runs this
      parser compiled for the browser or a twin held to it by the same
      properties (`fast-check` beside `proptest`).
37. **Search after its first pass.** Superseded 2026-09-26 (§10) by
    measurement first, [`docs/evaluation-plan.md`](evaluation-plan.md);
    the steps below stand as what the measurement is for. Since written,
    epoch 6 of the corpus has been published from a build that has the
    first pass (`docs/inbox/serving-memory.md`). As written 2026-09-25:
    the first pass (2026-09-25, §7) has not met the corpus: the local store's newest epoch (8) is built
    from merged v2.9, far older than the merged layer this build reads,
    so the server waits rather than opens it, and the first pass was
    tested against a fake and small indexes in memory only. In the
    order proposed on 2026-09-25, and not yet agreed as a plan:
    1. *Deploy it and read the query log.* What people type, which
       queries find nothing — where the tokenizer fails them — and which
       are slow, which is what a search costs at forty-two million
       works. Both items below would be steered by it. For the
       deployment: `--search-threads` defaults to half the cores the
       process is given, which is the pod's CPU limit, so the manifest
       may want it said outright; and the query log is on standard
       output beside the fault log on standard error, one stream in the
       pod's log, holding what people typed — fine while the only user
       is the user, and worth a second look before anyone else is one.
    2. *The tokenizer* (item 11, `docs/inbox/text-analysis.md`): a
       change of mapping, so a new index version and a rebuild on the
       box. The query parser needs nothing from it: the engine splits.
    3. *The language's second pass and the highlighter* (item 36),
       designed together, on top of whatever the tokenizer makes of a
       word.

    Beside them, loose:
    - *Counting every match.* A search's total and its facet counts
      visit every document that matches, so no top-k shortcut prunes
      anything, and under "any word matches" a broad query is most of
      the corpus. The time limit bounds it; whether exact counts are
      what the product wants is the ranking's question, with item 11's
      any-or-every.
    - *The other calls still run on the request threads.* `expand`,
      `related` and `path` are bounded by their caps (§7) and were left
      where they are; if one is ever slow enough to matter, it takes the
      search's way.
    - *The browser client* retries an `internal` once and a refusal
      never, so a `429` (busy) or a `504` (past the limit) is shown as
      its code and message and not retried; whether "busy" deserves a
      retry after a moment, and plainer words for either, is the
      client's.
    - *The engine* stays Tantivy for now (item 4).

38. **A warm cache for search — recorded, not to be built yet** (the
    user, 2026-09-29, who had something shaped like the first idea in
    mind). What makes search cold (M, 2026-09-29, epoch 7):
    - **Not a restart.** The server maps its index files, so the pages it
      has read are the node's — the page cache over ZFS's ARC — and not
      the process's. A pod started on a new image answered "crispr
      screen" (432,412 hits) in 198 ms ninety seconds after it started,
      holding 38 MiB.
    - **A new epoch.** Its files have never been read. The first two
      searches of epoch 7 took 10 s (timed out, "Test") and 3.0 s
      (`"must match"`, 1.55 million hits).
    - **The box's other work.** Those two ran while `epoch check` read
      every merged set. A merge, a publish or a survey streams hundreds
      of gigabytes through the same caches and pushes the index out.
    - **A reboot**, which empties the ARC.

    The ideas, in the order proposed:
    1. *Warm a new epoch before it is served.* The server opens an epoch
       and swaps it in, the old one answering until then (§5), and a pod
       reports ready only once it has an epoch, the rollout keeping the
       old pod until then (`maxUnavailable: 0`). Between opening and
       swapping, or before reporting ready, read the lexical index's term
       dictionaries and fast fields and replay the latest queries of the
       query log.
    2. *Keep the batch Jobs from evicting it.* ZFS `primarycache=metadata`
       on the bulk dataset, so one-pass reads of the normalized and
       merged sets do not fill the ARC, the serving dataset keeping
       `all`. A homelab storage change; whether ZFS LocalPV sets it is to
       check.
    3. *A cache that survives a reboot.* An NVMe partition as the pool's
       L2ARC, which OpenZFS keeps across reboots, cold reads coming from
       NVMe rather than the SATA SSDs. A pool change.
    4. *See it.* Every line of the query log carries its time in
       milliseconds; p50 and p95 over time would show when and why
       search goes cold.

## 12. Sources consulted

The serving-shape and feeds conversations of 2026-09-15 and the notes
kept of them; the graph plan's §5 and §11; crates.io on 2026-09-15 for
`tonic` 0.14, `hyper` 1 and `axum` 0.8; the six publisher feeds probed
read-only the same day; the data terms of Crossref, NLM, OpenAlex,
Semantic Scholar and arXiv as noted 2026-09-15.
