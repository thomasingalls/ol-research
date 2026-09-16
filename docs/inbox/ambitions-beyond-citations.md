# Beyond the citation graph: ambitions

**Status:** notes, 2026-09-15 — ambitions voiced in conversation, not a
plan; nothing here is scheduled. Written so that MM3's views, MM4's edge
model and the search sinks (MM5) do not foreclose them. Companion to the
[architecture notes](citation-graph-architecture-notes.md).

## 1. The ambitions, as stated

1. **People, institutions, funders, mentorship.** Authorship and
   co-authorship, funding sources, associated institutions, and the
   mentor–trainee tree of academictree.org, held in detail and queryable.
   The questions are partly graph-shaped and partly facet-shaped; the
   first home is the search engine.
2. **The split, from the start.** The search engine will struggle past
   two hops of citation traversal, so the graph is a separate layer from
   the beginning rather than an optimisation of the search engine.
3. **Structure from full text.** Topic modelling, named-entity
   recognition, clustering, or some mix of them, over works whose full
   text is available, to improve search: disambiguating a query ("dna
   ancestry" is commercial services, evolution or basic genetics — a case
   tags already cover), and detail at the level of methods sections
   (real-time PCR, not reverse-transcription PCR), which may itself become
   graph-shaped.
4. **Tracing a concept** back through the literature without reading
   whole articles to find it (§7).
5. **Conferences as a data source:** an index of what happens at
   academic conferences — posters, talks — if one exists (§7).

## 2. The split as a rule

- **Facet-shaped:** answered by filtering and counting over fields of
  works — who, where, funded by whom, when, what kind, what about. The
  search engine's job: facet fields and aggregations, with at most one
  join, an entity's id on the work.
- **Hop-shaped:** answered by following edges more than once — a
  person's co-authors' co-authors, a lineage, a citation neighbourhood to
  depth three, the institutions collaborating with those collaborating
  with one. The graph's job.
- **Two hops is the boundary.** Below it either layer answers; above it
  only the graph does.

Two consequences. First, **one id per entity, shared by both layers:**
the search engine holds a person's id as a facet value and the graph
holds it as a node, so the `WorkId` discipline — minted, opaque, dense,
never leaving the system — extends to persons, institutions, funders and
venues, and the layers meet on ids. "Works using real-time PCR that cite
W" is a facet set intersected with a one-hop set, by id, in the query
layer; neither layer has to know the other's shape.

Second, **projections are not stored.** A co-authorship graph is the
two-hop projection person → work → person, and storing it is quadratic in
author-list length (a 3,000-author paper is 4.5 million pairs).
Institution collaboration, funder co-funding and method co-occurrence are
the same shape. The graph answers them on demand, which is what a graph
layer is for; the search engine's aggregations give the top few counts
for one pivot when that is all a page needs.

## 3. What the sources already carry

Most of the raw material for ambition 1 is in the four sources, and much
of it is already normalised (ingestion plan §2, §9). What is missing is
identity — one id per person, institution, funder, venue — and the
edges that follow from it.

| entity | Crossref | PubMed | OpenAlex | Semantic Scholar | namespace of record |
|---|---|---|---|---|---|
| person | `author[]`: given, family, ORCID, affiliation, sequence; organisational authors as `name` | `Author`: LastName, ForeName, Initials, ORCID as `Identifier`, `AffiliationInfo`, `CollectiveName` | `authorships[]`: author id and ORCID, `raw_author_name`, position, corresponding, institutions, countries; `author.id` null on 4 % | `authors[]`: authorId, one name string | ORCID — self-asserted, partial coverage; OpenAlex `A…` and S2 author ids are an aggregator's clustering, assertions like their work ids |
| institution | affiliation names; ROR ids on newer deposits | affiliation strings; ISNI and ROR identifiers on recent records | institutions with ROR, type, country; `raw_affiliation_strings` | — | ROR: a CC0 bulk file with parent–child links and external ids (GRID, ISNI, Wikidata, Funder Registry) |
| funder, award | `funder[]`: Funder Registry DOI, award ids | `GrantList`: GrantID, Acronym, Agency, Country (empty on 92 %) | `grants[]`: funder `F…` id, award id | — | Open Funder Registry DOIs (`10.13039/…`), being folded into ROR — verify the current state; award ids have no registry |
| venue | ISSN, container-title, publisher | Journal ISSN, `NlmUniqueID`, `MedlineTA` | sources: `S…` id, ISSN-L, host organisation | venue, publicationvenueid | ISSN-L; merge plan §11 item 6 |
| topic | subjects | MeSH headings and qualifiers, chemicals | topics, subfield, field, domain; keywords; MeSH on PubMed-derived works | fields of study | MeSH UIs with the tree; OpenAlex topic ids |
| mentorship | — | — | — | — | academictree.org: people, mentor–trainee relations with type and years, institutions, publications matched to people |
| method | — | — | — | — | none; ambition 3 |

academictree.org, checked 2026-09-15 with three read-only page requests:
the site footer licenses the data CC BY 3.0 with attribution; the front
page shows no bulk export or API, and the FAQ sits behind a bot check
that was not attempted. Whether a bulk file or a research export exists
decides whether it can be a source at all under the ingestion plan's
bulk-first rule; the contact given is admin at neurotree.org. To check
by hand.

## 4. In the merge's vocabulary

The machinery generalises without new words.

- **A registry per kind.** Each entity kind gets its own append-only log
  and minted id — `PersonId`, `InstitutionId`, `FunderId`, `VenueId`,
  later `MethodId` — with names in the kind's namespaces (ORCID, ROR,
  Funder DOI, ISSN-L, MeSH UI) as assertions with verdicts, records from
  sources, stubs (an ORCID no record holds; a ROR only affiliations
  name), promoted fields with disagreements kept. Precedence is registry
  over aggregator, as for works: ORCID over OpenAlex's `A…` over S2's for
  persons; ROR over OpenAlex's `I…`; the Funder Registry over OpenAlex's
  `F…`. The minting code should take a kind rather than assume works.
- **Typed edges across kinds.** Merge plan §6's relation stream
  `(from, to, kind, by)` is the shape; `from` and `to` become ids of
  stated kinds. Each (from kind, edge kind, to kind) is a **slice** with
  its own forward and transpose CSR and its own label type (architecture
  notes §2):

  | slice | label | from |
  |---|---|---|
  | work → person `authored-by` | position, corresponding, sources | the authorship list |
  | work → institution `affiliated` | sources | the authorship list |
  | work → funder `funded-by` | sources; the award id stays on the work's record | grants |
  | work → venue `published-in` | sources | the promoted venue |
  | work → topic `about` | source, or process and version | MeSH, topics; later models |
  | person → person `mentored-by` | relation type, years | academictree |
  | work → method `uses` | process and version, evidence count | ambition 3 |

- **Authorship is the hard fact.** It is ternary — work, person,
  institutions, plus position — and the sources disagree on all three
  parts. The merged record keeps the authorship list as structured data
  (MM3's view keeps it; do not flatten it into strings for the sink); the
  work → person and work → institution slices are derived from it, and a
  person's affiliation history is derived from their authorships with
  years. Whether authorship deserves a node of its own is open; the
  recommendation is no — a list on the work, and derived slices.
- **Person identity** is the entity-resolution problem the merge plan
  defers (§11 item 6). The first cut promotes one source's author list
  and counts disagreements. The first real cut is ORCID-anchored and
  aggregator-assisted, and never joins two persons on a name alone — the
  same posture as works, where nothing joins on text except to corroborate
  a conflict. The disagreement between OpenAlex's and S2's author clusters
  is the same kind of data the merge exists to keep visible.

## 5. Where each question is answered

| question | layer | note |
|---|---|---|
| works by a person, at an institution, funded by F, in venue V, about T | search facets | entity ids as facet fields, from the registries |
| a person's co-authors, ranked by shared works | graph, two hops, count | not stored (§2) |
| a person's trainees, and theirs | graph, n hops over `mentored-by` | a tiny slice |
| institutions collaborating with I | graph, two hops through works, count | the top few via a search aggregation |
| works using method M that cite W | search facet ∩ graph one hop | by id, in the query layer |
| how topics drift along a citation chain | graph n hops, then facets over the frontier | the funnel of the [candidate-management note](candidate-management.md) |
| "did you mean" | search | the facet distribution over the top results (MeSH, topic, method) offered as refinements |

The last row is the whole of ambition 3's first version: no new query
engine, only annotations as facet fields and a page that shows their
distribution.

## 6. Structure from full text

- **Where full text comes from, and on what terms.** PMC's open-access
  subset (bulk, a licence per article; Europe PMC carries the same with
  parsed references — the ingestion plan's long tail), arXiv e-prints (a
  licence per submission; may not be stored and served without one),
  CORE (an application), Crossref's text-and-data-mining links, OpenAlex's
  `pdf_url` per location. Full text is a separate world from metadata
  terms: it lands under `raw/` with receipts and its licence recorded per
  object, and it is never served — only what is derived from it is.
- **Derived facts are annotations:** `(work, label, process, version,
  evidence)` — an assertion whose asserter is a process rather than a
  source. A cache, like the merged sets: keyed by process and version,
  rebuilt when either changes; raw plus the registries stay the record.
- **Fail fast meets probability.** Model output carries scores; the
  no-thresholds rule applies to the pipeline's mechanics — a version
  mismatch, a missing text, a label outside the closed vocabulary fail the
  run — and scores are stored raw and cut at query time or in the sink.
  To decide when the first process lands.
- **Label spaces before models.** MeSH already distinguishes Real-Time
  Polymerase Chain Reaction from Reverse Transcriptase Polymerase Chain
  Reaction, and NLM assigns both — at article level, for indexed articles,
  with a lag. The topic signal already ingested covers the "dna ancestry"
  case as facets. Cheapest first: dictionary recognition over methods
  sections against MeSH's techniques tree (E), then OBI and EDAM where
  MeSH is thin; a learned model when the dictionary's recall has been
  measured and found wanting; topic models and embedding clusters for the
  disambiguation experience, where unlabelled clusters are acceptable
  because the user sees example works, not labels.
- **Sections need structure.** PMC XML has them; PDFs need a parser
  (GROBID or similar, a service on the box) — a dependency of the
  methods-level ambition, not of the topic-level one.
- **When methods become graph-shaped:** work → method is a bipartite slice
  like work → funder; method co-occurrence and "the methods used by the
  works citing W" are one and two hops; the same typed model absorbs it.
  Its label is a process and version, which is why the label type is per
  slice.

## 7. Concepts, and conferences as a source

Recorded 2026-09-15 as stated. How to do either is an open question, and
no advice is wanted on it yet.

- **Tracing a concept.** The wish: follow a concept back through the
  literature without reading whole articles to find it — selective
  sweeps, evolutionary rescue, "promoters can have long-range effects", a
  feature of a neural-network architecture. Citation graphs, academic
  lineages and co-authorship graphs are a cheat code for where to look.
  But concepts also transfer at academic conferences and through
  informal channels, and are independently discovered; the literature is
  the signal that is available, so a trace is reconstructed from it, with
  those transfers as its gaps.
- **Conferences as a source.** An index of what happens at academic
  conferences — posters, talks, and the rest — would be interesting to
  ingest in some form. What the sources hold today: proceedings papers as
  a kind (58 conference papers on the sample, merge plan §7; DBLP in the
  ingestion plan's long tail for computer science). Posters and talks
  have no source among those assessed.

## 8. What not to foreclose now

- **MM3's views:** keep authorships, affiliations, grants, MeSH and topics
  structured in the merged record.
- **The registry:** minting takes a kind; one log per kind under `merged/`.
- **MM4's edges:** slices with a label type per slice; `Sources` widened
  before the ninth source (architecture notes §2).
- **MM5's sinks:** entity ids, MeSH and topics as facet fields from the
  first sink; a method field later is one more field.
- **The store:** full text under `raw/` with a licence per object;
  annotations under a derived prefix keyed by process and version.
- **Serving:** works have a public id by decision (serving plan §6);
  persons will need one when they are served.

## 9. Open questions

1. Authorship as a node or as a list on the work (§4).
2. Person identity: the precedence order, and the never-on-name-alone
   rule against the aggregators' clusters.
3. academictree.org: bulk availability and the terms behind the bot check.
4. Full-text terms per article in the PMC subset, and what "derived facts
   only" needs to hold for the non-commercial licences.
5. Scores against fail-fast (§6).
6. Whether OpenAlex's topics are good enough that topic modelling is not
   worth doing, and whether S2's embeddings (key required) replace our
   own.
7. How a concept is traced through the literature (§7). Not designed; no
   advice wanted yet.
8. Whether any index of conference posters and talks exists in bulk (§7).
