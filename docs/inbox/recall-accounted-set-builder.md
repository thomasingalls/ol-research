# The recall-accounted set builder

**Status:** idea note, 2026-09-28, recorded at the user's request. It
holds the conversation's answer to a question the user put at the end of
the conversation behind [searching within a result set](search-within-results.md):
which one specialist tool looks most promising. The recommendation is
the conversation's, not a decision, and nothing here is built, scheduled
or decided; nothing in it amends the plans of record. Scope, the user's:
academic literature, and associated scientific data later. Facts are
marked **P** (primary: this repository, or a paper read directly on
2026-09-28), **S** (secondary: read through a search engine's summaries
on 2026-09-28) or **U** (from memory, not checked). The bracketed tags
**[P]**, **[A]** and **[S]** are the research reports' own (full source,
abstract only, summary only); the reports are kept verbatim in
[search-within-results-agent-reports/](search-within-results-agent-reports/).
Related: [searching within a result set](search-within-results.md) §6
(what a step can show), §10 (the funnel of a strong model and a cheap
one), §11.1 (cuts scored against marks), §12 (agents and set tools) and
§12.5 (the testable claim); the [evaluation plan](../evaluation-plan.md)
§3.4; [retrieval benchmarks](retrieval-benchmarks.md) (the
systematic-review collections); [candidate management](candidate-management.md)
§1; the [serving plan](../serving-plan.md) §11, item 36.

## 1. The question

In the user's words:

> given all of this, what seems like the most promising specialist tool
> that would (1) be useful to human researchers, (2) be useful to
> agents, (3) blow retrieval/agentic search benchmarks out of the water

## 2. The answer

A set builder that keeps account of recall: an explicit edge that can be
read and edited; counts and samples of what each step keeps and drops; a
cheap classifier trained on marks; and an estimate, with an interval, of
what the set is still missing. It looks the most promising on all three
counts, with one tempering: on benchmarks, the likely win is equal
recall for far fewer model calls, not a leap in raw recall (§6).

## 3. The tool

Words, beyond those of [searching within a result set](search-within-results.md) §2:

- **Edge:** the explicit definition of a set: words, filters, and later
  the query language's AND, OR and NOT. Readable and editable.
- **Marks:** judgements of single works as on topic or off, by a person
  or by a strong model.
- **Recall:** the share of all the relevant works that a set holds.
- **Elusion:** the share of relevant works among those a step left out.

Four operations:

```text
within(query)        → a set with an explicit edge, and its count
step(set, clause)    → count kept, count dropped, a random sample of each
label(set, marks)    → a cheap classifier trained on the marks, applied
                       to every work in the set
missing(set)         → estimated recall, with an interval, from samples
                       of what was left out (§5)
```

It mirrors what reviewers already do, and adds the piece no search tool
offers:

1. **An edge drawn wide, for recall.** Expert queries for systematic
   reviews sit near 2 % precision on purpose: 0.021 on CLEF TAR
   ([reassessment, SIGIR 2025](https://arxiv.org/abs/2505.07155), [P]),
   and 0.021 on the 125-review collection of Scells, Zuccon and Koopman
   2019 (P, their Table 3).
2. **A classifier inside the edge, for precision:** screening, done by a
   cheap model trained on a few hundred marks.
3. **Sampling, for accounting:** how much the edge and the classifier
   have left out. This is the new part.

The pieces come from [searching within a result set](search-within-results.md):
the samples of kept and dropped (§6, items 1 and 2), the funnel of a
strong model for the few and a cheap one for the many (§10), and cuts
scored against marks (§11.1).

## 4. Why it meets each criterion

### 4.1 Researchers

"Give me the set, and tell me how much I am missing" is the systematic
reviewer's job, and it is the user's pile of thirty million papers
(search-within-results §1), sliced with a number on every cut. A search
box has never answered the second half; the hit count answers neither
(§4 of that note).

### 4.2 Agents

It fits what agents are good and bad at.

- **Good at vocabulary.** Writing an edge suits them. But the edge is
  where they lose recall: on CLEF TAR, o1's queries reached recall 0.655
  against the experts' 0.844; on the Seed collection, guided by known
  relevant studies, 0.736 against 0.724; AutoBool, a small model trained
  against PubMed, 0.70
  ([reassessment, SIGIR 2025](https://arxiv.org/abs/2505.07155), [P];
  [AutoBool, EACL 2026](https://arxiv.org/abs/2602.00005), [P]). So
  `missing()` is not a nicety for agents: it is what catches the recall
  their edges lose (§5, account b).
- **Bad at knowing what they have missed.** No literature agent found in
  the research is told what it left out (search-within-results §12.1).
- **Expensive at reading.** Reading is where the money goes: per query,
  $0.003 for OpenScholar on its own 8B model, $0.05 on GPT-4o, and
  $0.3–2.3 for PaperQA2 ([OpenScholar](https://arxiv.org/abs/2411.14199),
  [P]); Undermind reads about 150 full texts per search
  ([whitepaper](https://www.undermind.ai/whitepaper.pdf), [P]); PaSa
  takes about 382 search and expand actions per query and reads every
  queued paper ([PaSa](https://arxiv.org/abs/2501.10120), [P]).
  `label()` replaces most of the reading: a strong model marks a few
  hundred works, a cheap one decides the rest.

The agent's loop becomes: write the edge, mark a sample, check what is
missing, widen or cut, again. It does not read a thousand papers.

### 4.3 Benchmarks

The headroom is in exhaustive recall, where the best scores are poor:

- ScholarQuest ([arXiv, Jun 2026](https://arxiv.org/abs/2606.20235),
  [P]): the best agent finds 0.355 of everything; dense retrieval alone
  0.290; a hybrid of BM25 and dense 0.244.
- AutoResearchBench ([arXiv, Apr 2026](https://arxiv.org/abs/2604.25256),
  [A]): 9.31 % set overlap on its wide task.
- WideSearch ([arXiv, Aug 2025](https://arxiv.org/abs/2508.07999), [P]):
  the best agent succeeds 5.1 % of the time.

Top-*k* search plus reading cannot reach the long tail within a budget.
Finding everything at a bounded cost is what legal technology-assisted
review and systematic-review screening were built for:

- A strong model plus continuous active learning, where a classifier is
  retrained as marks arrive: 80.5 % of the screening work saved at 95 %
  recall, against 71.4 % for the standard method, on one dataset of
  4,836 records ([Bron et al., IAL 2024](https://ceur-ws.org/Vol-3770/paper8.pdf),
  [P]).
- Zero-shot screening calibrated to a target recall: recall 0.93–0.97
  with 50–54 % of the work saved, though only half the topics met the
  95 % target ([ECIR 2024](https://arxiv.org/abs/2401.06320), [P]).
- A model's scores seeding active learning: 42–43 % of the work saved at
  near-perfect recall over 28 reviews (Akinseloyin et al., medRxiv 2025,
  [S]).

The research found no one joining those methods to agents on the
literature benchmarks: no benchmark result for agents with count or
facet tools, no classifier distilled from model judgements measured on
them, and no peer-reviewed continuous active learning with a model as
the marker (reports [2](search-within-results-agent-reports/02-agents-and-set-tools.md)
and [3](search-within-results-agent-reports/03-benchmarks-and-cost.md)).
The agent world and the review world are solving the same "find all of
it" problem apart. This tool is the bridge between them.

## 5. The hard part: knowing what is missing

**The elusion test (S).** Legal e-discovery estimates what a review left
behind by sampling its "discard pile", the documents ranked below the
cutoff and never coded, and reporting the share of relevant ones with a
margin of error
([Relativity's documentation](https://help.relativity.com/Server2025/Content/Relativity/Active_Learning/Elusion_Test.htm)).
Its known weakness: it presumes the first pass was coded correctly
([ACEDS, "The Elusion Illusion"](https://aceds.org/the-elusion-illusion-and-the-ai-revolution-aceds-blog/)).
Here the marks would often come from a model, and models disagree with
themselves: two identical GPT-5.4 screening runs differed on 94 records,
29 of them eligible ([arXiv, Aug 2026](https://arxiv.org/abs/2608.26885),
[A]).

**The target method (S).** Cormack and Grossman (2016) draw documents at
random until they hold 10 relevant ones, then review until all ten are
found; ten targets guarantee recall of at least 0.7 with 95 % confidence
([as described in "Certifying One-Phase Technology-Assisted Reviews"](https://arxiv.org/abs/2108.12746)).
Higher recall wants more targets.

**Why the edge matters to the accounting.** Across a whole corpus,
relevant works are rare, so a random sample finds almost none of them,
and drawing ten targets at random means reading a great many works.
Inside a well-drawn edge, relevant works are common enough to sample
(experts' edges hold about 2 % relevant, §3). Hence two accounts, the
conversation's reasoning and not a tested design:

- **(a) Inside the edge:** the classifier's misses. Sample what it
  rejected, weighted toward the works it was least sure of; estimate
  elusion there, with an interval.
- **(b) Beyond the edge:** the edge's own misses, which is where agents
  lose recall (§4.2). A cheap classifier is cheap enough to score works
  outside the edge too; sample where it scores them high. That estimates
  what the edge left out, and finds some of it.

The interval's honesty is the tool's core requirement, not a nicety: a
confident wrong "you have 95 %" is worse than no number. Stopping rules
are a field of their own, to read before designing `missing()`:
[certifying one-phase reviews](https://arxiv.org/abs/2108.12746) (S),
[stopping by point processes](https://dl.acm.org/doi/full/10.1145/3631990)
(S), [counting processes with classification](https://arxiv.org/abs/2312.03171)
(S), and [confidence-based stopping, SIGIR 2026](https://arxiv.org/html/2606.15380)
([A], report 3: 85.3 % of documents saved at 92.6–97 % agreement with
full review).

## 6. Where to temper "out of the water"

- **Answer keys penalize finding what they missed.** ScholarQuest's key
  was built by retrieval, citation expansion and model adjudication
  ([P]); the review collections' keys are the studies found by the
  reviews' own boolean queries. A tool that finds relevant works outside
  those pools is scored wrong for them. Scells 2019's bounds show the
  size of it: recall 0.67 counting unjudged citations as irrelevant, 0.71
  estimated, 0.92 if all were relevant (P).
- **So the measurable win is cost.** Equal recall for a fraction of the
  model calls is the likelier result than a leap in raw recall, which is
  the user's affordability hypothesis and search-within-results §12.5's
  claim; the evidence is a cost–recall curve per arm.
- **Nothing here is measured.** The most promising candidate, not a
  proven one. The rule that an answer key be independent of what it
  tests ([evaluation plan](../evaluation-plan.md) §3.4) points to the
  review collections over ScholarQuest, with their pooling bias read
  first.

## 7. Why not the others first

- **Counts per clause alone.** Cheap and useful, and the tool's front
  end; alone they give size, not relevance, the user's complaint about
  the hit count. Boolean tools for agents so far show modest gains with
  fewer tokens: Sieve, +0.7 to +3.1 points with 20.7–50.6 % fewer tokens
  ([arXiv, Aug 2026](https://arxiv.org/abs/2608.02751), [P]).
- **Suggested cuts (search-within-results §11.1).** Novel, and good for
  exploring; but the good score is agreement with marks, which leads
  back to the classifier, and many overlapping cuts are hard to show.
  Its own research program: [suggested cuts](suggested-cuts.md).
- **Shape (search-within-results §8).** The best first look for a
  person; the weakest evidence (citation clusters on systematic reviews
  were "highly variable",
  [Bascur et al., 2022](https://arxiv.org/abs/2207.03299), [A]) and the
  least use to an agent. Its own research program:
  [the shape of a result set](shape-of-a-result-set.md).

## 8. A path, measurement first

Each step measurable on its own, in the order the evaluation plan wants:

1. **Counts, and samples of kept and dropped,** over what the index
   already does: it counts every match in one pass (P,
   `crates/search/src/index.rs`). Needs filters and searching within a
   set (serving plan §11, item 36).
2. **A systematic-review collection in the evaluation plan,** described
   in [retrieval benchmarks](retrieval-benchmarks.md): CLEF TAR or the
   Seed collection, human-judged keys over PubMed, which the index holds;
   and beyond biomedicine, which is no reason to leave a set out (the
   user, 2026-09-28: "pubmed was just a convenient place to start"),
   SR4CS (1,212 computer-science reviews with their expert boolean
   queries, references by DOI, CC BY 4.0) and SYNERGY (26 reviews across
   medicine, psychology, computer science, biology, chemistry and
   mathematics, CC0), as Crossref and OpenAlex bring their fields into
   the corpus. None is among the plan's published sets (E3) yet.
3. **`label()`,** on vectors or GLiNER2 (search-within-results §8 and
   §10), **and `missing()`,** its two accounts (§5).
4. **The control-against-treatment test** of search-within-results
   §12.5: a cost–recall curve per arm, paired intervals, a frozen test
   half.

## 9. Open

1. The name.
2. The classifier: vectors (whose idea of similar,
   search-within-results §9), GLiNER2, or both.
3. Who marks: the user, a strong model, or both; and how the account
   carries the marks' own noise (§5).
4. The target recall, and which stopping rule gives the interval.
5. Account (b)'s reach: scoring works outside the edge means a cheap
   classifier over much of the corpus, on a box shared with the search
   engine.
6. Which collection first.
7. How `missing()` is shown to a person: a number, its interval, and a
   sample to read.
8. For agents, one call or four: the one-call front door of
   [future ambitions](future-ambitions.md) ("expand, filter, score, keep
   the top *k*") against separate tools, given HarnessTax's finding that
   tool schemas and answer sizes are paid by every agent on every call
   ([configuration](future-ambitions-configuration.md)).
