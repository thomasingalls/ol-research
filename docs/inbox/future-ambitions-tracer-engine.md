# Future ambitions: the tracer's engine

**Status:** speculative notes, 2026-09-16 — one conversation, recorded at
the user's request. Nothing here is a plan, a decision or scheduled; where
a note suggests an order or a design, it is a suggestion the user has not
taken up. What would do the reading in the concept trace of [future
ambitions](future-ambitions.md) §6: a classifier that takes plain-language
instructions, its precedents, and a local cascade as the alternative.
Companion to the [retrieval frontier](retrieval-frontier.md). Facts are
marked **primary** (the vendor's or project's own pages, read), **search**
(search results summarising those pages, not read directly),
**secondary** (someone else's account) or **from memory** (not checked
that day).

## 1. Jev

TypeSafe AI's Jev, in early access from the week of 2026-09-14
(secondary), was raised by the user as "the engine of the concept
tracer". Its founder, Diogo Almeida, is a former OpenAI researcher who
worked on the instruction-following research behind ChatGPT (primary:
the launch post; secondary: press) — not an OpenAI cofounder, and an
author of the InstructGPT paper (from memory).

What it is (primary:
[launch post](https://typesafe.ai/blog/introducing-system-one-models-and-jev),
[introduction](https://docs.typesafe.ai/introduction),
[patterns](https://docs.typesafe.ai/patterns),
[confidence](https://docs.typesafe.ai/confidence),
[System One](https://docs.typesafe.ai/concepts/system-one)):

- **A "System One model":** fast, structured decisions for software, named
  after Kahneman's fast, intuitive thinking. It "gives up string
  generation".
- **Input:** text, with an emphasis on structured program state. A
  request budget of about 32,000 tokens is secondary.
- **Output:** typed answers to questions defined in advance, never outside
  them — *choice* (up to 255 options), *score* (on a rubric), and a
  yes/no probability — each with its probabilities and a confidence
  value. Many questions in one call ("speculative fan-out"); confidence
  as a second routing axis ("confidence-gated routing").
- **Training:** "Reinforcement Learning for Calibrated Decisions": its
  probabilities "optimized against outcomes". No calibration evidence is
  published; how confidence is computed is deferred to a cookbook.
- **Speed and price:** 70–500 ms end to end; $0.042 per million input
  tokens, output free.
- **Evaluation:** workflow evaluations the team built, which the post
  says are biased toward it; its headline gains described as the high
  end of real-world ones.
- **Not disclosed:** architecture, size, training data, rate limits, batch
  support, data retention.
- **Access:** a hosted API behind a waitlist; the model alias
  `jev-latest` is secondary.

## 2. Jev and the trace

The trace's steps (future ambitions §6.2) against the three question
types:

| step | fit | how |
|---|---|---|
| does this abstract discuss the concept | excellent | a yes/no probability |
| central or in passing | excellent | a score on a rubric the trace writes |
| which references it credits for the concept | excellent, with citation contexts | one short yes/no per citing sentence: does this credit the cited work for the concept |
| quote the passage | good, differently | the text split into sentences, one chosen |
| what the work calls the concept | no | naming is string generation |
| rewriting the query; writing up the trail | no | generation |

- **Citation contexts are the fit.** Following only the references a
  work credits for the concept is what keeps a trail thin (future
  ambitions §6.3); as yes/no questions over short sentences it is the
  high-volume, short-input shape Jev is built for.
- **A pick instead of a quote.** A sentence chosen from sentences the
  trace supplied is the source's own text, so no quote can be invented
  and the quote check of future ambitions §6.3 is not needed; a confident
  wrong pick remains possible.
- **Reduction by selection.** The small models of the original idea
  summarized; a classifier keeps the sentences that matter, so what the
  capable model reads is still verbatim.
- **Probabilities order the frontier.** The best-first loop of candidate
  management §2 needs a score per candidate; confidence-gated routing
  sends the uncertain ones to a larger model.
- **What it cannot do** is the vocabulary that grows backward: the name
  a work uses for the concept. Either candidate names offered as a choice
  (key phrases, MeSH terms, keywords; 255 at most), or a generative model
  on the few works that pass the filter.
- **Beyond the trace:** matching references that carry no name — BM25
  finds candidates, a choice picks the cited work or none (the merge
  plan's matching stage) — and the merge's hard identity cases: short
  equal titles (merge plan §11, item 13), translations (item 12),
  journal-issue DOIs (item 17), its answers assertions of a named model
  and version ranked below the sources.
- **Cautions.** A hosted service days old from one startup; abstracts
  sent to a third party, where the data terms carry their risk; an alias
  that may move under the answers, so outputs kept by model and date;
  calibration on scientific text untested, and it is what the frontier's
  order rests on. Suggested: the tracer designed against the three
  question types — choose, score, yes/no — with Jev one implementation of
  them.

## 3. Precedents

The user's view: this is not the first classifier of its kind, and they
had the idea about a year before. The interface is old; the strands:

1. **Zero-shot classification by entailment,** 2019–2020 (from memory).
   Yin, Hay and Roth turned each label into a sentence and asked a model
   trained on entailment how likely the text implies it; Hugging Face made
   it a pipeline in 2020. Plain-language labels in, probabilities out, from
   small models with shallow understanding, one pass per label.
2. **Odds on answer words,** 2020–2021 (from memory). GPT-3's few-shot
   classification and PET's cloze prompts compare the probabilities of
   label words; FLAN and T0 made plain task descriptions work without
   examples. "Calibrate Before Use" showed example order and label wording
   skewing the probabilities.
3. **Rerankers and reward models,** 2019 onward (from memory). A query and
   a document read together, one relevance number out (monoBERT, monoT5);
   reward models the same for the quality of a response, the core of
   RLHF. Jev's score in essence, trained per purpose.
4. **Classifiers that read a written policy,** 2023–2025. Llama Guard
   (2023) reads a list of harm categories and answers safe or unsafe with
   the categories; ShieldGemma (2024) reads a policy and scores by the
   probability of "Yes" (both from memory). gpt-oss-safeguard (2025-10,
   search: [OpenAI](https://openai.com/index/introducing-gpt-oss-safeguard/),
   [guide](https://developers.openai.com/cookbook/articles/gpt-oss-safeguard-guide)):
   open weights at 20B and 120B under Apache 2.0, a developer's policy
   read at inference, reasoning in writing before it decides. The closest
   in spirit, trained for safety only.
5. **Small encoders driven by a schema,** 2023–2025. GLiNER (2023, from
   memory) finds entities of types named at run time; GLiNER2 (Fastino,
   EMNLP 2025, search: [post](https://fastino.ai/blog/gliner2),
   [repository](https://github.com/fastino-ai/GLiNER2),
   [model](https://huggingface.co/fastino/gliner2-base-v1)): 205 million
   parameters, classification, entities and structured extraction from a
   declared schema in one pass on a CPU. The closest in shape — typed
   output, one pass, fast and cheap — and able to point at spans, which
   Jev cannot; small, so shallow on subtle instructions.

Two claims with their own history (from memory): type safety is
guaranteed for generative models by constrained decoding (Outlines, 2023;
OpenAI's Structured Outputs, 2024); and preference tuning damaging
calibration is documented (the GPT-4 technical report, 2023), which
training on outcomes answers.

| approach | subtle instructions | fast and cheap | general |
|---|---|---|---|
| entailment zero-shot | weakly | yes | yes |
| odds on answer words | yes | no | yes, badly calibrated |
| rerankers, reward models | for their task | yes | no |
| policy classifiers | yes | the reasoning ones are slow | safety only |
| GLiNER2 | shallowly | very | yes |
| Jev, claimed | yes | yes | yes |

**What Jev adds, if its claims hold:** frontier-level judgment at
classifier latency with a full distribution per answer, as a general
component; a training target of calibrated probabilities; embedding-like
pricing for a judge — OpenAI's small embedding model launched at $0.02
per million tokens (from memory) — which makes judging every candidate
cheaper than choosing which to judge; many questions sharing one reading
of the input, mechanically old (prefix caching) and built into the
product. The interface was never the hard part. Understanding subtle
instructions is (the user's conclusion), with honest probabilities and a
price low enough to call on everything; that is Jev's bet, and it can be
tested.

## 4. A local cascade instead

The user's question: GLiNER2 is fast and cheap — can a recent small model
make up the difference in two or three passes (entailment, GLiNER2, a
reward model) rather than one?

### 4.1 Where small models fall short

Most candidates in a trace are easy, and a recent small model gets them
right. The subtle minority it does not: near neighbours (a hard sweep, a
soft sweep, background selection); credit against mention (cited *for*
the idea against cited beside it); negation and hedging ("unlike the
sweep model of…"). A cascade does not need the small model to be right on
those — only to know they are hard, so that only they reach something
expensive. If the cheap passes know when they are unsure, the whole costs
close to all-small and scores close to all-capable.

### 4.2 The passes, adjusted

- **Entailment dropped.** An instruction-following reranker does its job
  better; an entailment model and GLiNER2 are both small encoders and
  fail on the same subtle cases, so stacking them adds little.
- **The reward model replaced by an instruction-following reranker.**
  Reward models are trained to rate chat responses, not whether a text
  meets a criterion. The Qwen3 reranker family (2025, Apache 2.0, 0.6B to
  8B; from memory) reads an instruction, a query and a document and scores
  by its own odds of answering yes: Jev's yes/no question, locally.
- **GLiNER2 kept, for spans.** It picks the sentence and extracts the
  name the work uses for the concept — the step Jev cannot do.

### 4.3 The cascade

1. **Candidates** from BM25 and the citation graph, as now.
2. **A rubric,** written once per trace by a capable model (§5).
3. **A small reranker** scores every citing sentence against the rubric,
   loosely: what it drops is gone, so it is tuned for recall.
4. **GLiNER2 on the survivors:** the key sentence, the name the work uses.
5. **Escalation** of the uncertain middle — and of cases where the passes
   disagree — to a capable model or to Jev. Disagreement between passes is
   an uncertainty signal where small models' own confidence cannot be
   trusted, as disagreement between sources is this repository's product.

## 5. A policy classifier for any query

The user set it aside — no way to write one for an arbitrary query. Two
ways that work:

- **The policy written per trace.** A trace judges thousands of sentences
  for one concept, so the work particular to the query happens once: a
  capable model writes the concept's definition, what counts as crediting
  a work for it, three to five near misses to reject, a few example
  sentences either side, and the other names it knows; every small-model
  call carries that rubric. An arbitrary query is one expensive call and
  thousands of cheap ones.
- **The concept as an input, not trained in.** One judge trained on
  (concept, citing sentence, credits or not) across hundreds of concepts:
  labels from a capable model once, sentences from Semantic Scholar's
  `citations` contexts (retrieval frontier §4). This is how
  instruction-following rerankers are trained, and it gives a
  concept-credit judge of one's own that takes any concept. After the
  rubric has shown where small models fall short, not before.
- **Between the two:** per trace, a capable model labels a couple of
  hundred candidates and a tiny classifier (SetFit-style, trained in
  seconds) learns that one concept and labels the rest.

## 6. What would decide it

- **The hand-labeled citation sentences** (Jev's test in the
  conversation): sentences for one of the user's concepts, labeled
  credits-the-cited-work-for-it or not, over four arms — one small
  reranker, the cascade, Jev, and a capable model as the ceiling. Counted
  separately: the true sentences a loose first pass dropped, and the
  wrong calls at the last pass; and whether each arm's probabilities
  match the labels. Needs the Semantic Scholar key for the contexts and
  Jev access for its arm.
- **Whether the box has a GPU.** A reranker of 0.6B to 4B over short
  sentences is fast on one and slow on a CPU; that decides whether the
  local cascade is practical.

## 7. Open questions

1. Whether Jev is clearly better than an open baseline on accuracy and
   calibration over scientific text, enough to depend on one vendor's
   hosted service.
2. Its data retention and terms, before abstracts are sent to it.
3. Whether the box has a GPU (§6).
4. Which names a choice can offer for the vocabulary step (§2), against
   leaving it to a generative model.
5. When, if ever, the concept-credit judge of §5 is worth training.
