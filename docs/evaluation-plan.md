# Evaluation plan

**Status:** draft 1, 2026-09-26. E1 built, its arithmetic matching
`trec_eval` 10.0; E2 scrapped the same day (§9); E3, the published sets,
first measured 2026-09-29 on epoch 7 (§13).
**Inputs:** the conversation of 2026-09-26 that put measurement ahead of
every search improvement; [serving plan](serving-plan.md) §11 items 11,
36 and 37 (what search leaves open); [text analysis](inbox/text-analysis.md)
§6 idea 1 (measure the ceiling first); [benchmarks for search and
research](inbox/retrieval-benchmarks.md) (the published sets);
[future ambitions](inbox/future-ambitions.md) §5.2 (the citation
scoreboard).

## 1. What this is for

The user, 2026-09-26: "before getting lost down a rabbit hole of
potential search improvements, i want a very reliable way to measure
'goodness' of the search i have." There are no users yet, so no query log
can say what works (serving plan §11 item 37's first step assumed one);
a measurement has to. Every search change after this one — the tokenizer
(item 11), the query language's second pass (item 36), ranking, recency —
is judged by it, and the first thing it measures is the search as it is:
planned over epoch 6 (39,984,110 held works, all of PubMed), first
measured over epoch 7, PubMed and Crossref, on 2026-09-29 (§13).

"Very reliable" means seven things, each a section below:

1. It measures what people do with a search box (§3).
2. Its arithmetic is proven against the standard scorer (§5).
3. The same inputs give the same numbers, byte for byte (§6).
4. Every number says how far it can be trusted: an interval on each, and
   an interval on every difference between two versions (§4).
5. Tuning cannot fool it: each suite keeps a test half nobody looks at
   while working (§7).
6. It is honest about what nobody judged (§4).
7. What defines relevance is independent of anything being tested: an
   idea about ranking is a treatment measured against a key that knows
   nothing of it, never the key itself (§3.4).

## 2. Words

The running example: the query `IL-6 receptor antagonist`.

- **Query.** One thing typed into the box, with an id: `q17	IL-6 receptor
  antagonist`.
- **Answer key** (a benchmark's *qrels*). For a query, the works someone
  judged and the grade they gave each: this work 2 (definitely
  relevant), that one 1 (possibly), another 0 (not). A work nobody
  looked at is **unjudged** — not the same as not relevant.
- **Suite.** A set of queries with their answer key, versioned:
  `trec-pm-2019 v1`, `relish v1`.
- **Run.** What the search returned for every query of a suite, in rank
  order, to a fixed **depth** (1,000): the file the measures are
  computed from.
- **Measure.** A number computed for one query from the run and the key
  (precision after 10 results), then averaged over the queries.
- **Interval.** The range the average would plausibly fall in had the
  suite's queries been a different draw of the same kind (§4).
- **Stratum.** A slice of a suite reported on its own: one year's
  topics of a TREC track, one kind of topic (cancer or genetic disease in
  TREC Precision Medicine 2019).
- **Working half, test half.** Each suite's queries split once (§7).

## 3. What is measured

### 3.1 Four kinds of search (the user, 2026-09-26)

| kind | what a person is doing | the answer key |
|---|---|---|
| known-item | finding a paper they know | a mechanism of its own, with its own tests (§10, item 7) |
| topical | finding papers on a subject | human judgments: TREC's sets |
| related work | finding papers like one they have | RELISH's judgments, by the seed papers' authors |
| recent topical | the same, sorted by or leaning toward recent | none yet (§10, item 6) |

### 3.2 The suites

**Built from the corpus — scrapped 2026-09-26.** Three suites were
drawn up here to use the corpus's own data: *known-item* (a work's title
as the query, the work as the answer, typed several ways), *citation* (a
paper's title as the query, what it cites as the answer, and a *recent
citation* variant for recency) and *MeSH as query* (a subject heading as
the query, the works PubMed's indexers tagged with it as the answer). The
user, reading them closely: "of the three, i think only the mesh terms
idea is likely to yield the kind of improvements we want (exact title
search probably needs a specialized mechanism, citations in search
results rests on shaky assumptions), and the mesh terms idea seems to
recapitulate external metrics that were intentionally designed and
iterated on already, so we may as well use those instead, first." So
known-item finding is a mechanism to design (§10, item 7); citations are
a hypothesis about ranking, to be tested as a treatment against a key
that knows nothing of them, never used as the key — measuring a citation
treatment by citations would show improvement by construction (§3.4);
and MeSH as query is set aside behind the published sets; its design,
settled the same day, is in §9's log should biomedicine's breadth ever
need it.

**Published, judged by people — these lead.** None of them steers
tuning alone: they run side by side, each its own row, and the
acceptance rule (§4) reads every row. Why they lead, in the user's words
(2026-09-26): "they have already gone to the trouble of meticulously
curating the structure of the test, and because the search as it
currently is is *so bad* that we risk little by applying improvements to
basic, foundational search index construction with such a test suite as
a baseline" — and with the same wariness of fitting to them (§12, item
14).

- **TREC Precision Medicine 2019–2020, scientific abstracts.** PMIDs
  over the MEDLINE 2019 baseline, three grades; 40 topics in 2019, 31 in
  2020's phase-1 key. Open on NIST's site. Precision oncology only, so
  never read alone.
- **RELISH.** One CC0 file of about a megabyte on figshare, the only copy
  left: seed papers across biomedicine (3,278 by the file, 2,233 analysed
  by the paper) with 60 candidates each, judged by the seeds' own authors
  in 91% of cases, three grades. Related work; the candidates were pooled
  by lexical methods, which flatters BM25.
- **TREC-COVID**, "Complete" (round 5's cumulative key). Deep graded
  judgments and a published BM25 figure; its ids reach PMIDs through
  CORD-19's `metadata.csv`. Tested like the others; the user,
  2026-09-26: "i'm opposed to optimizing exclusively around it."
- **TREC Genomics 2004–05.** 100 topics, about a thousand judged works a
  topic, PMIDs, open; over MEDLINE 1994–2003.
- **TREC Clinical Decision Support 2014–16.** 90 graded clinical topics
  by PMC id; judged on full text, so an index of abstracts scores low on
  it, and says so.
- **Later, as sources land:** SYNERGY (systematic reviews across six
  fields, CC0, OpenAlex ids), OAG-QA (22 fields), iSearch++ (physics), a
  reference-string known-item set built from Crossref's references;
  TripClick and TripJudge need a form emailed to Trip and are
  non-commercial, the user's call. The survey of 2026-09-26 with every
  verdict is the benchmarks note's §2.4.

No judged set over papers in the humanities, law, engineering, materials
or earth science was found; breadth there waits for the sets that come
with Crossref and OpenAlex (§12, item 8).

A published set's key names works by PMID or DOI; the harness resolves
each to its work through the names lookup and reports the share it could
not (its **coverage**: a judged work the corpus lacks can never be
found).

Each published set is scored three ways (E3), since its judges saw a
corpus years older than ours and only the works its pools reached:

- **Open**: over the whole corpus as searched. Every work published
  since the set's snapshot, and every work no pool reached, counts as not
  relevant, so these figures are a lower bound.
- **Condensed**: the same run with every unjudged work taken out of each
  ranking before scoring (Sakai's condensed lists; `trec_eval -J` does
  the same), so what is scored is only what someone judged.
- **Restricted**: only works published by the set's snapshot, the cutoff
  applied after the search since search has no date filter — the run asks
  for more than its depth and keeps the first works old enough.

The second and third are the comparable ones; the first is the corpus as
it is used.

### 3.3 The measures (decided 2026-09-26)

The headline is precision, recall and F; nDCG and reciprocal rank are a
side column, for comparing with published figures.

- **At depth *k*** for *k* of 10 (what is read first), 50 (the web
  client's page, `web/src/api.ts`) and 1,000 (what a reranker or agent
  could still reach):
  - precision@*k*: relevant works in the first *k*, over *k*;
  - recall@*k*: relevant works in the first *k*, over the relevant works
    the key knows;
  - F@*k*: their harmonic mean.
- **R-precision:** precision after as many results as the key has
  relevant works — where precision equals recall.
- **Over everything returned** (to the run's depth): set precision, set
  recall, set F. Over everything that *matched* is a different
  question: under any-word matching a query matches most of the corpus,
  so recall there is near 1 and precision near 0; recall of the whole
  match under every-word matching is the evidence item 11's any-or-every
  decision wants, and precision of a match of millions cannot be
  measured without judging a sample of it, which the report says rather
  than print a number.
- **Graded keys**, both ways: **strict** (the top grade only) and
  **lenient** (any positive grade). A key with one positive grade
  reports once.
- **Side column:** nDCG@10 and reciprocal rank, as `trec_eval` computes
  them.

### 3.4 Hypotheses are treatments, never yardsticks

The principle every experiment here follows, in the user's words
(2026-09-26), on why citations cannot stand in for relevance:

> relevance scores are relevance scores, we have to treat them as a
> black box. that means we need a good way to judge what relevance is,
> and we're still missing that fundamental measure, while the external
> suites have already found a definition for this. we might
> *hypothesize* that the cited papers of a target paper would boost
> relevance, start with no treatment of citations in search, then apply
> a treatment to citations and see if it improves relevance. it would be
> a hill-climbing optimization if we started with the assumption that
> citations contribute to relevance and then built an "experimental
> setup" around it that bakes in this assumption. further, we'd be
> guaranteed to see improvement on our metric, because it wouldn't be a
> generic relevance metric, it'd be a metric that measures whether we've
> done the thing we said we'd set out to do, and we definitely would have
> done that. not an experiment. no control, no blind experimental setup,
> low rigor.

So:

- **What defines relevance is independent of anything being tested.**
  Here that is the published keys: people judged relevance without
  knowing how any of our search works. An idea about ranking — citations,
  recency, a tokenizer, MeSH headings — never becomes the key it is
  judged by.
- **An idea is a treatment.** The control is the search without it, the
  treatment the search with it; both run over the same human-judged
  suites and are compared query by query, with an interval on every
  difference (§4). The blind part is the test half (§7): nobody looks at
  it while the treatment is built, and its totals give the verdict.
- **The independent key has a bias of its own, and it runs against new
  treatments.** A published key was pooled from the systems of its day,
  so a treatment that surfaces works no pool reached has them counted as
  not relevant. When a treatment changes what comes back, the share of
  the first *k* that was judged, precision had every unjudged work been
  relevant, and the condensed scoring (§3.2) are read before the
  headline. Where the judged share falls, the headline is not evidence
  either way.

## 4. Error bars

Every number carries its uncertainty, by source (decided 2026-09-26):

1. **Which queries happened to be drawn.** A 95% interval from 10,000
   resamples of the suite's queries with replacement, the same resamples
   for every measure, from a fixed seed — so the intervals themselves
   reproduce. A suite drawn in strata is resampled within them.
2. **What nobody judged.** Beside precision, two bounds: every unjudged
   result counted not relevant (the low bound, which is what `trec_eval`
   reports) and every one counted relevant (the high); and the share of
   the first *k* that was judged. Recall is recall *of the relevant works
   the key knows*, and is labelled so; the true recall is lower by an
   amount no key bounds.
3. **Judges disagreeing.** One key cannot measure it; the report states
   it and draws no bar it cannot back.
4. **The harness.** Two runs of the same inputs must be identical files;
   checked, not barred.

**Comparing two versions** of the search — two index builds over the same
epoch — puts the interval on the per-query difference, resampled as
pairs. Two bars that overlap say nothing about whether a change is real;
the bar on the difference does. The headline measures are named before a
comparison is run, since across dozens of measures and strata some
difference will look real by chance.

**The acceptance rule** (decided 2026-09-26): a change is taken only if
no suite and no stratum gets worse by more than its interval. So a gain
on COVID paid for elsewhere is a failure.

A report line: measure · value · 95% interval · unjudged bounds · share
judged · queries.

### 4.1 The arithmetic of the intervals

- A resample draws *n* query indices uniformly with replacement from the
  suite's *n* queries and takes each measure's mean over them.
- The interval is the 2.5th and 97.5th percentiles of the 10,000 means:
  sorted, the means at positions 250 and 9,749 (counting from 0).
- The generator is SplitMix64 written out in the crate, and indices are
  drawn from it by Lemire's multiply-and-reject, which is unbiased —
  not a library generator, whose sequence a new version may change,
  which would move every published interval.
- The seed is a constant in the crate; changing it is a code change,
  seen in review.
- Paired: one draw of indices, applied to both runs' per-query values;
  the interval is of the mean difference.
- Stratified: each stratum is resampled to its own size, and the means
  combine with the strata's shares of the suite.
- **A floor of 30 queries** (decided 2026-09-26). Resampling a few
  queries sees only the spread those few happen to have, so its interval
  is narrower than the uncertainty: against the textbook interval for a
  mean (Student's *t*), about 6% narrower at 30 queries and 18% at 10,
  and a point at 2. Below 30 a mean is printed with no interval, and two
  runs get no verdict (`evaluate compare` exits 1 saying so). The same
  floor holds for a stratum.

With fifty topics an interval is wide. That is the honest answer, and
a reason to run several sets side by side; RELISH, with some two
thousand seed papers, is the one large set.

## 5. The scorer, proven

The measures are computed in `crates/evaluation`, and must equal `trec_eval`'s
exactly — NIST's standard scoring program (github.com/usnistgov/trec_eval),
a separate project tied to no dataset, which reads a run and an answer key
and prints the measures. The version is **10.0** (tag `v10.0`, released
2026-06-17); 9.0.8, the long-standing one, reads scores as `float` and
treats a query the run lacks differently. It is a tool the user builds and
runs, never redistributed: its licence is a NIST notice, and its files'
own headers say research and non-commercial use.

Its conventions, read from its source on 2026-09-26 and followed by the
crate:

- **Rank order** is by score, highest first, and among equal scores by
  the work's name in *descending* byte order (`comp_sim_docno`); the rank
  column of a run is ignored. 0 and -0 are one score.
- **Precision at *k*** divides by *k* even when fewer came back;
  **R-precision** divides by *R* likewise; **set F** is β = 1.
- **A negative grade** is "in the pool, not judged": neither relevant nor
  judged not relevant.
- **nDCG** takes the grade as the gain, discounts by log₂ of the rank
  plus one, and ignores the relevance level, so it is the same at both
  levels.
- **Every query of the key is evaluated** with `-c`; a query the run
  lacks scores 0. Queries only the run has are ignored.
- **Columns** part at C's white space, a no-break space inside a column.
- **Output** is four decimals.

One difference, on purpose (§10, item 2): `trec_eval` averages a query
with no relevant work at the level in at 0; the crate leaves it out.

First run against the user's build of 10.0, 2026-09-26: every case
failed at first, and none on arithmetic — the generator could make a run
of no lines, which `trec_eval` refuses to read, and each shrink repeated
it. With every generated run holding a line, both properties pass, 256
cases each, and the test now fails outright if it compared fewer values
than it had cases.

The check: a property test generates answer keys and runs — grades
below, at and above the level, negative grades, unjudged results, equal
scores, 0 and -0, queries only one file has — writes them as files, and
requires every per-query value to match `trec_eval -c -q`'s to its
printed four decimals. `trec_eval` is built once by the user (`make`);
the test runs when `TREC_EVAL` names the binary and says so when it does
not. F@*k*, the unjudged bounds and the judged share are not `trec_eval`
measures: they are checked from its counts at depth *k* (`-M k`:
`num_ret`, `num_rel`, `num_rel_ret`, `num_nonrel_judged_ret`). Kept out
of the generated files is what `trec_eval` itself refuses or mishandles:
a work twice in one query, blank and comment lines, a query whose every
grade is negative (its nDCG reads the previous query's grade levels).

Beside it, three sanity runs over any key: the ideal (every relevant
work first, best grade first) scores 1 on R-precision and nDCG; the
reversed scores no better; a random ranking over a large pool scores
near 0.

## 6. Files and where they live

Nothing here is committed: suites and runs are derived from data (the
store's rule).

- A **suite** is a directory: `queries.tsv` (`id<TAB>text`, the text as
  typed), `key.qrels`, `filters.tsv` (`id<TAB>year<TAB>work`, `-` for
  none: the year a restricted run keeps works up to, and a work never in
  that query's results — RELISH's seed), `halves.tsv` (`id<TAB>working`
  or `test`) when the suite is halved, and `about.json`: the set's files
  with their digests, the epoch its PMIDs were found in, the query field,
  what the corpus holds of the key by grade, and every query left out
  with why.
- A **queries file** alone, tab-separated lines as above, runs too, with
  no filters.
- An **answer key** is TREC's qrels, `query 0 work grade`, the work named
  by its public id (`ol:…`), never a `WorkId`: a key outlives a merge,
  and a `WorkId` never leaves the system.
- A **run** is TREC's run format, `query Q0 work rank score tag`, ordered
  as `trec_eval` orders it (§5), with a sidecar `<run>.about` naming the
  epoch, the search index's version and mapping, the queries file's
  digest and the depth.
- In the store: the sets' files as fetched in `raw/evaluation/<set>/`
  (the homelab's `job-fetch-evaluation-v1`, with `SOURCES.tsv`),
  `evaluation/suites/<suite>/v<n>/` and
  `evaluation/runs/<suite>/v<n>/epoch-<e>/<label>` (run, sidecar, report).

Commands:

- `ol-rht evaluate load <trec-pm <topics> <qrels> | relish
  <RELISH_v1.json.gz>> --out <suite> [--store <root>]` reads a published
  set's own files into a suite: its queries, its key with every PMID found
  through the store's names (a PMID no single work carries stays in the
  key as `pmid:<n>`, so a judged work the corpus lacks counts as missed),
  its filters, its halves, and `about.json`; with `--held-out` the whole
  suite is its test half.
- `ol-rht evaluate run <suite | queries> --out <run> [--store <root>]
  [--limit <depth>] [--workers N] [--restricted]` runs every query through
  the store's search — the query parser and translation the search box
  uses, in process, no time limit and no page cap — and writes the run,
  a query's excluded work always left out and, with `--restricted`, every
  work published after its year or of no known year.
- `ol-rht evaluate score <suite | key> <run> [--test]` prints the
  report; of a suite, the working half (or the whole of one too small to
  halve, and nothing of one held out), open and condensed; with `--test`,
  the test half, at a milestone.
- `ol-rht evaluate compare <suite | key> <base run> <new run> [--test]`
  prints both and the intervals on their differences, and the verdict
  over open and condensed.

## 7. What is frozen

- **The split.** Each suite's queries, with their keys, shuffled once
  from a fixed seed and split into a working half and a test half, the
  split written into the suite's directory and never redrawn — when each
  half keeps 30 queries with a relevant work, the floor of §4.1. A suite
  under 60 such queries is kept whole — every TREC year (31 to 40
  topics) — or held out whole (`evaluate load --held-out`): its every
  query the test half. RELISH, some 3,200 seeds, is halved. The TREC
  years split by year instead (the user, 2026-09-26: "go with 2019
  working, 2020 held out"): Precision Medicine 2019 is worked on whole,
  2020 held out whole (§10, item 9).
- **The working half** is for looking at anything: failures per query,
  examples, variants tried.
- **The test half** runs at milestones — before a new index version
  replaces the live one — and only its totals are read (`evaluate score
  --test`, `evaluate compare --test`; without it a held-out suite reports
  nothing). Once anyone
  studies its failures one by one it is spent, and a new split is drawn
  as a new suite version.
- **The suites** are loaded once from the published files, into a
  version, and kept, so next month's numbers compare with today's; a
  grown corpus changes the coverage of a key (more of its works found in
  the corpus), which is a new version beside the old.
- **The corpus during a comparison.** Two versions of the search are
  compared over the same epoch; more works mean more competition, which
  moves every score by itself.

## 8. Milestones

- **E1. The scorer.** `crates/evaluation` (formats, measures, bounds,
  intervals, report), `ol-rht evaluate run | score | compare`, the
  `trec_eval` property test and the sanity runs. No downloads. Built
  2026-09-26; the property test passes against the user's build of
  `trec_eval` 10.0.
- ~~**E2. The corpus-built suites.**~~ Scrapped 2026-09-26 (§3.2, §9).
  What it would have built that E3 still needs moves there: suites as
  directories, slices and halves read by `evaluate score`, plain Jobs,
  a first small run to learn what a search costs.
- **E3. The published sets — under way.** Built 2026-09-26: the readers
  of TREC Precision Medicine's topics and RELISH's JSON (their counts
  checked on the delivered files), suites on disk with their halves,
  `evaluate load`, runs of a suite with its filters and `--restricted`,
  the condensed scoring (matching `trec_eval -J`), and reports and
  comparisons of a suite. Not built: slices, and the run's summary JSON
  for the homelab's proposed table. The smallest difference a set can
  detect is read, for now, as the half-width of each interval in its
  first report. First run 2026-09-29: the homelab's
  `job-evaluate-baseline-v1` on epoch 7, the three suites loaded, run
  open and restricted, and scored; the baseline is §13. Next: the other
  sets, and the first treatment measured against it.
  The plan as written before: TREC Precision Medicine and RELISH
  first, then TREC-COVID, TREC Genomics 2004–05 and TREC Clinical
  Decision Support; the user downloads them; each gets a loader into a
  suite (§6) — its topics as queries, its key with PMIDs resolved to
  public ids through the names lookup, a coverage report, the split —
  and runs on the box as plain Jobs writing into the store, each run's
  summary JSON shaped for the homelab's proposed `experiment_runs` table
  (§12, item 12). `evaluate score` reads a suite: every slice, the
  working half. A first run of one set learns what a search costs over
  forty million works. The first thing each set reports is the smallest
  difference it can detect: the half-width of each measure's interval on
  the search as it is — a change smaller than that will not show, and a
  paired comparison is usually tighter, so it is a cautious guide. If the
  differences expected of a change are smaller, the sets cannot judge it
  (§10, item 8). SYNERGY, OAG-QA, iSearch++ and a reference-string
  set follow the sources they need.
- **E4. Comparisons at milestones:** the frozen halves in use, and the
  acceptance rule as a command's verdict.
- **E5. Recent topical:** three mechanisms simulated in the harness —
  recent years only, ranked by relevance; everything that matched, newest
  first; relevance with a decay for age — over whatever answer key §10
  item 6 settles on; the numbers say which to build.

## 9. Decision log

- **2026-09-26.** Measurement before any search improvement; no users,
  so no query log to steer by. Precision, recall and F are the headline,
  at depths 10, 50 and 1,000, with R-precision; nDCG and reciprocal rank
  a side column. Graded keys reported strict and lenient. A 95% interval
  from 10,000 seeded resamples of the queries on every number and on
  every difference. Four kinds of search, recent topical the user's
  addition. Corpus-built suites lead; the published sets check them;
  TREC Precision Medicine and RELISH first, TREC-COVID tested but not
  optimized around, and more sets surveyed. The acceptance rule of §4.
  A frozen test half per suite. `trec_eval` as the arithmetic's check.
- **2026-09-26, later.** The crate is `evaluation` and the command
  `ol-rht evaluate`. `trec_eval` 10.0's conventions read from its source
  and followed (§5), one difference on purpose (§10, item 2). The survey
  of more sets (benchmarks note §2.4) adds TREC Genomics 2004–05, TREC
  Clinical Decision Support and MeSH as query now, SYNERGY, OAG-QA,
  iSearch++ and a reference-string set later; it corrects TREC Precision
  Medicine's topic counts and RELISH's BM25 figures, and finds no judged
  set over papers in the humanities, law, engineering, materials or
  earth science.
- **2026-09-26, later still.** Queries with no relevant work are left
  out (§10, item 2); intervals have a floor of 30 queries (§4.1, §10
  item 5). The property test against `trec_eval` 10.0 passes (§5).
- **2026-09-26, E2.** The user took every recommendation — "so long as
  we're keeping track of what we're compromising on in documentation as
  we go", which is §12: plain Jobs and files, the summary shaped for the
  homelab's proposed table; known-item titles more than one work carries
  left out and counted; citation papers citing at least five held works,
  the date cutoff after the search; MeSH main topics only, check tags
  out, three slices by frequency; a pilot before the sizes. Human-assigned
  headings only was decided the same day, on finding the records mark
  NLM's automated indexing. The randomization test and BCa are written
  down for later (§11).
- **2026-09-26, E2 scrapped.** The user, on reading the three suites
  closely (§3.2): known-item finding "probably needs a specialized
  mechanism", citations as relevance rest "on shaky assumptions", and
  MeSH as query "seems to recapitulate external metrics that were
  intentionally designed and iterated on already, so we may as well use
  those instead, first". The published sets lead; E3 is next and takes
  what of E2's machinery it needs. The user's clarification the same day,
  which is §3.4: relevance must be defined independently of what is
  tested; citations are a hypothesis to test as a treatment against
  such a key, never the key, since a key made of citations would show a
  citation treatment improving by construction — no control, nothing
  blind. Known-item finding becomes a question for the search's design
  (§10, item 7); recent topical loses its key (§10, item 6); breadth now
  rests on the published sets (§12, item 8).
- **2026-09-26, the halves of the published sets.** A suite is halved
  only when each half keeps 30 queries with a relevant work; RELISH is,
  the TREC years are not. The user's call on the TREC years: Precision
  Medicine 2019 worked on whole, 2020 held out whole (§7, §10 item 9);
  `evaluate load --held-out` and `evaluate score --test` built the same
  day.
- **2026-09-26, the published sets and known-item.** The published sets
  lead because they are curated and today's search is poor enough that
  foundational changes risk little against them, the user wary of
  fitting to them all the same (§3.2, §12 item 14); each set first
  reports the smallest difference it can detect (E3), and judgments of
  our own are the way on if that is too coarse (§10, item 8). Known-item:
  the requirement is the user's — a typed exact title brings that paper
  first, every time — and the mechanism is open, perhaps a citation
  parser, a lookup and high-confidence matches rather than BM25, with
  mistyped titles as its robustness measure (§10, item 7).
- **2026-09-28, what is never a reason to leave a set out.** The user,
  on the benchmarks note: "full text is a goal to include, it's just not
  here yet. full text is the gold standard!"; "'it is not in english' is
  no reason to exclude something of use"; "'it is not biomed' is no
  reason to exclude something of use. biomed has a broad potential
  customer base, but academics study all kinds of things, and it's not
  just pharmas that need support"; and "pubmed was just a convenient
  place to start". So a set judged on full text, in another language or
  outside biomedicine is kept and waits on the corpus reaching it; the
  benchmarks note was amended the same day, and §12 item 8's breadth
  compromise now has SR4CS (computer science) and SYNERGY's other fields
  beside the sets it names. Later the same day, document type joined
  them: "i would not exclude any kind of artifact just because of
  document type" (patents, code, internal documents, DNA sequences,
  proofs, chemical formulae, experimental data and images are all in
  scope in principle); "academic literature is the wedge i know best",
  and for the short term the product is a tool for academics and
  researchers.

- **2026-09-29, the first measurement.** Epoch 7 landed (the first with
  Crossref; model 12, merge 16, serve 8), and `job-evaluate-baseline-v1`
  on the image serving it loaded TREC Precision Medicine 2019 and 2020
  and RELISH, ran each open and restricted to 2018, and scored them:
  §13 is the baseline every change is compared against. Precision
  Medicine 2020 was read once, as milestone zero, and is sealed again.
  RELISH resolves differences near one point, the TREC years near eight
  (§13.4).

## 10. Open items

1. ~~**What counts as typing a title.**~~ Gone with E2 (2026-09-26); the
   question returns with item 7.
2. **Queries with no relevant work** at a level (a topic with nothing
   "definitely relevant") are left out of that level's averages, and the
   count left out is reported. `trec_eval` averages them in at 0, which
   lowers every measure by the share of such queries for every version of
   the search alike; leaving them out keeps a strict level's average about
   the queries it can speak to. *Decided 2026-09-26:* left out — the
   user, "seems like that's a weird choice by trec_eval, agree to leave
   them out". A published figure is compared at the level where every
   topic has a relevant work, where the two agree.
3. **Equal scores at the depth's edge.** Within a run, equal scores are
   ordered as `trec_eval` orders them; which of several equal-scoring
   works the engine keeps at the 1,000th place is the engine's, and rare.
4. **A judge for what nobody judged.** Judging a sample of unjudged
   results — by the user, or by a model checked against the human keys
   first — would narrow the bounds of §4; not before the keys have
   shown how wide they are.
5. **Too few queries for an interval.** *Decided 2026-09-26:* a floor of
   30 (§4.1), the user's "ok to add a floor". The other methods — the
   randomization test above all, BCa, the textbook *t* interval — are
   written down in §11 for when they are wanted.
6. **Recent topical's answer key.** The recent-citation suite went with
   E2, and a published key is judged at its snapshot, so what is recent
   to the corpus is unjudged to every set. What there is: TREC-COVID's
   per-round keys over five CORD-19 releases of April to July 2020, a
   small built-in time slice (benchmarks note §2.4); a sample of recent
   results judged by hand; a set published after the corpus's newest
   works. Whatever it is, recency cannot define it (§3.4): a key of
   "recent works are relevant" would crown every recency mechanism.
   Open.
7. **Finding a paper someone knows** — its title, or part of it, or an
   identifier, typed into the box. The requirement, the user's
   (2026-09-26): "if a user types in what they think is the exact title
   of a paper, the first result should be that paper every time. how we
   get there is still a question mark." A guess at the how, the user's
   and held loosely ("i might be wrong about the citation parser
   pathway! we'll discover as we go"): not BM25 but a mechanism beside
   it — the typed text parsed as a citation into structured data, looked
   up, and a short list of high-confidence matches (the parsing is
   `docs/inbox/citation-parsing-landscape.md`; identifiers typed into the
   box are serving plan §11 item 36). Whatever it becomes, it has tests of
   its own, and realistically mistyped titles — the ways of typing drawn
   up for E2's known-item suite: hyphens dropped, Greek letters spelled
   out, accents dropped, a word missing — are its robustness measure; the
   answer key is the task itself, so it is no stand-in for relevance.
   Open.
8. **Judgments of our own.** Should the published sets prove unable to
   detect the changes that matter (E3's first report), or once the gains
   get subtle, a key can be built the way TREC built theirs: pool the top
   results of the control and treatment runs, and have a person judge the
   pool without knowing which run gave what. Independent of the
   treatment, as §3.4 requires, and the one key that sees what a new
   treatment surfaces, so it also answers the pooling bias. Not started.
9. ~~**The whole TREC years: looked at or held out.**~~ *Decided
   2026-09-26:* by year — Precision Medicine 2019 is the working set,
   2020 held out whole and read only in total at milestones, the usual
   practice of tuning on earlier years and testing on later ones; each
   keeps enough topics for intervals (40 and 31). The user: "go with 2019
   working, 2020 held out". Later sets are placed the same way when they
   come.

## 11. Methods held for later

Written down on the user's word (2026-09-26: "let's make sure we write
down BCa and especially the method i'd most likely want later, the
randomization test, somewhere we can find it again, but right now we
don't have to build it"). The code points here from
`crates/evaluation/src/interval.rs`.

### 11.1 The randomization test — the one most likely wanted

**What it answers.** Whether a difference between two versions of the
search is real or could be luck — as a probability, where the interval
of §4 says how big the difference is.

**How it works, plainly.** Two runs over the same queries give each
query two scores, one from each version. If the change made no
difference, it would not matter which score came from which version: for
any query, swapping its two scores gives a result just as likely as the
one observed. So swap at random — each query independently, as a coin
decides — thousands of times, and take the average difference each
time. The share of those shuffled averages at least as far from zero as
the real one is the *p*-value: how often chance alone would produce a
difference that large. With few queries every one of the 2ⁿ ways of
swapping can be counted exactly (2²⁰ is about a million), so the test is
exact however small the suite — which the floor of §4.1 keeps the
intervals from being.

**Why this one.** It assumes nothing about the shape of the per-query
scores (precision at 10 over a few queries is mostly 0s and a few 1s,
nothing like a bell), it is exact at small sizes, and it is what IR
research has settled on: Smucker, Allan and Carterette ("A comparison of
statistical significance tests for information retrieval evaluation",
CIKM 2007) found it, the *t*-test and the bootstrap largely agree and
recommended it, and advised against the sign and Wilcoxon tests.

**How it would fit.** `evaluate compare` would print a *p*-value beside
each difference and its interval, from the same seeded generator (§4.1)
so that it reproduces. Across dozens of measures and strata some *p* will
be small by chance: either the headline measures stay named in advance,
as now, or the *p*-values are adjusted for how many were looked at
(Holm's method is the usual). Whether the acceptance rule then reads the
interval, the *p*-value or both is the user's call when it is built.

### 11.2 BCa, the corrected resampling interval

**What it fixes.** The interval of §4.1 takes the 2.5th and 97.5th
percentiles of the resampled means as they fall. When the resampled
means sit lopsidedly around the real one — as they do for a measure near
0 or near 1, where the spread cannot run past the end — that interval is
off-centre. BCa (bias-corrected and accelerated; Efron, 1987) shifts
which percentiles are taken, by how far the resampled means' middle sits
from the real mean (the bias) and by how much each query moves the mean
(the acceleration, from recomputing the mean with each query left out in
turn).

**What it does not fix.** An interval over few queries still sees only
the spread those few have; BCa corrects the shape more than the width,
so it would not remove the floor.

**What it would cost.** One extra pass per measure, leaving each query
out once — small beside 10,000 resamples.

### 11.3 The textbook interval

The mean plus or minus Student's *t* times the standard error: wider for
few queries, as it should be, but it assumes the per-query scores are
roughly bell-shaped, or many enough that their average is. Easy to print
beside the resampled interval as a check.

## 12. What we are compromising on

Kept as we go, at the user's request (2026-09-26: "so long as we're
keeping track of what we're compromising on in documentation as we
go"). Each line: the compromise, what it costs, what would lift it.

**The arithmetic**

1. **Percentile resampling for intervals**, not BCa or *t* — about 6%
   too narrow at 30 queries, lopsided near 0 and 1. Lifted by §11.2 or
   §11.3.
2. **No interval under 30 queries**, and no verdict — small suites, small
   strata and a strict level with few topics give averages only. Lifted by
   the randomization test (§11.1), which is exact at any size.
3. **Queries with no relevant work left out** — our averages differ from
   `trec_eval`'s wherever such a query exists, so a published figure is
   compared at the level where every topic has a relevant work.
4. **Recall is of the relevant works the key knows** — true recall is
   lower by an amount no key bounds. Lifted by judging a sample of what
   nobody judged (§10, item 4).
5. **Judges' disagreement is not measured** — one key, one judgment per
   work.

**What is run**

6. **Measured in process, not through the server** — the server's ten
   seconds, its busy refusals and the transport are not in the numbers; a
   query the server would time out on is scored as if answered. Lifted by
   a run through `/v1/search`.
7. **Equal scores at the 1,000th place** are kept as the engine keeps
   them.

**What scrapping E2 leaves (2026-09-26)**

8. **Breadth rests on the published sets**, and they are all biomedicine:
   precision oncology, genomics, clinical cases and COVID, with RELISH
   across biomedicine; no judged set over papers outside the sciences and
   medicine was found (benchmarks note §2.4). The guard against tuning to
   one narrow set is running several side by side, each its own row under
   the acceptance rule. Lifted, in part, as Crossref and OpenAlex bring
   SYNERGY, OAG-QA, iSearch++ and SR4CS within reach. Biomedicine is
   where the published sets happen to be, not the scope: "pubmed was
   just a convenient place to start" (the user, 2026-09-28; §9).
9. **Finding a paper someone knows is not measured yet** — it waits for
   the mechanism of §10 item 7, whose own tests and mistyped titles will
   measure it.
10. **Recent topical has no answer key** (§10, item 6).
11. **Small suites.** A TREC year is 30 to 50 topics, so its intervals
    are wide (§4.1); RELISH is the one set in the thousands. Each set's
    smallest detectable difference is reported first (E3), and judgments
    of our own (§10, item 8) lift it.

The E2 compromises this replaces — titles more than one work carries,
citations standing in for relevance, indexing standing in for relevance,
sizes from a pilot — went with it.

**Where the numbers live**

12. **Plain Jobs writing files into the store**, not the homelab's
    proposed experiment tracking (its `docs/decisions/experiment-tracking.md`:
    runs started by a push, a row of metrics per run in Postgres, Grafana
    to compare) — comparing runs over time means reading reports. Each
    run's summary is JSON shaped like that proposal's `metrics` column, so
    moving to it is a copy.

**The published sets (E3)**

13. **Judged against old snapshots** — works published since, and works a
    pool never reached, count as not relevant, so an open run's figures
    are a lower bound; the condensed and restricted scorings of §3.2
    are the comparable ones. The pools were built from that era's systems,
    which counts against any treatment that surfaces works they never
    reached (§3.4); the judged share is read first.
14. **Starting on the published sets risks fitting the search to them** —
    to their topics, their fields and their pools. Accepted while the
    search is poor and the changes are foundational (the user: "we risk
    little by applying improvements to basic, foundational search index
    construction with such a test suite as a baseline"); guarded by
    several sets side by side, each its own row, and the frozen test
    halves; lifted by judgments of our own (§10, item 8) when the gains
    get subtle.
15. **One field of a topic is the query**, the one closest to what a
    person types (the user's go, 2026-09-26): TREC Precision Medicine's
    disease and gene — and in 2020, whose topics each ask about one
    treatment, the treatment too — and for RELISH the seed paper's
    title. The sets' fuller fields (a patient's demographics, a
    narrative, a seed's abstract) are left out, so a figure here is not
    a published run's, which mostly used more; and an abstract as the
    query waits on any-word matching (serving plan §11 item 11). Lifted
    by running a set again with another field, as its own row.
16. **RELISH: one annotation per seed**, the first in the file. 84 of its
    3,278 annotations are a second person's on a seed already annotated
    (seed 29463761's two annotators graded the same 60 candidates 19, 41,
    0 and 21, 22, 17 relevant, partial, irrelevant); they are left out,
    counted in `about.json`, and with them what their disagreement says
    (item 5).
17. **RELISH's query is the seed's title as our works index holds it**,
    not as RELISH's annotators saw it; a seed the corpus lacks, holds only
    as cited, or holds without a title is left out, with why.
18. **The restricted run asks for twice its depth** and keeps the first
    works published by the set's year; a work of no known year is left
    out, and a query whose first 2,000 were mostly newer ends short of
    1,000 — counted in the run's `.about` as `short_of_depth`.
19. **A set under 60 queries is not halved.** Precision Medicine 2019 is
    worked on whole, so it is tuned to and is no check on itself; the
    check is 2020, held out whole — the other year of the same track, so
    an improvement fitted to precision oncology's topics would pass both.
    RELISH's test half and the other sets are the guard against that
    (§10, item 9).
20. **The title reaches half of RELISH's relevant candidates** —
    measured 2026-09-29: recall@1000 0.44 over the whole corpus, 0.49 up
    to 2018 (§13.4). What items 15 and 17 cost, now a number. Lifted by
    running RELISH again with the seed's abstract as the query, its own
    row, once any-word matching allows it.
21. **A run searches both halves.** RELISH's runs rank the test half's
    seeds too, and those rankings sit in the run files on the claim; only
    the reports keep to the working half. The guard is that nobody reads
    them per query. Lifted by a run that takes one half.
22. **The year cutoff is read from each result's record** — up to 2,000
    rows a query in a restricted run. It cost TREC's restricted runs four
    to six times the open ones' time, RELISH's 15% more (§13.5), where the
    search itself dominates. Lifted by the year held in the index.

## 13. Measurements

### 13.1 The baseline, 2026-09-29

The search as it was on 2026-09-29: epoch 7, the first with Crossref
(model 12, merge 16, serve 8), searched by `main-b7afccd`, the image
serving it, through the homelab's `job-evaluate-baseline-v1`: the query
parser and translation of the search box, any word matching, depth 1,000.
Runs and full reports are on the bulk claim under
`evaluation/runs/<set>/v1/epoch-7/ol-rht-evaluate-baseline-v1/`; the
suites under `evaluation/suites/<set>/v1/`. Every figure is a mean with
its 95% interval (§4.1); *lenient* counts any positive grade relevant,
*strict* the top grade only (§3.3).

The three columns are the three scorings of §3.2: **up to 2018** is the
comparable one — the corpus cut to the year the judges saw; **unjudged
removed** is the condensed list of that run; **everything** is the open
run over the whole corpus, a lower bound.

### 13.2 What the corpus holds of each key

| set | judged works | held (searchable) | stubs | missing | judged twice as one work |
|---|---|---|---|---|---|
| TREC Precision Medicine 2019 | 18,316 | 18,302 | 0 | 14 | 1 |
| TREC Precision Medicine 2020 | 17,366 | 17,358 | 0 | 8 | 8 |
| RELISH | 191,580 | 191,533 | 0 | 47 | 17 |

RELISH keeps 3,193 of its 3,194 seeds (28430671 is not in the corpus),
halved 1,597 working and 1,596 test. Coverage is not what limits any
figure below.

### 13.3 The figures

**TREC Precision Medicine 2019** — the working set, whole: 40 topics
lenient, 38 strict.

| lenient | up to 2018 | unjudged removed | everything |
|---|---|---|---|
| P@10 | 0.413 (0.330–0.495) | 0.535 (0.433–0.638) | 0.238 (0.183–0.293) |
| share of the first 10 judged | 0.755 | 1.000 | 0.480 |
| recall@1000 | 0.518 (0.441–0.592) | 0.518 | 0.455 (0.380–0.529) |
| R-precision | 0.205 (0.164–0.246) | 0.332 | 0.156 |
| nDCG@10 | 0.389 (0.315–0.465) | 0.489 (0.401–0.577) | 0.230 |
| reciprocal rank | 0.613 (0.502–0.723) | 0.705 | 0.427 |

Strict, up to 2018: P@10 0.297 (0.224–0.376), recall@1000 0.647
(0.560–0.731), nDCG@10 0.386; unjudged removed, P@10 0.361.

**TREC Precision Medicine 2020** — held out whole; milestone zero, read
once: 31 topics lenient, 28 strict (under the floor: no intervals).

| lenient | up to 2018 | unjudged removed | everything |
|---|---|---|---|
| P@10 | 0.252 (0.161–0.355) | 0.358 (0.239–0.487) | 0.135 (0.084–0.194) |
| share of the first 10 judged | 0.761 | 1.000 | 0.432 |
| recall@1000 | 0.462 (0.404–0.524) | 0.462 | 0.416 (0.360–0.476) |
| nDCG@10 | 0.240 (0.151–0.342) | 0.326 (0.215–0.446) | 0.132 |
| reciprocal rank | 0.435 (0.302–0.576) | 0.485 | 0.298 |

**RELISH** — the working half: 1,597 seeds lenient, 1,559 strict.

| lenient | up to 2018 | unjudged removed | everything |
|---|---|---|---|
| P@10 | 0.250 (0.240–0.261) | 0.757 (0.744–0.771) | 0.172 (0.165–0.181) |
| share of the first 10 judged | 0.294 | 0.962 | 0.201 |
| recall@1000 | 0.492 (0.481–0.504) | 0.492 | 0.436 (0.424–0.448) |
| R-precision | 0.153 (0.147–0.159) | 0.451 (0.441–0.462) | — |
| nDCG@10 | 0.242 (0.232–0.252) | 0.656 (0.643–0.668) | 0.169 |
| reciprocal rank | 0.518 (0.499–0.538) | 0.906 (0.895–0.917) | 0.403 |

Strict, up to 2018: P@10 0.170 (0.161–0.179), recall@1000 0.547
(0.533–0.561), nDCG@10 0.244; unjudged removed, P@10 0.465, nDCG@10
0.661. RELISH judged only 60 candidates a seed, so its open and restricted
precision mostly measures what nobody judged; the condensed column is
the one to read, and the 0.96 judged share of its first 10 says it is
nearly complete there.

### 13.4 What the figures say

- **Resolution.** The half-width of an interval is the smallest
  difference a single run can show (E3): about **±0.08** on P@10 and
  nDCG@10 for TREC 2019's 40 topics, about **±0.01** for RELISH's 1,597.
  A paired comparison is tighter than both. So RELISH's working half is
  the instrument fine enough to judge a change of a point or two, and the
  TREC years are the coarse, human-judged check beside it (§10, item 8
  is not needed yet).
- **Pooling.** Cutting the corpus to 2018 lifts the judged share of the
  first 10 from 0.48 to 0.76 on TREC 2019 and from 0.43 to 0.76 on 2020:
  half of what the whole corpus returns first is newer than the judges'
  corpus or from Crossref, and counts as not relevant in the open run.
  The unjudged-removed figures are nearly the same between the two runs,
  as they should be: the judged works are old, and cutting the new ones
  does not reorder them.
- **The title as RELISH's query** finds about half of its relevant
  candidates in the first 1,000 (recall@1000 0.44 open, 0.49 up to 2018):
  RELISH's candidates were pooled from methods that read whole papers,
  and the query here is the seed's title alone (§12, items 15 and 20).
- **No published figure is placed beside these.** None was found for
  TREC Precision Medicine 2019 (benchmarks note §2.4); RELISH's own BM25
  (MRR 0.859 over its 60 candidates) ranked the candidates themselves, a
  different task from searching the corpus.

### 13.5 What the run cost

| step | open run | restricted run | memory at its peak |
|---|---|---|---|
| load (three suites) | 15 s in all | | |
| TREC 2019, 40 queries | 7.8 s (slowest 3.2 s) | 28.6 s | 1,017 MiB |
| TREC 2020, 40 queries | 4.2 s | 23.9 s | 449 MiB |
| RELISH, 3,193 queries | 3,294 s (slowest 15.0 s) | 3,780 s (slowest 11.6 s) | 20,690 MiB |

Six search threads; the Job took 120 minutes, RELISH nearly all of it. In
the restricted RELISH run 939 of 3,193 queries ended short of 1,000
works published by 2018 in their first 2,000 (§12, item 18).
