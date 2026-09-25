# Undermind alternatives and Mistral overlap

Reviewed 2026-09-24. This compares public product documentation, not measured
quality. The broader [capability matrix](competition-list.md) records each
service's span. Blank capabilities there mean unestablished, not absent.

## What counts as the same work?

Undermind's [tool catalogue](https://www.undermind.ai/mcp) combines iterative
literature discovery, citation following, reference resolution, paper
reading, and tools other agents can call. Competitors can overlap the
researcher workflow, the underlying retrieval service, or both.

The strongest documented matches for the scholarly discovery loop in this
review are Asta, Edison Literature, and scienceOS. Valyu is particularly
relevant to the proposed retrieval API, and Causaly to biomedical concept
graphs and enterprise research. This grouping is an assessment of feature
overlap, not a performance ranking.

## Closest comparisons

| Service | Documented overlap | What still needs establishing |
| --- | --- | --- |
| **Ai2 Asta / Paper Finder** | [Asta](https://allenai.org/asta/agents) decomposes questions, searches papers, follows citations, evaluates relevance, and synthesizes findings. Its [MCP tools](https://allenai.org/asta/resources/mcp) expose search, citations, paper metadata, and passages to other agents. | How well it reconstructs historical concept lineage and preserves the resulting relationships for later traversal. |
| **Edison Literature / PaperQA3** | [Literature](https://edisonscientific.com/news/edison-literature-agent) reads text, figures, and tables, checks provenance and retractions, and has an API. The [platform docs](https://docs.edisonscientific.com/) describe citation-graph-aware synthesis. **Precedent** searches across fields for whether an idea has been tried before. | Novelty search overlaps origin-finding; it does not establish a retained graph of renamings, attribution, and independent discovery. Hosted and open-source capabilities need separate evaluation. |
| **scienceOS** | Its [agent description](https://www.scienceos.ai/articles/how-does-the-ai-research-agent-scienceos-work/) specifies query generation, citation-network expansion, PDF reading, libraries, and repeated tool use. It demonstrates checking manuscript claims against their cited sources. | API availability, reproducible corpus snapshots, and persistent concept relationships are not established in the reviewed material. |
| **Valyu** | The [life-sciences offering](https://www.valyu.ai/life-sciences) supplies scientific full text and specialist databases through REST, SDK, and MCP. Search, Answer, and DeepResearch cover retrieval through multi-step reports. | Citation-graph traversal, reference repair, and source-disagreement preservation are not established. Full-text coverage depends on the source and licensing. |
| **Causaly** | [SIRS](https://www.causaly.com/ai-platform/scientific-rag) combines similarity and graph retrieval over biomedical concepts and relationships. This makes it relevant to both research agents and the concept-graph idea. | Biomedical relationship graphs do not establish the proposed historical/credit semantics. Compare enterprise deployment and graph exports directly. |

Other relevant comparisons already in the matrix are
[Elicit](https://elicit.com/) for evidence extraction and reviews and
[Consensus](https://help.consensus.app/en/collections/10600168-explore-features)
for literature answers, deep reviews, and citation graphs. Their presence
does not establish that they use the same search loop as Undermind.

Two further additions broaden the review-workspace comparison:

- [AnswerThis](https://answerthis.io/) advertises literature screening,
  claim verification, shared libraries, and writing with citations.
- [Iris.ai / RSpace](https://iris.ai/blog/announcement-new-features-in-r-space-1-3)
  documents full-text chat, OpenAlex integration, monitoring, and
  collaborative datasets. Its
  [topic/concept analysis](https://iris.ai/blog/tech-deep-dive-topic-modelling-with-the-analyze-tool)
  is relevant to concept discovery, although the reviewed material does
  not establish an autonomous citation-following loop.

## Why Mistral belongs

Mistral has public overlap across research interfaces, retrieval
infrastructure, and custom deployments. Inclusion does not depend on
guessing what its forward-deployed engineers are building privately.

| Public evidence | Competitive relevance |
| --- | --- |
| [Vibe, formerly Le Chat](https://mistral.ai/products/vibe/) offers deep research across web sources, internal documents, and connected tools, plus shared workspaces. | A general research interface capable of incorporating scholarly tools. |
| [Agentic Search](https://mistral.ai/news/agentic-search/) adds iterative search, document opening, navigation, reading, and pattern lookup to existing indexes. It is available through Search Toolkit and Libraries in Studio/Vibe. | A documented implementation of the retrieval-and-reading loop, with cloud and on-premises options. The published evaluations concern financial/document QA, not scientific concept tracing. |
| [Search Toolkit](https://mistral.ai/news/search-toolkit/) packages ingestion, retrieval, and evaluation infrastructure. | A potential substitute or component for teams building their own research agents. |
| [Wiley Scholar Gateway](https://www.wiley.com/en-de/research/wiley-scholar-gateway/) integrates with Vibe and returns passages from scholarly full text with metadata and DOIs. | Concrete scholarly retrieval through a publisher connector. Wiley also belongs in the inventory as a retrieval provider. |
| The [Pierre Fabre case study](https://mistral.ai/customers/pierre-fabre/) describes a tailored solution for document processing, analysis, synthesis, and production. | Evidence of customization in a pharmaceutical/cosmetics organization; it does not identify a scientific citation-search deployment or the team that built it. |

The appropriate comparison is Mistral's platform plus relevant content
connectors and implementation work. Public evidence reviewed here does
not establish its private FDE project scope, a dedicated scholarly
citation graph, or Undermind-equivalent research quality.

## Implication for the proposed product

The broad search–read–expand–synthesize loop is shared by several products.
API/MCP access is also already offered by multiple providers. Neither is
a demonstrated differentiator for this project.

The narrower [concept-tracer proposal](future-ambitions.md#66-build-then-traverse)
still supplies useful comparison questions:

- Can the service trace an idea across changing terminology and fields,
  distinguishing credit, equivalence, influence, and independent discovery?
- Can a user inspect, correct, retain, and export those relationships,
  then traverse them in different ways?
- Are conflicting source records and provenance retained, and can a
  result be reproduced against a known corpus state?
- What coverage, latency, cost, and deployment control does the same
  task actually achieve?

These remain questions to test against the closest services, not claims
that the competitors lack those capabilities.
