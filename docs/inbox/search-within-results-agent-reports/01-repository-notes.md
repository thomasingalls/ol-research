# Research report 1: agents, tools and costs in the project's own notes

**What this is:** the final report of a read-only research subagent (a
repository explorer, no web access used), delivered on 2026-09-28 to the
conversation that wrote [searching within a result set](../search-within-results.md),
and kept verbatim at the user's request. It is model output, not checked
line by line; the note's §12 draws on it. It used 22 tool calls over
about 2.5 minutes. Its `file:line` citations point into `docs/inbox/`
as it stood on branch `worktree-search-within-results` that day. The
prompt it was given follows the report.

---

All files are in /Users/thomasingalls/Code/ol-rht/.claude/worktrees/search-within-results/docs/inbox/ and are cited below as file:line.

**1. Systems and the tools their agents get**
- Common loop across the systems (retrieval-frontier.md:16-30): a keyword/BM25 or Semantic Scholar API pass mixed with dense embeddings over rewritten queries, then a model reads every candidate, then citation expansion, repeated. The note says "the first-stage retriever is a commodity; the quality is in the reading" (:29-30).
- **PaperQA2:** Semantic Scholar search (12 candidates), a hybrid chunk embedding picking the top 30 chunks, and an LLM that summarizes and scores each 0–10 ("RCS"). Citation traversal only from papers scoring 8 or more (retrieval-frontier.md:36-49). The open-source paper-qa uses a local Tantivy index and has no traversal (:50-53).
- **OpenScholar:** a trained dense retriever, a BGE reranker, the S2 API and web search (:71-78).
- **Ai2 Scholar QA:** 0.6 embedding / 0.4 BM25, a cross-encoder keeping the top 50 (:79-84).
- **Paper Finder:** S2 keyword search, three dense snippet indices, rewritten queries, forward and backward citations. An LLM judges relevance "per sub-criterion", and ranking mixes in recency and citation count (:85-91). Asta's MCP offers search, citations, metadata and passages (competition-list.md:151).
- **Anthropic multi-agent:** subagents act as "intelligent filters" (retrieval-frontier.md:99-105). OpenAI Deep Research gets browsing and Python (:106-109); Scholar Labs searches per topic (:118-121).
- **Undermind:** candidates from embeddings, citations and model reasoning. GPT-4 sorts each paper into three classes, and a discovery curve decides when to stop (:126-132). Its MCP offers paper/citation search, bulk resolution and parallel PDF reading (competition-list.md:194).
- **Consensus:** embeddings plus BM25, quality signals, and a larger model reranking the top 20. Deep Search runs up to 20 searches over 1,000+ papers (retrieval-frontier.md:138-144; industry-landscape.md:233-236).
- **scite:** a deep-learning classifier labels citation statements (retrieval-frontier.md:145-151), used "for labels, not traversal" (:231-232).
- **Others:** scienceOS does query generation and citation-network expansion (undermind-alternatives.md:26). Mistral does iterative search and "pattern lookup" (:56). Iris.ai extracts concepts "for filtering" (competition-list.md:168).
- **Absent everywhere:** facets or filters exposed to an agent, hit counts, boolean queries, clustering of a result set, subgroup discovery. None are in the notes for any system.

**2. Costs**
- Anthropic's system uses about 15× a chat's tokens, and token usage explained 80 % of the variance (retrieval-frontier.md:103-105).
- Kosmos costs $200 a run (science-assistant-landscape.md:179) and reads about 1,500 papers a run (retrieval-frontier.md:59-60). A per-paper figure is not stated.
- Consensus's API is $0.05 a call (industry-landscape.md:226-227). OpenAlex gives $1 free a day, then usage pricing (science-assistant-landscape.md:435-436).
- Jev costs $0.042 per million input tokens (future-ambitions-tracer-engine.md:44-45).
- BRANE: characterizing a query costs about $0.001, and profiling cost about $11,000 (future-ambitions-configuration.md:70-72). Cost per query spans 3–4 orders of magnitude (:28-30).
- Where the cost goes: "The reading loop is where the frontier spends" (retrieval-frontier.md:224). Full text, evidence extraction and reranking, "not the graph" (candidate-management.md:56-61). Building a concept graph costs "every model call" (future-ambitions.md:302-305).

**3. Set / exhaustive-recall benchmarks and top scores**
- **CLEF TAR** is "the standard test of recall… whether all of it was found" (retrieval-benchmarks.md:57-59). It covers screening prioritisation of what a Boolean query retrieved (:116-122). No BM25 figure was gathered (:48).
- Other systematic-review sets:
  - CSMeD, 325 reviews (:123-127).
  - Seed Collection, 40 reviews with seeds and Boolean queries (:221-224).
  - SR4CS, expert Boolean queries; BM25 MAP 0.175, R@100 0.262 (:213-216).
  - SYNERGY, 26 reviews (:298).
- AutoResearchBench "Wide" is scored by set overlap, IoU (:347-350).
- The notes give no system scores on CLEF TAR, CSMeD, SR4CS, SYNERGY or the Seed Collection.
- Top scores on literature-search benchmarks:
  - AutoResearchBench: 9.39 % Deep and 9.31 % IoU Wide (:353-354).
  - LitQA2 (PaperQA2): 85.2 % precision, 66.0 % accuracy (retrieval-frontier.md:48-49).
  - LitSearch (Paper Finder): 89 % "perfectly relevant" (:91).
  - ScholarQA-CS2: about 85 % or more (retrieval-benchmarks.md:386-388).
  - AstaBench: Asta v0 53.0 % against ReAct 43.3 % (science-assistant-landscape.md:501).

**4. Narrowing, counts, facets, boolean queries, active learning, TAR**
- The repo's own funnel runs hard filters, then cheap scoring, then a bounded frontier of "100–1,000", then expensive reasoning over "10–100" (candidate-management.md:14-24). It is run best-first (:29-43).
- An agent front door is proposed: "expand, filter, score, keep the top *k* — as one server call, so an agent reads only the short list" (future-ambitions.md:166-169). Also "then the facet filters" (:150-152) and "a question turned into filters" (:160).
- LLM-generated boolean queries: not in the notes. Boolean queries appear only as expert-written ones in the benchmarks (retrieval-benchmarks.md:119-120, 214, 224).
- Counts used as feedback: not in the notes. The nearest thing is Undermind's discovery-curve stopping (retrieval-frontier.md:130-131, 239-240).
- Active learning is not named. The nearest is a capable model labelling "a couple of hundred" candidates, then a SetFit-style classifier labelling the rest (future-ambitions-tracer-engine.md:226-228).
- "Technology-assisted review" appears only as CLEF TAR (retrieval-benchmarks.md:48, 116-118). Clustering appears only as S2's "embedding clusters" (future-ambitions.md:137). Subgroup discovery: not in the notes.

**5. HarnessTax**
Seven models were run in Claude Code, Codex CLI and Pi on SWE-bench Lite and Terminal-Bench 2.0. Success varied only about ±2 % / ±5 % across harnesses while cost varied up to 5× (future-ambitions-configuration.md:84-97). The four-tool Pi reached the cost–success frontier, and Claude Code's initial context was over ten times Pi's (:98-102). The note's reading is that a front door's tool schemas and answer size are "paid by every agent on every call" (:189-196). It adds a ceiling-effect caveat (:136-138).

**6. Passages arguing cheap operations should replace expensive reading**
- Pricing like an embedding "makes judging every candidate cheaper than choosing which to judge" (future-ambitions-tracer-engine.md:155-158).
- The cascade "costs close to all-small and scores close to all-capable" (:178-181).
- "One expensive call and thousands of cheap ones" (:217-218).
- BRANE is "evidence for the cheap route" (future-ambitions-configuration.md:170-175). Its classical predictors beat fine-tuned 4B models (:66-68).
- The query path could become a costed menu where reading is "none, the classifier cascade… or a capable model" (:157-166).
- The capable model reads the trail, "never the works themselves" (future-ambitions.md:241-242).
- Among the edges product-shapes.md lists: "Speed and cost low enough to call on everything" (product-shapes.md:46).
- On counting and filtering specifically, the only passages are the funnel and the one-call front door above (future-ambitions.md:166-169; candidate-management.md:14-24). No passage argues that counting should replace reading.

---

## The prompt it was given

> Read-only research in the repository at /Users/thomasingalls/Code/ol-rht (do not modify anything). The project is a scholarly-literature search engine (BM25 over titles/abstracts via Tantivy, a citation graph, facets). The user is asking whether AI literature/deep-research agents are given "set-oriented" search tools — search within a result set, hit counts, facets, filters, boolean queries refined with count feedback, clustering of a result set, subgroup discovery, cheap classifiers over a set — and whether such tools are known to help on retrieval benchmarks or cut cost.
>
> Read these notes (medium thoroughness; skim for relevant parts): docs/inbox/retrieval-frontier.md, docs/inbox/science-assistant-landscape.md, docs/inbox/undermind-alternatives.md, docs/inbox/retrieval-benchmarks.md, docs/inbox/future-ambitions-configuration.md, docs/inbox/future-ambitions.md, docs/inbox/future-ambitions-tracer-engine.md, docs/inbox/product-shapes.md, docs/inbox/industry-landscape.md, docs/inbox/competition-list.md, docs/inbox/candidate-management.md.
>
> Report, citing file:line for every claim:
> 1. Which literature / deep-research agent systems the notes describe, and what tools each system gives its agent (keyword/BM25 search, vector search, citation expansion, filters or facets, hit counts, boolean queries, clustering, per-candidate LLM relevance judgments, cheap classifiers). Say "not stated" where the notes don't say.
> 2. Any stated costs (per query, per paper read, tokens, dollars) and where the notes say cost goes.
> 3. Any benchmark in the notes that measures set retrieval or exhaustive recall (systematic-review collections such as CLEF TAR, CSMeD, seed-study collections) as opposed to ranked retrieval; and what the notes say top systems score on literature-search benchmarks.
> 4. Anything about agents narrowing a large result set, using counts or facets, LLM-generated boolean queries, active learning, or technology-assisted review.
> 5. What the "HarnessTax" finding in future-ambitions-configuration.md is, in two or three sentences.
> 6. Any passage that already argues cheap operations (counting, filtering, classifiers) should replace expensive LLM reading.
>
> Keep the report under 900 words. Quote short phrases where precision matters. Do not speculate beyond the notes; say "not in the notes" where absent.
