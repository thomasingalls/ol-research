# The shape of a result set

**Status:** research-program note, 2026-09-28, written at the user's
request ("and another document around the shape/clusters proposal"), a
companion to [the recall-accounted set builder](recall-accounted-set-builder.md)
and [suggested cuts](suggested-cuts.md), built on
[searching within a result set](search-within-results.md) §8.
Speculative, not a plan: nothing here is built, scheduled or decided,
and nothing in it amends the plans of record. Scope, the user's:
academic literature in every field as the wedge ("pubmed was just a
convenient place to start"), for academics and researchers, with no kind
of artifact out of scope in principle for its document type (see
[searching within a result set](search-within-results.md)'s status). Facts are marked **P**
(primary: this repository, or a source read directly on 2026-09-28),
**S** (secondary: read through a search engine's summaries that day) or
**U** (from memory, not checked). The bracketed tags **[P]**, **[A]**
and **[S]** are the research reports' own (full source or result
tables, abstract only, summary only); the reports are kept verbatim in
[cuts-and-shape-agent-reports/](cuts-and-shape-agent-reports/):
[1, the project's own notes](cuts-and-shape-agent-reports/01-repository-notes.md)
and [3, a result set's shape](cuts-and-shape-agent-reports/03-shape-of-a-result-set.md).
The third report's own cost arithmetic is left out here: this project
records measured figures, not estimates. Related:
[searching within a result set](search-within-results.md) §6 item 5
(how a set differs from the corpus), §8 (the shape, quickly), §9 (whose
idea of similar), §14 (costs); the set builder's §7, which puts this
tool third; [ambitions beyond citations](ambitions-beyond-citations.md)
§6 (unlabelled clusters, shown with example works);
[the retrieval frontier](retrieval-frontier.md) §3–§4 (SPECTER, Semantic
Scholar's embeddings); the [serving plan](../serving-plan.md)'s
`related` call (co-citation and shared references);
[retrieval benchmarks](retrieval-benchmarks.md); the
[evaluation plan](../evaluation-plan.md) §3.4.

## 1. The proposal

In the user's words:

> having a 'what is the shape of this results list' would be really,
> really useful, especially if you could generate this easily and
> quickly.
>
> this really feels like a hybrid search question. having vectors on the
> results set would make the generation of and display of a one-off
> segmentation of a particular results set easier - nay, possible in the
> first place.

The pipeline as first sketched
([searching within a result set](search-within-results.md) §8): take a
random sample of the set, cluster the sample's vectors, name each
cluster or show a few of its works, and give every other work in the set
its nearest cluster. The sample's size sets how precise the picture is;
the set's size hardly matters.

The set builder note put this tool third: "the best first look for a
person; the weakest evidence ... and the least use to an agent". This
note asks what the shape is for (§3), what is known (§4), where it breaks
(§5), and how to find out (§6–§8).

## 2. What it is

Words:

- **Sample:** works drawn at random from the set.
- **Cluster:** a group of works close to one another by some measure of
  similarity; its **centre** is the average of its members.
- **Lens:** the measure of similarity: text vectors, citations, or both.
- **Label:** a cluster's name, from its distinctive words, from a small
  model, or none, showing example works instead.
- **Shape:** the clusters with their sizes, labels or examples, beside
  the set's years, venues, facets and distinctive words.

**It has a name: Buckshot.** Clustering a sample of about √N works and
using the result to seed k-means over everything costs time in
proportion to the set's size (Manning, Raghavan and Schütze,
*Introduction to Information Retrieval* §17.8, [P]); crediting Buckshot
to Scatter/Gather's paper (Cutting et al. 1992) is from memory (U).
BERTopic ([arXiv 2203.05794](https://arxiv.org/abs/2203.05794), [A]) is
the modern form: embed, reduce with UMAP, cluster with HDBSCAN, label by
class-based TF-IDF; its `approximate_predict` gives new points a fixed
clustering's cluster, or marks them as noise ([P], its documentation).

**Three lenses, and the choice is a judgement** (search-within-results
§9):

- **Text vectors.** SPECTER learned "similar" from citations
  ([the retrieval frontier](retrieval-frontier.md) §3, P); Semantic
  Scholar publishes SPECTER2 vectors in bulk behind an API key (§4 of
  that note, P). NCBI publishes MedCPT embeddings of all of PubMed on its
  FTP site: 768 dimensions, about 105 GB as listed, last updated October
  2023 and March 2024, the model public domain ([P], the listing).
- **Citations.** Co-citation (cited together) and bibliographic coupling
  (citing the same works); the serving plan's `related(work, by)`
  offers both, ranked by Salton's cosine, and measured raw overlap as
  "nearly no signal" on its own (per report 1). Citation clusters do not
  care what language a work is written in.
- **Both.** Boyack et al. built 28,743 clusters over 18 million PubMed
  documents from citations and text together (Scientific Data 2020, [S]).

**Ready-made labels exist, as someone's claims.** OpenAlex's 4,516
topics are citation clusters, named by a model and assigned by a
classifier ([S]); a study of 837 works found label precision between
54.2 % and 82.7 % depending on the field
([Zenodo, 2026-09-07](https://zenodo.org/records/22656226), [A]). MeSH
labels PubMed's indexed works. Both belong beside the shape as signed
claims, not as its clusters
([searching within a result set](search-within-results.md) §9).

## 3. What it is for

Four uses, which the evidence treats differently:

1. **A first look for a person:** what this pile is made of, and whether
   the query meant what the user thought (a set that looks like the
   corpus is a query that did nothing, search-within-results §6 item 5).
2. **Coverage:** which parts of the pile have been read, and which not.
3. **Parts for the other tools:** candidate steps for
   [suggested cuts](suggested-cuts.md), and strata for the set builder's
   samples *inside* its edge. Not for what the set missed: certifying how
   much was missed takes sampling outside the candidate set
   ([arXiv 2607.21480](https://arxiv.org/abs/2607.21480), Jul 2026, [A]),
   so clusters of the set cannot vouch for it; the set builder's account
   (b) stands.
4. **Allocating an agent's reading:** one work per cluster before a
   second from any (§6.2).

## 4. What is known

### 4.1 Clustering search results

- **Scatter/Gather** (Cutting et al., SIGIR 1992, [A]): clustering "does
  not appreciably improve retrieval" when used to enhance search;
  proposed instead for browsing, with algorithms in linear time.
- **Hearst and Pedersen** ([SIGIR 1996](https://people.ischool.berkeley.edu/~hearst/papers/sg-sigir96/sigir96.html),
  [P]): TREC-4 topics, the top 100–1,000 results in 5 clusters; the top
  cluster "almost always" held at least half the relevant documents
  retrieved; the best cluster beat the ranked titles at cutoffs of 100,
  250 and 500; four users picked the best cluster in 31 of 38 cases.
- **As Hearst's *Search User Interfaces* (2009, ch. 8) reports**
  ([book](https://searchuserinterfaces.com/book/sui_ch8_navigation_and_search.html),
  [P]):
  - Pirolli et al. (CHI 1996): Scatter/Gather conveyed the corpus's
    structure, but clustering alone was less effective than similarity
    search.
  - Käki's Findex (2005; 16 users over two months): categories were used
    on 26 % of queries, rising to 39 %, and helped mostly when relevant
    items ranked low; 45 % of users said they began typing less precise
    queries. In another of Käki's studies, 27 users: 40 groups slowed
    people down, and both 20 and 40 felt too many.
  - Hearst's verdict: facets are "superior to clusters and flat lists in
    usability studies"; clusters suffer from unpredictability, several
    dimensions mixed together, and groups hard to name.
- **Kules and Shneiderman** (IP&M 2008, 24 users, [S]): with categorized
  overviews people explored deeper and felt more organized, and the
  quality of what they found did not differ.
- **Systems and surveys:** Carpineto et al., ACM Computing Surveys 2009
  ([A]); Lingo, which finds phrases first and then assigns documents to
  them (Osinski and Weiss, 2005, [A]); Vivísimo's Clusty, sold to Yippy
  in 2010 ([S]; search-within-results §11.3).

### 4.2 Benchmarks for clustered results, all on the web

AMBIENT (44 ambiguous queries × 100 Yahoo results, labelled with
Wikipedia senses) and MORESQUE (114 queries), scored by adjusted Rand
index, Jaccard, F1, subtopic recall at K and subtopic precision (Di Marco
and Navigli, [Computational Linguistics 2013](https://aclanthology.org/J13-3008.pdf),
[P]); ODP-239 (Carpineto and Romano, [S]). Lingo scored ARI −0.53 there,
graph-based methods 61–70, and an all-singletons baseline reached F1 68.2,
so F1 alone can be gamed. All are web snippets for ambiguous queries,
not scholarly collections.

### 4.3 Naming clusters

- **TopicGPT** (NAACL 2024, [A]): purity 0.74 against human-labelled
  Wikipedia topics, against 0.64 for the best baseline.
- **TnT-LLM** ([KDD 2024](https://arxiv.org/abs/2403.12173), [P]): GPT-4
  agreed with resolved human labels more than the humans agreed with one
  another (κ about 0.38–0.48); a logistic regression trained on GPT-4's
  labels matched GPT-4 (0.658 against 0.655). That is also evidence for
  the set builder's cascade.
- **Scholarly labels:** GPT-4 names made from BERTopic's keywords were
  accurate over 34,797 biology papers, three-word labels best (Kozlowski
  et al., arXiv 2408.07003, [S]); model-written labels perform "at or
  near" labels built from extracted terms (Murray et al., Scientometrics
  2025, [arXiv 2511.02601](https://arxiv.org/abs/2511.02601), [A]);
  across 100 published bibliometric analyses, models describe clusters
  nearly as well as people but are unreliable at inferring the clusters
  themselves, so an algorithm should make them and a model name them
  ([arXiv 2605.24351](https://arxiv.org/abs/2605.24351), May 2026, [A]).
- **Or no names:** unlabelled clusters are acceptable when the reader
  sees example works ([ambitions](ambitions-beyond-citations.md) §6, P).

### 4.4 Literature maps

- **Knowledge Navigator** ([arXiv 2408.15836](https://arxiv.org/html/2408.15836),
  [P]): up to 1,000 Google Scholar results, UMAP and a Gaussian mixture
  with k chosen by silhouette, a model naming clusters and dropping
  off-topic ones. On ClusTREC-COVID (2,284 documents in 50 subtopic
  clusters titled by experts): ARI 0.516; 2.3–3.18 clusters cover 80 % of
  a topic's documents; names match the experts' titles 88 % of the time.
  On SciTOC (50 Annual Reviews tables of contents): 71.6 % of the
  reviews' headings covered. No user study, no cost reported.
- **Open Knowledge Maps:** the top 100 results from PubMed or BASE, Ward
  clustering, TF-IDF labels; no evaluation described ([P], its FAQ).
- **Connected Papers:** similarity from co-citation and coupling ([S]).
  **ResearchRabbit, Litmaps, Inciteful:** no evaluations found; library
  guides call them supplements ([S]). **VOSviewer:** no evaluation
  described ([P]). Semantic Scholar: facets and feeds, no clustering of
  results found.
- **DimInd** (arXiv 2504.18496, 23 researchers, [A]): less effort than a
  ChatGPT baseline. **ClusterChat** (SIGIR 2025 demo, [A]): 4 million
  PubMed abstracts.
- **No controlled study of usefulness** was found for any mainstream
  literature-mapping tool.

### 4.5 Citations or text

- Citation clusters on 25 systematic reviews were "highly variable and
  unpredictable", worth it when recall is valued 2–8× over precision,
  and complemented query search (Bascur et al.,
  [arXiv 2207.03299](https://arxiv.org/abs/2207.03299), [A]).
- Co-citation beat coupling, which beat direct citation; the three
  combined were best, and text helped (Sjögårde and Ahlgren,
  arXiv 2403.09295, [A]).
- Over 2.15 million MEDLINE records, PubMed's related articles were best,
  BM25 on titles and abstracts second, MeSH alone worse (Boyack et al.,
  PLoS ONE 2011, [P]).
- The comparisons disagree: co-citation worst in Ahlgren et al. (QSS
  2020, [S]), direct citation best in Klavans and Boyack (JASIST 2017,
  [S]). Which lens is best is not settled; it depends on the task.

## 5. The hard parts

- **Clusters mix dimensions.** One cluster holds a topic, another a
  method, a third an organism, and they overlap; a facet is one
  dimension at a time. Hearst's verdict (§4.1) argues for showing the
  shape beside the facets, not in place of them.
- **Few groups.** Käki's users found 20 groups too many; a small number,
  with Scatter/Gather's loop to open one, is the known answer.
- **Clusters do not find things.** Clustering alone was worse than
  ranked search at finding documents (Cutting; Pirolli). The shape is
  for orientation and coverage, not recall.
- **Whose similar.** A text model's idea of similar is its trainers'
  (SPECTER's is citations); citation lenses disagree among themselves
  (§4.5). A choice to make on purpose, perhaps offering two lenses side
  by side.
- **A sample misses the small.** A rare subtopic can vanish from a
  sample; sizes drawn from a sample carry error, which the shape should
  show, as the set builder shows its intervals.
- **What the text holds.** Works without abstracts get vectors from
  titles alone (search-within-results §14); full text, the gold
  standard, would change what a text lens sees. Citation lenses need no
  text at all.
- **Languages.** A text lens needs a model that reads the set's
  languages, and a work not in English is no reason to leave it out (the
  user, 2026-09-28); the citation lens is indifferent to language.
- **The box.** Full-precision vectors for the corpus do not fit, so
  compression is required, and vectors would break byte-identical
  rebuilds ([future ambitions](future-ambitions.md), per report 1). A
  query encoded by another model is a hard failure (same note).
  Measured costs so far are other people's: HDBSCAN clusters 200,000
  points in 10 dimensions in about 20–30 seconds ([P], its documentation);
  ours are to be measured on the box.
- **Reproducible shapes.** A shape should be reproducible from its query,
  the epoch, the vector model's version and a seed.

## 6. Why each audience

### 6.1 Researchers

The first look at a pile, the user's reason for the idea. The evidence
supports orientation, not accuracy: structure conveyed (Pirolli), used
when the ranking fails (Käki), deeper exploration with the same outcome
(Kules and Shneiderman). Sold as "see what is in here", it keeps its
promise; sold as "find more", it does not.

### 6.2 Agents

- **Coverage at a fixed budget.** Read one work per cluster before a
  second from any, and see which clusters have no reads. In
  retrieval-augmented generation, drafting from one document per k-means
  cluster beat random subsets by 1.22 points (Speculative RAG, ICLR
  2025, [P]), and diverse documents raised answer correctness by 17–47 %
  where duplicates did not ([arXiv 2608.13956](https://arxiv.org/abs/2608.13956),
  Aug 2026, [A]). **Not found:** a literature agent that clusters results
  to allocate its reading, with a measured effect.
- **A compact answer:** a handful of clusters with sizes, distinctive
  words and two example ids each, one call
  ([configuration](future-ambitions-configuration.md)'s HarnessTax, per
  report 1).

### 6.3 Benchmarks

Retrieval benchmarks do not score a shape. What could:

- **ClusTREC-COVID:** 2,284 documents in 50 subtopic clusters titled by
  experts; adjusted Rand index, the clusters needed to cover 80 % of a
  topic, and label agreement (Knowledge Navigator, [P]).
- **SciTOC:** how many of a review's headings the clusters cover
  (Knowledge Navigator, [P]).
- **TREC Genomics 2006–07 aspects:** relevant passages grouped into
  aspects named by MeSH, scored by aspect MAP ([P], 2006; 2007, [S]).
  Judged on full text, so it waits on full text, and is kept for it
  ([retrieval benchmarks](retrieval-benchmarks.md)).
- **CoverageBench** ([arXiv 2603.20034](https://arxiv.org/abs/2603.20034),
  Mar 2026, [A]): collections with judged nuggets for coverage.
- **Diversity measures:** subtopic recall (Zhai et al., SIGIR 2003),
  α-nDCG (Clarke et al., SIGIR 2008; the TREC Web diversity task
  2009–12), intent-aware measures (Agrawal et al., WSDM 2009), all [S].
- **ResearchArena** (arXiv 2406.10291, [A]): discovery, selection and
  mind-map tasks over 12 million papers.

**The testable claim** for agents: at a fixed reading budget, reading
by cluster covers more of a topic's subtopics or aspects than reading
the top of the ranking. Tempered: the shape does not find more
(§5); the win to test is coverage per work read.

## 7. Beside the other two tools

- **The set builder** ([note](recall-accounted-set-builder.md)): clusters
  as strata for its samples inside the edge, never as evidence about
  what the edge missed (§3); TnT-LLM's result (§4.3) supports its
  cascade.
- **Suggested cuts** ([note](suggested-cuts.md)): clusters as candidate
  steps, the shape's distinctive words as candidate clauses.
- **Order.** Third, as the set builder's note says, for vectors. But a
  first shape needs no vectors: the set's facets against the corpus's
  (the index computes both, P: `facets_of_all` in
  `crates/search/src/index.rs`), its distinctive words, and "the works
  this set cites most" from the graph
  ([searching within a result set](search-within-results.md) §13) are
  all within reach before any embedding.

## 8. A path, measurement first

1. **A shape without vectors:** facets of the set beside the corpus's,
   distinctive words, and the works the set cites most.
2. **A clustering set in the evaluation plan:** ClusTREC-COVID now;
   TREC Genomics 2006–07's aspects when full text lands.
3. **Vectors, chosen on purpose:** SPECTER2 through Semantic Scholar,
   MedCPT's published PubMed embeddings, or a model of our own;
   compressed; size and time measured on the box.
4. **Sample, cluster, assign, name:** scored on ClusTREC-COVID, text lens
   against citation lens against both.
5. **Agents:** reading by cluster against reading the top of the ranking
   at a fixed budget, scored by subtopic and aspect coverage.

## 9. Open

1. Which lens, or lenses: text (which model), citations, both.
2. How clusters are named: distinctive words, a small model, examples,
   with OpenAlex's topics and MeSH shown beside as others' claims.
3. How many clusters, and how to open one.
4. The sample's size, and how the shape shows its own uncertainty and
   its rare subtopics.
5. Reproducibility: seed, model version, epoch.
6. The shape beside the facets, as Hearst's findings suggest.
7. Languages: a multilingual text model, the citation lens, or both.
8. Vectors on the box: compression, and vectors as an artifact of their
   own ([future ambitions](future-ambitions.md)).
9. Computed for every search, or on request.
