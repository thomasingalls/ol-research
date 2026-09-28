# Searching within a result set

**Status:** idea note, 2026-09-28, recorded at the user's request from a
conversation the same day. In the user's words it is "still an incomplete
thought": the user is circling the idea, and this note records how far
the circling had got. Its scope, set by the user later the same day:
"this is specifically about searching academic literature (and
associated scientific data, later)". The research behind §12 is kept
verbatim in [search-within-results-agent-reports/](search-within-results-agent-reports/).
Speculative, not a plan: nothing here is built,
scheduled or decided, and nothing in it amends the plans of record. The
words for the mechanism (facet, step, slice) describe the outcome wanted,
not a design; the user: "it's not prescriptive for the mechanism, it's
supposed to describe the desired outcome". Facts are marked **P**
(primary: this repository's code or notes, or a project's own repository
page, read 2026-09-28), **S**
(secondary: press coverage, papers or product documentation, read
through a search engine's summaries on 2026-09-28) or **U** (from memory, not checked). Related:
[ambitions beyond citations](ambitions-beyond-citations.md) §5 (its "did
you mean" row: the facet distribution over the top results offered as
refinements), [candidate management](candidate-management.md) §1 (the
funnel), [the retrieval frontier](retrieval-frontier.md) §3–§4 (SPECTER;
Semantic Scholar's published embeddings), [text analysis](text-analysis.md)
§3 (what the tokenizer does to `vitamin A`),
[citation parsing](citation-parsing-landscape.md) §8 (GLiNER and its
kin), [product shapes](product-shapes.md) §5 (the tracer's
reranker-and-GLiNER2 cascade), the
[evaluation plan](../evaluation-plan.md) §3.4 (hypotheses are treatments,
never yardsticks), and the [serving plan](../serving-plan.md) §11, items
11, 36 and 37.

## 1. The idea, in the user's words

As first put:

> given a results set from an initial query, *search within that results
> set* (1) to evaluate whether it's a good initial query or not, (2) to
> create ad-hoc faceting, (3) because that's how i'd search if i had to
> look through 30 million matching physical papers, i'd say "wow, that's
> a lot of papers" and i'd immediately start trying to slice that results
> set into smaller pieces.

On the hit count:

> i've always thought of 'this many hits' as a bad, crude estimator of
> search relevance, it's just that it's the only estimator that's
> available so it gets used.

A worked example:

> if i *know* that i want to search within the articles that contain, for
> example "mouse OR mus OR rat OR whatever the genus name of rat is,
> rattus?" and the corpus doesn't have tagging or labeling on every
> document that contains mouse/rat-related work, then i'd be interested
> in being able to create my own, on-the-fly, custom facets [...] ie
> start with the "mus" or query, then *within that set* narrow it down to
> biomed, then within that set narrow it down to studies that used
> real-time pcr, then ... etc.

and the question it raised: "am i just describing boolean search? maybe
my problem with boolean search is the interface".

On what would make it possible:

> having a 'what is the shape of this results list' would be really,
> really useful, especially if you could generate this easily and
> quickly.
>
> this really feels like a hybrid search question. having vectors on the
> results set would make the generation of and display of a one-off
> segmentation of a particular results set easier - nay, possible in the
> first place.

And a thesis about labels:

> my assumption is that it's basically never worth it to pre-compute
> labels or tags or categories on a collection of, well, anything,
> because the labels are only meaningful in the context and to the entity
> creating those labels. somebody else's understanding of the labeling
> system will always be different, and somebody else's context will
> certainly involve meaningful disagreements about classification.
>
> today, though, with classifiers (see: the newly released jev and the
> bevy of copycats, small llms, and even less expensive methods [...]) i
> feel like this is tractable

The less expensive method, named later the same day: "gliner is the
method i was trying to think of. or gliner2, to be precise" (§10).

And later again:

> it occurs to me that beam search might be an applicable technique to
> the large results set stage, but i'm kind of loosely throwing ideas
> around so i'm not sure exactly what i mean by that

Of the readings offered, the user kept two as worth recording (§11).

The sections below are the conversation's reading of these, not
decisions.

## 2. Words

- **Result set:** every work a query matches, not the page of the
  top-ranked ones.
- **Step:** one narrowing of a result set: a query run within it, a facet
  value chosen, a cluster picked. Steps stack, and each has a count.
- **Probe:** a query asked of a set only for its count ("how many of
  these mention mice?"), without narrowing.
- **Custom facet:** a named definition (a list of words, a description, a
  classifier) that can be counted over any set, beside the built-in
  facets such as year and genre. A step is "within this set, only what
  the facet holds".
- **Shape:** what a set is made of: its years, venues, distinctive words,
  and the groups its works fall into.
- **Vector:** a list of numbers a model computes from a work's text, so
  that works the model deems similar get nearby lists.
- **Classifier:** anything that says yes or no, or which of a few, for
  one work.

The running example is the user's chain: *mouse OR mus OR rat OR
rattus*, then biomedical, then used real-time PCR.

## 3. What search does today

- **Counts, but no narrowing (P).** A search counts facets over every
  match in the same pass that ranks them (`facet_collector` in
  `crates/search/src/index.rs`): year, form, genre, language, status,
  held_by and access (`facets` in `crates/search/src/schema.rs`). Nothing
  can be narrowed by them yet; facets as filters wait on the query
  language's second pass (serving plan §11, item 36).
- **Any word matches (P).** A query's words are joined with OR and
  ranked by BM25; whether every word must match is the indexing policy's
  question, undecided (serving plan §11, item 11).
- **The tokenizer splits and lowercases (P)** (`DESCRIPTION` in
  `crates/search/src/schema.rs`): no stop list, no stemming.
- **Only titles, abstracts, author names and venues are searched (P)**
  (`Fields::searched`). No full text.
- **MeSH stops before the index (P).** PubMed's MeSH headings are read
  into the normalized records (`crates/model/src/mesh.rs`,
  `crates/source-pubmed/src/normalize.rs`) and go no further: neither the
  merge nor the index carries them.

## 4. Mostly counts, not lists

Slicing a pile of thirty million means asking "how big is this piece?"
again and again. The ranked list matters only at the end, when a slice is
small enough to read. So the main output of a step is a count, which has
to be cheap and quick, and the ranking can wait until the slice is small.

This is also why the user's complaint about the hit count lands: a count
measures the size of a set, not whether it is the right set. It is read
as a measure of relevance because it is the one number shown.

## 5. A set needs an edge

What does "in the results" mean? Under today's rules `vitamin A
deficiency` matches nearly every work with an English abstract, because
nearly every abstract contains the word *a*, and the tokenizer keeps it
([text analysis](text-analysis.md) §3). A ranked list hides that, since
the good works float to the top. A set to search within shows it at once.

Searching within a set only means something if the set has an edge that
means something. That pushes toward the librarians' model: membership is
yes or no (every word, or the later AND/OR/NOT language), and ranking is
only an order within the set. The open any-versus-every question (item
11) gets harder to put off.

That model is already practice (U): searchers for systematic reviews
build a strategy one numbered line at a time, in Ovid or in PubMed's
search history (`#1 AND #2`), and the count beside each line is how they
judge the line. The user's first purpose, judging the query, is what
those counts are for, done with one number.

## 6. Is this boolean search?

The logic, yes. The chain is a stack of ANDs over an OR group: `(mouse OR
mus OR rat OR rattus) AND biomedical AND real-time-PCR`. (The rat genus
is *Rattus*; the laboratory rat is *Rattus norvegicus*, the laboratory
mouse *Mus musculus*. U.) A custom facet is a named query that can be
counted over any set, and a step is an AND with it.

What a boolean search box lacks is feedback: it takes one string written
blind and gives back one number. A stack of steps can show, at every
step:

1. **A random sample of what the step kept:** precision, judged by eye.
   The user's own example needs it. The tokenizer lowercases (P), so
   *rat* also finds RAT, the rapid antigen test, across the COVID
   literature; *mus* finds MUS, medically unexplained symptoms; *mouse*
   finds computer mice in the human–computer interaction papers. (U: the
   abbreviations are common usage; none is counted in our index.)
2. **A random sample of what the step threw out:** recall, which no
   boolean interface we know of shows. It is the only guard against
   narrowing too far: a count cannot say that the PCR step dropped half
   the studies that used PCR.
3. **The count at each step, and the overlaps between steps,** so the
   step that did the damage can be seen, taken out, and the counts
   watched as they change.
4. **The count with each word left out,** which says which word blew the
   set up: in the `vitamin A` example, *a*.
5. **How the set differs from the whole corpus:** its years and venues
   beside the corpus's, and the words unusually common in it
   (Elasticsearch calls this the "significant terms" aggregation, U). A
   set that looks like the corpus is a query that did nothing.

A random sample, not the top of the ranking: the top is the best-looking
part of the pile, and a physical pile is judged by pulling from the
middle. Twenty works marked on-topic or not give a rough share of the set
that is on topic, with a wide error bar.

So the interface is the problem, as the user suspected, but less its
syntax (parentheses, precedence) than the missing feedback, and §7's
concept steps.

## 7. Steps that are concepts, not words

*Biomedical* is not a word abstracts use. Boolean search reaches a
concept only through labels someone else assigned, or a long list of
words typed by hand. With what the project holds:

- **Biomedical ≈ "PubMed holds it"** (P: the `/held_by/` facet). Crude
  both ways: much biomedical work is not in PubMed, and some of what
  PubMed holds is not biomedical.
- **Used real-time PCR** is usually said in a methods section, and only
  titles and abstracts are indexed (P). The words are many and messy
  (qPCR, RT-qPCR, TaqMan, SYBR Green), and *RT-PCR* is written for both
  reverse transcription and real time (U). So the step's recall has a
  ceiling set by which text is held, and the count does not show where
  the ceiling is.
- **MeSH is where people have labelled works with concepts** such as
  *Mice* and *Rats* (U). It keeps Real-Time Polymerase Chain Reaction
  apart from Reverse Transcriptase Polymerase Chain Reaction, assigned
  per article, for indexed articles only, with a lag
  ([ambitions](ambitions-beyond-citations.md) §6, P). It is not in the
  index (§3).

**A word rule measured where labels exist, applied where they do not.**
Where MeSH exists, the system could say: your mouse-or-rat words catch
this share of the works labelled *Mice* or *Rats*, and this share of what
they catch carries neither label. Once satisfied, the user applies the
words to the Crossref and OpenAlex works that have no MeSH. That is what
makes a custom facet more than a saved boolean string: measured where a
reference exists, extended where none does. MeSH is a reference here,
not the truth; where the two disagree is what to look at.

It does not break the rule that an answer key be independent of what is
tested ([evaluation plan](../evaluation-plan.md) §3.4): MeSH judges the
user's own definition, not the search's ranking, and the definition is
not built from MeSH.

## 8. The shape of a set, quickly

Vectors, as the user says, are what make a one-off segmentation possible
at all: words cannot group papers that say the same thing in different
words. Quickly means from a sample; thirty million works need not be
clustered to see the shape of thirty million:

1. Take a random sample of the set.
2. Cluster the sample's vectors.
3. Name each cluster, from its most distinctive words or by a small model
   reading a few members, or leave it unnamed and show a few of its
   works: [ambitions](ambitions-beyond-citations.md) §6 already holds
   that unlabelled clusters are acceptable when the reader sees example
   works, not labels.
4. Give every other work in the set its nearest cluster: one comparison
   of vectors per work.

The sample's size sets how precise the picture is; the set's size hardly
matters, so a set of thirty million and one of thirty thousand cost about
the same to sketch once the vectors exist. BERTopic packages this
pipeline: embed, reduce, cluster, label with distinctive words (U).
Scatter/Gather (Cutting, Karger, Pedersen and Tukey, SIGIR 1992) was the
interactive form: cluster the results, pick the clusters that look right,
cluster those again (U).

**A lens, not the net.** Hybrid search usually means vectors that *find*
works; here they *organize* a set found some other way. The difference
matters because a nearest-neighbour search has no edge (every work is
some distance from the query), which is §5's problem again. So three
jobs:

- an explicit outer edge, from words and filters, that can be read and
  edited;
- vectors to show the shape inside it;
- classifiers to draw the inner edges asked for.

## 9. Labels, and who makes them

The user's thesis (§1) is that pre-computed labels are basically never
worth it. The conversation agreed with its core and read it as a split:
**pre-compute what is not a judgement (vectors, word statistics); compute
judgements at query time, for the person asking.** Two caveats:

- **Vectors are not neutral either.** A model's "similar" is its
  trainers'. SPECTER learned it from citations
  ([the retrieval frontier](retrieval-frontier.md) §3, P): two papers are
  close if the literature cites them together. Less opinionated than a
  label, not opinion-free, and a choice to make on purpose.
- **Other people's labels are worth keeping, as signed claims, not
  truth.** MeSH is a committee's view; OpenAlex's topics are an
  aggregator's clustering. The project already treats OpenAlex's and
  Semantic Scholar's author clusters that way
  ([ambitions](ambitions-beyond-citations.md) §3, P). Kept so, their
  labels are what the user's own category is measured against (§7), and
  one more kind of evidence a classifier can use. "Never pre-compute
  labels" becomes "never pre-compute *your* labels; keep theirs, with who
  said them".

The project already has a form for a signed label from a process:
[ambitions](ambitions-beyond-citations.md) §6's annotation, `(work,
label, process, version, evidence)`, with scores stored raw and cut at
query time (P). Cutting at query time is a middle ground the user's
thesis would still question: the cut is the asker's, but the list of
labels was fixed before anyone asked.

## 10. Why it is tractable now

The funnel of [candidate management](candidate-management.md) §1 (a
costly reader for the few, a cheap one for the many), applied to a label:

1. The user describes the category in a sentence ("experiments on live
   mice or rats"), or marks a couple of dozen examples.
2. A stronger model labels a few hundred works from the sample: GLiNER2
   on the box, a small LLM, or a Jev-like classifier.
3. A cheap model over the pre-computed vectors learns that boundary and
   applies it to every work in the set: one multiply-and-add per work,
   nearly free beside reading them.
4. The user checks three samples (what it kept, what it dropped, and the
   works nearest the boundary), marks them, and it learns again.
5. Marks used to train cannot also say how well it works, so some are
   held back: the evaluation plan's frozen half again, inside the
   product.

**GLiNER2, the method the user meant.** A small encoder model (not a
text generator) from the GLiNER line: GLiNER (Zaratiana, Tomeh, Holat and
Charnois, NAACL 2024) finds spans of any entity type named at call time;
GLiNER2 (Fastino, EMNLP 2025 System Demonstrations) adds text
classification and structured extraction in the same model, driven by a
schema passed with each call. The facts, benchmarks and licences are in
[citation parsing](citation-parsing-landscape.md) §8 (P); what bears on
this idea:

- **Labels named when asked.** Classification takes its label names in
  the call (`classify_text(text, {"organism": ["mouse", "rat",
  ...]})`, P: the project's README), with no training. That is the
  user's thesis in a model's interface: the labels are the asker's, made
  at the moment of asking.
- **Entities as well as labels.** It can pull the spans out of an
  abstract (*real-time PCR*, *C57BL/6*, *Rattus norvegicus*), which gives
  a custom facet whose values are found in the set rather than listed in
  advance: name a facet, "organism studied", and each work's value is
  what the model finds in it. The spans also show why a work is in a
  step, which a vector cannot.
- **On the box.** It runs on CPU ("CPU first", P: the README), and the
  v2 and later checkpoints are Apache-2.0; the v1 checkpoints are
  non-commercial, so each checkpoint's licence is checked, not the
  repository's (citation parsing §8, P). Nothing leaves the box, there is
  no charge per call, and the abstracts stay where the terms want them,
  which answers Jev's three problems below. `gline-rs` runs these models
  from Rust (§8, P).
- **Learns from the user's marks.** It can be fine-tuned with LoRA (P:
  the README), so step 4's marks could tune it as well as the cheap model
  over vectors.
- **Where it sits in the funnel.** A transformer read per work: far
  dearer than step 3's multiply-and-add, far cheaper than an LLM. So it
  fits step 2's few hundred, or the whole of a slice once the slice is
  small, not the first thirty million. Zero-shot it is weaker than a
  large model at classification (average 0.72 against GPT-4o's 0.84 in
  its paper, §8, P), which is one more reason for step 4's checks and
  step 5's held-back marks. Use GLiNER2.5 if it is used: classic GLiNER
  cannot see a span longer than its width limit (§8, P).

Other cheap methods named in the conversation, the conversation's guesses
before the user remembered GLiNER2 (U): SetFit (few-shot tuning of a
sentence-embedding model); logistic regression on embeddings, called a
linear probe (step 3's cheap model); fastText; zero-shot classification
with NLI models, or GLiClass; Snorkel-style weak supervision (hand-written
rules that vote); active learning, to pick which works to ask about.

**Jev (S).** TypeSafe AI's classifier: limited early access from
2026-09-15, open to everyone 2026-09-20, new signups paused 2026-09-22.
It reads text and returns one of a fixed set of options the caller
defines, with a probability, and writes no prose. Its shape is a step's
shape exactly. But it is hosted and proprietary: every work it
classifies leaves the box, its cost grows with the set, and abstracts
sent to a third party are where the sources' terms carry the most risk
(serving plan §7, "abstracts are the risk"). So it fits the few hundred
of step 2 better than thirty million, and GLiNER2 would fill the same
slot without leaving the box. Its published figures (fewer
mistakes than an LLM judge, faster, far cheaper) are TypeSafe's and the
press's, not tested here. Coverage read:
[Wikipedia](https://en.wikipedia.org/wiki/Jev_(AI_model)),
[The Batch](https://www.deeplearning.ai/the-batch/models-built-to-do-one-thing-well),
[innFactory](https://innfactory.ai/en/blog/jev-system-one-model-classifier-not-llm/),
[The Register](https://www.theregister.com/devops/2026/09/23/shut-up-and-calculate-jevs-new-ai-primitives-for-coders/5298431).

## 11. Beam search over steps

The user's idea (§1): beam search at the stage where the set is large,
its meaning unsettled. Of three readings offered, the user kept the two
below.

**Beam search:** explore a tree of choices, keeping only the best *k*
partial paths (the beam) at each depth: extend each by one step, score
the children, keep the best *k* again. A stack of steps (§2) is a path in
such a tree: each step a choice, each longer chain a node one level down.

### 11.1 The machine proposes cuts

The system tries many candidate first steps (probes, facet values,
clusters), keeps the best *k*, tries second steps under each, and so on.
It shows a handful of readable chains, each with its count ("mice → since
2015 → knockout"): several ways to cut the pile, not one. Data mining
calls this *subgroup discovery*: finding subsets, described by conditions
joined with AND, that differ interestingly from the whole. Beam search is
its usual method, and rule learners such as CN2 search the same way (U).
§4 is what makes it affordable here: scoring a candidate step is a count,
not a ranked list.

Everything turns on the score, what makes a cut good:

- it splits the pile evenly: a step that halves the pile says more than
  one that trims a percent (a decision tree's information gain);
- its slice looks unlike its parent (§6, item 5);
- its works sit close together in vector space (§8);
- it agrees with the user's marks (§10).

Only the last is the user's own judgement. The others are the machine's
idea of interesting, which is what the user's thesis on labels (§9) is
about.

**Scored against the user's marks** is the form that interested the
conversation most. Once the user has marked some works on topic or off
(§10, step 4), the beam looks for the chain of word and facet steps that
best separates them. In effect it writes a boolean query that agrees with
the user's judgements, one the user can read and edit: §8's explicit
edge. It stands beside the cheap classifier over vectors (§10, step 3),
with a trade. The classifier catches papers that say the same thing in
other words, but its edge cannot be read; the chain can be read and
edited, but catches only the words it names. Having both shows where they
disagree. With few marks the chain overfits, so step 5's held-back marks
apply here too.

Two known traps:

- **Near-duplicates.** The beam fills with variants of one cut (since
  2015, since 2016, ...). DSSD, "diverse subgroup set discovery" (van
  Leeuwen and Knobbe, *Data Mining and Knowledge Discovery*, 2012, S),
  was built for this: it treats the answer as a set of subgroups, not a
  list of single ones, and removes redundancy inside the beam.
- **Chance.** Trying thousands of slices finds some that look surprising
  by luck alone; a suggested cut needs the caution owed to any comparison
  made many times.

### 11.2 The user keeps several branches open

The same algorithm with the user as the scorer. Instead of one stack, try
three different second steps side by side, keep the two that look right,
extend those. The librarians' numbered lines (§5) do this informally. The
interface becomes a tree to prune, not a stack to pop, and a step's
samples of what it kept and dropped (§6) are compared across branches as
well as down one.

### 11.3 Together, beside the sample, and where it shows up

The two combine: the machine proposes the best *k* next steps at each
node (§11.1), and the user chooses which branches to keep (§11.2). That
is Scatter/Gather's loop (§8) with readable steps in place of clusters
and a beam in place of one path.

A beam and a sample do different jobs. A random sample says what is in
the pile (§6, §8); a beam says which ways of cutting it look most
promising. The shape wants the first, suggested cuts the second.

The user asked why, if the technique is established, it is so seldom
seen: hidden from view, or nobody has built an interface around it? Some
of each. It shows up in pieces, in places that share a few conditions;
general search lacks them.

**Where it shows up, and why there.** For each: what it specifically is,
which part of this note it matches, and why it lives where it does.
Facts are S unless marked.

- **Power BI's "top segments"**, in the key influencers visual of
  Microsoft's business-intelligence tool. *What it is:* the analyst picks
  an outcome column (a churn flag, a rating); the visual runs a decision
  tree (ML.NET) over the other columns and reports segments, readable
  combinations of values such as "long-term customers in the west
  region", where the outcome is unusually high or low. It drops segments
  with too few rows to be more than an anomaly. Greedy, one split at a
  time, not a beam. *Matches* §11.1 with a machine's score. *Why there:*
  a table has a few dozen tidy columns and the analyst names the target:
  the two things a pile of documents lacks.
- **Automatic boolean query refinement for systematic reviews**, Scells
  and Zuccon at the ielab research group ("Automatic Boolean Query
  Refinement for Systematic Review Literature Search", WWW 2019; a
  formulation paper, WWW 2020). *What it is:* research methods that take
  a review's boolean query and walk a chain of up to five changes
  (operators swapped, fields changed, MeSH terms exploded or widened to
  their parent, clauses removed, terms expanded through concept
  embeddings), at each step keeping the one candidate a learned selector
  scores best: a beam of width one (P: the WWW 2019 paper, read
  2026-09-28). The abstract says precision rose "while maintaining, or
  even increasing, recall"; the averaged results say it is a trade. Over
  35 test reviews, the best selector (trained for F1) roughly doubled
  precision, 0.021 → 0.042, and recall fell significantly, 0.84 → 0.67,
  counting citations the reviews never judged as not relevant; the
  paper's bounds for those put recall between 0.71 (estimated) and 0.92
  (if all were relevant). Selectors trained for recall raised it to
  0.88–0.89, at a cost in precision (P). Precision near 4 % still means
  most of what is retrieved is not in the review. *Matches* §11.1 scored
  against marks: the nearest published relative. *Why there:* a systematic review has a target
  (studies known to belong) and a professional who writes boolean queries
  line by line (§5), so both the score and the readable steps exist
  already.
- **searchrefiner**, from the same group (2018). *What it is:* an
  open-source web application, not a product for sale: Go, MIT licence,
  code on GitHub, and a hosted copy described as a demo (P: the
  project's repository page). It shows a review's boolean query in parts;
  checks the query against "validation citations", studies known to be
  relevant that it must find; lets clauses be edited by drag and drop in
  a structured editor; and carries the group's query-formulation tools.
  It searches PubMed through NCBI's Entrez service, configured by
  whoever runs it, and wants a local account even when self-hosted (P).
  *Matches* §11.2's interface more than §11.1: a person sees each part of
  a query and what it does. *Why there:* as above; it is the interface
  someone did build, for one profession.
- **Screening with active learning: ASReview** (van de Schoot et al.,
  *Nature Machine Intelligence* 3, 125–133, 2021). *What it is:*
  open-source software in which a reviewer marks records include or
  exclude, and a model trained on those marks reorders the rest so that
  likely includes come first. *Matches* §10's loop of marks and a
  classifier, used to rank; not subgroup discovery. *Why there:*
  screening thousands of abstracts is the job, so the marks exist whether
  or not a machine uses them.
- **Technology-assisted review in legal discovery**, also called
  predictive coding. *What it is:* lawyers mark documents responsive or
  not, and a system trained on their marks classifies the rest. A court
  first endorsed it in *Da Silva Moore v. Publicis Groupe* (S.D.N.Y.,
  2012, Magistrate Judge Andrew Peck), over about three million
  documents; Grossman and Cormack's 2011 article in the *Richmond Journal
  of Law and Technology* argued it could be more effective and cheaper
  than reading everything. *Matches* §10's loop again, not beam search.
  *Why there:* the sets are enormous, every document read is billed, a
  missed document has legal cost, and the marks are the job.
- **Clustering on the open web: Clusty**, from Vivísimo (a Carnegie
  Mellon spin-off). *What it was:* a metasearch engine (2004) that grouped
  results into labelled folders; a search for *cell* split into biology,
  battery and prison. *Matches* §8's shape, for web search. It did not
  last there: Clusty was sold to Yippy in 2010, and the site later
  redirected to DuckDuckGo (2021–2022). Vivísimo's enterprise side was
  bought by IBM.

**Why it is rare in general search.** The conversation's reasoning, not
a finding:

1. **It needs a target.** Subgroup discovery scores slices against
   something: churn, revenue, a column. In search the target is
   relevance, which only the searcher knows, and search products seldom
   ask. Marks exist where the job makes them (screening, legal review),
   and that is where the tools are. §10's marks are the piece that makes
   §11.1 possible here.
2. **Documents have no columns.** The method grew up on tables with a
   few dozen tidy attributes. Text offers hundreds of thousands of words
   as attributes, sparse and full of synonyms, so candidate steps
   explode and near-duplicates pile up; DSSD's own stated motivation is
   that attributes with many values, and attributes that move together,
   make top-*k* answers redundant (S). Concept steps (MeSH, GLiNER2
   extractions, §7 and §10) shrink the space to something searchable.
3. **Web engines avoid exact counts.** "About 1,230,000 results" is an
   estimate: counting a large set exactly is costly and does nothing for
   the first ten links. Every candidate step here is a count. This
   index already counts every match (§3; item 37's loose end), so here it
   is comparatively cheap.
4. **Overlapping slices are hard to show.** Many slices, overlapping,
   with labels that are often poor; Clusty's fate on the open web is one
   data point. Most web searches look something up rather than explore.
5. **The audience is professionals.** Librarians, review authors, patent
   examiners and litigation teams use specialist tools (Ovid, patent
   search systems, technology-assisted review platforms) or research
   code like searchrefiner, which most people never see.

**The pattern.** It appears where four things hold together: a target or
marks; attributes or concepts to cut by; a searcher who needs the whole
set rather than the top hit; and reading everything costs real money.
General web search mostly lacks all four. Literature search for reviews
has all four, which is where its pieces are, but as separate tools. None
combining them for general literature search was found (a search, not a
thorough one, 2026-09-28).

The reading not kept: best-first expansion over the citation graph
([candidate management](candidate-management.md) §2, which is a beam
without the fixed width). It finds works rather than slicing a set.

## 12. Agents and set tools

The user, once the scope was literature: "it would be remarkable to be
unleashing agentic ai on a task like literature review (deep research
agents, etc) without knowing about such existing techniques. is it known
whether anyone is working on giving ai agents such tools? similarly, is
it known whether such tools show improvement on retrieval benchmarks?
sota retrieval is hybrid vector/bm25 with agentic judgement, as far as i
know, and is pretty expensive. i'd imagine that combining these other
known techniques with agents would make these methods dramatically more
affordable."

**Where this section's facts come from.** Three research subagents,
2026-09-28, whose reports are kept verbatim in
[search-within-results-agent-reports/](search-within-results-agent-reports/):
[1, the project's own notes](search-within-results-agent-reports/01-repository-notes.md),
[2, agents and set tools](search-within-results-agent-reports/02-agents-and-set-tools.md),
[3, benchmarks and cost](search-within-results-agent-reports/03-benchmarks-and-cost.md).
The bracketed tags are the reports' own: **[P]** the reporter read the
primary paper, docs, code or result tables; **[A]** the abstract or
landing page only; **[S]** a search engine's summary or a third party's
write-up only. Only one source was read for this note directly: Scells,
Zuccon and Koopman 2019, whose figures §11.3 now gives (P). Where the
reports disagree, both are given. Absence below means absence in the
reports' searches, not proof; this corner moved fast in 2026.

### 12.1 What literature agents are given

The common shape (report 1; [the retrieval frontier](retrieval-frontier.md)
§1): a keyword or Semantic Scholar pass mixed with dense retrieval over
rewritten queries, a model reading every candidate, citation expansion,
repeat. That note's reading: "the first-stage retriever is a commodity;
the quality is in the reading". The tools per system:

- **PaSa** ([ACL 2025](https://arxiv.org/abs/2501.10120), [P]): Search
  (Google restricted to arXiv and a cutoff date), Expand (citations from
  one named section) and Stop. No counts, filters or boolean syntax.
  About 382 Search and Expand actions per query, and a selector model
  reads every queued paper (report 2).
- **Ai2 Paper Finder** ([blog, Mar 2025](https://allenai.org/blog/paper-finder),
  [P]): criteria for author, venue and year routed to metadata lookups;
  several reformulations over three dense snippet indices plus Semantic
  Scholar keyword search; forward and backward citations; a model judges
  each sub-criterion separately.
- **Asta's corpus tools** ([MCP](https://allenai.org/asta/resources/mcp),
  [P]): seven tools with date-range, venue and paper-id filters; no
  totals, facets or boolean syntax documented.
- **PaperQA2** ([arXiv, Sep 2024](https://arxiv.org/abs/2409.13740);
  [code](https://github.com/Future-House/paper-qa), [P]):
  `paper_search(query, min_year, max_year)`, `gather_evidence`,
  `gen_answer`, `clinical_trials_search`, and citation traversal in the
  paper. The agent sees a status line, "Paper Count / Relevant Papers /
  Current Evidence": counts of its own working set, not of the corpus.
- **Undermind** ([whitepaper, Jan 2024](https://www.undermind.ai/whitepaper.pdf),
  [P]): GPT-4 classifies each candidate from its full text, about 150
  papers per search, and fits a discovery curve, f = 1 − e^(−n/τ), to
  estimate how many relevant papers exist and when to stop. A set-level
  estimate, but made by reading, not counting.
- **Consensus Deep Search** ([S]): up to 20 searches, citation traversal,
  and filters the agent applies (minimum citations, study type); reviews
  1,000+ papers and reads 50–100.
- **Elicit** ([P], blog Feb 2025): database imports plus up to 500 papers
  from semantic search, then model screening against editable criteria.
- **OpenAI deep research** ([API docs](https://developers.openai.com/api/docs/guides/deep-research),
  [P]): web search, file search, a code interpreter; an outside tool
  server is usable only if it offers `search(query)` and `fetch(id)`,
  which is the top-*k* interface exactly. **Gemini Deep Research**
  ([API docs](https://ai.google.dev/gemini-api/docs/deep-research), [P])
  is similar.
- **PaperPilot** ([arXiv, Jul 2026](https://arxiv.org/abs/2607.00597),
  [A]): the agent builds a graph of operators (keyword search, citation
  expansion, filter, score, rerank); Hit@5 rose from 58 to 77.

**Nearest to this note's tools:** OpenAlex's
[LLM quick reference](https://help.openalex.org/api/llm-quick-reference/)
(updated 2026-09-21, [P]) teaches agents its filter syntax and
`group_by`, and a community tool server,
[openalex-mcp-server](https://github.com/cyanheads/openalex-mcp-server)
([P]), returns a hit count with every search and offers a group-by tool
of up to 200 groups: counts and facets in an agent's hands, evaluated by
nobody (report 2). Beside it, PaperQA2's working-set counts, Undermind's
discovery curve, Consensus's filters.

**Elsewhere:** agents writing ES|QL with aggregations in Elastic's Agent
Builder ([S]); LLM-generated facets for human shoppers, GenFacet, SIGIR
2026 ([S]); AGGBench ([arXiv, Feb 2026](https://arxiv.org/abs/2602.01355),
[S]), "find all" aggregation queries where a disambiguate–filter–aggregate
agent beats retrieval-augmented generation; WideSearch
([arXiv, Aug 2025](https://arxiv.org/abs/2508.07999), [P]), set-collection
tasks with only search and read tools, best agent 5.1 % success.

**Not found:** an agent with the full searcher's toolkit (counts per
clause, facets, samples, suggested cuts); an evaluated agent with facet or
count tools; an LLM combined with classical subgroup discovery.

### 12.2 LLMs writing boolean queries

An active line of research, led by the group behind searchrefiner (§11.3).
None of it lets the model iterate on hit counts (report 2):

- **Wang, Scells, Koopman, Zuccon**, "Can ChatGPT Write a Good Boolean
  Query for Systematic Review Literature Search?"
  ([SIGIR 2023](https://arxiv.org/abs/2302.03495), [P]). The model gets
  the review's title and, for refinement, the existing query; it sees no
  results and no counts. On CLEF TAR, from scratch: precision 0.075–0.117
  against the experts' 0.021, recall 0.12–0.50 against 0.83. Recall
  varied by 12 % of its mean across ten runs (report 3).
- **Staudinger et al.**, a replication
  ([SIGIR-AP 2024](https://arxiv.org/abs/2411.14914), [S]): recall 0.139
  where the original reported 0.504 (CLEF TAR, one prompt).
- **Wang et al., a reassessment**
  ([SIGIR 2025](https://arxiv.org/abs/2505.07155), [P]): ten models, no
  feedback loop. On CLEF TAR, o1 reached precision 0.108 and recall
  0.655 against the experts' 0.021 and 0.844; on the Seed collection,
  seed-guided o1 reached recall 0.736 against the experts' 0.724, at very
  low precision. Most of the replication gap came from malformed queries
  nobody validated.
- **AutoBool** ([EACL 2026](https://arxiv.org/abs/2602.00005), [P]): a
  4-billion-parameter model trained by reinforcement, rewarded by running
  its queries on PubMed during training; one query at inference. Recall
  0.70, against GPT-4o's 0.44 and o3's 0.69; it claims "10–16× fewer
  documents" than expert queries, the body 17× on CLEF.
- **DeepRetrieval** ([arXiv, Mar 2025](https://arxiv.org/abs/2503.00223),
  [P]): reinforcement against live PubMed and ClinicalTrials.gov,
  rewarded on recall at 3,000; one turn. Publication recall 65.1 %.
- **TrialMind** ([npj Digital Medicine, Aug 2025](https://pmc.ncbi.nlm.nih.gov/articles/PMC12331930/),
  [P]): the only one found where the model sees results; the first
  query's abstracts feed one refinement pass. Recall 0.78 against a weak
  hand-built baseline of 0.19.
- **LITERAS** ([Computers in Biology and Medicine, 2025](https://pubmed.ncbi.nlm.nih.gov/40383055/),
  [S]): a critic agent reruns PubMed searches until enough papers pass,
  2.2 loops on average; it counts relevant papers, not hits.
- Before models: automatic formulation from a review's title
  ([Scells et al., WWW 2020](https://ielab.io/publications/pdfs/scells2020conceptual.pdf),
  [P]) matched a simplified expert query at best (recall about 0.49–0.59
  against the full query's 0.91), and its authors expect "only marginal
  gains" (report 3).

**Agents with a boolean engine, new in 2026:**

- **Sieve** (Wang, Zuccon et al., [arXiv, Aug 2026](https://arxiv.org/abs/2608.02751);
  [P] in report 3, [A] in report 2): the agent writes fielded boolean
  filters, then ranks with BM25 and dense retrieval and fetches only
  selected sections. Accuracy +0.7 to +3.1 points with 20.7–50.6 % fewer
  tokens, on BrowseComp-Plus's structured variant, HotpotQA and MuSiQue;
  boolean filtering helped all nine ranker and collection pairs. The
  agent sees result cards; no hit counts or facets were found in it.
- **Clarke and Smucker**, "Boolean queries are all you need?"
  ([arXiv, Jul 2026](https://arxiv.org/abs/2607.11362); [P] in report 2,
  [A] in report 3): a GPT-5.5 agent with only a boolean substring engine,
  one ranked snippet at a time and no hit counts; nDCG@10 0.686 on TREC
  RAG 2024, at 100 calls per topic. Exploratory, with possible
  contamination from training data, in the authors' words.

### 12.3 What the benchmarks measure

**Systematic-review collections**, whose answer keys are the studies each
review included, judged by its authors: CLEF eHealth TAR 2017–2019, the
Seed collection ([SIGIR 2022](https://arxiv.org/abs/2204.03096)), CSMeD
(325 reviews from nine collections, NeurIPS 2023 datasets track, [A]);
[retrieval benchmarks](retrieval-benchmarks.md) describes them. On these,
refinement and model-written queries raise precision and lose recall
against expert queries, every time (§11.3, §12.2). Expert queries sit
near 2 % precision on purpose: recall comes first, and screening does
the rest. No refinement result on CSMeD itself was found.

**Clustering:** in Scatter/Gather's evaluation
([Hearst and Pedersen, SIGIR 1996](https://people.ischool.berkeley.edu/~hearst/papers/sg-sigir96/sigir96.html),
[P]) the top cluster almost always held at least half the relevant
documents retrieved, and users picked the best cluster in 31 of 38
cases. Citation clusters on 25 systematic reviews
([Bascur et al., 2022](https://arxiv.org/abs/2207.03299), [A]) were
"highly variable", useful only when recall is weighted 2–8× over
precision, and complemented boolean search. No modern Scatter/Gather-style
result was found.

**Literature-search benchmarks mostly score the top few or an answer:**
LitSearch ([EMNLP 2024](https://arxiv.org/abs/2407.18940), [A/S]; R@5
74.8 for a dense retriever, 24.8 points above BM25); LitQA2 (PaperQA2
85.2 % precision, 66.0 % accuracy, [S]); PaSa's RealScholarQuery (50
queries; recall 0.58 at 20 and 0.66 at 50, precision 0.52, [P] in report
3; report 2 gives recall 0.61); SPARBench
([arXiv, Jul 2025](https://arxiv.org/abs/2507.15245), [P]; F1 0.30).

**Exhaustive recall, where set tools should matter, is young and hard:**

- **ScholarQuest** ([arXiv, Jun 2026](https://arxiv.org/abs/2606.20235),
  [P]): 1,111 queries scored by recall of everything. Best agent 0.355;
  dense retrieval alone 0.290; a hybrid of BM25 and dense 0.244, so the
  hybrid did *not* beat dense alone. Its answer keys were built with
  retrieval, citation expansion and model adjudication (86 % strict
  precision on audit), so recall is relative to that pool, and the key is
  not independent of systems like the ones scored
  ([evaluation plan](../evaluation-plan.md) §3.4).
- **AutoResearchBench** ([arXiv, Apr 2026](https://arxiv.org/abs/2604.25256),
  [A]): best 9.39 % on its deep task, 9.31 % set overlap on its wide one.
- **WideSearch**: 5.1 % (§12.1).

**Facets, counts, samples of kept and dropped, suggested cuts: no
benchmark result at all**, and no one has tried set tools on the
exhaustive benchmarks.

### 12.4 Where the money goes, and a cheaper route

**Reading is the cost** (report 1: "the reading loop is where the
frontier spends", retrieval frontier; and
[candidate management](candidate-management.md) §4). Per query, where
stated (report 3): OpenScholar with its own 8B model $0.003, with GPT-4o
$0.05, PaperQA2 $0.3–2.3 ([OpenScholar](https://arxiv.org/abs/2411.14199),
[P]); frontier models on BrowseComp-Plus
([ACL 2026](https://arxiv.org/abs/2508.06600), [P]) about $0.43–0.48 for
GPT-5 and $2.2–2.5 for Opus 4, totals divided by queries. Anthropic's
multi-agent research uses about 15× a chat's tokens, and tokens explained
80 % of the variance in performance (retrieval frontier, report 1).

**Evidence for a cheaper route, from nearby work:**

- *Small models.* OpenScholar-8B is roughly 100–700× cheaper than
  PaperQA2 and more often correct ([P]); PaSa's 7B selector beats its
  GPT-4o version by 30 % in recall ([P]); RankZephyr, 7B, matches or
  beats GPT-4 at listwise reranking
  ([arXiv](https://arxiv.org/abs/2312.02724), [A]).
- *Cascades.* BARGAIN ([SIGMOD 2026](https://arxiv.org/abs/2509.02896),
  [A]): cheap-model cascades with guarantees on precision or recall. A
  screening cascade sent only 12.96 % of records up to GPT-4.1
  ([arXiv](https://arxiv.org/abs/2512.11261), [S]). LOTUS (VLDB 2025,
  [S]) and TnT-LLM (KDD 2024, [S]) train cheap models on a strong
  model's labels and run them over whole sets: §10's funnel, published.
- *Budget-aware agents.* Google's BATS
  ([COLM 2026](https://arxiv.org/abs/2511.17006), [S]): same accuracy
  with 31.3 % lower cost.
- *Boolean filtering.* Sieve's 20.7–50.6 % fewer tokens (§12.2).
- *A strong model with active learning, in screening.* Bron et al.
  ([IAL 2024](https://ceur-ws.org/Vol-3770/paper8.pdf), [P]), one
  dataset of 4,836 records: work saved at 95 % recall 80.5 % against
  71.4 % for the standard method (AutoTAR). LLM scores as seed labels for
  active learning (Akinseloyin et al., medRxiv 2025, [S]): 42–43 % of the
  work saved at near-perfect recall over 28 reviews. Zero-shot screening
  calibrated to a target recall
  ([ECIR 2024](https://arxiv.org/abs/2401.06320), [P] in report 3):
  recall 0.93–0.97 and 50–54 % of the work saved, but only half the
  topics met the 95 % target.
- *In this repository:* the tracer-engine note's cascade, which "costs
  close to all-small and scores close to all-capable"
  ([future-ambitions-tracer-engine](future-ambitions-tracer-engine.md));
  the one-call front door, "expand, filter, score, keep the top *k*"
  ([future ambitions](future-ambitions.md)); and HarnessTax, where cost
  varied up to 5× across agent harnesses while success barely moved
  ([configuration](future-ambitions-configuration.md)), all per report 1.

So the order-of-magnitude savings on record come from small models and
cascades, and boolean filtering adds tens of percent. Replacing reading
with counting, the dramatic version, is untested.

**The risks are all on recall.** Recall is what set tools are for, and
model-written queries fall short on it (§12.2). A count cannot show what
was dropped (§6). None of five papers claiming screening savings priced
the studies they missed (LLM4SCREENLIT, [S]). Two identical GPT-5.4
screening runs disagreed on 94 records, 29 of them eligible
([arXiv, Aug 2026](https://arxiv.org/abs/2608.26885), [A]). And the
replication gap in §12.2 came from unvalidated output.

### 12.5 The testable claim

Stated so that it could fail:

> An agent given set tools (counts per clause, facets, samples of what a
> step kept and dropped, a cheap classifier trained on its marks) reaches
> the same exhaustive recall as an agent that reads its candidates, for a
> fraction of the model calls.

**Why it is plausible.** If reading is both the quality and the cost
(§12.4), the question is how to choose what to read without losing
recall, and choosing cheaply is what counts, facets, samples and cheap
classifiers do. An agent may be a better customer for them than a person:
it knows the vocabulary (synonyms, MeSH) and never tires of reading count
after count; what it lacks is a sense of what is missing, which is what a
sample of dropped works supplies (§6).

**How it could be tested here:**

- *Control:* an agent with top-*k* search and reading, as in §12.1.
  *Treatment:* the same agent, the same model and the same budget rules,
  with the set tools added. One change between them.
- *Answer key:* the systematic-review collections (§12.3): included
  studies judged by the reviews' authors, independent of both agents,
  over PubMed, which this index holds; and, since biomedicine is no
  reason to stop there (the user: "pubmed was just a convenient place to
  start"), SR4CS's 1,212 computer-science reviews and SYNERGY's reviews
  in psychology, biology, chemistry and mathematics as the corpus reaches
  their fields ([retrieval benchmarks](retrieval-benchmarks.md)).
- *Measures:* recall of the included studies, with precision beside it;
  and cost as model calls, tokens and dollars. The result is a
  cost–recall curve per arm, as HarnessTax drew cost against success.
- *Rigour:* paired intervals per review and a frozen test half
  ([evaluation plan](../evaluation-plan.md) §3.4 and §4).
- *The key's own bias:* included studies were found by the reviews' own
  boolean queries, so a treatment that finds relevant studies a review
  missed is scored wrong. Scells 2019's bounds for unjudged citations
  (§11.3) show how large that can be; read judged fractions and the
  unjudged bounds before any verdict.
- *What would refute it:* at matched cost, recall below the control's,
  beyond the intervals.

CLEF TAR and its kin are in the benchmarks note but not among the
evaluation plan's published sets (E3); adding one would come first.

Asked which one tool looks most promising for researchers, agents and
benchmarks alike, the conversation answered with a note of its own:
[the recall-accounted set builder](recall-accounted-set-builder.md).

## 13. The same building block as the collection idea

The user's other idea of the same day, to upload a collection (a Zotero
library, an EndNote file, a search export) and situate it in the graphs
of citation, authorship, funding, institutions and topics, wants the same
building block: a set of works to hold and ask questions of. A result
set, a step's slice of it, an uploaded library and the works citing X are
all sets. Facets, probes, samples, distinctive words, shape, and "where
does this sit in the graph" are operations on any of them. One of those
joins the two ideas: "the works this set cites most" is a facet that
splits a set by the founding papers each part descends from. If both
ideas survive, the set is the part to design once.

## 14. Costs, and what is open

Costs, as the conversation saw them:

- **Recompute each time, or hold the set.** Recomputed: the stack of
  steps is the query, run again on every request; nothing is kept per
  person, and a step can be reproduced from its query and the epoch.
  Held: the base set computed once and kept as one bit per work, probes
  checked against it; quicker for many probes of a broad set, at a cost
  in memory per session and item 36's open question of keeping "a little
  state per client". The conversation's recommendation: recompute first,
  and hold sets only if that measures too slow.
- **Counting every match** is already a loose end of item 37, and this
  leans on it harder: several probes over a broad set are several counts.
- **Vectors for the whole corpus** would be the largest new cost, on a
  box shared with the search engine. Semantic Scholar publishes SPECTER2
  vectors for 1.2 × 10^8 records ([the retrieval
  frontier](retrieval-frontier.md) §4, P), which could cover part of it,
  with SPECTER's idea of similar (§9).
- **Works without abstracts** get a vector from the title alone, so a
  shape leans toward works that have abstracts.

Open:

1. What "in the results" means: every word, the query language, or
   something else (§5).
2. Whose "similar": which vectors, from which model, trained on what
   (§9).
3. Where classification runs: on the box (GLiNER2 makes this a
   candidate), or a hosted service for the few (§10).
4. Recompute or hold (above).
5. Whether custom facets are saved, named and reused, and whether they
   are shared between people, given the user's thesis that a label means
   something only to its maker.
6. How to know it helps. The published sets score ranked lists, not
   sessions of narrowing, so they cannot say whether steps help; the
   per-step estimates from the user's own marks are the user's, with
   their error bars. A measurement is its own question.
7. Naming clusters: distinctive words, a model reading a few members, or
   no name and a few example works (§8).
8. Extracted entities, pre-computed or not: running GLiNER2 over the
   corpus once for a fixed schema (organisms, methods) would be
   pre-computing labels, which the user's thesis argues against; but
   mentions found in the text sit nearer the text than a category does.
   Where that line falls is the user's.
9. The score for suggested cuts (§11.1): an even split, difference from
   the parent, closeness in vector space, or agreement with the user's
   marks; and whether suggested cuts come before any marks exist.
10. The testable claim (§12.5): whether to test it, on which
    collection, with what control agent and budget; and adding a
    systematic-review collection to the evaluation plan first.
11. The rest of the circling: the user called this an incomplete thought.

Prior art to read (U, from memory): Scatter/Gather (1992), and Hearst and
Pedersen, "Reexamining the cluster hypothesis: scatter/gather on
retrieval results" (SIGIR 1996); Hearst's Flamenco project and *Search
User Interfaces* (2009) on faceted search; Shneiderman's dynamic queries
and query previews (counts before results); Solr's `facet.query`, and
Elasticsearch's `filters` and `significant_terms` aggregations; Carrot2's
results clustering; BERTopic; subgroup discovery and CN2's beam search
over rules (§11).
