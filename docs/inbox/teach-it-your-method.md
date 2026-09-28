# Teach it your method

**Status:** research-program note, 2026-09-28, a workup of the user's
pitch, written at the user's request ("the teach it your method pitch
should be given a full workup treatment like the others, given what we
currently know"). A companion to [the recall-accounted set builder](recall-accounted-set-builder.md),
[suggested cuts](suggested-cuts.md) and [the shape of a result set](shape-of-a-result-set.md).
Speculative, not a plan: nothing here is built, scheduled or decided,
and nothing in it amends the plans of record. Scope, the user's:
academic literature as the wedge, for academics and researchers, with no
kind of artifact out of scope in principle for its document type (see
[searching within a result set](search-within-results.md)'s status).
Facts are marked **P** (primary: this repository, or a source read
directly on 2026-09-28), **S** (secondary: read through a search
engine's summaries that day) or **U** (from memory, not checked). The
bracketed tags **[P]**, **[A]**, **[S]** and **[K]** are the research
reports' own (full source, abstract only, summary only, the reporter's
own knowledge unchecked); the reports are kept verbatim in
[teach-it-your-method-agent-reports/](teach-it-your-method-agent-reports/):
[1, the project's own notes](teach-it-your-method-agent-reports/01-repository-notes.md)
and [2, the evidence outside](teach-it-your-method-agent-reports/02-evidence-outside.md);
a few figures come from the first note's
[reports](search-within-results-agent-reports/), linked where used.
Report 2's web searches were exhausted before it began; it read primary
pages and the Europe PMC and arXiv APIs directly, so its "not found" is
weaker than usual. Its own arithmetic of corpus-scale costs is left out
here: this project records measured figures, not estimates. Related:
[searching within a result set](search-within-results.md) §9 (labels, and
who makes them), §10 (the funnel), §12 (agents and set tools);
[classification feedback](classification-feedback.md) (the blind spot a
labelled sample closes); [product shapes](product-shapes.md);
[the serving plan](../serving-plan.md) (freshness and the batch clock);
the [evaluation plan](../evaluation-plan.md) §3.4.

## 1. The pitch, in the user's words

> the thing that has struck me now, is that if i were to pitch an
> academic on why they would want to be able to use my tool, i think the
> most compelling thing i could say is 'you can teach the search engine
> your method / your criteria and it will apply it faithfully. you can
> refine it over time, and even have more than one methodology. and you
> can start from the subset of journals/researchers/whatever that
> interest you' - and i want to distinguish this idea from "hey that's
> just skills files" because no it's not, we've already established that
> search over very large corpora is likely to require some training or
> otherwise ml work.
>
> you already know how to do it, this thing can do what need how you like
> and doesn't get tired doing it.

It is also the product form of the user's thesis on labels
([searching within a result set](search-within-results.md) §1, §9):
labels mean something only to the person who made them, so the labels
worth computing are the asker's own, made by the asker and kept for the
asker. Pre-computing them is fine; pre-computing someone else's is what
fails.

## 2. The claim

In one line: **teach it your method; it applies the method to
everything, tirelessly, and tells you how faithfully.**

The pitch makes six promises; each needs a mechanism (§6):

1. You can teach it your method or criteria.
2. It applies them faithfully.
3. You can refine the method over time.
4. You can keep more than one method.
5. You can start from the subset that interests you.
6. It does not tire.

## 3. What a taught method is

Words:

- **Method:** the thing taught, kept as an artifact (below).
- **Marks:** the researcher's judgements of single works, on topic or
  off, include or exclude.
- **Description:** the criteria in the researcher's own words.
- **Edge:** where the method starts: journals, researchers, years,
  fields, a query ([the set builder](recall-accounted-set-builder.md) §3).
- **Agreement:** how often the method agrees with marks it was not
  trained on, with an interval.

**The artifact.** A method is: its edge, its description, its marks
(those trained on and those held back), the model and its version, the
epoch it last ran at, and its measured agreement. Same artifact, same
epoch, same answer.

**How it is taught** ([searching within a result set](search-within-results.md)
§10, the funnel):

1. The researcher writes the description and marks a few dozen works.
2. A stronger model labels a few hundred more from a sample, using the
   description.
3. A cheap classifier learns the boundary: a logistic regression over
   vectors, a SetFit model, or GLiNER2 tuned with LoRA.
4. It scores every work inside the edge.
5. The researcher checks three samples, what was kept, what was dropped
   and what sits near the boundary, marks them, and it learns again: a
   new version, with its agreement measured on held-back marks.
6. The set builder's `missing()` estimates what the method's set still
   lacks ([the set builder](recall-accounted-set-builder.md) §5).
7. Each batch of new works is scored as it arrives (§6.3).

A readable face is optional: [suggested cuts](suggested-cuts.md) can
write a chain of steps that approximates the method, so the researcher
can see what it mostly means.

## 4. Not a skills file

A skills file is plain-language instructions for a model, applied by the
model reading each work. At corpus scale that fails in five ways, and a
trained method answers each:

| | a skills file | a taught method |
|---|---|---|
| applied by | a model reading each work | a classifier scoring each work |
| consistency | varies run to run | same version, same answer |
| wording | small changes move results | the boundary is learned from marks |
| model versions | results move when the host changes models | the version is pinned in the artifact |
| faithfulness | asserted | measured on held-back marks |
| cost | paid per work, per run, per model change | one multiply-and-add per work once vectors exist |

**The evidence (report 2 unless marked):**

- *Consistency.* Rerunning the same prompts for systematic-review
  queries with different seeds was "neither reproducible nor reliable"
  (Staudinger et al., SIGIR-AP 2024, [P]). Five models at "deterministic"
  settings varied by up to 15 % in accuracy over ten runs (Atil et al.,
  [arXiv 2408.04667](https://arxiv.org/abs/2408.04667), [A]). At
  temperature 0 one model gave 80 distinct completions in 1,000 runs,
  because the serving kernels are not batch-invariant (Thinking Machines,
  Sep 2025, [P]). Two identical GPT-5.4 screening runs disagreed on 94
  records, 29 of them eligible
  ([the set builder](recall-accounted-set-builder.md) §5, [A]).
- *Wording.* Formatting alone moved accuracy by up to 76 points (Sclar
  et al., ICLR 2024, [A]). In screening, performance depended on how
  clearly criteria were written (Delgado-Chaves et al., PNAS 2025, 18
  models, [A]), and "minor adaption" of the setup mattered (Dennstädt et
  al., Systematic Reviews 2024, [A]). Elicit traced about half its 2025
  misses to badly generated criteria (its own evaluation, [P]).
- *Model versions.* GPT-4's accuracy at identifying primes fell from
  84 % to 51 % between its March and June 2023 versions (Chen, Zaharia
  and Zou, [arXiv 2307.09009](https://arxiv.org/abs/2307.09009), [A]). In
  screening, GPT-4 Turbo's specificity was 0.98 against GPT-3.5's 0.51 at
  similar sensitivity (Oami et al., JMIR Medical Informatics 2025, [A]).
  A benchmark whose judge moved from Gemini-2.5-Pro to GPT-5.5 no longer
  compares old scores with new
  ([retrieval benchmarks](retrieval-benchmarks.md), DeepResearch Bench, P).
- *Accuracy.* Fine-tuned BERT-style models "significantly outperform"
  zero-shot GPT-3.5, GPT-4 and Claude Opus on several classification
  tasks (Bucher and Martini, [arXiv 2406.08660](https://arxiv.org/abs/2406.08660),
  [A]). A logistic regression over embeddings, trained on GPT-4's labels,
  matched GPT-4 on a human-labelled test (0.658 against 0.655; TnT-LLM,
  [arXiv 2403.12173](https://arxiv.org/abs/2403.12173), [P]). SetFit with
  8 examples per class was competitive with RoBERTa-large fine-tuned on
  3,000, in 30 seconds of training
  ([arXiv 2209.11055](https://arxiv.org/abs/2209.11055), [P]).
- *Cost.* Screening with GPT-5 cost $0.002–0.0036 per abstract
  (Vivekanantha et al., 2025, [A]), paid again for every work, every
  rerun and every change of model; Wang et al. estimated GPT-4 would have
  cost about $80,000 for their experiments
  ([ECIR 2024](https://arxiv.org/abs/2401.06320), [P]).

**Where prose still belongs.** The description bootstraps the first
labels (§3 step 2); a model can take the queue of works the classifier
is unsure of; a model can explain a disagreement. What applies the
method to millions of works is trained.

## 5. What exists, and the gap

**Tools that learn from a researcher's marks, inside one review's
imported set** (report 2):

- **ASReview** ([arXiv 2006.12166](https://arxiv.org/abs/2006.12166),
  [P]): active learning on the user's labels; open source and local; a
  simulation mode; an update mode where the first run's includes train
  the model before the update is screened. Across four datasets, one from
  software engineering, 83 % of screening work saved on average at 95 %
  recall (67–92 %).
- **Rayyan** (Systematic Reviews 2016, [A]): suggestions after 5 included
  studies.
- **Covidence** ([P]): "most relevant" sorting learned from votes, after
  at least 25 decisions with 2 includes and 2 excludes; it "does not
  remove any studies".
- **DistillerSR** ([P], vendor): re-ranking learned from team decisions,
  classifiers, and an audit log "with version control"; median 47.1 %
  less screening at 95 % recall across 10 reviews (Hamel et al., 2020,
  [A]); its fully automatic mode "came with increased risk of missing
  relevant records" (Gates et al., 2019, [A]).
- **Nested Knowledge** ([P]): both kinds side by side. A screening model
  of gradient-boosted trees over metadata, abstract, citations and
  embeddings, trained on the review's own decisions after 50 screened and
  10 included (its model card: AUC 0.88, recall 0.76, precision 0.40
  across hundreds of reviews), and a separate model answering yes or no
  per criterion "without prior training", capped at 5,000 abstracts.

**Following instructions rather than learning:** Elicit's systematic
review ([P], its own evaluations): a model drafts criteria the user can
edit; overrides are recorded, but nothing says they train anything;
screening capped at 5,000–40,000 papers by plan; 96.9 % sensitivity and
92.5 % specificity against 994 Cochrane reviews in 2026, up from 93.6 %
and 62.8 % in 2025.

**Over a whole corpus, and over new works:**

- **EPPI-Reviewer's OpenAlex auto-update** ([P]): machine learning finds
  records "similar to the ones already included" across OpenAlex's 260
  million records at each roughly monthly snapshot, and users apply
  "their own, or pre-built classifiers" to them. The nearest thing to the
  pitch. Its forerunner, a living map of COVID-19 research built on the
  Microsoft Academic Graph with machine learning, found 469 more eligible
  articles at one month and cost £3,179 a week less than boolean searches
  of Medline and Embase (Shemilt et al., Wellcome Open Research, [A]).
- **Semantic Scholar's research feeds** ([P], its FAQ): a paper-embedding
  model trained contrastively, with papers saved to a library folder as
  positives and "not relevant" ratings as feedback; daily, across all
  fields, from the last three months' papers; no measurement or version
  shown to the user.
- **arxiv-sanity-lite** ([P]): one SVM per user tag over TF-IDF of
  abstracts, a daily email; arXiv only.
- **Google Scholar alerts** ([P]): on a query, new citations or an
  author's new articles; nothing learned from marks.
- **Cochrane's RCT classifier** (Thomas et al., J Clin Epidemiol 2021,
  [P]): one fixed classifier, two SVMs calibrated to 99 % recall, trained
  on 280,620 records; it found 43,783 of the 44,007 randomized trials
  included in Cochrane reviews (99.5 %), and in 2018 spared Cochrane's
  crowd of screeners 185,039 of 449,480 records.

**Methods as published artifacts, the precedent:** validated search
filters. The ISSG Search Filters Resource ([P]) lists filters with
appraisals and an appraisal checklist, "not an endorsement of its
validity". The Hedges filters were validated against hand-searched
journals: 99.3 % sensitivity at 70.4 % specificity for treatment studies,
99.9 % at 52 % for systematic reviews, and strategies built in 1991 still
worked in 2000 (report 2, [A]).

**The gap.** Every tool that learns from marks does so inside one
review's set; the tools that reach a whole corpus either learn without
measuring (research feeds) or apply one fixed classifier (Cochrane's).
No tool was found that keeps a researcher's trained method as a
versioned, shareable artifact, measured against held-back marks, run
over a whole corpus and over each batch of new works. Report 2's
searching was constrained (status), so "not found" is weaker than usual.

## 6. The six promises, examined

### 6.1 Teach it

Marks and a description. Products start learning after 5 includes
(Rayyan), after 25 decisions (Covidence), after 50 screened and 10
included (Nested Knowledge); SetFit was competitive from 8 examples per
class (§4). The stronger model's first labels shorten the start but are
noisy (§4, consistency).

### 6.2 Faithfully

Make it a promise to check, not to trust:

- **Faithful means measured:** agreement with held-back marks, with an
  interval, and the set builder's estimate of what is missing. Marks used
  to train cannot also measure
  ([searching within a result set](search-within-results.md) §10).
- **What reviewers ask for:** evidence that a tool is "non-inferior or
  superior in accuracy to current practice", in precision and recall
  against human reviewers, reported reproducibly (O'Connor et al., "A
  question of trust", Systematic Reviews 2019, [P]). The joint position
  of Cochrane, Campbell, JBI and the Collaboration for Environmental
  Evidence (Flemyng et al., 2025, [P]): reviewers stay responsible, may
  need to "pilot (or calibrate)" a tool within their own review, and
  must report the tool's "name(s)… version(s) and date(s) used"; it
  endorses RAISE (Responsible AI in Evidence Synthesis). A taught method
  measured on the researcher's own marks is that pilot, built in.
- **The human baseline:** in studies found earlier, human screeners
  reached 82–83 % recall while keeping 42–45 % of records
  ([arXiv 2608.26885](https://arxiv.org/abs/2608.26885), [A], in
  [report 3](search-within-results-agent-reports/03-benchmarks-and-cost.md)
  of the first note), and dual human review 81.7 % sensitivity against a
  model screener's 96.7 % (otto-SR, medRxiv 2025, [S], in
  [report 2](search-within-results-agent-reports/02-agents-and-set-tools.md)).
- **The blind spot it closes:** the project's own
  [classification feedback](classification-feedback.md) note finds that
  a classification "uniformly, confidently wrong" can only be caught by
  "a labelled sample … or a human reading", and none is wired in (P, per
  report 1). Held-back marks are that labelled sample, for the
  researcher's criteria.

### 6.3 Refine it over time

Each refinement is a new version with its own agreement. Methods age:
living systematic reviews are "continually updated, incorporating
relevant new evidence as it becomes available" (Elliott et al., J Clin
Epidemiol 2017, [A]); PRISMA-S asks a review to report its update
methods ("rerunning searches, email alerts") and the date of its last
search (items 12 and 13; Rethlefsen et al., 2021, [P]). A method run on
each batch reports what is new that fits it. The freshness path is
there in part: an incremental batch of 4,989 records merged in 2.7 s and
was answerable 1.3 s later; but nothing yet runs fetch, merge and
publish on a clock, and scoring each batch with a method is not in the
plans (serving plan, per report 1). The user's own reason for freshness,
that the first to index a paper gets the sign-up, applies here twice.

### 6.4 More than one method

Methods side by side, each with its agreement. Where two disagree is
where the interesting works are, as with a chain against a classifier
([suggested cuts](suggested-cuts.md) §3). How methods combine (both, 
either, one but not the other) is open (§12).

### 6.5 Start from a subset

The edge. Today: year, form, genre, language, status, holding source and
access are facets, counted but not yet usable as filters (serving plan
§11, item 36); venues and authors are searchable only as text. Journals
as an edge need venue ids, and researchers, institutions and funders
need registries of their own, all deferred (per report 1). Until then,
an edge by journal or researcher is a crude match on names.

### 6.6 It does not tire

A pinned artifact gives the same answer every time; people drift and
tire (§6.2's human baseline); Cochrane's classifier took 41 % of one
year's records off its crowd's desk (§5). But consistency cuts both
ways: a tireless method with a wrong boundary is wrong everywhere,
without complaint ([the shape of a result set](shape-of-a-result-set.md)
§5). Consistency is worth what the measurement says it is worth.

## 7. The academic hook: a method you can publish

PRISMA-S asks for search strategies "copied and pasted exactly as run"
and for published filters to be cited (items 8 and 10, [P]); reviews
publish their boolean queries "for reproducibility purposes; for
example, when the recommendation of a systematic review is in doubt or a
review needs to be updated" (P, Scells, Zuccon and Koopman 2019); the
joint position asks for tool versions and dates. A taught method fits
that practice: publish the artifact (edge, description, marks, model
version, agreement), cite it as a filter is cited, and let another group
rerun it at the same epoch for the same answer, or at a later epoch for
what has appeared since. Validated search filters are the precedent
(§5): a method as a published object with its sensitivity and
specificity. A skills file cannot offer this: a prompt is interpreted
afresh on every run.

## 8. The hard parts

- **Getting started, and noisy first labels** (§6.1): the stronger
  model's labels vary between runs and models (§4).
- **Criteria that live in full text.** "Used real-time PCR" is in a
  methods section; abstracts cap how faithfully such a method can be
  taught. Full text is the gold standard, and a goal
  ([retrieval benchmarks](retrieval-benchmarks.md)).
- **New vocabulary.** A method trained on past marks meets works written
  in words it has not seen; the queue near the boundary is where the
  researcher stays in the loop. The Hedges result, filters from 1991
  still working in 2000, is encouraging for methodological filters, not
  proof for topical ones.
- **Rare relevance at corpus scale.** Across a whole corpus, relevant
  works are rare, and precision is hardest where they are rarest; the
  edge is what makes both the method and its accounting tractable
  ([the set builder](recall-accounted-set-builder.md) §5).
- **A method per researcher, on one box.** Scoring is cheap per work,
  but many methods over many works is a cost to measure on the box, not
  to estimate.
- **Terms.** A hosted model labelling abstracts sends them off the box,
  where the sources' terms carry the most risk; an on-box model does not
  (serving plan §7; GLiNER2 in
  [searching within a result set](search-within-results.md) §10).
- **Whose marks.** Marks and methods are the researcher's; sharing is
  their choice, and what can be seen by whom is a product question.
- **Trust takes evidence:** non-inferiority, reproducible reporting, a
  pilot in the researcher's own review (§6.2).

## 9. Why each audience

### 9.1 Researchers

The pitch as the user put it, with "faithfully" made checkable (§6.2)
and the method made publishable (§7). The appetite is not only
biomedical: active learning found 95 % of relevant studies after reading
"an order of magnitude fewer papers" in software engineering (FASTREAD,
[arXiv 1612.03224](https://arxiv.org/abs/1612.03224), [A]); text mining
cut screening by 88–90 % over collections of 800,000 and a million
records in public health and behavioural economics (Shemilt et al.,
2014, [A]); GPT-4 reached 100 % recall with half the screening time on
about 12,000 records in environmental evidence (Nykvist et al., 2025,
[A]); GPT-4 screened 43,480 articles in public policy (Rubinstein et al.,
2025, [A]). Tools for the humanities and education specifically were not
found.

### 9.2 Agents

A taught method is a tool an agent can call: "score these by the
researcher's method" applies the researcher's criteria without the agent
reading each work, so the agent's reading goes where the method is
unsure. Agents can also help teach, as the stronger model of §3 step 2,
with the researcher's held-back marks as the check on them.

### 9.3 Benchmarks

- **The researcher's own key:** agreement with held-back marks. The marks
  are the researcher's, not built from the method, so the key is
  independent of what it tests ([evaluation plan](../evaluation-plan.md)
  §3.4).
- **Systematic-review collections with decisions:** train on part of a
  review's include and exclude decisions, test on the rest, as ASReview's
  simulation mode does; on CLEF TAR, SYNERGY (medicine, psychology,
  computer science, biology, chemistry, mathematics) and SR4CS
  ([retrieval benchmarks](retrieval-benchmarks.md)). Measured as work
  saved at 95 % recall and cost per work, against active learning
  (ASReview's 83 % average) and calibrated zero-shot model screening
  (Wang et al., 93 % recall at a 95 % target, 50–54 % of work saved).
- **Freshness:** new eligible works found per batch by a method, against
  a boolean query rerun, as Shemilt et al. compared a machine-learning
  living map with boolean searches (§5).
- **Tempered:** vendors' figures are their own; model screening results
  vary widely by model, prompt and review (§4).

## 10. Beside the other tools

- **The set builder** ([note](recall-accounted-set-builder.md)): its
  `label()` is the method's engine, its `missing()` the method's
  account of recall, its edge the method's starting subset.
- **Suggested cuts** ([note](suggested-cuts.md)): the method's readable
  face, and a second opinion that disagrees usefully.
- **The shape** ([note](shape-of-a-result-set.md)): the first look before
  any marks, and strata for checking a method's samples.
- **Classification feedback** ([note](classification-feedback.md)): the
  same lesson from the pipeline's side; its near-miss counters are the
  queue near a method's boundary, and its blind spot is closed only by a
  labelled sample (§6.2).

## 11. A path, measurement first

1. **The edge:** facets as filters and searching within a set (serving
   plan §11, item 36).
2. **A collection with decisions in the evaluation plan:** SYNERGY,
   CLEF TAR or SR4CS; offline simulation, marks in and held-back marks
   out, in the manner of ASReview's simulation mode.
3. **The classifier on the box:** vectors or GLiNER2, trained on marks;
   agreement, work saved at 95 % recall, cost per work, measured.
4. **The artifact:** edge, description, marks, model version, epoch and
   agreement kept together, and a rerun at the same epoch checked for the
   same answer.
5. **Each batch:** the method scored over new works, once something runs
   the pipeline on a clock.
6. **A researcher's own review:** the pilot the joint position asks for.

## 12. Open

1. The name.
2. Where marks and methods live, who can see them, and how they are
   shared or published.
3. The model: a logistic regression over vectors, SetFit, GLiNER2, or
   gradient-boosted trees over metadata and embeddings as Nested
   Knowledge uses.
4. The stronger model for first labels: on the box, or hosted with
   abstracts leaving it.
5. How faithfulness is shown: which interval, over which sample, and how
   a researcher is told the method has drifted.
6. A target recall, and calibration to it, as Cochrane's classifier is
   calibrated to 99 %.
7. How each batch's new matches reach the researcher.
8. How several methods combine, and how disagreement between them is
   shown.
9. A published method's format, and how it is cited.
10. The cost of many methods on the box, measured.
11. The tool an agent calls, and what it returns.
