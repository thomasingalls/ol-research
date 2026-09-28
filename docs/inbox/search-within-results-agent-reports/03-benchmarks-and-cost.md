# Research report 3: do set-oriented techniques help, and what does agentic literature search cost?

**What this is:** the final report of a web-research subagent, delivered
on 2026-09-28 to the conversation that wrote
[searching within a result set](../search-within-results.md), and kept
verbatim at the user's request. It is model output, not checked line by
line; the note's §12 draws on it. One claim was checked for the note:
Scells, Zuccon and Koopman 2019's Table 3 (item 1.1), read from the
paper and confirmed (note §11.3). It used 96 tool calls (web searches
and page fetches) over about 12 minutes. Its tags are its own: [P] it
read the full text or its result tables; [A] abstract or landing page
only; [S] a search engine's summary only. The prompt it was given
follows the report.

---

**Report: do set-oriented retrieval techniques help, and what does agentic literature search cost?** (web research, 2026-09-28)

How much I read of each source: **[P]** means I read the full text or its result tables. **[A]** means I read only the abstract or landing page. **[S]** means I saw only a search-engine summary. Per-query dollar figures marked "my division" are totals divided by the number of queries.

**Bottom line:**
- On systematic-review collections, automatic Boolean refinement and LLM-written Boolean queries reliably raise precision and F-measure, but they almost always lose recall compared with expert queries.
- I found no benchmark result for agents given facet, hit-count, clustering or subgroup tools.
- The closest results are two Boolean-tool agent papers from 2026 (1.7, 1.8). Both report good effectiveness, and Sieve (1.7) also reports token savings.
- Agentic literature search today pairs dense or hybrid first-stage retrieval with LLM selection and citation expansion.
- Cost per query ranges from about $0.003 to more than $2.

## 1. Set-oriented techniques on measured retrieval

1.1 **Scells, Zuccon, Koopman, WWW 2019, automatic Boolean refinement** [P]. http://bevankoopman.github.io/papers/scells2019refining.pdf
- Setup: 125 reviews, 35 test topics.
- The abstract says precision rose "while maintaining, or even increasing, recall". The averaged table says otherwise.
- The best selector (F1n) doubled precision (0.021 → 0.042) and raised F1 (0.037 → 0.066), but recall fell significantly (0.84 → 0.67).
- The recall-targeted selectors raised recall only to 0.88–0.89, and precision dropped.

1.2 **Scells et al., WWW 2020, automatic formulation from the review title (CLEF TAR 2018)** [P]. https://ielab.io/publications/pdfs/scells2020conceptual.pdf
- Original expert queries: recall 0.91, precision 0.025.
- The best automatic variants roughly matched a *simplified* expert query (recall about 0.49), at best recall about 0.59 with precision 0.011–0.016.
- The authors predict "only marginal gains".

1.3 **Wang et al., SIGIR 2023, ChatGPT Boolean queries** [P]. https://arxiv.org/abs/2302.03495
- CLEF TAR, writing a query from scratch: precision 0.075–0.117 against the originals' 0.021, but recall 0.12–0.50 against 0.83.
- Refining queries produced by the objective method: recall 0.81 (against 0.91), precision 0.046 (against 0.0002).
- Across 10 runs, recall variance was 12% of its mean.

1.4 **Reassessment, SIGIR 2025** [P, tables via fetch]. https://arxiv.org/abs/2505.07155
- o1 on CLEF TAR: precision 0.108, recall 0.655, against manual 0.021 / 0.844.
- On the Seed collection, seed-guided o1 reached recall 0.736 against manual 0.724, with very low precision.

1.5 **Collections.**
- Seed collection (SIGIR 2022, arXiv 2204.03096).
- CSMeD (NeurIPS 2023 Datasets and Benchmarks track): 325 reviews gathered from nine collections [A]. I found no Boolean-refinement result reported on CSMeD itself.
- CLEF TAR 2017–19 "No Boolean" subtask: organisers called it the hardest subtask [S]. I did not find its numbers.

1.6 **Clustering.**
- Hearst & Pedersen, SIGIR 1996 (Scatter/Gather on TREC-4 results) [P]. https://people.ischool.berkeley.edu/~hearst/papers/sg-sigir96/sigir96.html
  - The top cluster almost always held at least 50% of the retrieved relevant documents.
  - Users picked the best cluster in 31 of 38 cases.
- Bascur et al. 2022/23, citation clusters on 25 systematic reviews [A]. https://arxiv.org/abs/2207.03299
  - Results were "highly variable"; clusters were useful only when recall is weighted 2–8× over precision.
  - Clusters complemented Boolean search.
- Knowledge Navigator (2024), LLM topic hierarchy over results [A]. https://arxiv.org/abs/2408.15836 No numbers in the abstract.
- A modern Scatter/Gather-style benchmark result: **not found**.
- Subgroup discovery applied to retrieval: **not found**.

1.7 **Sieve (Wang, Zuccon et al., arXiv Aug 2026)** [P]. https://arxiv.org/abs/2608.02751
- The agent writes fielded Boolean queries, then ranks with BM25 + dense and fetches only selected sections.
- Tested on BrowseComp-Plus (structured variant), HotpotQA and MuSiQue.
- Accuracy +0.7 to +3.1 points, using 20.7–50.6% fewer tokens. Boolean filtering improved all 9 ranker × collection pairs.
- The agent sees result cards. I found no hit-count or facet feedback in the paper.

1.8 **Clarke & Smucker, "Boolean queries are all you need?" (arXiv Jul 2026)** [A]. https://arxiv.org/abs/2607.11362
- An LLM agent using a Boolean substring engine reached nDCG@10 0.6863 on TREC 2024 RAG (86 topics, 100 calls per topic).
- The authors call it exploratory and flag possible training-data contamination.

1.9 **Agents given facet or count tools:** no benchmark result found.

## 2. Agentic literature-search benchmarks: what top systems do and cost

- **LitSearch, EMNLP 2024** [A/S]. https://arxiv.org/abs/2407.18940
  - 597 queries. GritLM R@5 74.8, which is 24.8 points above BM25.
  - GPT-4 reranking adds 4.4. Google lags by up to 32 points.
- **PaSa, ACL 2025** [P]. https://arxiv.org/html/2501.10120v2
  - Crawler (search + citation expansion) plus a 7B selector.
  - RealScholarQuery (50 queries; annotation cost $304/query): R@20 0.580, R@50 0.656, precision 0.515.
  - PaSa-7B beats PaSa-GPT-4o by 30% in recall. No inference cost is stated.
- **SPAR, arXiv Jul 2025** [P]. https://arxiv.org/abs/2507.15245
  - SPARBench (50 queries): F1 0.30 (recall 0.31, precision 0.29). PaSa scores 0.10, Google Scholar 0.004.
  - Its citation expansion (RefChain) raised recall from 0.13 to 0.15 but cut precision from 0.22 to 0.16.
- **ScholarQuest, arXiv Jun 2026** [P]. https://arxiv.org/abs/2606.20235
  - 1,111 queries, measuring exhaustive recall (R@All).
  - Best agent (PaperScout): R@100 0.314, R@All 0.355.
  - Single-shot dense retrieval: 0.208 / 0.290. BM25+dense hybrid (RRF): 0.214 / 0.244, so hybrid did *not* beat dense on R@All.
  - PaSa used 60 tool calls and 744 candidates per query.
  - Gold sets were built with retrieval, citation expansion and LLM adjudication (86% strict precision in audit), so recall is relative to that pool.
- **AutoResearchBench, arXiv Apr 2026** [A + fetch summary]. https://arxiv.org/abs/2604.25256
  - 1,000 problems. Best scores: 9.39% accuracy on Deep Research, 9.31% IoU on Wide Research.
  - Claude Opus 4.6: 642 s and 27 tool calls per query. The reported token count (~19k per query) is uncertain.
- **BrowseComp-Plus, ACL 2026** [P, cost table]. https://arxiv.org/abs/2508.06600
  - 830 queries over 100K documents.
  - GPT-5: 55.9% with BM25, 70.1% with Qwen3-Embedding-8B, with fewer search calls (23.2 → 21.7).
  - Total API cost for GPT-5 fell from $400 to $361 (about $0.48 → $0.43 per query, my division). For Opus 4 it fell from $2,044 to $1,842 (about $2.2–2.5 per query).
- **OpenScholar, Nature Feb 2026** [P, arXiv html]. https://arxiv.org/abs/2411.14199
  - ScholarQABench: 2,967 queries.
  - Pipeline: dense retriever + Semantic Scholar keyword API + cross-encoder reranker + self-feedback.
  - Cost per query: OpenScholar-8B $0.003, OpenScholar-GPT4o $0.05, PaperQA2 $0.3–2.3.
- **PaperQA2 / LitQA2, 2024** [S]. https://arxiv.org/abs/2409.13740 Precision 85.2%, accuracy 66.0%, against humans at 64.3% / 63.1%.
- **BRIGHT, ICLR 2025** [S]. https://arxiv.org/abs/2407.12883
  - Original best was 24.3 nDCG@10; LLM query reasoning added up to 12.2.
  - DIVER (LLM query expansion + reasoning retriever + LLM reranker) reached 41.6, later reported 46.8 (https://arxiv.org/abs/2508.07995).
- **SIGIR 2026 cost study** [A]. https://arxiv.org/abs/2604.03676 Large LLM bi-encoders add "substantial latency for modest gains"; their scores are poorly calibrated.

## 3. Cost-efficiency of agentic retrieval

- **BATS / Budget Tracker (Google, COLM 2026)** [S, from VentureBeat; abstract has no numbers]. https://arxiv.org/abs/2511.17006 Same accuracy with 40.4% fewer searches, 19.9% fewer browses, 31.3% lower cost.
- **BARGAIN, SIGMOD 2026** [A]. https://arxiv.org/abs/2509.02896 Cheap-model cascade with precision or recall guarantees; up to 86% more cost reduction than the previous best method.
- **Small models beating large ones:**
  - OpenScholar-8B: roughly 100–700× cheaper than PaperQA2 and more correct [P].
  - PaSa-7B beats its GPT-4o variant [P].
  - RankZephyr 7B matches or beats GPT-4 at listwise reranking [A]. https://arxiv.org/abs/2312.02724
- **Screening cascade** [S]. https://arxiv.org/abs/2512.11261 Only 12.96% of records were escalated to GPT-4.1.
- **A distilled LLM-judgment classifier measured on these literature benchmarks:** not found.

## 4. High-recall retrieval with LLMs and active learning

- **Cormack & Grossman, SIGIR 2014** [S]. Continuous active learning seeded by keyword search needs significantly less review effort (p < 0.01).
- **Zero-shot calibrated LLM screening, ECIR 2024** [P, html]. https://arxiv.org/abs/2401.06320
  - Five collections (CLEF TAR 2017–19, Seed).
  - The ensemble reached recall 0.93–0.97 and work saved 0.50–0.54, but only 50–52% of topics met the 95% recall target.
- **LLM pseudo-labels seeding active learning, medRxiv Aug 2025** [S; blocked]. 42–43% workload saved with "near-perfect" recall and no manual labels.
- **Relativity, GPT-4 first-pass e-discovery review (2023 draft)** [P]. Up to 95% recall and 85% precision against attorneys.
- **Human vs LLM screening, arXiv Aug 2026** [A]. https://arxiv.org/abs/2608.26885
  - Humans: 82–83% recall while keeping 42–45% of records.
  - Gemini 3.1: 83.9% recall while keeping 56.7%.
  - Two identical GPT-5.4 runs differed on 94 records, 29 of them eligible.
- **Stopping rules, SIGIR 2026** (confidence-based, CLEF TAR, no LLMs) [A/html]. https://arxiv.org/html/2606.15380 85.3% documents saved at 92.6–97% decision agreement.
- **LLM4SCREENLIT** [S]. None of the 5 papers claiming workload savings priced false negatives.
- **TREC Total Recall-style continuous active learning with an LLM as the labeler, with recall and cost numbers:** not found.

---

## The prompt it was given

> Web research task (read-only; do not create or modify any files). Today is 2026-09-28. Context: a user building a scholarly-literature search engine believes state-of-the-art literature retrieval is hybrid BM25 + dense vectors plus LLM-agent relevance judgments, and that it is expensive. They wonder whether older "set-oriented" techniques (boolean queries refined with hit-count feedback, facets, clustering of result sets, subgroup discovery, relevance feedback / active learning, cheap classifiers distilled from LLM judgments) have been shown to improve retrieval benchmarks, or to make agentic retrieval much cheaper.
>
> Find out, with sources:
> 1. Evidence that set-oriented techniques improve measured retrieval, especially for scientific/medical literature: results of automatic boolean query refinement/formulation on systematic-review collections (Scells & Zuccon, WWW 2019 and 2020; CLEF eHealth TAR 2017–2019; CSMeD; the Wang et al. seed-studies collection); LLM-generated boolean query results (recall and precision vs expert queries); interactive/clustering search evaluations (Scatter/Gather-style) if any modern ones exist.
> 2. State of the art on agentic literature-search benchmarks: LitSearch, PaSa's RealScholarQuery, SPAR/SPARBench, ScholarQA-CS / ScholarQABench, LitQA2, BrowseComp-Plus, BRIGHT, and any 2025–2026 benchmarks for exhaustive/high-recall literature search. What do the top systems do (BM25 + dense, rerankers, LLM judgments, citation expansion), what do they score, and what do they cost per query (tokens, dollars, latency) where stated?
> 3. Work on the cost-efficiency of deep research or agentic retrieval: cascades, distilling LLM relevance judgments into small classifiers or rerankers, budget-aware agents, early stopping, with numbers (cost reduction at what quality loss).
> 4. High-recall retrieval with LLMs + active learning: TREC Total Recall-style continuous active learning (Cormack & Grossman) combined with LLMs; LLM-assisted TAR; stopping rules; any cost/recall numbers.
>
> Return a report under 1,200 words: a list of findings, each with the URL, date/venue, what was measured, the key numbers, and whether you read the primary source or only a search summary. Say plainly "not found" where you searched and found nothing — absence is a useful answer (e.g., "no benchmark result found for agents given facet/count tools"). Do not invent systems, numbers or citations; flag anything uncertain.
