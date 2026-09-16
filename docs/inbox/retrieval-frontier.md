# The retrieval frontier

**Status:** notes, 2026-09-16 — what current systems for searching and
reading the scientific literature do, from a web survey made the same day
(searches and page reads only, including Semantic Scholar's public release
listing, no key). Context for [future ambitions](future-ambitions.md) §5
and §6; speculative, not a plan. Every fact is marked by how it was
checked: **primary** (the paper, official post or docs), **secondary**
(someone else's account), **unverified** (not confirmed). Three pages
refused the reader — OpenAI's Deep Research post, Consensus's docs page,
scite's journal article — and the system card, Consensus's blog and scite's
preprint stood in for them.

## 1. The common shape

Nearly every system surveyed runs the same loop:

1. **A cheap, wide first pass:** keyword search (BM25, or the Semantic
   Scholar API) mixed with dense embeddings, often over queries a model
   has rewritten.
2. **A model reads every candidate** against the question and judges,
   scores or summarizes it.
3. **Citation expansion,** backward and forward, from candidates already
   judged relevant.
4. **Repeat** until coverage looks sufficient, sometimes with parallel
   subagents returning compressed findings.
5. **A final model writes** from the summaries, with citations checked.

The first-stage retriever is a commodity; the quality is in the reading,
the expansion and the loop.

## 2. Systems

### 2.1 FutureHouse and Edison Scientific

- **PaperQA2** (2024, primary: [paper](https://arxiv.org/html/2409.13740)).
  An agent calling four tools in any order. *Paper search* through
  services like Semantic Scholar, twelve candidate papers by default.
  *Gather evidence*: a hybrid chunk embedding (text-embedding-3-large with
  a 256-dimension keyword vector) picks the top 30 chunks; a model
  summarizes each against the question and scores it 0–10 — ranking and
  contextual summarization, RCS. *Citation traversal*: backward through
  Semantic Scholar and Crossref, forward through Semantic Scholar, only
  from papers scoring 8 or more, keeping a cited paper only if several
  source papers cite it. *Generate answer* from the top summaries. The
  agent ran on GPT-4-Turbo, which also summarized best against
  Claude-Opus and Gemini-1.5-Pro. Removing RCS hurt accuracy
  significantly (p < 0.001). On LitQA2: 85.2 % precision and 66.0 %
  accuracy, against human experts' 73.8 % and 67.7 %.
- **The open-source paper-qa** (primary:
  [repository](https://github.com/Future-House/paper-qa)) searches a
  local full-text index built with Tantivy, defaults both models to
  gpt-4o-2024-11-20, and has no citation traversal yet.
- **The platform** (primary, 2025-05-01:
  [announcement](https://www.futurehouse.org/research-announcements/launching-futurehouse-platform-ai-agents)):
  Crow for quick answers, Falcon for deep reviews, Owl for "has anyone
  done X", Phoenix for chemistry.
- **Kosmos** (primary, 2025-11-05, Edison Scientific:
  [announcement](https://advances.edisonscientific.com/research/announcing-kosmos/)):
  about 1,500 papers read per run; 79.4 % of its conclusions judged
  accurate.
- **PaperQA3** (primary, 2026-02:
  [news](https://edisonscientific.com/news/edison-literature-agent)):
  reads figures and tables across 150 million or more papers and
  patents, the agent's model choosing which figures to look at rather
  than a multimodal embedding. Its models and scores are unverified (the
  scores are published as images).

### 2.2 Ai2

- **OpenScholar** (primary: [paper](https://arxiv.org/html/2411.14199);
  [Nature](https://www.nature.com/articles/s41586-025-10072-4), the
  publication date secondary). A datastore of 45 million papers, about
  234 million passages of 250 words; a Contriever-based retriever trained
  further on the corpus; a BGE reranker fine-tuned on synthetic data from
  Llama 3 70B; the Semantic Scholar API and web search beside it. A
  self-feedback loop — draft, up to three feedback notes, revise — then a
  citation check. GPT-4o alone fabricated 78–90 % of its citations.
- **Ai2 Scholar QA** (primary: [paper](https://arxiv.org/html/2504.10861)).
  A Vespa index of 285.6 million passages from 11.7 million full-text
  papers and over 100 million abstracts; retrieval scored 0.6 embedding
  (mxbai-embed-large-v1) and 0.4 BM25; the mxbai-rerank-large
  cross-encoder keeps the top 50; Claude 3.7 Sonnet extracts quotes,
  outlines sections, writes the report.
- **Paper Finder** (primary:
  [post](https://allenai.org/blog/paper-finder)). A model breaks the
  query into parts and routes it to sub-flows: Semantic Scholar keyword
  search, three dense snippet indices, rewritten queries, forward and
  backward citation following over several rounds. Relevance is judged
  per sub-criterion by a model; the final ranking mixes relevance with
  recency and citation count. 89 % "perfectly relevant" on LitSearch.
- **Asta** launched 2025-08-26 (primary:
  [post](https://allenai.org/blog/asta)); its Find Papers runs "Deep
  search" by default, searching again when results fall short (primary:
  [newsletter, 2026-08](https://allenai.org/newsletters/2026-08-newsletter)).

### 2.3 General deep-research agents

- **Anthropic** (primary, 2025-06-13:
  [post](https://www.anthropic.com/engineering/multi-agent-research-system)).
  A lead agent (Opus 4) plans and starts parallel subagents (Sonnet 4)
  that act as "intelligent filters" and return compressed findings; a
  separate citation agent places citations. 90.2 % better than a single
  agent on the internal evaluation; token usage explained 80 % of the
  variance; about fifteen times a chat's tokens.
- **OpenAI Deep Research** (primary, 2025-02-25:
  [system card](https://cdn.openai.com/deep-research-system-card.pdf)): an
  early o3 tuned for browsing, trained with reinforcement learning on
  browsing and Python tasks, graded against answers or rubrics.
- **Google Gemini Deep Research** (primary:
  [overview](https://gemini.google/overview/deep-research/)): an
  asynchronous task manager sharing state between a planner and task
  models, a million-token context with retrieval. Deep Research Max,
  2026-04-21, on Gemini 3.1 Pro, adds MCP data sources (primary:
  [post](https://blog.google/innovation-and-ai/models-and-research/gemini-models/next-generation-gemini-deep-research/)).
  That it is a single agent trained with reinforcement learning is
  secondary.
- **Google Scholar Labs** (primary, 2025-11-18:
  [post](https://blog.google/products-and-platforms/products/education/google-scholar-labs/)):
  breaks a question into topics, searches Scholar for each, explains how
  each paper answers it. An update of 2026-06-03 titled "Search 10x
  faster, 3x deeper" is unverified beyond its title.

### 2.4 Commercial literature tools

- **Undermind** (primary, 2024-01:
  [whitepaper](https://www.undermind.ai/whitepaper.pdf)). Then 2.3
  million arXiv full texts; candidates from embeddings, citations and
  model reasoning; GPT-4 sorts each paper into highly relevant, closely
  related or ignorable; searches again adaptively on what it found; stops
  when a fitted discovery curve says the search is nearly complete. Its
  current corpus is unverified (secondary: Semantic Scholar).
- **Elicit** (primary:
  [support](https://support.elicit.com/en/articles/553025)): 138 million
  papers from Semantic Scholar, OpenAlex and PubMed. A 2024 post names
  SPLADE as what it is "working on" (primary:
  [post](https://elicit.com/blog/semantic-search/)).
- **Consensus** (primary:
  [post](https://consensus.app/home/blog/how-consensus-works/)). Over 200
  million documents from Semantic Scholar, OpenAlex, its own crawl and
  publisher agreements; embeddings with BM25, then quality signals
  (recency, citations, journal), then a larger model reranking the top
  20. Deep Search (2025-07): up to 20 searches, over 1,000 papers
  reviewed. A "citation crawl" is secondary.
- **scite** (primary:
  [preprint](https://www.biorxiv.org/content/10.1101/2021.03.15.435418v1),
  [features](https://scite.ai/features)). Citation statements extracted
  from full text, each classified supporting, contrasting or mentioning
  by a deep-learning model: 23 million articles and 800 million
  statements in 2021, over 1.6 billion statements now. How its assistant
  retrieves is unverified.

## 3. Retrievers: what the evidence says

- **LitSearch** (primary: [paper](https://arxiv.org/abs/2407.18940)).
  Dense retrievers beat BM25 by 24.8 points of recall@5 on literature
  search questions; reranking with a model adds 4.4 % over the best dense
  retriever; Google Search trails the best dense retriever by up to 32
  points of recall.
- **BRIGHT** (primary: [paper](https://arxiv.org/abs/2407.12883), read
  from the PDF). Reasoning-intensive retrieval: BM25 averages 14.5
  nDCG@10, the top MTEB model 18.3. Querying with a model's written
  reasoning improves retrieval by up to 12.2 points, and BM25 improves
  the most — the rewritten queries are unlike what trained retrievers
  saw.
- **ReasonIR** (primary: [paper](https://arxiv.org/abs/2504.20595)). A
  retriever trained for reasoning: 29.9 on BRIGHT without a reranker,
  36.9 with one. Read by the survey's fetch tool and not by eye, so less
  certain: BM25 goes from 14.8 to 26.5 with GPT-4 reasoning queries and
  ReasonIR-8B from 24.4 to 29.9; other dense retrievers stop improving as
  queries lengthen while BM25 and ReasonIR keep improving.
- **SPECTER and SPECTER2** (primary:
  [SPECTER](https://arxiv.org/abs/2004.07180),
  [SPECTER2 card](https://huggingface.co/allenai/specter2),
  [SciRepEval](https://arxiv.org/abs/2211.13308)). SPECTER is pretrained
  on the citation graph as its relatedness signal; SPECTER2's base on
  over 6 million citation triplets, its task adapters on other task data
  beside 600,000 citation triplets.
- **Late interaction and learned sparse retrieval.** No public statement
  found of ColBERT-style retrieval in a production literature search
  tool; for SPLADE-style, only Elicit's 2024 "working on". The stacks
  that say what they run are dense with BM25 and a cross-encoder (Ai2)
  and embeddings with BM25 (Consensus).

## 4. Semantic Scholar's datasets

Primary, the release of 2026-09-09
([release listing](https://api.semanticscholar.org/datasets/v1/release/latest)):

- **`citations`:** 2.4 × 10^9 records, carrying `contexts` (the text
  around the citation), `intents` and `isinfluential`. How many records
  have a context is not checked.
- **`embeddings-specter_v2`:** 1.2 × 10^8 records.
- **Licences:** `citations`, `papers`, `abstracts` and `s2orc` are
  ODC-BY; the embeddings dataset's readme says Apache 2.0.

All need the API key to download (ingestion plan §2).

## 5. Benchmarks named in the survey

- **LitQA2** (in LAB-Bench): PaperQA2's figures above.
- **LitSearch** and **BRIGHT**: §3.
- **LABBench2** (primary: [paper](https://arxiv.org/abs/2604.09554)):
  about 1,900 tasks, FutureHouse and Edison.
- **AutoResearchBench** (primary:
  [paper](https://arxiv.org/abs/2604.25256)): the best systems reach
  9.39 % on its deep-research tasks.

Which of these can be reproduced, and which benchmarks test search alone,
is in [benchmarks for search and research](retrieval-benchmarks.md).

## 6. A reading against the ambitions

Interpretation, 2026-09-16, not findings.

- **The working assumption** (future ambitions §5): plain BM25 loses to
  dense retrieval on literature search (LitSearch), but BM25 behind
  model-written queries is a strong, cheap first pass (BRIGHT, ReasonIR),
  and no system that describes its stack runs BM25 alone — the mix with
  embeddings is cheap beside the reading. BM25, the graph and rewritten
  queries are a sound first stage; embeddings later are an ordinary
  addition, not a redesign.
- **The reading loop is where the frontier spends,** and where the
  concept trace (future ambitions §6) would spend: PaperQA2's RCS is the
  directed reading, Anthropic's compressing subagents the small readers
  feeding a capable model.
- **What the frontier does not describe:** tracing where an idea came
  from rather than answering a question; walking backward in time and
  searching each era again under the older names a trace finds
  (Undermind's adaptive search is the nearest); following citations by
  their context rather than by how many relevant papers share them —
  scite has the contexts and uses them for labels, not traversal.
- **The corpus layer is rented.** PaperQA2, Paper Finder, Elicit and
  Consensus lean on Semantic Scholar or OpenAlex for search and citations,
  and their loops call heavily (Consensus up to 20 searches over 1,000
  papers, Kosmos about 1,500 papers a run). A fast, cheap, local corpus
  that keeps its provenance is the layer this repository builds; agents
  make cheap to serve matter more.
- **Stopping is a design question:** Undermind's discovery curve is one
  answer to when a trace stops walking back.
- **Far from solved:** AutoResearchBench's best is under 10 %.
