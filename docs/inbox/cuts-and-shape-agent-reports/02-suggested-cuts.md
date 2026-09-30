# Research report 2: suggested cuts, prior work

**What this is:** the final report of a web-research subagent, delivered
on 2026-09-28 to the conversation that wrote [suggested cuts](../suggested-cuts.md),
and kept verbatim, following the practice the user asked for with
[searching within a result set](../search-within-results.md)'s reports.
It is model output, not checked line by line. It used 94 tool calls (web
searches and page fetches) over about 14 minutes. Its tags are its own:
[P] it read the full text or the result tables; [A] abstract or landing
page only; [S] a search engine's summary only. The prompt it was given
follows the report.

---

**Suggested cuts: prior work (read-only research, 2026-09-28)**

Tags: [P] I read the full text or the result tables; [A] abstract or landing page only; [S] search-engine summary only.

**1. Subgroup discovery**
- **CN2-SD**, Lavrač et al., JMLR 5, 2004 [P] https://www.jmlr.org/papers/volume5/lavrac04a/lavrac04a.pdf
  - Defines WRAcc = p(Cond)·(p(Class|Cond) − p(Class)). It uses weighted covering: rows already covered by earlier rules count less.
  - On 23 UCI data sets it gave far fewer rules, higher coverage and significance, and slightly better AUC than CN2.
- **Survey:** Kralj Novak, Lavrač & Webb, JMLR 2009 [A] https://www.jmlr.org/papers/v10/kralj-novak09a.html. It puts contrast sets, emerging patterns and subgroup discovery in one framework.
- **DSSD**, van Leeuwen & Knobbe, DMKD 25, 2012 [A] https://link.springer.com/article/10.1007/s10618-012-0273-y
  - Three levels of redundancy (description, cover, compression). A selection step for each level runs inside the beam search.
  - Evaluated on cover diversity, best quality found and runtime.
  - The full text is paywalled. The defaults (beam width 100, depth 5) come from Proença et al. [P].
- **SSD++**, Proença, Grünwald, Bäck & van Leeuwen, DMKD 2022 [P] https://arxiv.org/abs/2103.13686
  - Builds subgroup lists under MDL. The greedy gain equals a Bayesian test plus a multiple-testing penalty.
  - Its comparison table marks CN2-SD and DSSD as having no statistical guarantees.
- **Interactive (your human-scorer mode):**
  - Dzyuba & van Leeuwen, IDA 2013 [A] https://link.springer.com/chapter/10.1007/978-3-642-41398-8_14. At each beam level the user likes or dislikes subgroups. The feedback prunes the beam and re-weights the quality measure. This is almost exactly your second mode.
  - Divisi, CHI 2025 [A] https://arxiv.org/abs/2502.10537. Think-aloud study with 13 practitioners; no numbers.
- **Exceptional model mining** [A]: Leman, Feelders & Knobbe, ECML PKDD 2008; Duivesteijn, Feelders & Knobbe, DMKD 30, 2016 https://dl.acm.org/doi/10.1007/s10618-015-0403-4. It finds subgroups where a model fitted to the subgroup differs from the model fitted to everything.
- **Text:**
  - Hendrickson, Wang & Atzmueller, ISMIS 2018 [S] https://link.springer.com/chapter/10.1007/978-3-030-01851-1_44. Subgroup discovery over topic-model distributions: 500 raters describing photos of 200 people.
  - Hedwig [S] https://github.com/anzev/hedwig. Rule search guided by an ontology hierarchy, used in biomedicine; relevant to MeSH.
  - Subgroup discovery applied to search result sets or bibliographic retrieval: **not found**.

**2. Readable Boolean queries learned from labels**
- **Kim, Seo & Croft, SIGIR 2011** [P] https://ciir-publications.cs.umass.edu/getpdf.php?id=974
  - Method: a C4.5 decision tree separates the top-100 pseudo-relevant documents from the rest. Each positive root-to-leaf path becomes an AND query with NOTs, giving about 176–238 candidates per topic. Candidates are ranked by query-quality predictors.
  - Data: USPTO patents (981,948 documents, 100 topics) and OHSUMED (348,566 documents, 106 topics). The evaluation is simulated: the user runs the suggestion unmodified and reads the top 100.
  - Best of the top 10 suggestions, R@100: 0.2991 vs baseline 0.2557 on patents; 0.4411 vs 0.4377 on OHSUMED.
  - The top-1 suggestion was worse than the baseline (0.2227; 0.3068). Only 4.6–7.8% of generated queries matched baseline recall.
- **Scells, Zuccon & Koopman, IR Journal 2021** [P] http://ielab.io/publications/pdfs/scells2020comparison.pdf
  - Compares automated versions of the "objective" and "conceptual" methods on CLEF TAR 2018 (75 reviews). Seed studies are split into development, validation and unseen parts, with 30 random splits per topic.
  - No automatic method beat the experts' queries. Experts: recall 0.934, precision 0.0217. Best automatic recall: 0.878 at precision 0.0002. Best automatic precision: 0.0053.
  - Objective beats conceptual, but conceptual queries are the better starting point for manual refinement.
  - Their follow-up refines queries by transformations ranked with learning to rank (WWW 2019) [S] https://dl.acm.org/doi/fullHtml/10.1145/3308558.3313544.
- **Hedges (Haynes et al.)**, BMC Medicine 2004, prognosis filters [A] https://pmc.ncbi.nlm.nih.gov/articles/PMC441418/
  - 49,028 hand-rated articles from 161 journals; 5,395 candidate terms, 4,862 unique.
  - Best balanced strategy: 83% sensitivity, 83% specificity. These filters are built into PubMed Clinical Queries.
  - This is the PubMed precedent for Boolean filters fit to labelled sets.
- **Database analogues:**
  - Query-by-Output, SIGMOD 2009 [S] https://cs598.github.io/papers/qbo.pdf. A decision tree reconstructs a selection query from example output.
  - Interactive Set Discovery, EDBT 2023 [A] https://arxiv.org/abs/2111.09917. Searches for a decision tree that reaches the target set with the fewest accept/reject questions.
- **LLMs writing Boolean queries:**
  - Wang et al., SIGIR 2023 [A] https://arxiv.org/abs/2302.03495. ChatGPT queries had high precision and lower recall.
  - SIGIR 2025 reassessment [S] https://arxiv.org/abs/2505.07155. Results depend strongly on the model and prompt; seed studies help.

**3. Facet and refinement suggestion**
- **Roy et al.** — it was CIKM 2008, not SIGMOD [P] https://ranger.uta.edu/~gdas/websitepages/preprints-papers/faceted.pdf
  - Cost is the expected number of facet questions needed to isolate the target tuple. Finding the minimum-cost tree is NP-hard.
  - A greedy method counts tuple pairs left indistinguishable, weighted by the probability the user can answer (from a 20-person survey).
  - Data: IMDB (~234k tuples), a car database (40k+ tuples, 43 attributes), and a 10M × 100 synthetic set.
  - The evaluation is entirely simulated. It beats the Indg, ID3 and PCA baselines, shown in figures only.
- **Chakrabarti, Chaudhuri & Hwang, SIGMOD 2004** [A] https://dl.acm.org/doi/abs/10.1145/1007568.1007653. An analytical cost model for exploring a category tree over query results.
- **Facetedpedia** (WWW 2010) [S]. Navigation cost = edges traversed plus the number of choices at each step.
- **Kong & Allan, CIKM 2014** [P] https://maroo.cs.umass.edu/pub/web/getpdf.php?id=1164
  - Simulated facet feedback, from an oracle and from annotators, with a time-cost user model (scanning a facet costs two term reads). Gain is ranking improvement measured against time.
  - Annotators picked only 44% of the effective terms, and only 28% of the terms they picked were effective. Human scorers are noisy.
- **User studies:**
  - Flamenco, CHI 2003 [S]: 32 students, 35,000 images; 90% preferred facets.
  - SciNet (Ruotsalo et al., TOIS 2018, scientific literature) [A] https://dl.acm.org/doi/10.1145/3231593: significant task improvements.

**4. Measuring effort to reach a target**
- Protocols in use:
  - Expected number of questions (Roy).
  - Time-cost user model (Kong).
  - Subtopic reach time for clustered results, Carpineto et al., IP&M 2012 [S] https://www.sciencedirect.com/science/article/abs/pii/S0306457311000835.
  - Minimum number of interactions (Interactive Set Discovery).
  - Kim's lower bound: the user runs the suggestion unmodified.
- The closest ready-made protocol for mark-driven query writing is Scells' repeated seed splits: marks go in, and the query is scored on the unseen relevant studies.
- A shared benchmark for "steps to reach a target set" in scholarly search: **not found**.

**5. LLMs and agents (2023–2026)**
- **D5**, Zhong et al., NeurIPS 2023 [P] https://arxiv.org/abs/2302.14233
  - An LLM proposes natural-language predicates that separate two corpora; a validator model scores them. OpenD5 has 675 problems.
  - Synthetic-set accuracy with GPT-4: 27% with goal and validator, 5% with neither.
  - Final discoveries were tested on held-out samples with Benjamini–Hochberg at FDR 10%; 13 of 21 were kept.
- **HypotheSAEs**, ICML 2025 [P] https://arxiv.org/abs/2502.04382
  - Sparse-autoencoder features over text embeddings, selected by how well they predict the target, then labelled by an LLM.
  - Bonferroni threshold 2.5e-3 over 20 candidates. 45 of 60 hypotheses were significant; at least +0.06 F1 on synthetic data.
- **PaSa**, ACL 2025 [S] https://aclanthology.org/2025.acl-long.572/. Crawler and selector agents; +37.78% recall@20 over Google with GPT-4o.
- **PaperPilot**, arXiv 2026-07 [A] https://arxiv.org/abs/2607.00597
  - An editable graph of search steps (keyword search, citation expansion, filters) that the user's feedback refines. This is closest to user-pruned branches.
  - Hit@5 rose from 58.0 to 77.0.
- **ScholarQuest**, arXiv 2026-06 [A] https://arxiv.org/abs/2606.20235. The best agent reaches Recall@100 of only 0.314.
- **Clarke & Smucker**, arXiv 2026-07 [A] https://arxiv.org/abs/2607.11362. An LLM agent with a Boolean engine on TREC 2024 RAG: nDCG@10 0.6863 with 100 calls per topic.
- **Sieve**, arXiv 2026-08 [A] https://arxiv.org/abs/2608.02751. Boolean field filters for deep-search agents; 20.7–50.6% fewer tokens.
- **LLM facet generation**, LREC-COLING 2024 [S] https://aclanthology.org/2024.lrec-main.519/.
- LLMs combined with beam-search subgroup discovery: **not found**. D5 and HypotheSAEs are the nearest analogues. "LLM as Dataset Analyst" (Springer 2024) [S]: I could not confirm the venue.

**6. False discoveries when many slices are scored**
- **Webb 2007**, Machine Learning [S; described in the tutorial below, P] https://link.springer.com/article/10.1007/s10994-007-5006-x
  - Either apply Bonferroni over the whole search space, or search on an exploratory split and test only the k chosen patterns on a holdout.
  - Webb 2008, "layered critical values": give simpler patterns looser thresholds, split by depth. This maps directly onto chain length.
- **Hämäläinen & Webb tutorial**, DMKD 2019 [P] https://arxiv.org/abs/1709.03904. Covers LAMP, Westfall–Young light, and hypothesis filtering and weighting.
- **LAMP**, PNAS 2013 [S] https://www.pnas.org/doi/10.1073/pnas.1302233110.
- **Duivesteijn & Knobbe, ICDM 2011** [S; described in van Leeuwen & Ukkonen, P] https://www.semanticscholar.org/paper/96be192aee9652e4e3b42dce148133768f2d33fe. Permuting the target gives a distribution of false discoveries per quality measure, used to validate subgroups and to compare measures.
- **van Leeuwen & Ukkonen, Discovery Science 2016** [P] https://eda.liacs.leidenuniv.nl/publications/2016/expect_the_unexpected_significance_of_subgroups-vanleeuwen,ukkonen.pdf
  - Tests against random subsets are too weak: real description languages always contain high-quality subgroups.
  - Implication: a word clause that differs from its parent set will almost always look "significant". Scores against marks need a holdout or an FDR procedure.
- **AWARE**, SIGMOD 2017 [A] https://arxiv.org/abs/1612.01040. In interactive exploration each view is an implicit test and standard FDR does not apply directly; they propose α-investing-style procedures.
- **SliceFinder**, ICDE 2019 [P, read the extended arXiv version] https://arxiv.org/abs/1807.06068. Lattice or decision-tree search over slices, Welch's t-test, an effect-size threshold, and α-investing to control marginal FDR.

---

## The prompt it was given

> Web research task (read-only; do not create or modify any files). Today is 2026-09-28. Context: a user building a scholarly-literature search engine (BM25 over titles/abstracts of ~40M+ works, PubMed included, with facet counts over every match) is considering a tool called "suggested cuts": given a large result set, the system explores many candidate narrowing steps (word clauses, facet values, clusters, concept labels) with beam search — keep the best k partial chains of AND-ed steps, extend each by one step, rescore — and shows the user (or an AI agent) a handful of readable chains with counts. The score can be generic "interestingness" (an even split, difference from the parent set) or agreement with the user's marks of on-topic/off-topic works, in which case it effectively writes a readable boolean query that separates the marks. A second mode lets the user keep several branches open and prune them (beam search with a human scorer). This is subgroup discovery applied to search.
>
> Find, with sources:
> 1. Subgroup discovery methods that matter here: quality measures (WRAcc and others), beam search as the standard search, redundancy (DSSD, van Leeuwen & Knobbe 2012), statistical validation of discovered subgroups against false discoveries (e.g. Duivesteijn & Knobbe 2011 or others), exceptional model mining, and rule learners such as CN2. Any application of subgroup discovery or pattern mining to text or document collections.
> 2. Learning readable boolean queries or rules from labelled examples: decision-tree or rule-learning query formulation (e.g. Kim et al., professional/patent search; the Scells et al. line on systematic reviews), query-by-example, "learning to formulate boolean queries". Results and measures.
> 3. Dynamic faceted search and facet/refinement suggestion: choosing which facets or refinements to show to minimise user effort (e.g. "minimum-effort driven dynamic faceted search", Roy et al. SIGMOD 2008, or similar), query suggestion and query refinement suggestion in web and professional search, and how these were evaluated (simulated users, navigation cost, user studies).
> 4. Evaluation methods for interactive refinement: simulated-user or session-based evaluation for faceted search / query refinement; any benchmark or protocol that measures how many steps or how much effort it takes to reach a target set.
> 5. LLMs or agents that propose refinements, facets or query branches for literature search, or LLMs combined with pattern mining / subgroup discovery; any 2024–2026 work.
> 6. Multiple-comparison / false-discovery issues when many candidate slices are scored, and how pattern-mining work handles it.
>
> Return a report under 1,300 words: findings with URL, date/venue, what exactly the method does, the key numbers if any, and whether you read the primary source or only a search summary (tag [P] primary full text or result tables, [A] abstract/landing page only, [S] search-engine summary only). Say "not found" plainly where you searched and found nothing. Do not invent systems, numbers or citations; flag anything uncertain.
