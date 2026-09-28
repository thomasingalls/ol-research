# Suggested cuts

**Status:** research-program note, 2026-09-28, written at the user's
request ("would you write up a similar research program around the
suggested cuts / beam search proposal?"), a companion to
[the recall-accounted set builder](recall-accounted-set-builder.md) and
built on [searching within a result set](search-within-results.md) §11.
Speculative, not a plan: nothing here is built, scheduled or decided,
and nothing in it amends the plans of record. Scope, the user's:
academic literature in every field ("pubmed was just a convenient place
to start"), and associated scientific data later. Facts are marked **P**
(primary: this repository, or a paper read directly on 2026-09-28),
**S** (secondary: read through a search engine's summaries that day) or
**U** (from memory, not checked). The bracketed tags **[P]**, **[A]**
and **[S]** are the research reports' own (full source, abstract only,
summary only); the reports are kept verbatim in
[cuts-and-shape-agent-reports/](cuts-and-shape-agent-reports/):
[1, the project's own notes](cuts-and-shape-agent-reports/01-repository-notes.md)
and [2, suggested cuts](cuts-and-shape-agent-reports/02-suggested-cuts.md).
Related: [searching within a result set](search-within-results.md) §6
(what a step shows), §10 (marks and the funnel), §11 (beam search over
steps) and §12 (agents and set tools); the set builder's §7, which puts
this tool second; [ambitions beyond citations](ambitions-beyond-citations.md)
§5 (the "did you mean" row); [retrieval benchmarks](retrieval-benchmarks.md)
(SR4CS, CLEF TAR, the Seed collection, SYNERGY); the
[evaluation plan](../evaluation-plan.md) §3.4; the
[serving plan](../serving-plan.md) §11, item 36.

## 1. The proposal

The user, while the idea was still loose: "it occurs to me that beam
search might be an applicable technique to the large results set stage,
but i'm kind of loosely throwing ideas around so i'm not sure exactly
what i mean by that". Of three readings, the user kept two
([searching within a result set](search-within-results.md) §11):

- **The machine proposes cuts** (§11.1 there): the system explores many
  candidate narrowing steps and shows a handful of readable chains, each
  with its count.
- **The user keeps several branches open** (§11.2 there): the same
  search with the user as the scorer, a tree to prune rather than a
  stack to pop.

The set builder note put this tool second, because "the good score is
agreement with marks, which leads back to the classifier". This note
asks what suggested cuts can do that a classifier cannot (§3), what is
known (§4), where it breaks (§5), and how to find out (§6–§8).

## 2. What it is

Words, beyond those of the earlier notes:

- **Step:** one narrowing of a set: a word clause, a facet value, a
  cluster, or a label from a taught method's classifier.
- **Chain:** steps joined with AND, read left to right, each with the
  count of what is left.
- **Beam:** the best *k* partial chains kept at each depth; each is
  extended by one step, the children scored, the best *k* kept again.
- **Score:** what makes a chain good (below).
- **Subgroup:** data mining's word for the set a chain describes.

Two running examples, one from computer science and one from
biomedicine, with counts left out because none has been run:

```text
graph neural network → molecule OR drug → since 2021 → benchmark
mouse OR mus OR rat OR rattus → knockout → since 2015 → real-time PCR
```

**Candidate steps at each depth:**

- **Words:** those unusually common in the current set against its
  parent (search-within-results §6, item 5).
- **Facet values:** today's roots are year, form, genre, language,
  status, held_by and access (P, `crates/search/src/schema.rs`); venue
  joins once it has an id (serving plan, per report 1), MeSH once it is
  indexed, entity ids later.
- **Clusters,** from [the shape of a result set](shape-of-a-result-set.md).
- **Labels,** from a taught method's classifier
  ([the set builder](recall-accounted-set-builder.md) §3).
- **Predicates a model proposes** in plain language, as D5 does (§4.4).

Each candidate costs a count within the set; the index already counts
every match in one pass (P, `crates/search/src/index.rs`), and searching
within a set waits on the query language's second pass (serving plan
§11, item 36). DSSD's defaults, a beam of 100 and a depth of 5, are a
starting point ([P] via Proença et al., report 2).

**Two families of score:**

- **Without marks, the machine's idea of interesting:** a step that
  splits the set evenly (information), a slice whose facets or words
  look unlike its parent's, a slice whose works sit close together in
  vector space. The user's thesis on labels
  ([searching within a result set](search-within-results.md) §9) is
  about exactly this: they are someone's judgement, here the machine's.
- **With marks, agreement with the user:** weighted relative accuracy,
  WRAcc = p(Cond) · (p(Class | Cond) − p(Class)), the size of the slice
  times how much richer in on-topic marks it is than its parent
  ([CN2-SD, JMLR 2004](https://www.jmlr.org/papers/volume5/lavrac04a/lavrac04a.pdf),
  [P]). A chain that scores well writes a readable boolean query that
  agrees with the user's marks.

## 3. What it offers that a classifier does not

- **A method that can be read, edited and published.** A chain is a
  query. Systematic reviews publish their boolean queries "for
  reproducibility purposes; for example, when the recommendation of a
  systematic review is in doubt or a review needs to be updated" (P,
  Scells, Zuccon and Koopman 2019). A classifier's boundary cannot be
  read; a chain can be argued with.
- **An explanation of the classifier.** Chains fitted to the
  classifier's decisions, not the marks, describe what a taught method
  mostly means ("knockout, not reviews, since 2015"): a readable stand-in
  beside the opaque one. The conversation's reasoning, not a tested
  design.
- **Exploration before any marks.** Scored without marks, cuts show ways
  to slice a pile nobody has marked yet: the user's thirty million
  papers (search-within-results §1).
- **A structured menu for agents** (§6.2).
- **Disagreement as a finding.** Where a chain and the classifier differ,
  the classifier has learned something the words do not say, or the
  words catch something it missed (search-within-results §11.1).

## 4. What is known

### 4.1 Subgroup discovery

- **CN2-SD** ([JMLR 2004](https://www.jmlr.org/papers/volume5/lavrac04a/lavrac04a.pdf),
  [P]): WRAcc, and weighted covering, where rows already covered by
  earlier rules count for less; on 23 UCI data sets, far fewer rules,
  higher coverage and significance, and slightly better AUC than CN2.
- **A survey** joining contrast sets, emerging patterns and subgroup
  discovery (Kralj Novak, Lavrač and Webb,
  [JMLR 2009](https://www.jmlr.org/papers/v10/kralj-novak09a.html), [A]).
- **DSSD** ([van Leeuwen and Knobbe, DMKD 2012](https://link.springer.com/article/10.1007/s10618-012-0273-y),
  [A]): three kinds of redundancy (description, cover, compression),
  each with a selection step inside the beam; judged on how diverse the
  covers are, the best quality found, and runtime.
- **SSD++** ([Proença, Grünwald, Bäck and van Leeuwen, DMKD 2022](https://arxiv.org/abs/2103.13686),
  [P]): subgroup lists under minimum description length, where the
  greedy gain is a Bayesian test plus a multiple-testing penalty; its
  own table marks CN2-SD and DSSD as without statistical guarantees.
- **Exceptional model mining** (Leman, Feelders and Knobbe, 2008;
  [Duivesteijn, Feelders and Knobbe, DMKD 2016](https://dl.acm.org/doi/10.1007/s10618-015-0403-4),
  [A]): subgroups where a model fitted to the slice differs from one
  fitted to everything; a generalization worth keeping in view.
- **Interactive, the user as scorer:** Dzyuba and van Leeuwen
  ([IDA 2013](https://link.springer.com/chapter/10.1007/978-3-642-41398-8_14),
  [A]) let the user like or dislike subgroups at each level of the beam;
  the feedback prunes the beam and re-weights the quality measure. That
  is §1's second mode, nearly exactly. Divisi
  ([CHI 2025](https://arxiv.org/abs/2502.10537), [A]): a think-aloud
  study with 13 practitioners, no numbers.
- **On text:** subgroup discovery over topic-model distributions
  (Hendrickson, Wang and Atzmueller,
  [ISMIS 2018](https://link.springer.com/chapter/10.1007/978-3-030-01851-1_44),
  [S]); rule search guided by an ontology's hierarchy,
  [Hedwig](https://github.com/anzev/hedwig) ([S]), which bears on MeSH.
- **Not found:** subgroup discovery applied to search result sets or to
  bibliographic retrieval.

### 4.2 Readable boolean queries learned from labels

- **Kim, Seo and Croft** ([SIGIR 2011](https://ciir-publications.cs.umass.edu/getpdf.php?id=974),
  [P]): a decision tree separates the top 100 pseudo-relevant documents
  from the rest, and each positive path becomes an AND query with NOTs,
  176–238 candidates per topic, ranked by predictors of query quality.
  On US patents and OHSUMED, the best of the top ten suggestions beat
  the baseline's recall at 100 (0.2991 against 0.2557 on patents; 0.4411
  against 0.4377 on OHSUMED), but the top suggestion alone did worse
  (0.2227; 0.3068), and only 4.6–7.8 % of generated queries matched the
  baseline's recall. The lesson for this tool: a menu of chains, not one
  answer.
- **Scells, Zuccon and Koopman** ([IR Journal 2021](http://ielab.io/publications/pdfs/scells2020comparison.pdf),
  [P]): automated versions of the "objective" and "conceptual" methods
  on CLEF TAR 2018, with seed studies split into development, validation
  and unseen parts, 30 random splits per topic. No automatic method beat
  the experts: expert recall 0.934 at precision 0.0217; the best
  automatic recall 0.878 at precision 0.0002. Objective beats
  conceptual, but conceptual queries are the better start for manual
  refinement. Their refinement paper (WWW 2019) is a beam of width one
  that traded recall 0.84 → 0.67 for precision 0.021 → 0.042
  ([searching within a result set](search-within-results.md) §11.3, P).
- **The Hedges filters** (Haynes et al.,
  [BMC Medicine 2004](https://pmc.ncbi.nlm.nih.gov/articles/PMC441418/),
  [A]): boolean filters fitted to 49,028 hand-rated articles from 161
  journals, 83 % sensitivity at 83 % specificity for prognosis, built
  into PubMed's Clinical Queries. The precedent that a boolean filter
  fitted to a labelled set can ship and last.
- **In databases:** Query-by-Output (SIGMOD 2009, [S]) rebuilds a
  selection query from example output with a decision tree; Interactive
  Set Discovery ([EDBT 2023](https://arxiv.org/abs/2111.09917), [A])
  reaches a target set with the fewest accept-or-reject questions.

### 4.3 Suggesting facets and refinements

- **Roy et al.** ([CIKM 2008](https://ranger.uta.edu/~gdas/websitepages/preprints-papers/faceted.pdf),
  [P]): the cost is the expected number of facet questions to isolate a
  target; the cheapest tree is NP-hard to find; a greedy method, weighted
  by the chance the user can answer, beat its baselines in simulation
  only.
- **Kong and Allan** ([CIKM 2014](https://maroo.cs.umass.edu/pub/web/getpdf.php?id=1164),
  [P]): simulated facet feedback with a time-cost model of the user.
  Annotators picked only 44 % of the terms that would have helped, and
  only 28 % of what they picked helped. People as scorers are noisy,
  which §1's second mode has to allow for.
- **User studies:** Flamenco (CHI 2003, [S]), 90 % of 32 students
  preferring facets over 35,000 images; SciNet
  ([Ruotsalo et al., TOIS 2018](https://dl.acm.org/doi/10.1145/3231593),
  [A]), interactive intent modelling over scientific literature, with
  significant task improvements. Chakrabarti, Chaudhuri and Hwang
  ([SIGMOD 2004](https://dl.acm.org/doi/abs/10.1145/1007568.1007653),
  [A]) and Facetedpedia (WWW 2010, [S]) give cost models for navigating
  categories.

### 4.4 Models and agents

- **D5** ([Zhong et al., NeurIPS 2023](https://arxiv.org/abs/2302.14233),
  [P]): a model proposes plain-language predicates that separate two
  corpora and a validator scores them; with GPT-4, 27 % accuracy on its
  synthetic set with a goal and a validator, 5 % with neither; final
  discoveries tested on held-out samples at a 10 % false-discovery rate
  (Benjamini–Hochberg), 13 of 21 kept.
- **HypotheSAEs** ([ICML 2025](https://arxiv.org/abs/2502.04382), [P]):
  sparse-autoencoder features over text embeddings, chosen by how well
  they predict a target and labelled by a model; 45 of 60 hypotheses
  significant under a Bonferroni threshold.
- **PaperPilot** ([arXiv, Jul 2026](https://arxiv.org/abs/2607.00597),
  [A]): an editable graph of literature-search steps refined by the
  user's feedback; Hit@5 from 58.0 to 77.0. The nearest thing to §1's
  second mode for literature.
- **LLM-generated facets** (LREC-COLING 2024,
  [ACL Anthology](https://aclanthology.org/2024.lrec-main.519/), [S]).
- **Not found:** a model joined to beam-search subgroup discovery. D5
  and HypotheSAEs are the nearest.

## 5. The hard part: false discoveries

**Every clause looks significant.** Van Leeuwen and Ukkonen
([Discovery Science 2016](https://eda.liacs.leidenuniv.nl/publications/2016/expect_the_unexpected_significance_of_subgroups-vanleeuwen,ukkonen.pdf),
[P]) show that testing a subgroup against random subsets is too weak:
real description languages always contain high-quality subgroups. For
this tool, a word clause that differs from its parent set will almost
always look "significant", so a beam of hundreds of candidates will
always find impressive cuts. What pattern mining does about it:

- **Search on one half of the marks, test on the other.** Webb (Machine
  Learning 2007, [S], described in the tutorial below, [P]): either
  correct over the whole search space or test only the chosen chains on
  a holdout. The held-back marks the set builder needs serve here too.
- **Looser thresholds for shorter chains.** Webb's layered critical
  values (2008) split the threshold by depth, which maps onto chain
  length.
- **When the user looks at many cuts,** each view is an implicit test and
  the usual corrections do not apply directly; AWARE
  ([SIGMOD 2017](https://arxiv.org/abs/1612.01040), [A]) and SliceFinder
  ([ICDE 2019](https://arxiv.org/abs/1807.06068), [P]) control the rate
  of false discoveries with α-investing, spending a budget of error as
  the user explores.
- **Permute the marks** to see how often a quality measure finds
  "discoveries" in noise (Duivesteijn and Knobbe, ICDM 2011, [S]).
- A tutorial covering these and more: Hämäläinen and Webb
  ([DMKD 2019](https://arxiv.org/abs/1709.03904), [P]).

**Near-duplicates** (DSSD's three redundancies) and **noisy human
scorers** (Kong and Allan's 44 %) are the other two. A chain's samples of
kept and dropped (search-within-results §6) are the plain check a person
can make on any of them.

## 6. Why each audience

### 6.1 Researchers

The librarian's numbered lines with a proposal engine beside them: ways
to cut a pile before anything is marked, and, once marks exist, a
readable query that agrees with them, which can be edited, argued over
and published with the review. SciNet's result is the one found for
interactive steering over scientific literature ([A]).

### 6.2 Agents

- **A menu, not a blank page.** An agent that picks among *k* chains
  with counts spends fewer model calls than one inventing queries blind,
  and Kim's result (the best of ten good, the first alone not) fits an
  agent choosing among proposals.
- **Vocabulary is where agents help the tool.** A model can propose
  candidate clauses and plain-language predicates (D5), which the beam
  then scores by counts.
- **One compact answer.** HarnessTax's finding, that tool schemas and
  answer sizes are paid by every agent on every call
  ([configuration](future-ambitions-configuration.md), per report 1),
  argues for one call returning a few chains, not a tool per step.

### 6.3 Benchmarks

- **No shared benchmark for steps to a target set** in scholarly search
  was found (report 2). The nearest protocols: the expected number of
  questions (Roy), a time-cost user model (Kong and Allan), the fewest
  interactions (Interactive Set Discovery), and a subtopic's reach time
  for clustered results (Carpineto et al., IP&M 2012, [S]).
- **The mark-scored mode can be measured now,** with Scells 2021's
  protocol: marks from some of a review's included studies go in, the
  chain is scored on the unseen ones. Against the experts' own queries
  and against model-written ones, on SR4CS (1,212 computer-science
  reviews with their expert boolean queries), CLEF TAR, the Seed
  collection and SYNERGY's fields ([retrieval benchmarks](retrieval-benchmarks.md)).
  The keys are the reviews' included studies, independent of this tool;
  their pooling bias is the same as in the set builder's §6.
- **The unmarked mode and the branches mode need a simulated user:**
  steps to reach a slice holding a stated share of a review's included
  studies at a stated precision, in the manner of Roy and of Kong and
  Allan; then people.
- **Tempered:** the record says learned boolean queries do not beat
  experts (Kim; Scells 2021). The result worth testing is a readable
  chain that keeps the recall model-written queries lose, for fewer
  model calls, not a leap past the experts.

## 7. Beside the other two tools

- **The set builder** ([note](recall-accounted-set-builder.md)): cuts
  scored against marks are the readable face of its `label()`; both use
  the same marks, and its `missing()` applies to a chain's set as to any
  other.
- **The shape** ([note](shape-of-a-result-set.md)): its clusters are
  candidate steps, its distinctive words candidate clauses.
- **Order:** the set builder first, as its note argues; the unmarked mode
  could come early, since it needs only counts and distinctive words.

## 8. A path, measurement first

1. **Candidates and their counts:** distinctive words of a set against
   its parent, and facet values, each counted within the set. Needs
   searching within a set (serving plan §11, item 36).
2. **Mark-scored chains, offline:** Scells 2021's split protocol on
   SR4CS, CLEF TAR and the Seed collection; learned chain against the
   experts' query and a model-written one; recall on the unseen included
   studies, precision beside it, each chain's score retested on held-back
   marks.
3. **False-discovery control from the start:** held-back marks, layered
   thresholds by depth, and a permutation check of the score.
4. **The unmarked mode and the branches mode** under a simulated user,
   then with people.
5. **Agents:** the menu of chains as one tool, measured as in
   [searching within a result set](search-within-results.md) §12.5,
   model calls against recall.

## 9. Open

1. Which candidate sources, and what each costs: words, facet values,
   clusters, labels, predicates a model proposes.
2. The beam's width and depth; DSSD's 100 and 5 to start.
3. The score for the unmarked mode.
4. The false-discovery procedure: holdout, layered thresholds,
   α-investing, permutation, or a mix.
5. How to show a few overlapping chains to a person, and as one compact
   answer to an agent.
6. The simulated user for the unmarked and branches modes.
7. Whether chains may use NOT, as Kim's paths did, and stay readable.
8. Languages. A word clause in one language misses works written in
   another, and a work not in English is no reason to leave it out (the
   user, 2026-09-28). Concept steps (MeSH, entity ids, a taught method's
   labels) do not depend on the language of the words; clauses may want
   their translations, or the tokenizer project's help
   ([text analysis](text-analysis.md)).
