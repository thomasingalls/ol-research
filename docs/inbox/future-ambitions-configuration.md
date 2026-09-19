# Future ambitions: configuration per query, and the harness tax

**Status:** speculative notes, 2026-09-17 — a paper and a study the user
raised, read the same day, recorded at the user's request. Nothing here is
a plan or a decision. What per-query configuration of a retrieval pipeline
and the cost of an agent's harness mean for this repository's query path,
the [tracer's engine](future-ambitions-tracer-engine.md), the agent front
door of [future ambitions](future-ambitions.md) §5.1 and the scoreboard of
[benchmarks for search and research](retrieval-benchmarks.md). Companion
to [AI as science assistant](science-assistant-landscape.md) §8. Facts are
marked **P** (primary: the paper, read in full; the study's post, read
from its site's repository data because the site renders in JavaScript),
**S** (secondary) or **U** (could not verify). Readings are marked as
readings.

## 1. BRANE: configuration per query

Melissa Z. Pan, Negar Arabzadeh, Mathew Jacob, Fiodar Kazhamiaka, Esha
Choukse and Matei Zaharia (UC Berkeley, University of Washington,
Microsoft Azure Research — Systems), *Natural Language Query to
Configuration for Retrieval Agents*, arXiv 2605.27361, 2026-05-26 (P).

- **The problem, "Query2Conf".** A retrieval pipeline has knobs — the
  LLM, the retriever, the number of documents *k*, the synthesis strategy
  — usually tuned once per workload. Given a query and an accuracy target
  (or a budget), choose from a fixed catalogue the configuration that
  minimises cost (or maximises accuracy).
- **Three observations.** Queries within one workload have different
  best configurations, no strategy best for all, per-query cost spanning
  three to four orders of magnitude on one workload. The LLM is one knob
  among many: on FinanceBench, varying retriever, *k* and synthesis with
  GPT-5-nano fixed spanned 38 points of accuracy and 93× cost, varying the
  LLM alone 17 points and 8×. The query characteristics that matter differ
  by workload; general ones (complexity, joint reasoning) put nearly every
  query of a benchmark in one bucket.
- **The configuration space.** Synthesis: LLM only; retrieved chunks
  concatenated; chunks summarised first; a search-augmented agent loop.
  Retrievers: BM25; E5 and GTE dense; BM25 hybrids with each; a Qwen3-8B
  dense retriever on BrowseComp-Plus. *k* from 1 to 100. Ten LLMs, from
  Llama 3.1 8B to GPT-5.4 and Gemini 3.1 Pro. 131 configurations profiled
  on MuSiQue, 60 on BrowseComp-Plus, 335 on FinanceBench, over pools of
  600, 600 and 150 queries.
- **The method.** Offline, every sampled query runs through every
  configuration, recording correctness (a GPT-5-mini judge against the
  reference answer) and dollar cost at list prices. A frontier LLM
  (GPT-5-mini) proposes about ten workload-specific yes/no characteristics
  from a sample of queries — "does the query reference a specific fiscal
  year?" — and a cheaper LLM labels each query against them. One
  lightweight classifier per configuration near the cost–quality frontier
  (logistic regression, decision tree, random forest, gradient boosting,
  XGBoost, LightGBM, chosen by cross-validation) predicts whether that
  configuration answers correctly. At query time: characterise once, then
  pick the configuration maximising predicted correctness minus λ times
  its mean cost; sweeping λ traces the frontier, and a target accuracy or
  budget maps to a λ. "Fuzzy" Pareto pruning keeps near-frontier
  configurations against sampling noise.
- **Results.** At the best static configuration's accuracy: 89.4 % lower
  cost on MuSiQue, 81.7 % on BrowseComp-Plus, 8.1 % on FinanceBench, 59.7 %
  on average; the only method reaching that accuracy on all three, against
  Murakkab (best static), METIS, Adaptive-RAG and CARROT, each extended to
  the full configuration space. At half the best static cost, the highest
  accuracy on every benchmark.
- **Ablations.** LLM-proposed yes/no characteristics are the only query
  representation reaching the strictest goal on every benchmark; an
  embedding saves more on MuSiQue but misses the goal on the other two.
  The classical per-configuration predictors beat fine-tuned BERT and
  Qwen3-4B (LoRA) — discriminative, cross-encoder and generative — in 7 of
  9 cells at this profiling budget. Savings plateau at about ten
  characteristics.
- **Costs.** Characterisation adds about $0.001 a query: 0.51–11.9 % of
  the best-accuracy configuration's generation cost. Profiling 600 queries
  against 60 configurations on BrowseComp-Plus cost about $11,000.
- **Limits, as stated.** Predictors are for stable workloads; a shift in
  query types or intent degrades them until retraining. Code and 526
  profiling traces promised on the final version (U whether released).

## 2. HarnessTax: what the harness costs

Melissa Z. Pan, Shuo Yang, Negar Arabzadeh, Wei-Lin Chiang (Arena), Ion
Stoica and Matei Zaharia, *HarnessTax: How Much Does the Harness Matter
for Coding Agents?*, a post of September 2026; its repository created
2026-09-14 (P). Discussed on Hacker News (S).

- **Setup.** 21 model–harness pairs: seven models across Claude Code,
  Codex CLI and Pi, a minimal open-source harness; 30 random tasks each
  from SWE-bench Lite and Terminal-Bench 2.0; three runs a task; each
  harness's native configuration at high effort; 100 agent turns at most;
  official evaluators; 95 % intervals from 10,000 bootstrap resamples;
  token costs from one price list dated 2026-09-01; network blocked for
  SWE-bench Lite.
- **Finding 1: the harness moves cost more than success.** The average
  harness effect on success stays within about ±2 % on SWE-bench Lite and
  ±5 % on Terminal-Bench 2.0, while cost differs up to 5×. Claude Fable 5
  solves 97.8 % in Claude Code, 96.7 % in Codex and 96.7 % in Pi, at $1.33
  against $0.67 an attempt in Claude Code and Pi. By geometric means of
  cost ratios, Claude Code costs 2.0× Pi and 1.6× Codex on SWE-bench Lite
  and 1.5× Pi on Terminal-Bench 2.0.
- **Finding 2: a simple harness is competitive.** Pi reaches the
  cost–success frontier on both benchmarks with four tools — read, write,
  edit, bash. Turn counts can match at twice the cost (Fable 5, 15.4 turns
  in Pi and 15.3 in Claude Code). Claude Code's mean initial context is
  over ten times Pi's, in longer instructions and larger tool schemas.
- **Finding 3: models do well outside their own harness.** Across the six
  Anthropic and OpenAI models and both benchmarks, another harness gives
  the highest observed success in nine of twelve comparisons; GPT-5.6 Sol
  solves 83.3 % of Terminal-Bench 2.0 in Pi against 78.9 % in Codex, at
  $0.42 against $0.76.
- **Caveats, as stated.** Two open benchmarks the models may have seen in
  training; results may differ elsewhere. Their closing view: everyday
  agents should favour cost and reliability; problems at the edge of a
  model's capability, scientific discovery among them, may still gain from
  harnesses that structure exploration, evaluation and feedback; harness
  selection should not be the user's job.

## 3. Squaring it with the science benchmarks

A reading. [AI as science assistant](science-assistant-landscape.md) §8
found the harness moving scores a great deal — CORE-Bench from 42 % to
78 %, PaperBench, ScienceAgentBench, MLE-bench. HarnessTax finds it
moving mostly cost. Both hold: those gaps sit at the edge of capability
and often set a bare model or an early scaffold against a mature agent;
HarnessTax sets three mature harnesses against tasks the models nearly
solve, so success converges and only cost differs. Below the edge the
harness decides what is paid; at the edge, whether the task is solved.
The concept trace works at the edge; an everyday lookup well below it.

**Whose claim this is.** The measured finding is HarnessTax's, and narrow:
three mature harnesses, near-equal success at different cost, on the two
benchmarks tested. The edge-of-capability framing is this note's
synthesis, not the study's result; the study only speculates, in its
closing, that problems at the boundary of capability may still benefit
from structured harnesses (§2).

**Caveats on the reading,** added 2026-09-17 at the user's request:

- **A ceiling effect.** At success near 97 %, harnesses cannot differ much
  in success rate; part of "the harness barely matters" may be
  measurement, not a property of tasks within capability.
- **Unlike contrasts.** The large harness effects of the science
  benchmarks often set a bare model or an early scaffold against a mature
  agent; HarnessTax compares three mature harnesses. How different the
  harnesses are may explain the gap as much as how hard the task is.
- **Same score, different tasks.** A CORE-Bench follow-up found Opus 4.5
  at 82.1 % in both CORE-Agent and OpenCode, disagreeing on 31 % of tasks
  ([science assistant](science-assistant-landscape.md) §8): equal headline
  success can hide different per-task behaviour — BRANE's per-query
  variance again (§1).
- **A faint consistent hint.** HarnessTax's harness effect was larger on
  Terminal-Bench 2.0, where success is lower (about ±5 %), than on
  SWE-bench Lite (about ±2 %) — suggestive at most, over two benchmarks of
  30 tasks each.

## 4. What they mean for this repository

Readings, 2026-09-17.

### 4.1 A menu of costed configurations

The query path as named configurations rather than one pipeline: the
retriever (BM25 now; dense or hybrid later, future ambitions §5.1), *k*,
graph hops and caps, and the reading strategy — none, the classifier
cascade (the tracer's engine §4), or a capable model — each with a
measured cost, so that a router can choose per query. The user's working
assumption (future ambitions §5) becomes the cheap end of the menu rather
than the pipeline, and BRANE shows how much of a workload the cheap end
answers.

### 4.2 The router is the tracer's engine

BRANE's recipe — a frontier model proposes about ten yes/no questions
once, a cheap model answers them per item, a classical classifier decides
— is the rubric written per trace and the tiny classifier per trace of
the tracer's engine §5, and the shape of Jev's typed questions (§1 there).
Classical predictors beating fine-tuned 4B models at small profiling
budgets is evidence for the cheap route.

### 4.3 Profiling made cheap by citations

BRANE's cost was LLM calls and an LLM judge. Retrieval configurations here
cost little to run, and the citation scoreboard of future ambitions §5.2
gives correctness labels without a judge — a work's own references as the
answer key — so a router over the retrieval half can be profiled nearly
free; only the reading configurations spend tokens. The caveat: a
reference list is an incomplete set of positives, so the labels suit
recall-shaped retrieval configurations, not reading ones.

### 4.4 The front door's harness tax

HarnessTax's gap came largely from context — instructions and tool
schemas. For an MCP or JSON front door, the size of its tool schemas and
the length of its answers are paid by every agent on every call: few tools,
like Pi's four (the seven gRPC calls are near that), terse schemas, compact
answers by default (future ambitions §5.1). Measurable: tokens per
answered question for a reference agent using this repository against the
same agent using the Semantic Scholar or OpenAlex APIs — cheap to use, as
a number.

### 4.5 How the scoreboard reports

Cost against quality as a frontier, not one accuracy figure; repeated
runs; bootstrap intervals; a fixed price list; a labelled harness —
HarnessTax's practice with AstaBench's labelling (science assistant §8),
for the scoreboard of [benchmarks for search and
research](retrieval-benchmarks.md).

### 4.6 Drift on the freshness clock

BRANE's predictors degrade when the query mix shifts. A paper going viral
and a burst of queries about it is that shift, so retraining a router
belongs on the clock the freshness work already keeps (serving plan §4).

## 5. Open questions

1. Which knobs become named configurations first, and how their cost is
   measured (§4.1).
2. Whether citation-derived labels are good enough to route retrieval
   configurations, given incomplete reference lists (§4.3).
3. The token cost of the front door's schemas and answers against the
   Semantic Scholar and OpenAlex APIs (§4.4).
4. How a router's retraining is tied to the batch clock (§4.6).
5. Whether BRANE's code and traces are released, and whether its
   characteristics transfer to scholarly queries (§1).

## Sources

[Natural Language Query to Configuration for Retrieval Agents](https://arxiv.org/abs/2605.27361);
[HarnessTax](https://harnesstax.github.io/);
[HarnessTax on Hacker News](https://news.ycombinator.com/item?id=49733726);
[Harness-Bench](https://arxiv.org/pdf/2605.27922);
[HARBOR: automated harness optimisation](https://arxiv.org/pdf/2604.20938);
[what makes a harness a harness](https://arxiv.org/pdf/2606.10106);
[Pi coding agent](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/README.md);
[Portkey: the harness tax](https://portkey.ai/blog/the-harness-tax/);
[Terminal-Bench](https://arxiv.org/abs/2601.11868);
[SWE-bench Lite](https://www.swebench.com/lite).
