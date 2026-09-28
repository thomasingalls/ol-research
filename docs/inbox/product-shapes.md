# Product shapes, through funhouse mirrors

> **Read this first.** This is not a plan, a decision, a direction or a
> priority. It is one view of a possible future, seen through a series of
> funhouse mirrors — the user's words, and the right ones — written on
> 2026-09-16 and 2026-09-17 at the user's request from one conversation,
> with later dated additions retaining the same speculative status.
> Every sentence below that sounds like "the product is" means "one
> distorted reflection suggests the product might be". Nothing here
> re-ranks, amends or outranks the ingestion, merge, graph or serving
> plans, which remain the documents of record, nor the speculative notes
> it draws on: [future ambitions](future-ambitions.md), [the tracer's
> engine](future-ambitions-tracer-engine.md), [the retrieval
> frontier](retrieval-frontier.md), [benchmarks](retrieval-benchmarks.md).
> Where the user corrected a reflection, the correction is recorded
> beside it, and the correction wins. The industry around these shapes —
> access, licensing, the sellers' fears, the edge-and-moat discussion of
> 2026-09-17 — is in [the industry around the
> corpus](industry-landscape.md).

## 1. The question

With the ambitions recorded, does the vision have a clear product shape
yet? One reading: not yet — a clear stack, a clear edge and a strong
candidate, but a portfolio of four or five products sharing one
foundation, none of them chosen.

## 2. What looked clear

**A stack,** each layer with a defined job:

1. **The corpus** — authoritative, every source's version kept with its
   disagreements (merge plan).
2. **Serving** — fast, cheap, stable public ids, answers stamped with
   their epoch (serving plan).
3. **Reading** — BM25, the citation graph and a cascade of classifiers
   (future ambitions §5; the tracer's engine §4).
4. **Applications** — open.

The first two largely built on the sample, the third on paper, the
fourth unchosen.

**An edge,** recurring through every note:

- **Disagreement and provenance** kept rather than smoothed over.
- **Speed and cost** low enough to call on everything.
- **Citations as structure:** citation sentences, graph walks, citations
  as relevance judgments.

The free sources (OpenAlex, Semantic Scholar) and the frontier systems
that rent them (retrieval frontier §6) offer none of these whole.

**Qualification, 2026-09-25:** that was an early assessment, not an
established competitive advantage. The workbench comparison in §10 adds
documented provenance and execution features that this framing missed;
speed, cost, and corpus-level disagreement handling still need comparison.

## 3. Candidate shapes

| shape | for whom | how much exists | distinctive or crowded |
|---|---|---|---|
| **A citation resolver and checker** | AI agents and writers: is this reference real, which work is it, where do the sources disagree | mostly built: resolve and work through the query core, public ids | distinctive at speed; OpenScholar found GPT-4o fabricating most of its citations (retrieval frontier §2.2) |
| **A corpus API for research agents** | teams building research agents, who lean on rate-limited free APIs today | built at sample scale; needs the corpus | competes with free sources, so wins only on volume, speed and the disagreement data |
| **The concept tracer** | researchers entering a field, writers of reviews and grants, historians of science | on paper; needs citation contexts, the cascade and an evaluation | direct overlap with Undermind; the narrower concept-lineage workflow remains an untested distinction (correction below) |
| **Slice-and-dice analytics** | research offices, funders, publishers | needs registries for persons, institutions and funders; none exist (ambitions §4) | crowded: Dimensions, SciVal |
| **The method on other data** | clinical trials, case law, internal data (future ambitions §3, §4) | nothing | a company's direction, not a first product |

**Correction, 2026-09-24.** The original “nobody at the frontier does it”
assessment was too strong. Undermind's current tools cover adaptive search,
citation following, parallel reading, reference resolution, and agent
access; its GSK case study also describes tracing evidence over time.
Whether the concept graph in these notes offers a useful distinction needs
direct comparison. See the sourced [Undermind overlap note](competition-list.md#undermind-overlap--2026-09-24).

## 4. One reflection

Distorted accordingly:

- **The corpus as foundation, not product.** On metadata alone it
  competes with a CC0 OpenAlex and a free Semantic Scholar; exposed as an
  API where it beats them — volume for agents, resolving and checking
  citations, the disagreement data.
- **The tracer as the product.** The most distinctive shape, the one the
  user keeps returning to, and the one whose test set is the user's own
  concepts (future ambitions §6.5): the user is its first user.
- **The citation checker as the nearest to shippable,** and a component
  the tracer needs anyway: every step of a trail resolves citations.

## 5. What a choice would change

If the tracer were chosen, the plans' order would bend toward what it
needs.

**What it would need:** the full PubMed baseline; citation contexts,
from PMC's open-access XML without a key or Semantic Scholar's dataset
with one; BM25 and the graph, which exist; the reranker-and-GLiNER2
cascade; the user's hand-labeled concepts as its evaluation.

**What it would not need first:** registries for persons and
institutions; embeddings at corpus scale; Crossref and OpenAlex at full
size, if its first version is biomedical; origins and feeds (SP6); and
freshness under ten minutes.

**The user's correction, 2026-09-17, on freshness** — the reflection had
it serving the agent API more than the tracer:

- The pattern is a paper hitting social media, an explosion of interest,
  and whoever has it indexed and available first getting the sign-up and
  the customer.
- There are only so many customers.
- None of these customers buy directly from the publishers.
- So not soon, but not to be ignored: freshness serves acquisition for
  whichever product is chosen.
- In general the user agreed with the reflection, "but none of what i
  wrote is arbitrary" — the plans' choices have reasons, stated or not.

## 6. A constraint the choice would raise

A trace's output is quoted passages. The ambitions note says full text
is never served, only what is derived from it (ambitions §6), and PMC's
open-access licences vary per article, some non-commercial. A tracer
would need a rule for what a passage may show — a short quote with a
link, a paraphrase, a pointer only — and that rule belongs to the product,
so it cannot be settled before one is.

## 7. The question that would make it clear

Who the first user is. If the user, tracing their own concepts: the
tracer, and the plans narrow. If the teams building research agents: the
API and the citation checker, and the freshness and serving work already
done is the head start. The user's leads for first conversations and
first customers, 2026-09-28: [first customers and first conversations](first-customers-and-conversations.md).
A reading of that day's notes, too early and held loosely:
[one engine, two doors](one-engine-two-doors.md) (review teams in every
field, and a literature connector for agents).

## 8. Another mirror

TypeSafe's use-case map (read 2026-09-17:
[graphs and knowledge graphs](https://docs.typesafe.ai/concepts/use-case-map#graphs-and-knowledge-graphs))
lists, under graphs and knowledge graphs: annotating and verifying
knowledge graphs with typed semantic decisions; classifying relationships
and entity types; detecting contradictions between records or claims;
probabilistic traversal and hierarchical classification. Read against
these notes: classifying relationships is the credit a citing sentence
gives (the tracer's engine §2); contradictions between records are the
merge's disagreements; probabilistic traversal is the trace's best-first
walk. The user's remark: they already know the use case — which says the
shape is legible from outside, and that others are looking at it too.

## 9. Decision trails, public datasets, and Europe-first — 2026-09-24

**The user's additions:** following a citation trail might help reconstruct
decisions in document-heavy civil-engineering workflows; tree-of-life,
species-map, or other public datasets might be another application; the
user would want to sell into the European market first, making language
and translation accuracy worth considering. The first two are possible
applications. Europe-first is a stated market preference; a language
feature or advantage remains a hypothesis. No discipline, dataset,
country, or language pair has been selected.

**A reading:** the connection is reconstructing how a conclusion came to
be, through its supporting records, transformations, and disagreements.
The citation trail is one instance. This extends “the method on other
data” in §3 and [other corpora/internal datasets](future-ambitions.md#3-other-corpora-for-the-same-treatment).

### 9.1 Engineering: reconstruct the documented rationale

An illustrative question: “Why was this design changed, and what was the
decision based on at the time?” A possible trail runs from an approved
revision through a change request, calculations, surveys, and the cited
edition of a standard. Alternatives considered and rejected could matter
as much as the adopted design. That suggests distinct relationships such
as cites, derived from, approved by, supersedes, and rejected because,
with dates and document versions retained.

The user's premise is that meticulous documentation in some disciplines
could supply the trail. Its availability, completeness, and accessibility
have not been established. A reference proves a documented connection;
establishing why a decision was made needs evidence of the rationale.
Missing links and model-inferred connections would need to remain visible.

A concrete source of complexity: the European Commission's
[Eurocodes guidance](https://eurocodes.jrc.ec.europa.eu/en-eurocodes-implementation/nationally-determined-parameters)
describes national annexes containing country-specific parameters and
references to complementary information. This supports keeping country
and edition attached to an engineering source. It does not establish
that a project's decision history is available or that a buyer wants
this product.

Possible uses include handover and design review. The open commercial
questions are whose time this would save, how often the question arises,
and whether their project records can be made available. The internal-data
constraints already recorded in [future ambitions §4](future-ambitions.md#4-internal-datasets)
would apply.

### 9.2 Public datasets: trace a result through data and methods

Two possible questions connect the dataset thought to the existing work:

- For a tree-of-life relationship: which source trees, classifications,
  and studies support it, which disagree, and what changed between releases?
- For a species map: which observations, taxonomic interpretation,
  geographic processing, filters, or model produced this result? Is it
  an observed occurrence or a predicted distribution?

Open Tree of Life is a concrete example to investigate, without assuming
it is the particular “treeoflife data” the user meant. Its
[browser already exposes supporting and conflicting sources](https://opentreeoflife.github.io/browse/).
[GBIF downloads](https://techdocs.gbif.org/en/data-use/download-formats)
already distinguish interpreted occurrence records from original verbatim
records. These are precedents and possible inputs; retaining provenance
alone is not an established gap in this domain.

The reusable part might be identity, versioned assertions, disagreement,
and traversable provenance. A biological relationship, a bibliographic
citation, and a data transformation would need different meanings and
evidence. Spatial, temporal, and phylogenetic operations would also need
domain-specific support. “Public datasets” names a source opportunity;
the useful product still needs a recurring question and a user.

### 9.3 Language: a requirement across possible applications

Keep three possibilities separate: an interface in the customer's
language; retrieval that finds relevant evidence written in other
languages; and translation that preserves the evidence's technical
meaning. The latter two connect most directly to the tracer idea.

An illustrative workflow: ask in French, find relevant German and English
records, and inspect an answer with translated passages aligned to the
originals. Candidate requirements include consistent specialist terms,
unaltered identifiers and units, faithful negation and uncertainty, and
visible ambiguity. Language and jurisdiction would remain separate:
translating a term would not itself establish equivalent rules or concepts.

This is worth retaining as a product hypothesis. Broad multilingual
support alone is not a demonstrated advantage: the Commission already
offers [translation and multilingual document tools](https://translation.ec.europa.eu/tools-and-resources/ai-translation-and-language-tools_en).
The narrower question is whether a selected professional workflow loses
important evidence or meaning with existing tools, and whether this
product can measurably improve that. Europe-first does not yet imply
support for every European language or a decision to build a translation
model. Language pairs, terminology needs, and acceptable error rates
remain open.

## 10. The workbench is part of the product — 2026-09-25

**The user's clarification:** “skills+ui+coding agent in a ui with
intentionally designed interfaces for these tools” is the dream. The
user reports having designed this in 2024 and expresses frustration on
finding several products with that shape. This records the intended
experience more clearly than §3's list of individual utilities.

**Correction to the earlier framing:** the product under discussion
includes a coherent environment in which a researcher can direct an
agent, use scientific procedures, inspect and revise outputs, and retain
the evidence and work that produced them. Treating the corpus, tracer,
and citation checker as separate candidate products understated that
intended experience.

The sourced [workbench comparison](competition-list.md#research-workbenches--2026-09-25)
adds OpenScience (Synthetic Sciences), Open Science Desktop (AI4S),
Claude Science, and K-Dense. Their documented skills, execution,
scientific interfaces, and provenance are substantive overlap with the
broader vision. Extensibility also matters: comparisons need to include
what a skill or connector enables within those environments.

This does not establish equal quality, adoption, or coverage of the
proposed concept-lineage task. It does mean that provenance, local
execution, and a purpose-built scientific interface cannot be treated
as unclaimed differentiators. The user has not chosen to narrow the
product, pivot to infrastructure, or abandon the workbench vision.

---

*Once more: a view of a possible future through funhouse mirrors. The
plans decide; this reflects.*
