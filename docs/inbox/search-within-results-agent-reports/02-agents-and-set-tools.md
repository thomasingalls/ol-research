# Research report 2: do agents get set-oriented search tools?

**What this is:** the final report of a web-research subagent, delivered
on 2026-09-28 to the conversation that wrote
[searching within a result set](../search-within-results.md), and kept
verbatim at the user's request. It is model output, not checked line by
line; the note's §12 draws on it. It used 102 tool calls (web searches
and page fetches) over about 15 minutes. Its tags are its own: [P] it
read the primary paper, docs or code; [A] abstract or landing page only;
[S] a search engine's summary or a third-party write-up only. The prompt
it was given follows the report.

---

Tags: [P] = I read the primary paper, docs or code. [A] = abstract or landing page only. [S] = search-engine summary or a third-party write-up only.

**Short answer:** I found no literature agent that has the full professional-searcher toolkit. The closest I found:
- OpenAlex's API (hit counts plus group_by), exposed to agents through a community MCP server.
- Undermind, which estimates how many relevant papers exist.
- PaperQA, whose agent sees counts of its own working set.
- Consensus, whose agent applies filters.

Everything else is search(query) → top-k, plus citation expansion and LLM reading. I found no published systematic-review Boolean-query work where the LLM iterates using hit counts.

## 1. LLM Boolean queries for systematic reviews
- **Wang, Scells, Koopman, Zuccon**, SIGIR 2023, https://arxiv.org/abs/2302.03495 [P]. The refinement prompt gives ChatGPT only the review title and the existing query. It sees no results and no counts, and "multiple interactions" means chat turns. Result: higher precision and lower recall than the baselines; refinement cost about 11% recall.
- **Staudinger et al.**, SIGIR-AP 2024, https://arxiv.org/abs/2411.14914 [S]. Replication got recall 0.139 where the original reported 0.504 (CLEF TAR, prompt p4).
- **Wang et al., reassessment**, SIGIR 2025, https://arxiv.org/abs/2505.07155 [P]. Tested 10 LLMs, with no feedback loop.
  - CLEF TAR: manual queries P 0.021 / R 0.844; GPT-3.5 R 0.51; o1 R 0.65.
  - Seed collection: manual R 0.724; guided-combined 0.736.
  - Most of the Staudinger gap comes from not validating malformed queries.
- **AutoBool**, EACL 2026, https://arxiv.org/abs/2602.00005 [P]. Qwen3-4B trained with RL; the reward comes from running its queries on PubMed during training. At inference it writes one query, with no loop.
  - Recall 0.70, against GPT-4o 0.44 and o3 0.69.
  - Claims "10–16× fewer documents" than expert queries at near-expert effectiveness. The body says 17× on CLEF, so the numbers conflict slightly.
- **DeepRetrieval**, UIUC, arXiv Mar 2025, https://arxiv.org/abs/2503.00223 [P]. RL against live PubMed and ClinicalTrials.gov, rewarded on Recall@3K. Single turn. Publication recall 65.1%, against LEADS 24.7% and GPT-4o 17.6%.
- **TrialMind**, npj Digital Medicine, Aug 2025, https://pmc.ncbi.nlm.nih.gov/articles/PMC12331930/ [P]. The only one I found where the LLM sees results: the abstracts retrieved by its first query feed one refinement pass. Recall 0.78 against a human baseline of 0.19; that baseline is weak (built by hand from UMLS terms).
- **Chained prompting**, AUB, arXiv Nov 2025, https://arxiv.org/html/2602.00011 [P]. No loop. Recall 0.87 against LEADS 0.24.
- **LITERAS**, Computers in Biology and Medicine 2025, https://pubmed.ncbi.nlm.nih.gov/40383055/ [S]. A critic agent re-runs PubMed searches until enough papers pass, averaging 2.2 loops. It counts relevant papers, not hits.
- **Outside systematic reviews:**
  - Clarke & Smucker, "Boolean queries are all you need?", Jul 2026, https://arxiv.org/abs/2607.11362 [P]. A GPT-5.5 agent with a Boolean engine sees one ranked snippet at a time and no hit counts. NDCG@10 0.686 on TREC RAG 2024, with 100 calls per topic.
  - Sieve (Wang, Zuccon et al.), Aug 2026, https://arxiv.org/abs/2608.02751 [A]. Field-scoped Boolean filters, then a ranker, then result cards. Uses 20.7–50.6% fewer tokens. Whether it shows hit counts is not stated.
- **Not found:** an LLM refining a systematic-review Boolean query line by line from hit counts.

## 2. Literature and deep-research agents
- **PaSa**, ACL 2025, https://arxiv.org/abs/2501.10120 [P]. Tools are Search (Google restricted to arXiv and a cutoff date), Expand (citations from one named section) and Stop. No counts, filters or Boolean syntax. About 382 Search+Expand actions per query (Table 8), and the Selector LLM reads every queued paper. RealScholarQuery: recall 0.61, precision 0.51.
- **Ai2 Paper Finder**, blog Mar 2025, https://allenai.org/blog/paper-finder [P]. Extracts author, venue and year criteria and routes to metadata sub-flows. Runs several reformulations over three dense snippet indices plus Semantic Scholar keyword search, then forward and backward citation tracking. The LLM judges each sub-criterion separately. Stops when it has enough or has scanned too many; no number is given.
- **Asta corpus MCP**, https://allenai.org/asta/resources/mcp [P]. Seven tools with date-range, venue and paper-id filters. No totals, facets or Boolean syntax documented.
- **Undermind whitepaper**, Jan 2024, https://www.undermind.ai/whitepaper.pdf [P]. GPT-4 classifies each candidate from its full text; a typical search evaluates 150 papers. It fits a discovery curve f = 1 − e^(−n/τ) to estimate the total number of relevant papers (median 24, τ = 80). 150 papers finds about 85%, 300 about 98%. It covered arXiv only at the time.
- **PaperQA2**, arXiv Sep 2024, plus code at https://github.com/Future-House/paper-qa [P].
  - Tools: `paper_search(query, min_year, max_year)`, `gather_evidence`, `gen_answer`, `clinical_trials_search`; the paper also describes citation traversal.
  - The agent sees a status line: "Paper Count / Relevant Papers / Current Evidence". Those are counts of its own working set.
  - The trials tool alone reports "among {total} total results".
- **OpenScholar**, Nature Feb 2026 [S]. Top-k passages from 45M papers, with a self-feedback loop that triggers more retrieval. No facets.
- **Elicit**, blog Feb 2025 [P]. Imported database results plus up to 500 papers from semantic search, then LLM screening against editable criteria. No Boolean syntax or facets mentioned.
- **Consensus Deep Search** [S; the docs returned 403]. Up to 20 searches, citation traversal, and agent-applied filters (minimum citations, study type). Reviews 1,000+ papers and reads 50–100.
- **SciSpace Deep Review** [S, a third-party blog]. Example run: 1,750 searched, about 320 relevant. Stops when relevant papers stop increasing.
- **OpenAI deep research API**, https://developers.openai.com/api/docs/guides/deep-research [P]. Web search, file search and a code interpreter. An MCP server is usable only if it implements `search(query)` and `fetch(id)`, which is exactly the top-k interface.
- **Gemini Deep Research**, https://ai.google.dev/gemini-api/docs/deep-research [P]. Google Search, URL context and code execution by default; MCP and File Search optional.
- **PaperPilot**, Jul 2026, https://arxiv.org/abs/2607.00597 [A]. The agent builds a DAG of operators: keyword search, citation expansion, filter, score, rerank. Hit@5 rose from 58 to 77.

## 3. Faceted, aggregate and set tools elsewhere
- **OpenAlex LLM Quick Reference** (updated 2026-09-21), https://help.openalex.org/api/llm-quick-reference/ [P]. Teaches agents filter syntax and `group_by`. The community server https://github.com/cyanheads/openalex-mcp-server [P] returns `meta.count` on every search and has a group-by tool (up to 200 groups). This is infrastructure; nobody has evaluated it.
- **Elastic Agent Builder** [S]. Agents write ES|QL queries, which support aggregations.
- **GenFacet**, SIGIR 2026 [S]. LLM-generated facets for human shoppers, not for agents.
- **AGGBench/DFA**, Feb 2026, https://arxiv.org/abs/2602.01355 [S]. Benchmark for "find all" aggregation queries; a disambiguate–filter–aggregate agent beats RAG.
- **WideSearch**, Aug 2025, https://arxiv.org/abs/2508.07999 [P]. Set-collection tasks with only search and read tools. Best agent succeeds 5.1% of the time.
- **Cheap classifiers over a whole set:**
  - LOTUS semantic operators, VLDB 2025 [S]: cheap proxy models plus statistical accuracy guarantees.
  - TnT-LLM, KDD 2024 [S]: LLM pseudo-labels train lightweight classifiers that run at scale.
  - Zhong et al., NeurIPS 2024 [S]: clusters and classifiers described by natural-language predicates.
  - SSD-LLM, ECCV 2024 [A]: finds subpopulations from image captions.
- **Not found:** an LLM combined with classical subgroup discovery (beam search over conjunctions). Also not found: evaluated research giving agents facet-count tools.

## 4. LLMs in screening and technology-assisted review
- **Wang et al.**, ECIR 2024, https://arxiv.org/abs/2401.06320 [A]. Eight zero-shot LLMs, with a threshold calibrated to a target recall. An ensemble saves screening time against the state of the art. I did not capture numbers.
- **Bron et al.**, IAL@ECML-PKDD 2024, https://ceur-ws.org/Vol-3770/paper8.pdf [P]. ChatGPT-3.5 labels each record against each criterion. Their LLM+CAL first runs AutoTAR over the LLM-included records, then switches to full CAL. One dataset only (4,836 records, 183 included):
  - LLM alone: recall 91.3%, WSS 64.5%.
  - WSS@95: LLM+CAL 80.5%, plain AutoTAR 71.4%.
- **Akinseloyin et al.**, medRxiv Aug 2025 [S]. LLM scores become top and bottom pseudo-labels that seed active learning. 28 reviews; 42–43% workload saved at near-perfect recall.
- **ASReview LAB v2**, Patterns Jul 2025 [P]. Its "agents" are feature-extractor-plus-classifier models, not generative LLMs, and the authors keep humans as the final judges.
- **otto-SR**, medRxiv Jun 2025 [S]. A GPT-4.1 screener run on the original search results: sensitivity 96.7%, against 81.7% for dual human review.
- **Mao/Zuccon**, SIGIR 2024 and DenseReviewer ECIR 2025 [S]. Dense retrieval with continuous feedback; no LLM.
- **Not found:** peer-reviewed work on TREC Total Recall-style CAL with LLMs. E-discovery has vendor whitepapers only.

---

## The prompt it was given

> Web research task (read-only; do not create or modify any files). Today is 2026-09-28. Context: a user building a scholarly-literature search engine wants to know whether anyone is giving AI agents (LLM literature-review / deep-research agents) "set-oriented" search tools, the kind professional searchers use: searching within a result set, hit counts per query or per clause, facets and filters, boolean queries built line by line and refined using hit-count feedback, clustering a result set, subgroup discovery (beam search over conjunctions of conditions), relevance feedback / active learning, cheap classifiers applied over a whole set.
>
> Find out, with sources:
> 1. LLMs generating or refining boolean queries for systematic-review literature search: e.g. Wang, Scells, Koopman, Zuccon, "Can ChatGPT Write a Good Boolean Query for Systematic Review Literature Search?" (SIGIR 2023) and follow-ups through 2026 (reproducibility studies, agentic versions). Key question: does any of this work let the LLM see hit counts or retrieved results and iterate (an agent loop), and what recall/precision did it get versus human queries?
> 2. Literature / deep-research agents and what tools they expose to the model, per their papers or docs: PaSa (paper search agent), Ai2's PaperFinder / Asta, Undermind, Elicit, FutureHouse PaperQA2 / Edison, OpenScholar, SciSpace, Consensus, OpenAI deep research, Google/Gemini deep research, and any 2025–2026 systems you find. For each: does the agent get filters/facets, counts, boolean syntax, citation expansion, clustering, or only "search(query) → top-k"? How many papers does it read with an LLM per question, if stated?
> 3. Any research on giving LLM agents faceted search, aggregation or count tools in any domain (e-commerce, enterprise search, data exploration), or combining LLMs with subgroup discovery / pattern mining.
> 4. LLMs in systematic-review screening or technology-assisted review (TAR): LLM + active learning, LLM relevance judgments replacing or seeding classifiers, e.g. work by Wang/Scells/Zuccon, ASReview + LLM, TREC Total Recall-style CAL with LLMs.
>
> Return a report under 1,200 words: a list of findings, each with the URL, date/venue, exactly what the system or method does, the key number if any, and whether you read the primary source (paper/docs) or only a search summary. Say plainly "not found" where you searched and found nothing — absence is a useful answer. Do not invent systems, numbers or citations; flag anything uncertain.
