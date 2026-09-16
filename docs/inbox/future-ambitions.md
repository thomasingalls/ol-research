# Future ambitions

**Status:** speculative notes, 2026-09-16 — one conversation, recorded at
the user's request. Nothing here is a plan, a decision or scheduled; where
a note suggests an order or a rule, it is a suggestion the user has not
taken up. Companion to [ambitions beyond
citations](ambitions-beyond-citations.md) and [candidate
management](candidate-management.md). Scale figures talked through in the
conversation are left out on purpose: napkin math, to be replaced by
measurements when the corpus runs.

## 1. The goal, as stated

A set of utilities that make an authoritative, highly automated,
low-maintenance corpus of the academic literature — fast, efficient and
cheap to serve — so that it can be sliced and diced this way and that, at
speed.

## 2. A review against the goal

Taken on 2026-09-16, after the serving plan's SP5 landed. A view, not a
decision.

- **Authoritative: holding.** Strict parsing and no silent repair;
  disagreements kept with who asserted each side; every artifact
  rebuildable byte for byte from raw, the registry and the key.
- **Cheap to serve: holding by design.** One box, no cluster, no hosted
  search; memory-mapped artifacts, disk-heavy and memory-modest.
- **Fast: on the sample.** A search under a millisecond, an expansion in
  a tenth of one, an epoch published in 13 s (serving plan SP2, SP3).
- **Unproven at size.** The sample is an island — 82,744 held works among
  4,039,076, a third of the held works with no arc either way (graph plan
  §11, item 11) — so neither the cost of serving nor the graph's shape
  is measured. What breaks first is already listed as work before the
  corpus: the registry as JSON and its fold (merge plan §11, items 10 and
  15; serving plan §11, item 12), pass 2's memory (merge plan §11, item
  16), a record index into the normalized layer (merge plan §11, item
  22).
- **Highly automated: the least developed.** Nothing runs fetch,
  normalize, merge and publish on a clock; the full downloads are by
  hand; the snapshot diff is not designed (ingestion plan §8, item 7);
  origins are SP6.
- **Low-maintenance: to be measured.** Under fail fast, upkeep is how
  often a new wart stops the nightly run. Every file so far brought some;
  the first corpus run says whether they taper.
- **Slicing: fields of the work only.** Year, kind, language and the
  like. By person, institution, funder or venue waits on a registry per
  kind (ambitions §4).
- **Suggested order.** The corpus on the box with a nightly full merge
  and epoch next; SP6, feeds and new sources after it, because the
  incremental path rests on the compaction under it, which at corpus
  scale takes hours (serving plan §11, item 16).

## 3. Other corpora for the same treatment

Books and patents set aside. What makes a corpus a fit: the same thing
under several ids in several sources; sources that disagree, with nobody
keeping the disagreement; free bulk data; an expensive closed incumbent;
records that link to each other. From memory and unverified here, in
rough order of fit:

1. **Clinical trials.** One trial registered in several registries
   (ClinicalTrials.gov, the EU's, national ones) under different ids;
   WHO's combined search platform links the duplicates only partly; the
   links from trials to their publications and results are unreliable.
   PubMed already carries trial ids, so this joins what is held.
2. **Grants and their outputs.** Public funder data (NIH RePORTER, NSF,
   UKRI, CORDIS) and no registry for award ids (ambitions §3); the good
   grant-to-output linkage is closed (Dimensions).
3. **Case law and legislation.** The nearest structural twin outside
   academia: opinions cite opinions; one case is cited under several
   reporter citations — one work, many names; courts stand where venues
   do; Westlaw and Lexis are the closed incumbents; free bulk data exists
   (CourtListener, Harvard's Caselaw Access Project).
4. **Vulnerabilities and packages.** One vulnerability under a CVE id, a
   GitHub advisory, an OSV id and distribution ids, the sources
   disagreeing on the affected versions, NVD's enrichment behind since
   2024; package dependencies are the graph. Freshness in minutes matters
   here more than it does for papers.
5. **Datasets and research software.** DataCite (on the ingestion plan's
   long tail), Zenodo, Software Heritage; citations to data and code are
   messier than citations to papers.
6. **Company and legal-entity registries.** LEIs, national registers,
   securities filings; ownership is the graph.
7. **Drug approvals and safety.** Regulators' labels, approvals and
   adverse-event reports, drug identity split across coding systems;
   joins trials, which join papers.
8. **Policy and grey literature.** Government and think-tank reports
   citing research (Overton the closed incumbent); mostly PDFs, so §4's
   unstructured case.

## 4. Internal datasets

The user's expectation: the less structured the data, the less the
approach applies; academic metadata is structured or semi-structured at
worst, the full text of papers another story. The approach has two
halves, and less structure affects them differently.

- **The parsing half weakens.** A strict model of each record needs a
  schema. Databases, event logs and exports have one; documents, slides
  and chat do not, and the step that makes records from them becomes an
  extraction model — probabilistic, versioned, sometimes wrong.
- **The record-keeping half matters more.** Raw kept as the record,
  everything else rebuildable, every fact with its asserter: that is what
  makes model output usable. The frame is ambitions §6's — a model's
  output is a claim from a named process and version, scores kept raw,
  mechanics failing fast — and identity, disagreement, graph and facets
  downstream of it are unchanged.
- **Anchors.** The literature has global ids (DOI, PMID, ORCID) and
  aggregators that already publish links between them, which is why
  authoritative is reachable here. Inside a company, structured records —
  customers, products, suppliers across several systems — are the merge's
  own identity problem, sold as master data management, but the
  assertions usually have to be generated from names, emails and
  addresses, where never joining on text alone is hard to keep. Where
  there is no anchor at all — concepts in documents, one project
  discussed in five places — identity is clustering with no ground truth:
  reproducible and honest about its sources, not authoritative in this
  repository's sense.
- **Two problems public corpora do not have.** Permissions from the first
  day, reaching into the indexes and not only the front door (serving
  plan §7 defers authentication). Deletion obligations against raw kept
  forever — where rebuilding everything helps: remove from raw, rebuild,
  gone downstream.
- **Where the cost goes.** With structured data the savings are in
  serving; with unstructured data extraction dominates, as candidate
  management §4 says of full text.

The full text of papers sits between the two: PMC's XML carries sections
and parsed references, PDFs need a parser first (ambitions §6).

## 5. Retrieval, models and agents

Nothing here is designed. The plans mention embeddings as one cheap
scoring signal and reranking and reasoning as the funnel's last stage
(candidate management §1, §2), and Semantic Scholar's embeddings and
embedding clusters (ambitions §6; §9, item 6).

**The user's working assumption:** BM25, the citation graph and a little
more data augmentation will perform extremely well, especially with
model-written query enhancement in front of them.

### 5.1 Where each piece would fit

- **Embeddings as a fifth artifact** beside names, works, graph and
  lexical: a vector per work by `WorkId`, a base at a full merge,
  appended per batch; a process and version in ambitions §6's sense, the
  model's version in the artifact's, and a query encoded by another model
  than the index's a hard failure.
- **Hybrid retrieval in the query core:** the lexical and vector lists
  fused by rank, then the facet filters, with the graph as a third signal
  — similar in wording, similar in meaning, and within two hops of a seed
  set — which few services can answer and whose pieces already share one
  process.
- **Small models in the pipeline before query time,** as asserters ranked
  below the registries and sources: matching references that carry no
  name to works (the merge plan's matching stage over the unresolved
  stream, where every match is an arc gained), person identity (ambitions
  §4), extraction from full text (ambitions §6). At query time, second:
  a reranker over the top candidates, and a question turned into filters.
- **Agents as clients of the query core.** The funnel is already an
  agent's loop. What suits agents: caps refused rather than clamped,
  stable public ids to carry between steps, every answer stamped with its
  epoch and batch. What is missing: compact answers — id, title, year and
  venue by default, the rest on request (serving plan §11, item 5); a
  session pinned to one watermark across many calls (item 20); the
  funnel's cheap stages — expand, filter, score, keep the top *k* — as one
  server call, so an agent reads only the short list, which is where
  cheap to serve becomes cheap to use; a tool-friendly front door, JSON or
  MCP, over the same core.

### 5.2 What it would cost

- **Vector memory.** At corpus scale, full-precision vectors do not fit in
  the box's share of 128 GB. Compression is mandatory; the usual shape is
  a coarse pass over compact vectors in memory and a rescoring of the top
  few hundred from full vectors on disk. To be sized beside the search
  engine's and the graph's measured footprints, not before them.
- **Byte-identical rebuilds.** Model output varies with hardware and batch
  size, and the common vector indexes are built with randomness.
  Suggested, the user's to decide: computed vectors kept like raw, keyed
  by model and version, and the index over them the cache, checked by
  agreement on neighbours rather than by bytes. It would be the first
  exception to the principle.
- **Knowing whether it got better.** A scoreboard before any of it:
  search with a work's title and abstract and count how many of its own
  references come back — citations as relevance judgments made by
  experts. It can start on the sample's held works, whose links among
  themselves are thin; the corpus gives the figures that count. SP2's six
  fixed queries are the only check today. Published benchmarks to run
  beside it are in [benchmarks for search and
  research](retrieval-benchmarks.md).
- **Policy and terms.** What text is embedded — which abstract, works with
  only a title, other languages — is indexing policy (serving plan §11,
  item 11), pinned. Abstracts are where the data terms carry risk:
  vectors from abstracts used inside are one question, vectors served to
  others another.

### 5.3 Suggested order

On the sample, cheap: the citation scoreboard, and a small embedding
experiment over the held works to see what vectors add to BM25 on it.
After the corpus: reference matching as the first model in the pipeline,
then vectors as an artifact, then the front door for agents.

## 6. Tracing a concept

Extends ambitions §7, which on 2026-09-15 recorded the wish and asked for
no advice; the user opened the question on 2026-09-16. What would do its
reading — Jev, its precedents, a local cascade — is in [the tracer's
engine](future-ambitions-tracer-engine.md).

### 6.1 The idea, as the user put it

Concepts cannot be enumerated — language embeddings may have something to
say about that, but permutations of meaning are a difficult state space
to work with — and so a graph traversal over concepts is a substantial
challenge at best. But if the amount of text a capable model has to chew
through is reduced, directed summarization ("find this concept") is
parallelized over small models that can handle it, and that data is
treated as ephemeral but traversable, maybe there is a chance.

### 6.2 A reading of it

Not a concept graph built ahead of time, but one grown per question along
the citation graph, with small models reading at each work. The concept
is the question rather than a node, so nothing is enumerated; the
structure comes from citations. It fits candidate management §2's
best-first loop, the small model being `evaluate(candidate)`:

1. **Entry points.** BM25 with model-written expansion finds recent works
   that plainly discuss the concept.
2. **Directed reading, in parallel.** Each candidate read with one narrow
   question: is the concept here — quote it; what does the work call it;
   which of its references does it credit for it.
3. **Expansion along the credited references only,** not the whole list.
4. **Repeat,** backward in time.
5. **The capable model reads the trail** — the quotes and the links
   between them — never the works themselves.

### 6.3 What would make it work

- **Citation context.** Following the one or two references a work cites
  *for this concept* keeps the trail thin; it needs the sentence around
  each citation. PMC's open-access XML marks where each citation sits in
  the text; Semantic Scholar's `citations` dataset carries them in bulk —
  confirmed 2026-09-16 on its release of 2026-09-09: 2.4 × 10^9 records
  with the surrounding text (`contexts`), an intent and an influence flag,
  under ODC-BY, downloadable with a key; how many records have a context
  is not checked ([retrieval frontier](retrieval-frontier.md) §4).
- **A vocabulary that grows backward.** BM25's blind spot is words
  changing over time, which is exactly what a trace meets: the idea
  behind selective sweeps was called hitchhiking before it was called a
  sweep. Citations reach the older works regardless of their words; the
  small model recognizes the idea under its old name and reports the
  name; a lexical search restricted to that era with the old name finds
  parallel work nobody cited — partly the independent discovery that
  ambitions §7 names as a gap. The trace is its own query expansion.
- **A quote, checked.** A narrow question with a required verbatim quote,
  and a plain string match that the quote is in the text, refusing the
  answer otherwise: fail fast, for model output.

### 6.4 Ephemeral and kept

Two kinds of reading, two lifetimes. Answers to the question are
ephemeral: they depend on its phrasing and rarely repeat. A reduction of a
work that does not depend on the question — what it contributes, what it
credits to whom, its key passages — is done once, the first time any
trace touches the work, and kept as a process and version (ambitions §6):
paid for only on works some question reached, and warming where questions
cluster. A trail is a session — works with their quotes and the names
they used, linked by the citations that carry the concept — explored,
extended, pruned, then dropped or saved, pinned to one epoch.

### 6.5 Where it would struggle

- **Full-text coverage.** Abstracts often do not carry the concept,
  method-level concepts least of all; full text exists for the open share
  only, so paywalled works are holes beside the conference and
  informal-channel gaps.
- **Reviews and hubs.** A review citing everything is a junction where the
  trail explodes; citation context is the defence.
- **Early wrong turns.** A false yes near the start misdirects everything
  after it; the quote check catches invented quotes, not misread ones.
- **Judging a trace.** The user's own examples as a hand-judged set —
  selective sweeps, evolutionary rescue, long-range promoter effects, the
  neural-network architecture feature — against reviews that tell those
  histories.

## 7. Open questions

1. Whether BM25, the graph and query expansion hold up on the citation
   scoreboard (§5.2), and what vectors add to them.
2. The byte-identical exception for model output (§5.2).
3. The vectors' share of the box, once the search engine and the graph
   are measured at corpus size.
4. Where citation contexts come from at scale, and on what terms (§6.3).
5. Whether answers to one phrasing of a concept can serve another (§6.4).
6. Whether an internal-data variant is ever in scope (§4).
