# Research report 1: suggested cuts and a result set's shape in the project's own notes

**What this is:** the final report of a read-only research subagent (a
repository explorer, no web access used), delivered on 2026-09-28 to the
conversation that wrote [suggested cuts](../suggested-cuts.md) and
[the shape of a result set](../shape-of-a-result-set.md), and kept
verbatim, following the practice the user asked for with
[searching within a result set](../search-within-results.md)'s reports.
It is model output, not checked line by line. It used 43 tool calls over
about 3.5 minutes. Its `file:line` citations point into the repository
as it stood on branch `worktree-search-within-results` that day. The
prompt it was given follows the report.

---

All paths are under /Users/thomasingalls/Code/ol-rht/.claude/worktrees/search-within-results/. Short names used below:
- SWR = docs/inbox/search-within-results.md
- RASB = docs/inbox/recall-accounted-set-builder.md
- RB = docs/inbox/retrieval-benchmarks.md
- ABC = docs/inbox/ambitions-beyond-citations.md
- CL = docs/inbox/competition-list.md
- RF = docs/inbox/retrieval-frontier.md
- FA = docs/inbox/future-ambitions.md
- EP = docs/evaluation-plan.md
- SP = docs/serving-plan.md
- GP = docs/graph-plan.md

**1. For (A), suggested cuts**
- SWR §11.1 is already the idea. Beam search over probes, facet values and clusters, shown as "readable chains, each with its count" and named as subgroup discovery or CN2 (SWR:410-418). It lists four candidate scores: even split, unlike the parent, close in vector space, agrees with marks (SWR:420-426). A chain scored against marks "writes a boolean query that agrees with the user's judgements"; it "overfits" when marks are few (SWR:432-442). Two traps are named, near-duplicates (DSSD) and chance (SWR:444-453). The branch mode is SWR §11.2 (SWR:455-462), and the two combined are "Scatter/Gather's loop… with readable steps" (SWR:466-469).
- Prior art (SWR:484-552):
  - Power BI "top segments": greedy, not a beam.
  - Scells and Zuccon's query refinement, WWW 2019: "a beam of width one". Precision went 0.021→0.042 and recall 0.84→0.67.
  - searchrefiner.
  - ASReview and legal TAR (screening with a trained classifier, not beam search).
  - Clusty.
- Why these tools are rare in general search (SWR:557-589). The notes say none combining the pieces was found, from "not a thorough" search.
- "Did you mean" is defined as "the facet distribution over the top results (MeSH, topic, method) offered as refinements" (ABC:139-143).
- Boolean search: the running chain is ANDs over an OR group (SWR:176-180). A stack of steps should show five kinds of feedback: samples kept, samples dropped, counts and overlaps, leave-one-word-out counts, and significant terms (SWR:186-204). Researchers "build a strategy one numbered line at a time" in Ovid or PubMed history (SWR:168-172).
- LLM-written boolean queries raise precision and lose recall, and "None of it lets the model iterate on hit counts" (SWR:692-731, 756-759). Agents with facet or count tools: "Not found" (SWR:686-688). The nearest are OpenAlex's `group_by` and openalex-mcp-server (SWR:668-676).
- Cost of counting: "Counting every match" visits every matching document, with no top-k pruning; the time limit bounds it (SP:2066-2071; SWR:927-928). Recompute or hold the set: recompute first (SWR:919-926).
- RASB puts suggested cuts behind the set builder: "the good score is agreement with marks, which leads back to the classifier" (RASB:227-229).
- No competitor in CL is noted for facet or refinement suggestions.

**2. For (B), shape**
- The pipeline is sample, cluster, name or leave unnamed, then assign by nearest cluster. "The set's size hardly matters". BERTopic and Scatter/Gather are cited (SWR:251-274). Vectors are "a lens, not the net" (SWR:276-285).
- "Unlabelled clusters are acceptable because the user sees example works, not labels" (ABC:171-173; echoed at SWR:262-264). Naming clusters is open (SWR:953-954).
- Embeddings:
  - SPECTER is "pretrained on the citation graph"; SPECTER2 on 6M citation triplets (RF:172-178).
  - Semantic Scholar's `embeddings-specter_v2` has 1.2×10^8 records under Apache 2.0 and needs an API key (RF:193-197; SWR:929-933).
  - Whether S2's embeddings replace our own is an open question (ABC:227-229).
  - "Vectors are not neutral either" (SWR:294-298).
  - Works without abstracts skew the shape (SWR:934-935).
  - Embeddings would be a fifth artifact, and a query encoded by another model is a hard failure (FA:145-149).
- Co-citation and coupling live in the serving plan, not the graph plan. `related(work, by)` offers `cited-alongside` and `cites-same`, ranked by Salton's cosine (SP:347-376, 758-786). Measured: raw overlap is "nearly no signal", with 911,071 candidates touched by one seed against 39,951 by two (SP:774-780). The graph plan only defers to the candidate manager (GP:38-42). The frontier score includes both measures (docs/inbox/candidate-management.md:40). "The works this set cites most" as a facet is at SWR:910-912.
- Competitors:
  - CL gives only one-liners: Connected Papers "visual exploration" (CL:157), Litmaps "citation maps" (CL:171), Open Knowledge Maps "topic maps" (CL:174), ResearchRabbit "concept maps" (CL:186).
  - Litmaps bought ResearchRabbit (docs/inbox/science-assistant-landscape.md:158-160).
  - Inciteful, VOSviewer and CiteSpace are not in the notes.
- Evidence is weak: citation clusters on 25 reviews were "highly variable" (SWR:765-769; RASB:230-234). Hearst and Pedersen 1996: the top cluster held at least half the relevant documents, and users picked the best cluster 31 of 38 times (SWR:761-765).

**3. Benchmarks**
- Systematic-review collections: CLEF TAR (RB:116-122), CSMeD (RB:123-127), the Seed collection (RB:221-224), SR4CS (RB:213-216) and SYNERGY (RB:298). CLEF TAR is not in EP's published sets and "adding one would come first" (SWR:895-896; RASB:244-248).
- Aspect or facet sets: DORIS-MAE, relevance per aspect but model-labelled, listed as a poor fit (RB:146-153, 235). CSFCube, query by example with a facet (RB:154-159).
- TREC Genomics 2004–05 ad hoc is in (RB:295; EP:444). 2006–07 was dropped as "passages of full text" (RB:305). Genomics aspect judgements are not in the notes.
- Diversity measures (α-nDCG, subtopic recall) are not in the notes.
- Interactive or session evaluation: "The published sets score ranked lists, not sessions of narrowing" (SWR:949-952). "Facets, counts… suggested cuts: no benchmark result at all" (SWR:793-795).
- EP has no session or simulated-user evaluation. Its only simulation is E5, three recency mechanisms (EP:461-464). The testable agent-arm design is at SWR:857-893.

**4. What the index does today**
- One pass does hits, count and facets: `TopDocs`, `Count` and `facet_collector()` (crates/search/src/index.rs:447-494). This is also noted at SP:1309-1310.
- `facets_of_all` gives the corpus baseline (index.rs:501-505). Facets return top-k values per root (index.rs:509-534).
- There are 7 facet roots: year, form, genre, language, status, held_by, access (crates/search/src/schema.rs:85-93). Venue is text only (schema.rs:20-21, 44); it becomes a facet "once it has an id" (SP:1447-1448).
- The parser is plain text only. Words are joined as a flat OR (`Occur::Should`) (crates/search/src/translate.rs:9-13, 67; crates/query-parser/src/lib.rs:13-20).
- AND/OR/NOT and facets as filters wait on SP item 36 (SP:2013-2026). Any-word versus every-word matching is undecided (SP:1468-1469).
- MeSH is not indexed (SWR:138-141).

**5. Constraints**
- An answer key must be independent of what is tested (EP:35-37, 207-224). MeSH may judge a user's definition without breaking this (SWR:246-249). ScholarQuest's key is not independent (SWR:784-788).
- No napkin estimates: "never a napkin estimate again" (GP:565). Plans keep only measured counts (GP:681-682). FA leaves napkin math out on purpose (FA:8-10).
- Compromises ledger: "keeping track of what we're compromising on" (EP:664-668).
- The box has 128 GB shared with the search engine and is "disk-heavy and memory-modest" (SP:85-93). Full-precision vectors "do not fit", so compression is mandatory (FA:177-181). Vectors would break byte-identical rebuilds (FA:182-187).
- Held-back marks, the "frozen half… inside the product" (SWR:330-332).
- Sending abstracts to a hosted classifier is a terms risk (SWR:384-388).
- Both notes are speculative and amend no plan (SWR:10-11; RASB:7-8).

---

## The prompt it was given

> Read-only research in the repository at /Users/thomasingalls/Code/ol-rht/.claude/worktrees/search-within-results (do not modify anything). The project is a scholarly-literature search engine (BM25 over titles/abstracts via Tantivy, a citation graph, facets counted over every match). Two tool ideas are being written up: (A) "suggested cuts" — beam search / subgroup discovery over narrowing steps (clauses, facet values, clusters, labels) of a large result set, scored by generic interestingness or by agreement with a user's marks, plus a mode where the user keeps several refinement branches open; (B) "shape of a result set" — cluster a sample of a result set's embedding vectors, name the clusters, assign the rest, show years/venues/distinctive words.
>
> Read (medium thoroughness; skim for relevant parts): docs/inbox/search-within-results.md (especially §6, §8, §11, §12), docs/inbox/recall-accounted-set-builder.md, docs/inbox/retrieval-benchmarks.md, docs/inbox/ambitions-beyond-citations.md (§5, §6), docs/inbox/competition-list.md, docs/inbox/industry-landscape.md, docs/inbox/product-shapes.md, docs/inbox/retrieval-frontier.md, docs/inbox/future-ambitions.md, docs/inbox/text-analysis.md, docs/graph-plan.md, docs/evaluation-plan.md, docs/serving-plan.md (search for facet, count, cluster, co-citation, coupling, embedding).
>
> Report, citing file:line for every claim:
> 1. Anything relevant to (A): facets or refinement suggestions, "did you mean", query refinement, boolean queries, rule learning, faceted counts and their cost, competitor products offering refinement or facet suggestions.
> 2. Anything relevant to (B): clusters, maps, topic models, embeddings (which models, which datasets, e.g. SPECTER/SPECTER2 and Semantic Scholar's embeddings dataset), co-citation or bibliographic coupling in the graph plan, literature-mapping competitors (Connected Papers, ResearchRabbit, Litmaps, Inciteful, VOSviewer, Open Knowledge Maps, etc.) and what the notes say about them, "unlabelled clusters acceptable" passages.
> 3. Benchmarks in the notes that could score either: subtopic/aspect judgements (e.g. TREC Genomics aspects), diversity measures, systematic-review collections, interactive or session evaluation; what the evaluation plan says about interactive/session evaluation or simulated users.
> 4. What the index does today that either tool would build on (facet roots, one-pass counting, the query parser's state), with file:line into crates/ if you can find it quickly (crates/search/src/index.rs, crates/search/src/schema.rs).
> 5. Any constraint or rule in the notes that either tool must respect (independent answer keys, no napkin estimates in plans, compromises ledger, the box's memory/disk limits).
>
> Keep the report under 1,000 words. Quote short phrases where precision matters. Do not speculate beyond the notes; say "not in the notes" where absent.
