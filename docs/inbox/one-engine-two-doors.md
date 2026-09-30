# One engine, two doors

**Too early; hold loosely.** This is a reading of desk research that the
user labelled too early, drawn while the product has no users, no chosen
shape and none of its tools built. No researcher, firm or institution
has said yes to anything. The direction is the conversation's reading,
offered for the user to weigh; it is not a decision.

**Status:** note, 2026-09-28, written at the user's request ("i'd
appreciate this as a write up in the inbox, yes thanks") after asking
"does all this point in a particular direction to you?". It gathers the
day's notes into one reading:
[searching within a result set](search-within-results.md),
[the recall-accounted set builder](recall-accounted-set-builder.md),
[suggested cuts](suggested-cuts.md),
[the shape of a result set](shape-of-a-result-set.md),
[teach it your method](teach-it-your-method.md),
[first customers and first conversations](first-customers-and-conversations.md),
[the legal and patent-search landscape](legal-and-patent-search-landscape.md)
and [what academics pay](what-academics-pay.md). Every fact here is
taken from those notes, with their marks and sources; this note adds
only the reading. Speculative, not a plan; nothing in it amends the
plans of record, and the choice is the user's.

## 1. The direction

**Build one engine, and take it through two doors.**

- **The engine:** search over open data that keeps account of recall,
  with methods taught on a researcher's marks, cheap to run on one box.
- **Door one, people:** research teams doing systematic, scoping and
  living reviews, in every field.
- **Door two, machines:** a literature connector that agents and other
  people's tools plug into.

Institutions and consortia come later, once there is evidence to show
them.

## 2. The engine

The [set builder](recall-accounted-set-builder.md) (an explicit edge,
counts and samples of what each step keeps and drops, a classifier
trained on marks, an estimate of what is still missing) and
[teach it your method](teach-it-your-method.md) (the method as a
versioned artifact, measured on held-back marks, rerun on each new
batch) are one engine seen from two sides. Its economics are the
user's: an expensive model teaches once, and a cheap, deterministic
specialist runs until a new epoch calls for re-teaching (teach it your
method §4). Its data is open (Crossref, PubMed, OpenAlex), and it runs
on one box. [Suggested cuts](suggested-cuts.md) and
[the shape](shape-of-a-result-set.md) are features of the engine, not
products of their own: the method's readable face, and the first look.

## 3. Door one: review teams, in every field

Nearly every thread of the day converges here.

- **Money already flows there for search and screening.** Covidence
  charges $339 a year for one review; a systematic review has been
  costed at about $141,000 ([what academics pay](what-academics-pay.md)
  §3).
- **Marks exist because the job makes them.** Screening is marking, so
  the input a taught method needs comes with the work
  ([searching within a result set](search-within-results.md) §11.3:
  "marks exist where the job makes them").
- **Recall is demanded, and has to be reported.** PRISMA-S asks for
  searches reported as run and filters cited; the 2025 joint position of
  Cochrane, Campbell, JBI and the Collaboration for Environmental
  Evidence asks reviewers to pilot a tool in their own review and report
  its version and dates ([teach it your method](teach-it-your-method.md)
  §6.2, §7). "Faithfully, and here is how faithfully" answers a
  requirement.
- **The gap found is concrete there.** Tools that learn from marks work
  inside one review's imported set; whole-corpus reach is monthly
  (EPPI-Reviewer's OpenAlex auto-update) or unmeasured (Semantic
  Scholar's feeds); no tool keeps a researcher's method as a measured,
  publishable artifact run on each new batch (teach it your method §5).
  The freshness work becomes living-review updates (§6.3 there).
- **It can be proven before there is a user.** CLEF TAR, the Seed
  collection, SYNERGY and SR4CS carry human-judged keys independent of
  the tool, across medicine, psychology, computer science, biology,
  chemistry and mathematics ([retrieval benchmarks](retrieval-benchmarks.md);
  teach it your method §9.3). That fits the project's rule of measuring
  before improving better than any other door.
- **It is not only biomedical.** Active learning for reviews has been
  studied in software engineering, public health, environmental evidence
  and public policy (teach it your method §9.1).

## 4. Door two: a literature connector for agents

- **Agents are not given set tools.** Every literature agent looked at
  searches, takes the top results and reads them; none is told what it
  left out ([searching within a result set](search-within-results.md)
  §12.1). OpenAI's deep research accepts an outside tool only if it
  offers `search` and `fetch` (same section).
- **There is an empty slot.** Anthropic's legal plugins run a patent or
  literature search only "if a connector is available"
  ([legal and patent-search landscape](legal-and-patent-search-landscape.md)
  §2, §7).
- **Exhaustive recall is where agents do worst:** 5.1 % success on
  WideSearch, 9.31 % set overlap on AutoResearchBench's wide task, 0.355
  of everything for ScholarQuest's best agent (searching within a result
  set §12.3). The engine is built for exactly that.
- **Cheap to serve matters most here:** agents make many calls, and tool
  schemas and answers are paid for on every one (HarnessTax, same note
  §12.4).
- **It is one of the first users already named:** the teams building
  research agents ([product shapes](product-shapes.md) §7).

## 5. Why the market story holds together

Europe-first is a stated preference ([product shapes](product-shapes.md)
§9). The user lives beside Université Paris-Saclay, which has signed the
Barcelona Declaration on open research information; France's
open-science strategy for 2026–2033 steers institutions towards funding
OpenAlex; Couperin publishes how a vendor enters its negotiations; and
European tools sell on data protection ([what academics pay](what-academics-pay.md)
§4, §5, §7). A cheap product on open data, with models on its own box,
fits all of that, and charges for what open data alone does not give:
measured recall, taught methods, freshness. It will be judged against
"OpenAlex plus a membership fee" (same note §5).

## 6. What it argues against, for now

- **IP law as a market.** Crowded and well funded, and the corpus holds
  no patents; keep it as a conversation, and non-patent literature as a
  possible use of the connector
  ([legal and patent-search landscape](legal-and-patent-search-landscape.md)
  §7).
- **A general research assistant for individuals.** Individuals pay
  $10–50 a month, 70 % use free tools, and the frontier labs compete
  there ([what academics pay](what-academics-pay.md) §2).
- **The shape and suggested cuts as products.** Features of the engine,
  later (§2).

## 7. What could make it wrong

- **The review-tool market is not empty.** Elicit reports 96.9 %
  sensitivity against 994 Cochrane reviews, and EPPI-Reviewer already
  reaches OpenAlex (teach it your method §5). The difference claimed
  (whole corpus, measured, publishable, cheap) has to be shown, not
  asserted.
- **Nobody has said they would pay.** Researchers mostly use free tools;
  researchers paying out of pocket or from grants was not found
  ([what academics pay](what-academics-pay.md) §2, §8).
- **The corpus beyond biomedicine** waits on Crossref and OpenAlex
  landing, and none of the engine's tools is built; facets as filters
  (serving plan §11, item 36) is the first gate.
- **Measurement has not started.** The evaluation plan's loaders and
  suites are built and "not yet run on the box"
  ([evaluation plan](../evaluation-plan.md), status).
- **This reading rests on desk research** labelled too early, by
  researchers without web search for the last three sweeps.

## 8. What to do next, measurement first

1. **Run the evaluation** the plan has ready, once an epoch allows.
2. **Add a systematic-review collection with its include and exclude
   decisions** to the evaluation plan: SYNERGY and SR4CS keep it broader
   than biomedicine, CLEF TAR and the Seed collection within it.
3. **Build item 36** (facets as filters, searching within a set), so
   there is an edge to search within.
4. **Run the offline simulation:** marks in, held-back marks out; a
   taught classifier against the experts' boolean queries, active
   learning in ASReview's manner, and zero-shot model screening, on
   recall and cost per work. One experiment tests both the pitch and the
   cost thesis ([teach it your method](teach-it-your-method.md) §11).
5. **Listen, in parallel:** three to five Paris-Saclay researchers who
   run reviews, in different fields; the relative's firm on non-patent
   literature; the questions are in
   [first customers and first conversations](first-customers-and-conversations.md)
   §3. Read Couperin's GDPR and AI questionnaires.
6. **Choose between the doors** after the conversations and the
   simulation, not before.

## 9. Open

1. Which door first: review teams or the connector.
2. Who pays, and how much: review teams per review or seat, agent builders
   per call, institutions through consortia later.
3. Whether the connector is a product or a way of reaching people.
4. When to approach Couperin, and with what evidence.
5. What to show in the first conversations, given what is built.
