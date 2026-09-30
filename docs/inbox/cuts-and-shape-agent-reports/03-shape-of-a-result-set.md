# Research report 3: the shape of a result set, prior work

**What this is:** the final report of a web-research subagent, delivered
on 2026-09-28 to the conversation that wrote
[the shape of a result set](../shape-of-a-result-set.md), and kept
verbatim, following the practice the user asked for with
[searching within a result set](../search-within-results.md)'s reports.
It is model output, not checked line by line. It used 101 tool calls (web
searches and page fetches) over about 19 minutes, and reports hitting the
session's web-search limit near the end. Its tags are its own: [P]
primary full text or result tables read; [A] abstract or landing page
only; [S] a search engine's summary only. Two lines in its §6 are its own
arithmetic ("my arithmetic"), estimates rather than measurements; the
note leaves them out. The prompt it was given follows the report.

---

**Report: shape-of-a-result-set tool, prior work (2026-09-28)**

Tags: [P] = primary full text or result tables read; [A] = abstract or landing page only; [S] = search-engine summary only. I hit the session's web-search limit near the end, so the last few items rest on pages fetched directly.

**Bottom line**
- The proposed method (cluster a random sample, then assign every other work to its nearest centre) is essentially **Buckshot**. Buckshot runs hierarchical clustering on a √N sample, uses the result to seed k-means, and costs Θ(N) overall (Manning et al., *IR book* §17.8 [P]). I attribute Buckshot to Cutting et al. 1992 from memory; that page did not credit it. **BERTopic** is the modern form: embed, reduce with UMAP, cluster with HDBSCAN, label with class-based TF-IDF.
- **Old evidence is mixed:**
  - Clusters do concentrate the relevant documents.
  - Clustering *alone* was worse than ranked search for finding documents.
  - People use categories mainly when the ranking fails.
  - Facets beat clusters in usability studies.
- **Scholarly benchmarks that could score "shape":** ClusTREC-COVID, the TREC Genomics 2006–07 aspect judgements, and CoverageBench (2026).
- **Literature-mapping tools:** I found no controlled usefulness study for any of the mainstream ones.

**1. Search-results clustering**
- **Cutting et al., SIGIR 1992 (Scatter/Gather):** says clustering "does not appreciably improve retrieval" when used to enhance search; proposes it for browsing instead, with linear-time algorithms [A].
- **Hearst & Pedersen, SIGIR 1996** ([Berkeley copy](https://people.ischool.berkeley.edu/~hearst/papers/sg-sigir96/sigir96.html)) [P]:
  - Setup: TREC/Tipster collection, TREC-4 topics, the top 100 to 1000 results split into 5 clusters.
  - The top cluster "almost always" held at least 50% of the retrieved relevant documents.
  - The best cluster beat ranked titles at cutoffs 100, 250 and 500.
  - 4 users chose the best cluster in 31 of 38 cases.
- **Hearst, *Search User Interfaces* ch. 8, 2009** ([link](https://searchuserinterfaces.com/book/sui_ch8_navigation_and_search.html)) [P, reporting other studies]:
  - Pirolli et al., CHI 1996: Scatter/Gather conveyed the corpus's structure, but clustering alone was less effective than similarity search.
  - Käki's Findex, 2005 (16 users over 2 months): categories were used on 26% of queries, rising to 39%, and helped mostly when relevant items ranked low. 45% of users said they began typing less precise queries.
  - Käki, 27 users: 40 groups slowed people down, and both 20 and 40 felt like too many.
  - Hearst's verdict: facets are "superior to clusters and flat lists in usability studies." Clusters suffer from unpredictability, several dimensions mixed together, and hard-to-label groups.
- **Kules & Shneiderman, IP&M 2008** (24 users): with categorized overviews, users explored deeper and felt more organized, but outcome quality did not differ [S].
- **Surveys and systems:** Carpineto et al., ACM Computing Surveys 41(3), 2009 [A]. Lingo (Osinski & Weiss, IEEE Intelligent Systems 2005) finds phrases first and then assigns documents to them [A]. Vivisimo's Clusty was sold to Yippy in 2010 [S].
- **Benchmarks** (Di Marco & Navigli, *Computational Linguistics* 39(3), 2013, [PDF](https://aclanthology.org/J13-3008.pdf)) [P]:
  - AMBIENT: 44 ambiguous queries × 100 Yahoo results, labelled with Wikipedia senses. MORESQUE: 114 queries.
  - Measures: adjusted Rand index (ARI), Jaccard, F1, subtopic recall at K, subtopic precision at r.
  - Lingo scored ARI −0.53 and F1 16.7. KeySRC scored F1 63.1. Graph-based methods scored ARI 61 to 70.
  - A trivial all-singletons baseline scored F1 68.2, so F1 alone can be gamed.
  - ODP-239 (Carpineto & Romano) [S].
  - All of these are web-snippet sets built on ambiguous queries, not scholarly collections.

**2. Labelling**
- **BERTopic** ([arXiv 2203.05794](https://arxiv.org/abs/2203.05794)) [A]. HDBSCAN's `approximate_predict` assigns new points to a fixed clustering and can mark them as noise (−1) [P, docs].
- **TopicGPT** (NAACL 2024): purity 0.74 against human-labelled Wikipedia topics, versus 0.64 for the best baseline [A].
- **TnT-LLM** (KDD 2024, [arXiv 2403.12173](https://arxiv.org/abs/2403.12173)) [P]:
  - The taxonomy was built from about 9.6k Bing Copilot conversations.
  - GPT-4 agreed with the resolved human labels more than the humans agreed with each other; human κ was about 0.38 to 0.48.
  - A logistic-regression classifier trained on GPT-4 pseudo-labels matched GPT-4 (0.658 vs 0.655).
- **Viswanathan et al., TACL 2024:** LLM keyphrase expansion and pairwise constraints improve clusters [A].
- **Kozlowski et al.** (arXiv 2408.07003, 34,797 biology papers): GPT-4 labels made from BERTopic keywords were accurate; 3-word labels worked best [S].
- **Murray et al.**, Scientometrics, Nov 2025 ([arXiv 2511.02601](https://arxiv.org/abs/2511.02601)): LLM-written descriptive labels perform "at or near" labels built from extracted terms [A].
- **arXiv 2605.24351** (May 2026), 100 published bibliometric analyses: LLM descriptions come close to human ones, but LLMs are unreliable at inferring the clusters themselves. Recommended split: an algorithm makes the clusters and the LLM names them [A].
- **OpenAlex Topics quality study** (Zenodo, 2026-09-07, [link](https://zenodo.org/records/22656226)): these are citation clusters named by an LLM. Across 837 works, label precision ranged from 54.2% to 82.7% by field [A].

**3. Literature-mapping tools**
- **Knowledge Navigator** ([arXiv 2408.15836](https://arxiv.org/html/2408.15836)) [P]:
  - Pipeline: up to 1,000 Google Scholar results, UMAP plus a Gaussian mixture model with k chosen by silhouette score; an LLM names the clusters and drops off-topic ones.
  - ClusTREC-COVID: 2,284 documents in 50 expert-titled subtopic clusters. ARI 0.516; 2.3 to 3.18 clusters cover 80% of a topic's documents; names match the expert titles 88% of the time.
  - SciTOC: 50 Annual Reviews tables of contents; 71.6% of review headings were covered.
  - No user study, and no cost reported.
- **Open Knowledge Maps:** uses the top 100 results from PubMed or BASE, Ward clustering, and TF-IDF labels. The FAQ describes no evaluation [P].
- **Connected Papers:** similarity from co-citation plus bibliographic coupling [S].
- **ResearchRabbit, Litmaps, Inciteful:** no evaluations found; library guides call them supplements [S].
- **VOSviewer:** its site lists the relation types it maps and describes no evaluation [P].
- **CiteSpace:** not examined.
- **Semantic Scholar:** no result clustering found; it offers facets and research feeds.
- **DimInd** (arXiv 2504.18496, 23 researchers): less effort than a ChatGPT baseline [A].
- **ClusterChat** (SIGIR 2025 demo): 4M PubMed abstracts [A].
- **Citation versus text:**
  - Bascur et al. (Scientometrics 2023, arXiv 2207.03299), 25 systematic reviews: citation clusters were "highly variable and unpredictable." They work best when recall is valued 2 to 8 times over precision, and they complement query search [A].
  - Sjögårde & Ahlgren (arXiv 2403.09295): co-citation beat bibliographic coupling, which beat direct citation. Combining the three was best, and adding text helps [A].
  - Boyack et al. (PLoS ONE 2011), 2.15M MEDLINE records: PubMed related articles best, BM25 on titles and abstracts second, MeSH-only worse [P].
  - Ahlgren et al. (QSS 2020): co-citation worst [S]. Klavans & Boyack (JASIST 2017): direct citation best [S].
  - Boyack et al. (Scientific Data 2020): 18M PubMed documents in 28,743 hybrid citation-plus-text clusters [S].

**4. Measures**
- **Diversity measures:** subtopic recall (Zhai et al., SIGIR 2003), α-nDCG (Clarke et al., SIGIR 2008; used in the TREC Web diversity task 2009–12), and intent-aware metrics (Agrawal et al., WSDM 2009) [S].
- **TREC Genomics 2006:** 162,259 full-text articles, 28 topics, relevant passages grouped into aspects labelled with MeSH terms, scored by aspect MAP [P]. 2007: 36 topics, with aspects defined as answer entities [S].
- **CoverageBench** ([arXiv 2603.20034](https://arxiv.org/abs/2603.20034), Mar 2026): collections with nugget judgements for measuring coverage [A].
- **arXiv 2603.08819:** coverage-based retrieval metrics correlate strongly with coverage in RAG output [A].
- **Cluster hypothesis:** Voorhees 1985; Smucker & Allan 2009 [S]. Tests of it on dense embeddings: not found.
- **MTEB clustering tasks** (arXiv, bioRxiv, medRxiv; scored by V-measure) rate embedding models, not result-set tools [S].

**5. Sampling, coverage, agents**
- **Cormack & Grossman, SIGIR 2015:** continuous active learning reached high recall on each facet, not just overall [S].
- **Clusters as strata for estimating recall in technology-assisted review:** not found. Strata there are usually by score or by included versus excluded pool.
- **arXiv 2607.21480** (Jul 2026): certifying how much was missed requires sampling *outside* the candidate set, on the order of N₀/m labels [A]. So clusters inside a result set cannot vouch for what the set missed.
- **Speculative RAG** (ICLR 2025): k-means over retrieved documents, one per cluster in each draft; +1.22 points over random subsets on TriviaQA and PubHealth [P].
- **arXiv 2608.13956** (Aug 2026): diverse documents raised answer correctness 17 to 47%; duplicates did not [A].
- **ResearchArena** (arXiv 2406.10291): 12M papers, with discovery, selection and mind-map tasks. A possible benchmark [A].
- **Agents that cluster results to allocate a reading budget, with a measured effect:** not found.

**6. Costs and scale**
- **HDBSCAN:** 200k points in 10 dimensions take about 20 to 30 s. High dimensions were not tested, hence UMAP first [P].
- **UMAP:** the docs give curves only, up to 70k points [P].
- **FAISS:** 1M 256-dimensional vectors into 20k centroids takes 14 s on 8 P100 GPUs [S].
- **Nearest-centre assignment (my arithmetic):** 10M 768-dimensional vectors against 50 centres is about 0.8 Tflop, seconds to a minute on a CPU.
- **Embedding storage (my arithmetic):** 40M works at 768 dimensions in float32 is about 120 GB.
- **Precomputed embeddings:**
  - SPECTER2 (SciRepEval, EMNLP 2023): more than 2 points above the previous single-embedding state of the art [A]. The Semantic Scholar Datasets API offers `embeddings-specter_v2` in bulk with an API key [S].
  - MedCPT: embeddings of all of PubMed on the NCBI FTP site, 768-dimensional, about 105 GB, last updated Oct 2023 / Mar 2024 [P listing]; the model is public domain.
  - OpenAlex's 4,516 topics are ready-made labels: CWTS citation clusters assigned by an mBERT classifier [S], with the quality figures in section 2.

**Uncertain:** crediting Buckshot to Cutting et al. 1992 is from memory. The Findex lab-study numbers were not retrieved.

---

## The prompt it was given

> Web research task (read-only; do not create or modify any files). Today is 2026-09-28. Context: a user building a scholarly-literature search engine (BM25 over titles/abstracts of ~40M+ works, PubMed included) is considering a "shape of a result set" tool: take a random sample of a (possibly huge) result set, cluster the sample's embedding vectors, name each cluster (by distinctive words, by a small LLM, or not at all, showing example works), then assign every other work in the set to its nearest cluster centre; show years, venues and distinctive words beside the clusters. It should help human researchers see what a result set is made of, help AI agents allocate reading and check coverage, and ideally show up on benchmarks.
>
> Find, with sources:
> 1. Search-results clustering: Scatter/Gather (Cutting et al. 1992; Hearst & Pedersen 1996) and later evaluations; Carrot2 / Lingo (Osinski & Weiss); surveys of web clustering engines (e.g. Carpineto et al., ACM Computing Surveys 2009); search-result-clustering benchmarks (AMBIENT, ODP-239, MORESQUE, others) and measures; user studies of clustered vs ranked-list interfaces (e.g. Käki's Findex, others) and what they found.
> 2. Cluster labelling, including recent LLM-based labelling and LLM topic modelling (BERTopic, TopicGPT, TnT-LLM, LLM-based clustering), with any measured quality.
> 3. Literature-mapping tools and their evaluations: Connected Papers, ResearchRabbit, Litmaps, Inciteful, Open Knowledge Maps, VOSviewer, CiteSpace, Semantic Scholar features, Knowledge Navigator (2024, arXiv 2408.15836), others; any user studies or measured usefulness; clustering by citation (co-citation, bibliographic coupling) versus text embeddings; Bascur et al. (arXiv 2207.03299) on citation clusters for systematic reviews.
> 4. Measures and benchmarks where "shape" could be scored: subtopic retrieval and diversity (subtopic recall, alpha-nDCG, intent-aware metrics; TREC Web diversity), literature collections with subtopic or aspect judgements (e.g. TREC Genomics 2006–2007 aspects), the cluster hypothesis and its tests.
> 5. Clusters used for sampling or coverage: stratified sampling by cluster to estimate recall or prevalence (e.g. in technology-assisted review / e-discovery), diversity-aware retrieval for agents or deep-research systems, clusters used to allocate an LLM's reading budget; any 2024–2026 agent work that clusters results before reading.
> 6. Costs and scale: clustering a sample (UMAP/HDBSCAN, k-means) at tens of thousands of points, nearest-centre assignment over millions, availability of precomputed scholarly embeddings (e.g. Semantic Scholar SPECTER2 embeddings dataset), and embedding-model choices for scientific text.
>
> Return a report under 1,300 words: findings with URL, date/venue, what exactly was done, the key numbers if any, and whether you read the primary source or only a search summary (tag [P] primary full text or result tables, [A] abstract/landing page only, [S] search-engine summary only). Say "not found" plainly where you searched and found nothing. Do not invent systems, numbers or citations; flag anything uncertain.
