# Candidate management

**Status:** note, 2026-09-15 — the product's side of the citation graph,
moved out of the architecture summary of 2026-09-14 when that was folded
into the [graph plan](../graph-plan.md), as the
[notes](citation-graph-architecture-notes.md) §5 asked. Not a plan;
nothing here is scheduled. What the graph owes it is the graph plan's §7.

## 1. The funnel

The question is not how to traverse a billion-node graph but how to keep
recall high without overwhelming what comes after:

```text
seed works
  → graph expansion
  → canonicalization and deduplication
  → hard filters: date, language, kind, retraction, access (the merge plan's promoted fields, §5)
  → cheap scoring: metadata, lexical search, embeddings, topology
  → a bounded frontier, perhaps 100–1,000 candidates
  → full-text acquisition
  → structured evidence extraction
  → expensive reranking and reasoning, perhaps 10–100 works
```

The search sinks (ingestion plan §9) are the cheap-scoring stage; the
graph is the expansion stage and nothing else.

## 2. Best-first, not breadth-first

```text
while frontier has budget:
    candidate = frontier.pop_best()
    evaluate(candidate)
    expand only promising nodes
```

The frontier score combines: distance from the seed set and citation
proximity; the number of already-promising works that cite or are cited
by the candidate; bibliographic coupling and co-citation; title and
abstract lexical relevance; embedding similarity; recency, kind and
citation-count signals; novelty — whether expanding the candidate reaches
new territory.

## 3. High-degree works

A methods paper with hundreds of thousands of citers is never expanded
whole; the operation is "the most promising bounded sample given the
current research context". Hence the graph's bounded, lazy `citers(work,
limit)` with a documented list order (graph plan §4 and §7), and, under
a graph-derived ordering, a side table of each such work's top citers by
year (graph plan §11, item 4).

## 4. Where the cost is

Acquiring clean full text and turning it into trustworthy structured
evidence, then reranking and reasoning — not the graph, which is the
cheap discovery and candidate-generation layer. The metadata and search
index and the candidate-ranking layer matter at least as much as the
graph; how they meet is in the [ambitions](ambitions-beyond-citations.md)
§5.
