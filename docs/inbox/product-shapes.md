# Product shapes, through funhouse mirrors

> **Read this first.** This is not a plan, a decision, a direction or a
> priority. It is one view of a possible future, seen through a series of
> funhouse mirrors — the user's words, and the right ones — written on
> 2026-09-16 and 2026-09-17 at the user's request from one conversation.
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

## 3. Candidate shapes

| shape | for whom | how much exists | distinctive or crowded |
|---|---|---|---|
| **A citation resolver and checker** | AI agents and writers: is this reference real, which work is it, where do the sources disagree | mostly built: resolve and work through the query core, public ids | distinctive at speed; OpenScholar found GPT-4o fabricating most of its citations (retrieval frontier §2.2) |
| **A corpus API for research agents** | teams building research agents, who lean on rate-limited free APIs today | built at sample scale; needs the corpus | competes with free sources, so wins only on volume, speed and the disagreement data |
| **The concept tracer** | researchers entering a field, writers of reviews and grants, historians of science | on paper; needs citation contexts, the cascade and an evaluation | nobody at the frontier does it (retrieval frontier §6) |
| **Slice-and-dice analytics** | research offices, funders, publishers | needs registries for persons, institutions and funders; none exist (ambitions §4) | crowded: Dimensions, SciVal |
| **The method on other data** | clinical trials, case law, internal data (future ambitions §3, §4) | nothing | a company's direction, not a first product |

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
done is the head start.

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

---

*Once more: a view of a possible future through funhouse mirrors. The
plans decide; this reflects.*
