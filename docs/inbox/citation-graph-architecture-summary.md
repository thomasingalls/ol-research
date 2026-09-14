# Citation-Graph Architecture: Current Summary

## Problem and scale

Assume approximately **1 billion scholarly works**, with an average of **20 references per work**:

```text
N ≈ 1B nodes
M ≈ 20B directed citation edges
```

Use the convention `A → B` = “work A cites work B.” The common query needs both:

- outgoing neighbors: references cited by a work;
- incoming neighbors: works that cite it.

The graph is read-heavy, needs random one-hop access, and should live primarily on SSD rather than requiring the whole structure in RAM. A practical caveat is that the graph may need nodes for referenced works that are absent from the main document corpus; the 1B-node assumption treats those as included.

## IDs and baseline storage

The graph should use dense internal node IDs, conceptually `WorkId(u32)`. One billion IDs fit comfortably in `u32`; DOI, OpenAlex, and other external identifiers belong in a separate mapping layer.

Plain CSR remains the right baseline mental model: an offsets array locates each adjacency list, and a contiguous target array stores neighbor IDs. With `N = 1B` and `M = 20B`:

| Component | Approximate size |
|---|---:|
| Neighbor IDs: `20B × u32` | 80 GB |
| Offsets: `(1B + 1) × u64` | 8 GB |
| One orientation | **88 GB / 82 GiB** |
| Forward + transpose | **176 GB / 164 GiB** |

The offsets need 64-bit values because the edge array contains 20B entries, even though node IDs fit in 32 bits. Memory mapping addresses the RAM constraint, but not the SSD footprint.

## Why the combined direction-bit layout is not a solution

For an edge `A → B`, fast lookup from `A` needs an occurrence containing `B`, while fast reverse lookup from `B` needs an occurrence containing `A`. Therefore, both directions require two stored incidences somewhere.

A combined structure with direction flags would contain approximately:

```text
40B × 4-byte neighbor IDs = 160 GB
1B × 8-byte offsets       =   8 GB
40B direction bits        =   5 GB
---------------------------------
                              173 GB
```

That is only about 1.7% below two ordinary CSRs, before implementation overhead. Byte-sized flags would be worse. The API can still expose one convenient `neighbors(work)` operation returning references and citers—preferably as lazy/streaming iterators rather than eagerly allocated vectors—but the storage still needs a reverse index.

## Assessment of storage options

- `petgraph::csr`: useful general-purpose CSR, but not a good fit for this scale and SSD-first workload.
- `graph_builder` / `graph`: credible high-performance conventional CSR and interesting when the graph can reside in memory, but not an extreme compression solution.
- `graph_csr`: its mmap concept is directionally correct, but it is small, effectively dormant, and no longer the preferred production choice. Earlier assumed ingestion APIs should not be treated as reliable.
- Custom mmap CSR: simple, predictable, and an excellent baseline.
- **WebGraph/BVGraph: current strongest production candidate.**

WebGraph-style compression exploits ID gaps, intervals, similarity between nearby adjacency lists, references to previously encoded lists, and variable-length codes. Its random-access index can use Elias–Fano encoding, and it supports mmap-backed access and transpose graphs. The natural logical layout is still two orientations:

```text
forward.graph + forward index       # work → references
transpose.graph + transpose index   # work → citers
```

The important benefit is that each orientation is compressed; the total should be far below 176 GB if the citation graph compresses well. Compression quality is ordering-dependent. An ordering that places related papers near one another can improve both compression and SSD/page-cache locality during neighborhood traversal.

Do not assume web-graph headline compression ratios will transfer to scholarly citations. Citation graphs have useful structure—subfield overlap, temporal constraints, clustered references, and skewed in-degree—but the actual rate must be measured. A broad exploratory range such as **5–15 bits per edge** is more defensible than budgeting on 1.5–3 bits per edge before testing.

Compressed CSR is a valuable middle control: packed successor IDs and Elias–Fano-style degree offsets may reduce overhead, but it is unlikely to solve the storage problem alone. It helps isolate the value of WebGraph’s more sophisticated list similarity and reference compression.

## Latency and traversal model

For one work, retrieving immediate references and immediate citers is still essentially one neighborhood query in each orientation. A deliberately conservative early estimate is:

- **~0.2–2 ms** for a cold-ish local-NVMe query;
- tens of microseconds when relevant pages are warm, roughly **under 100 µs** as a warm planning figure;
- use **~1 ms per node/query** as a deliberately bad initial capacity-planning number.

This is not the same as recursive traversal. With roughly 20 references and 20 citers, one hop returns about 40 neighbors. Expanding those neighbors creates a rapidly growing candidate set—nominally about 1,600 second-hop incidences and 64,000 third-hop incidences before overlap and deduplication. Real citation neighborhoods have substantial overlap, and concurrent, deduplicated, locality-aware reads should perform much better than serial multiplication of the one-hop estimate.

The practical distinction is:

```text
one-hop lookup       random-access latency problem; likely cheap
recursive exploration batch throughput, deduplication, locality, and ranking problem
```

## The real bottleneck: bounded candidate management

The architecture has shifted from “how do we traverse a billion-node graph?” to “how do we maintain high recall without overwhelming downstream processing?” A useful funnel is:

```text
seed works
  → graph expansion
  → canonicalization and deduplication
  → hard filters (date, language, type, retractions, access)
  → cheap scoring (metadata, lexical search, embeddings, topology)
  → bounded frontier (perhaps 100–1,000 candidates)
  → full-text acquisition
  → structured evidence extraction
  → expensive reranking/reasoning (perhaps 10–100 works)
```

Best-first or frontier-based search is generally a better product abstraction than BFS:

```text
while frontier has budget:
    candidate = frontier.pop_best()
    evaluate(candidate)
    expand only promising nodes
```

The frontier score can combine:

- distance from the seed set and citation proximity;
- number of already-promising works that cite or are cited by the candidate;
- bibliographic coupling and co-citation;
- title/abstract BM25 or similar lexical relevance;
- embedding similarity;
- recency, document type, and citation-count signals;
- novelty: whether expanding the candidate reaches new territory.

High-degree nodes need explicit handling. A methods paper with hundreds of thousands of citers should not be fully expanded; the useful operation is “return the most promising bounded sample given the current research context.” This makes the metadata/search index and candidate-ranking layer at least as important as the graph itself.

The likely downstream bottlenecks are acquiring clean full text and converting it into trustworthy structured evidence, followed by reranking and reasoning. The graph is primarily a cheap discovery and candidate-generation layer.

## Mutable updates

Because compressed graph formats are best treated as immutable and citation data arrives append-only, a likely production design is:

```text
large immutable forward + transpose base snapshot
small mutable forward + transpose delta layer
periodic merge/compaction into a new compressed snapshot
```

Queries read both base and delta, merge and deduplicate results, and tolerate a bounded amount of delta overhead. Compaction can rebuild ordering and compression periodically rather than rewriting the full graph for every new citation batch.

## Current recommendation

Use dense `u32` internal IDs with a separate external-ID mapping. Treat two logical orientations as unavoidable, but store them in compressed, mmap-friendly form. Keep custom/raw CSR as a correctness and performance baseline; use compressed CSR as an intermediate control; and make WebGraph/BVGraph the leading production candidate, subject to real measurements. Build the application around bounded best-first candidate generation, not unrestricted recursive BFS.

## Next experiment

Take a representative real citation subgraph—not a synthetic toy—and build both orientations using the same ID mappings and several ordering strategies. Benchmark:

1. raw mmap CSR;
2. compressed CSR;
3. WebGraph/BVGraph.

Record total bytes including indexes and metadata, build time, and compaction cost. Measure warm and cold-ish one-hop p50/p95 latency for both directions, batched multi-node throughput, recursive expansion after deduplication, locality sensitivity, and behavior on extreme high-degree nodes. Also measure the mutable-delta query and merge overhead. The result should determine the storage format, ordering strategy, and whether the graph layer is worth optimizing before the candidate-management and full-text pipeline.
