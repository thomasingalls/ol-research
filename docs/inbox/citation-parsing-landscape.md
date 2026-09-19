# Parsing and matching reference strings

**Status:** notes, 2026-09-19 — one web survey made the same day
(searches and page reads, plus four corpora downloaded and counted
locally; nothing registered, no key sent anywhere), recorded at the
user's request. Speculative, not a plan: what the published work says
about turning a plain-text citation into structured fields, and about
landing such a string on a known work. Context for the ~1.15 M
references in the corpus that carry citation text and no usable
identifier. Companion to the [retrieval frontier](retrieval-frontier.md)
and [retrieval benchmarks](retrieval-benchmarks.md).

Every figure is marked by how it was checked:

- **P** — primary: the paper, repository file, model card or official docs.
- **S** — secondary only: somebody else's account of it.
- **U** — could not verify; §12 lists these.
- **M** — measured here: computed directly from a downloaded corpus or
  repository file, because the figure is published nowhere.

Three conventions worth stating once, because the literature routinely
breaks them. **Field-level F1 on one corpus is not comparable to
instance-level accuracy on another.** **A parser's isolated score (gold
reference strings handed in) is not its end-to-end score (PDF in).** And
**a matcher's precision measured only over the links it accepted says
nothing about the ones it declined.** Nearly every headline number in
this area is the flattering member of one of those pairs.

The one section to read if only one: **§3, title-less references.** It
reframes the problem.

## 1. The two named systems

The user named two systems by ear. Both are real; neither is what the
name suggests.

### 1.1 needle3 — Cactus Compute **Needle 3**

[site](https://cactuscompute.com/needle) ·
[repository](https://github.com/cactus-compute/needle) ·
[weights](https://huggingface.co/Cactus-Compute/needle3) · **Apache-2.0**,
code and weights (P).

29–121 M parameters shipping as an **8–29 MB binary** in Cactus's CQ2
2-bit format; a "Laddered Simple Attention Network" in which every depth
from 2 to 20 layers is independently deployable (P). Structured
extraction is grammar-constrained — "a byte-level grammar compiled from
your schemas constrains every token" — and every response carries a
**calibrated confidence score from a learned head** (P). Throughput on a
Raspberry Pi 5: **400–4 k tok/s decode, 1–10 k tok/s prefill** (P).
Trained on 360 B proprietary tokens; training data and code are closed,
and fine-tuning runs through Cactus's commercial platform (P). Benchmarks
named: DroidCall, BFCL v4, DSTC8, SNIPS; the extraction metric is **field
micro-F1** (P). The actual extraction numbers exist only inside an SVG
plot on the site and model card, not as text (**U**).

This is an on-device model and the box is not a device. Judged on
accuracy per unit of local GPU time it is unlikely to beat a 1–2 B model
run here. The two transferable ideas are the **byte-level grammar
compiled from the schema** and the **calibrated per-response confidence
head** — available respectively from vLLM + XGrammar (§10) and from a
temperature-scaled discriminative scorer (§4, rung 5).

### 1.2 jev — TypeSafe AI **Jev**, their first "System One" model

[announcement](https://typesafe.ai/blog/introducing-system-one-models-and-jev) ·
[docs](https://docs.typesafe.ai/introduction) · also served on Cloudflare
Workers AI (P).

Three primitives: **Noul** (is this statement true → probability 0–1),
**Choice** (pick one of N options → per-option probability plus
confidence), **Score** (rate against ordered levels → continuous score,
distribution, confidence). Questions in one request are evaluated in
parallel and independently (P). Trained with "Reinforcement Learning for
Calibrated Decisions". Latency **70–500 ms** end to end; pricing
**$0.042/MTok input, output free** (P). API-only, early access, **no
downloadable weights, licence not disclosed, parameter count not
disclosed** (**U**).

**Jev cannot parse.** It returns only values declared in advance, so it
has no way to emit a span. What it is natively shaped for is the
*verification* rung: given a reference string and N candidate works,
which one, with a calibrated probability. That is precisely Crossref's
SBMV validation step (§6.1) expressed as a model. Two cautions: 1.15 M
strings × ~20 candidates is a large amount of input token spend through a
third party, and it means shipping the corpus off the box. Also, search
results for "jev" are dominated by a swarm of near-identical
"awesome-jev" repositories from unrelated accounts; treat that ecosystem
as marketing. The two links above are the primary sources.

## 2. What the field looks like

There is now a dedicated venue: the **Workshop on Citation Extraction and
Parsing (CiteX)**, Frankfurt, 28–29 May 2026, organised by DIPF and
GESIS, proceedings CC BY 4.0
([doi:10.5281/zenodo.20382199](https://zenodo.org/records/20382199), P).
Two 2026 benchmarks — RenoBench and the SSH benchmark (§7) — reach the
same architectural conclusion independently, and it is not the one the
tooling assumes.

## 3. Title-less references

**The single most important finding in this survey.** A large minority of
real reference strings carry no article title at all — author, journal,
volume, page, year and nothing else. The parsing benchmarks are almost
free of them, nobody reports parsing accuracy conditioned on title
presence, and the one group that measured the *matching* side found the
damage is severe and concentrated at exactly the place a pipeline makes
its accept/reject decision.

### 3.1 How common

Crossref's own characterisation of its own corpus, from the structured-
references post
([2019-07-08](https://www.crossref.org/blog/what-if-i-told-you-that-bibliographic-references-can-be-structured/),
P):

> **76 % of structured references without DOIs do not contain the article
> title**, and 15 % have fewer than four metadata fields.

Title absence is therefore not a physics edge case. In the identifier-
less population — which is exactly the ~1.15 M — it is the **majority
condition**.

Measured rates across the labelled corpora (a "title-less journal
reference" has a container title but no article title; books and reports
excluded, since they legitimately have none):

| corpus | labelled strings | title-less | |
|---|---|---|---|
| CORA `cora-ie` | 500 | **0 (0.0 %)** | M |
| CORA `cora-refs` | 1,879 | 8 (0.4 %) — all 8 variants of one work | M |
| AnyStyle `core.xml` (ships with the default model) | 1,514 | 32 (2.1 %) | M |
| AnyStyle `ugly.xml` | 3,847 | 381 (9.9 %) | M |
| **GROBID citation training corpus** | 8,301 | **1,921 (23.1 %)** | M |
| **GROBID held-out citation eval set** | 327 | **143 (43.7 %)** | M |
| CSL independent styles never rendering a journal title | 2,770 | 50 (1.8 %) | M |

GROBID's 23 % is not uniform. Its general corpus is ~5 %; the headline
comes from physics and chemistry files someone added deliberately — one
MNRAS file is 100 % title-less, a PhysRevLett file 93 %, a Canadian
Journal of Chemistry file 95.2 % (M). **GROBID's maintainers are the only
ones who visibly treated this as a case worth covering.**

### 3.2 What it costs a matcher

Crossref generated 7,498 strings per style across 11 styles and released
both the datasets and the raw match results
([repository](https://github.com/CrossRef/reference-matching-evaluation),
MIT). The per-style numbers are published only as a figure, so they were
recomputed string-level from the released results (M):

| style | top-1 accuracy | accuracy at threshold | precision |
|---|---|---|---|
| `apa` | 97.6 % | 95.3 % | 98.1 % |
| `chicago-author-date` | 97.5 % | 95.4 % | 97.9 % |
| title words **scrambled** | 97.6 % | 95.2 % | 98.1 % |
| `american-chemical-society` | 85.5 % | **56.1 %** | 92.7 % |
| `elsevier-without-titles` | 82.2 % | **45.8 %** | 90.0 % |
| `american-institute-of-physics` | 82.0 % | **45.3 %** | 90.9 % |
| `american-institute-of-physics` **+ noise** | 66.9 % | **22.8 %** | 89.6 % |

Crossref's own notebook says it plainly: "SBM with a simple threshold is
worse than STQ for styles that do not include the title in the reference
string (acs, aip, ewt)" (P).

Three consequences, and the second is not obvious:

1. **Title absence costs ~15 points of top-1 accuracy and ~50 points once
   a relevance threshold is applied.** The threshold is the killer. A
   single global confidence cut-off tuned on title-bearing references
   silently destroys half of physics and chemistry recall while looking
   healthy in aggregate.
2. **Word order in the title is irrelevant; title *presence* is
   everything.** Scrambled titles still score 97.6 %, identical to clean
   APA. The title functions as a bag of high-entropy tokens, and nothing
   else in the string carries comparable signal.
3. **Noise compounds multiplicatively with titlelessness.** AIP alone
   82.0 %, AIP plus typos 66.9 %; APA plus the same noise only drops 97.6
   → 95.2. The title is also the error-correction budget. (This is the
   measurement behind the degradation model in §14.2.)

### 3.3 Somebody else hit this wall and wrote it in a footnote

S2ORC, Lo et al., ACL 2020
([paper](https://aclanthology.org/2020.acl-main.447/), P), footnote to
Table 4:

> "The lower number of linked bibliography entries in LaTeX parses is due
> to large numbers of papers (mostly in the field of physics) for which
> the bibliography entries are formatted without paper titles. Our
> linking algorithm strongly depends on titles and fails to link these
> entries."

The damage is in their Table 4: PDF parses average 27.6 bibliography
entries with **19.3 linked (69.9 %)**; LaTeX parses, physics-heavy,
average 21.9 entries with **6.8 linked (31.1 %)** (P). Their Table 6
reports linking precision 1.00 on title and 0.96 on authors — but
measured **only over accepted links**, so it says nothing at all about
the entries that never linked. Nobody has reported recall on that
population.

### 3.4 Nobody has measured the parsing side

Checked and absent: the GIANT paper, Grennan & Beel 2020, RenoBench, the
SSH benchmark, FOSSIL, and GROBID's own benchmark pages. **None reports
parsing accuracy conditioned on title presence** (M, by search over the
extracted text). GROBID reports a per-field `title` F1 and never splits
by whether a title exists — and its own held-out eval set is 43.7 %
title-less, so its published citation numbers are already partly
measuring this case without saying so.

## 4. The cascade

The architecture under discussion, endorsed by the user. Seven rungs,
each cheaper and more certain than the one below it; a reference stops at
the first rung that can answer it.

1. **Deterministic** — a DOI, PMID, arXiv id or ISBN present in the
   string. Regex, no scoring. Crossref's `reference-parsed-doi` and
   `reference-parsed-isbn` strategies are exactly this rung (P).
2. **Structural** — the reference arrived as structured fields rather
   than as a string, so no parsing is needed before retrieval.
3. **Parse** — fields out of the string, for use as *features*, not as a
   precondition for retrieval.
4. **Retrieve** — candidates from the index using the **raw string**,
   BM25 over the Tantivy sink.
5. **Verify** — re-score candidates locally on corroborating fields, with
   a calibrated score and a support count.
6. **Escalate** — the residue that verification cannot settle goes to a
   larger model.
7. **Abstain** — emit no edge. A reference that reaches this rung stays
   unmatched, and that is a correct outcome, not a failure.

**Rungs 3 and 4 are in that order deliberately, and the evidence is
Crossref's own A/B on the same 2,000 strings** (§6.1): parse-then-match
F1 **0.9251**, search-then-validate F1 **0.9629** (P). Parsing accuracy
stops gating recall once retrieval works on the raw string. The
title-less finding of §3 makes the same argument from the other side: a
retrieval step that never needs a parsed title cannot be broken by a
style that omits one. Crossref's own canonical README example is,
tellingly, a title-less physics reference —
`Jen CK, Foner SN, Cochran EL, Bowers VA (1958) Phys Rev 112:1169` (P).

Rung 5 is where the precision lives, and it is cheap: pure local scoring
over candidates already fetched. Crossref's measured gain from adding it
is precision **0.7712 → 0.9809** for −0.0335 recall (P).

Rung 7 is the rung the literature does not serve (§12, §16).

## 5. The established parsers

| tool | algorithm | licence | status | |
|---|---|---|---|---|
| **GROBID** | CRF (Wapiti) default; optional BiLSTM-CRF / SciBERT via DeLFT | **Apache-2.0** | **actively maintained**, v0.9.1 (2026-08-04) | P |
| **AnyStyle** | CRF via Wapiti, Ruby | **BSD-2-Clause** | maintained, slow cadence, v1.6.0 (2025-05-11) | P |
| **ParsCit** | linear-chain CRF, Perl | **LGPL-3.0** | **dead**, last push 2020-05-06 | P |
| **Neural ParsCit** | BiLSTM-CRF + char embeddings | **none — `NOASSERTION`, no LICENSE file** | **dead**, 2022-05-27 | P |
| **CERMINE** | SVM (zones) + CRF (references) | **AGPL-3.0** | dormant, 2022-06-30 | P |
| **refextract** | **pure regex + knowledge base, no ML** | **GPL-2.0-or-later** | actively maintained, 2026-04-16 | P |
| **science-parse** | CRF (v1); BiLSTM (spv2) | Apache-2.0 (v1); **none** (spv2) | **abandoned**, spv2 last push 2019-11-20 | P |

Two licences are hazards: **CERMINE is AGPL-3.0**, whose network clause
reaches a hosted service, and **Neural-ParsCit has no licence at all**,
which makes it unusable regardless of quality.

**GROBID's real numbers**
([PMC benchmark](https://github.com/kermitt2/grobid/blob/master/doc/benchmarks/Benchmarking-pmc.md),
P), `PMC_sample_1943`, 90,125 expected citation instances, v0.9.0:

| matching | field micro-F1 | title F1 | `inTitle` F1 | **instance-level F1** |
|---|---|---|---|---|
| strict | 83.13 | 76.85 | **71.65** | **44.03** |
| Levenshtein 0.8 | 87.63 | 90.29 | 84.33 | **63.20** |

The instance-level row matters more than the field row for a matching
pipeline: **under half of references are parsed entirely correctly end to
end on clean English PMC PDFs**, 63 % under a fuzzy match. The README's
".90 instance-level / .95 field-level" is the **isolated** measurement
with gold strings handed in; the ecosystem quotes that one. And
`inTitle` — the journal name, the field matching depends on most when
there is no title — is the weakest of all eight.

Isolated citation model, 10-fold CV on GROBID's own data: **CRF Wapiti
0.9448, best DL 0.9522** (P). GROBID's `Deep-Learning-models.md` claims
the DL citation model is "+3 to +5 points in F1"; its own
`Benchmarking-models.md` shows **+0.74**. Unreconciled (P, both).

**Throughput — GROBID is the only project in this survey that publishes
any** (P): 0.75 s/PDF (1,943 PDFs in 1,467 s, 16 CPU / 32 threads); CRF
Wapiti **23,685 tok/s single-thread → 100,879 at 8 threads**; BiLSTM-CRF
24,774–30,520 tok/s on GPU; SciBERT+CRF 4,729–5,060 tok/s on GPU. In the
real cascade batches fill with 1–2 sequences, so GROBID's own docs say DL
is "**up to 100 times slower**". Run CRF at corpus scale. Published
throughput for AnyStyle, CERMINE, refextract, science-parse and ParsCit:
**none exists** (U).

**The cross-tool studies disagree, and both are internally valid.**
Tkaczyk et al., JCDL 2018 ([arXiv:1802.01168](https://arxiv.org/abs/1802.01168),
P), proprietary chemistry corpus, 64,495 test references: GROBID **.89** >
CERMINE **.83** > ParsCit **.75** > Science Parse .63 > AnyStyle **.54**;
retrained on 10 k in-domain references .92 / .92 / .87. Cioffi & Peroni
2022 ([arXiv:2205.14677](https://arxiv.org/abs/2205.14677), P), 56 PDFs
across **27 subject areas**: **AnyStyle best** at .77, CERMINE .71,
**GROBID fifth at .54**. The reconciliation: study A was single-domain
with a field mapping that punished AnyStyle's whole-author-block output;
study B scored reference *identification* across 27 disciplines, where
GROBID's segmenter leaves its training distribution. The durable finding
from A is that **ML versus rules is a recall story** — ML averages P 0.77
/ R 0.66, rules P 0.76 / **R 0.22**.

**Where GROBID breaks.** On FOSSIL (law and humanities, footnote-embedded
citations, [arXiv:2606.01109](https://arxiv.org/abs/2606.01109), P)
default GROBID scores micro-F1 **0.36** — precision 0.89–0.94, recall
**0.21–0.27** — and a specialised retrain reaches **0.72**. On RenoBench
(multilingual, 30 % books) its recall is surname 71.6, title 65.2,
**source 51.8, given-name 18.8** — but it is the **only system tested with
100 % structurally valid output** (P).

## 6. How the aggregators do it

Only two of the five publish real matching metrics. Crossref's are from
2018–2019 blog posts and were never peer-reviewed; OpenCitations' are
from a November 2025 preprint. **OpenAlex and MAG have never published a
precision or recall figure for reference matching or work
de-duplication.** Semantic Scholar publishes an accuracy figure that is
not a precision/recall pair. Europe PMC publishes nothing.

### 6.1 Crossref — SBMV, and the thresholds

The production constants, from `MatchRequest.java` in
[search_based_reference_matcher](https://gitlab.com/crossref/search_based_reference_matcher)
(**MIT**), corroborated in the 2018 Python evaluation framework and in the
current Marple strategy (P):

```
DEFAULT_CAND_MIN_SCORE  = 0.4    // candidate gate: search_score / len(reference_string)
DEFAULT_UNSTR_MIN_SCORE = 0.34   // validation similarity, unstructured references
DEFAULT_STR_MIN_SCORE   = 0.76   // validation similarity, structured references
DEFAULT_UNSTR_ROWS      = 20     // candidates fetched, unstructured
DEFAULT_STR_ROWS        = 100    // candidates fetched, structured
```

**Normalising the Solr score by reference-string length is the whole
trick** — raw Lucene scores grow with query length, so an unnormalised
threshold is meaningless across strings of different sizes.

The method: clean the string (strip DOIs, arXiv ids, bracketed content),
one `GET /works?query.bibliographic=…&rows=N`, keep candidates above the
normalised gate, re-score each with a **generalized Jaccard**
`sum(min)/sum(max)` over volume, year, issue, first-page, title,
container-title and author, then apply a **support count** — title fuzzy
ratio > 0.7 (0.75 in current Marple), author > 0.7, year within ±1 counts
0.5, volume exact, first-page exact. **If support < 3 (< 4 for journal
articles in the newer strategy) the similarity is forced to zero** (P).

Unstructured evaluation, 2,000 strings without DOIs, targets assigned by
verification plus manual search
([2018-12-18](https://www.crossref.org/blog/reference-matching-for-real-this-time/),
P):

| approach | P | R | F1 |
|---|---|---|---|
| Legacy (parse-then-match) | 0.9895 | 0.8685 | 0.9251 |
| Search-based, simple threshold | 0.8686 | 0.8191 | 0.8431 |
| Search-based, normalised threshold (**no validation**) | 0.7712 | 0.9121 | 0.8358 |
| **SBMV** (**with validation**) | **0.9809** | **0.9456** | **0.9629** |

Errors: **1.91 % false positives, 5.44 % false negatives**. That table
*is* the strict-versus-fuzzy comparison — same retrieval, same threshold,
validation on or off, precision 0.7712 → 0.9809. Structured references:
Legacy .9929 / .9280 / .9593; SBMV .9831 / .9495 / .9660 (P).

Scale: **over 2 billion citation links, roughly half publisher-asserted
and half from automated matching** — so Crossref's matcher is responsible
for about a billion (P). Provenance is exposed per reference as
`doi-asserted-by: crossref | publisher`.

Code: [marple](https://gitlab.com/crossref/marple) (Python, **MIT**),
`search_based_reference_matcher` (Java, MIT),
[reference-matching-evaluation](https://github.com/CrossRef/reference-matching-evaluation)
(Python, MIT, archived). **There is no repository named
`search-based-ref-matching` or `bibliographic-matching`, and no Rust
implementation** (P, searched). **No peer-reviewed SBMV paper exists** —
the results live only in blog posts, and there is no paper titled
"Artificial Intelligence in Reference Matching" (P, checked against
Tkaczyk's Google Scholar, dblp and arXiv). Her pre-Crossref matching work
is Fedoryszak, Tkaczyk & Bolikowski, TPDL 2013
([arXiv:1303.6906](https://arxiv.org/abs/1303.6906)): blocking by author-
token rotation index, linear SVM, single-link clustering; CORA-ref
pairwise P 95.90 % / R 94.80 % / F1 95.33 %, cluster recall 71.95 %,
scaled to 450 k documents on Hadoop (P).

One caveat that matters: the strategy that actually runs on Crossref
deposits is `reference-cs-combined`, explicitly marked **"no code is
available"** (P). What is open is the reference implementation, not
production. And no precision or recall has ever been published for the
current Marple-era strategies (U).

### 6.2 OpenAlex — read at the code level

The docs claim "a fuzzy matching algorithm built on title normalization
plus author names, tuned over the years for precision and recall". In
[openalex-guts](https://github.com/ourresearch/openalex-guts) (MIT) it is
string equality on a derived key (P). `_try_match_reference`: exact
normalised-title equality, `LIMIT 50`, an integer score with maximum 3
(+1 author-string overlap, +1 candidate not already merged, +1 year
within ±1), then `return work_matches_by_title[scores.index(max(scores))]`.

**There is no minimum score.** A candidate scoring 0 — wrong author,
wrong year, already merged — is returned as the match so long as the
normalised title matched exactly (P).

Scale, from an in-code comment: **3,758,687,070 reference rows,
1,526,343,813 (40.6 %) with `cited_work_id` NULL** (P).
`referenced_works` is `COLLECT_SET(…) WHERE cited_work_id IS NOT NULL`,
and the docs confirm it: "references to unknown works are dropped" (P).
**OpenAlex discards exactly what this project is trying to keep.**

Coverage: ~48–51 % of DOI-bearing journal articles have at least one
reference; only ~2 % of 5 M+ books do (S, Aaron Tay's analysis). Two
third-party evaluations, and OpenAlex publishes none of its own: Culbert
et al., *Scientometrics* 130:2475–2492 (2025),
[arXiv:2401.16359](https://arxiv.org/abs/2401.16359) — on a 16.8 M
publication corpus shared with WoS and Scopus, OpenAlex averages 34.863
source references against 33.416 and 33.363, but 7.572 over its whole
corpus; they explicitly do not check matching accuracy and hypothesise "a
comparatively poorer reference-matching algorithm" (P). Alonso-Álvarez &
van Eck, [arXiv:2409.01120](https://arxiv.org/abs/2409.01120) — manual
audit, n = 30 per subset: **44 % of "missing" references are actually
indexed in OpenAlex**, i.e. recall loss rather than coverage loss (P,
small sample).

**MAG's method was never published, and the citation trail is circular.**
Sinha et al., WWW'15, describes de-duplication as exact normalised title
plus exact venue and describes **no reference matching at all**; its
"above 95 % accuracy" is a blanket graph-level claim against unpublished
internal Microsoft test sets (P). Wang et al., QSS 1(1):396–413 (2020),
says the conflation method "is described in detail in Wang et al. (2019)"
— and that paper does not contain it (P). The only published MAG
threshold is for authors: 97 % confidence, with an admission it "leads to
author underconflation" (P).

### 6.3 Semantic Scholar

S2ORC's linking is title-only: character 3-grams, harmonic mean of
Jaccard and containment, `S = 2JC/(J+C)`, **accept the highest above
0.8**, else leave unlinked (P). Its Table 6 figures (1.00 title, 0.96
authors) are over **accepted links only**, n = 500 GROBID parses plus 100
LaTeX, one entry each — precision-like, with no recall companion.
380.5 M citation links; dataset **ODC-By 1.0** (P).

S2AG (Kinney et al., [arXiv:2301.10140](https://arxiv.org/abs/2301.10140))
says the whole of it verbatim: "We are using fuzzy text-matching
heuristics on title and authors for citation linking, but anticipate that
the S2APLER model can eventually be adapted to this problem." **No
metrics are published for citation linking** (U). 225 M papers, 2.8 B
citation edges (P).

S2APLER does have numbers, but it is **paper de-duplication, not
reference matching** — do not transfer them: blocking on title, pairwise
XGBoost, agglomerative clustering; at epsilon 0.7, **B³ F1 0.9986**
against the previous S2 algorithm's 0.9971; code Apache-2.0, data ODC-BY
(P).

### 6.4 OpenCitations

COCI does **no matching at all** — it is a projection of Crossref's
`reference` array keeping only entries that explicitly specify a DOI, so
**references without DOIs are silently dropped** (P). Its quality ceiling
*is* Crossref's matcher; do not attribute matching metrics to it. The
consolidated Index holds 2.01 B links across 91.4 M resources (July 2024)
and > 2.2 B by November 2025, deduplicated through OMID; code ISC, data
CC0 (P).

The matching work is new:
[ref-matcher](https://github.com/opencitations/ref-matcher), **ISC**,
Guenci, Heibi, Parravicini, Peroni & Soricetti,
[arXiv:2511.18408](https://arxiv.org/abs/2511.18408) (P). Motivation:
**698,198,538 of 1,812,811,997** references in the April 2025 Crossref
dump — **38.51 %** — are discarded at ingestion. Method: six cascading
SPARQL queries against OpenCitations Meta with early stop; a weighted
score out of **48** (DOI exact 15; title 14/13/13/12/11/10 at
100/95/90/85/80/75 % similarity and 0 below; author exact 7; page 8;
volume 3; year exact 1); GROBID 0.8.2 fallback for unstructured strings.
**Acceptance at 26/48 (54.5 %), with adaptive relaxation to an effective
floor of 23/48.**

| | Gold Standard (112 TEI files) | Crossref subset, no DOI queries | with DOI |
|---|---|---|---|
| precision | 100 % | 100 % | 100 % |
| recall | 75.67 % | 58.31 % | 69.45 % |
| F1 | 86.15 % | 73.67 % | 81.97 % |

Three caveats, and the second is serious. Their own §5.1 prose says recall
79.01 % and F1 88.26 %, but Table 2's own counts give 75.67 % / 86.15 % —
**cite the table**. The **100 % precision is an evaluation artifact**:
candidates come from SPARQL against Meta, and a false positive is defined
as a predicted DOI *not in Meta* (the negative set had six members), so
matching a reference to the **wrong** work that happens to be in Meta
scores as a true positive. And the scale is 100 files / ~3,555
references — a prototype; the "404 million additional citation links" is
an extrapolation from 58 %, not a run.

### 6.5 Europe PMC

**Nothing is published.** The entire documented methodology is one
sentence: citations are "matched to Europe PMC records based on available
metadata… if a citation can be confidently identified, it will appear as
a publication link", and must resolve to a PMID (P). No metrics, no
thresholds, no matching code — checked against the NAR database issues of
2015, 2017, 2020 and 2023, Rosonovski et al. *Current Protocols* 2023,
the help pages, the blog, the tech blog, all eleven GitHub repositories
and the GitLab group (U). 316,930,434 citation links as of February 2018
(P).

A trap for anyone writing this up: Kafkas et al., PLOS ONE, reports
P 100 % / R 95.24 % on 150 documents — that is **database
accession-number text mining, not bibliographic reference matching**.
Secondary sources conflate the two.

### 6.6 Two comparators worth having

**NIH-OCC / iCite** (Hutchins et al., *PLOS Biology* 2019,
[doi:10.1371/journal.pbio.3000385](https://doi.org/10.1371/journal.pbio.3000385),
data **CC0**) is architecturally SBMV's closest sibling with better-
reported numbers: tokenize, retrieve by highest percentage of matched
terms, score candidates on title, authors and journal, plus an LSTM
separating reference text from body text in PDFs and a Random Forest
false-positive filter. **Precision 0.98 and recall 0.98** when trained on
papers from the same journal; on heterogeneous Unpaywall-sourced papers
**recall drops to 0.78–0.89, precision constant** (P). Their admission is
worth stealing for §16: "manual inspection of our false positives
indicated that **over 90 % of these were actually false negatives in the
gold standards**."

**EXmatcher** (Ghavimi, Otto & Mayr 2019,
[arXiv:1906.04484](https://arxiv.org/abs/1906.04484),
[code](https://github.com/exciteproject)) is the clearest published
blueprint for rungs 4–5: Solr tf/idf blocking, top-5 per reference (1–39
retrieved, average 14 candidates), then features — Levenshtein and
Jaccard on segments, longest common substring on the raw string,
source-abbreviation detection — **plus the parser's own segmentation
probability for the first author as a weighted feature**, into an SVM.
**P 0.972 / R 0.926 / F1 0.948** on 816 gold references against Sowiport
(9 M records); **R 0.913 / F1 0.941** pipeline-level including blocking
failures (P). Small German social-science gold set; do not
over-generalise. The design idea worth keeping is feeding **parser
confidence** into the matcher as a feature rather than treating the parse
as settled.

Dataset-level completeness, for orientation only — this measures
**datasets, not algorithms** (Liang, Mao, Lu & Li,
[arXiv:2111.00172](https://arxiv.org/pdf/2111.00172), against the PubMed
2020 Baseline, 5,512,064 publications, pre-OpenAlex):

| | COCI | COCI.Upd | Dimensions | MAG | NIH-OCC | S2ORC |
|---|---|---|---|---|---|---|
| coverage | 65.57 % | 72.29 % | 99.98 % | 87.36 % | 100.00 % | 63.03 % |
| precision | 99.87 % | 99.87 % | 99.60 % | 97.87 % | 99.90 % | 97.66 % |
| recall | 84.69 % | 85.18 % | 98.80 % | 90.80 % | 98.99 % | 79.00 % |
| F1 | 90.59 % | 90.95 % | 99.07 % | 93.37 % | 99.34 % | 86.27 % |

## 7. Benchmarks and labelled corpora

| dataset | size | licence | |
|---|---|---|---|
| CORA `cora-ie` | **exactly 500** labelled strings, 13 tags | **none stated anywhere** | M, P |
| CORA `cora-refs` (matching) | 1,879 strings in 822 clusters | none stated | M |
| **GROBID citation corpus** | **8,301** labelled `<bibl>` | Apache-2.0 | M |
| GROBID end-to-end eval sets | PMC 1,943 / bioRxiv 2,000 / PLOS 1,000 / eLife 984 PDFs | **CC-BY**, 13.3 GB, [zenodo](https://zenodo.org/records/7708580) | P |
| **AnyStyle** | 12,061 sequences over 5 files; `core.xml` 1,514 ships with the model | BSD-2-Clause | M |
| **GIANT** | **991,411,100** = 633,895 records × **1,564** styles | **CC BY 4.0**, 438 GB, [doi:10.7910/DVN/LXQXAO](https://doi.org/10.7910/DVN/LXQXAO) | P |
| **unarXive 2022** | 1.88 M papers, **63.4 M references, 28.1 M (44.4 %) linked to OpenAlex** | code MIT; full corpus **restricted access** | P |
| S2ORC | 81.1 M papers, 380.5 M citation edges | ODC-By 1.0 | P |
| **RenoBench** (2026) | 10,000 labelled citations, 8 languages, 30 % books | **CC BY-NC-ND 4.0** | P |
| **SSH benchmark** (2026) | CEX 5,160 / EXCITE 10,171 / LinkedBooks 1,194 references | not stated | P, U |
| **FOSSIL** (2026) | 7,600+ references, 96 articles, law and humanities, α = 0.917 | **CC BY 4.0** | P |
| LinkedBooks / Venice | 40,000+ annotated references | CC BY 4.0 | P |
| CiteSeerX | ~1,563, cited secondhand | — | **U**, data page unreachable |

The CORA line is the one to sit with. **The most-cited benchmark in the
field has 500 labelled strings, no licence statement, computer science
only, and zero title-less references.** The "1,295" and "1,148" figures
circulating in the literature are cleaned and merged variants, not the
original file (M).

**unarXive is the most interesting corpus here for a different reason:**
it retains `bib_entry_raw`, the unparsed reference string, alongside the
parsed form and the resolved link. That makes it the largest available
pool of real reference strings paired with an identifier. Its labels are
GROBID silver, not gold, and 44.4 % is coverage rather than accuracy —
but it is 57 % physics, so its ~35 M *unlinked* references are probably
the richest naturally-occurring reservoir of title-less strings in
existence, unlabelled and unmeasured.

**The two 2026 benchmarks that matter.** RenoBench (Sarin, Alperin,
Buttrick & Mentis, [arXiv:2603.25640](https://arxiv.org/abs/2603.25640),
P) — 10,000 citations from SciELO 47 %, Redalyc 24 %, Open Research
Europe 14 %, PKP 14 %, deliberately non-Anglophone. Recall by field:

| model | valid XML | surname | given | title | source | volume | year |
|---|---|---|---|---|---|---|---|
| GROBID | **100.0** | 71.6 | **18.8** | 65.2 | 51.8 | 90.0 | **97.9** |
| Qwen2.5-7B-Instruct | 87.6 | **95.8** | 74.5 | 94.0 | 53.3 | 96.0 | 90.6 |
| **Qwen3-0.6B + citation-parsing LoRA** | 95.0 | 92.6 | **77.9** | 93.4 | **73.3** | **98.6** | 97.2 |
| Qwen3-32B | 78.4 | 86.8 | 69.2 | 84.7 | 55.6 | 92.3 | 86.2 |

**A 0.6 B fine-tune is the best system on the board**, beating 32 B
models — and its valid-XML rate rose from 81.4 % base to 95.0 % with the
adapter. The authors explicitly disown their precision column: manual
review of 330 flagged errors found the annotation wrong 92 % of the time
for year, 85 % for source, 72 % for title. **The benchmark's ground truth
is the error source, not the models.**

The SSH benchmark (Zhu, Colavizza & Romanello,
[arXiv:2603.13651](https://arxiv.org/abs/2603.13651), SCOLIA 2026 @ ECIR,
P) quantifies the crossover. Parsing micro-F1 from gold strings: CEX
**GROBID 0.856 vs best LLM 0.7475**; EXCITE **GROBID 0.692 vs
Mistral-Small-3.2-24B 0.862**; LinkedBooks GROBID 0.687 vs ~0.78.
Extraction "largely saturates"; **parsing is the bottleneck**, with the
stated cause "structured-output brittleness under noisy layouts". LoRA
gives +0.5–2.5 % on the easy corpora and **+9.4–10.6 % on LinkedBooks** —
fine-tuning buys exactly the hard cases. Their recommendation is a
**router**, not a winner.

## 8. Span taggers: GLiNER and kin

| model | params | encoder | context | licence |
|---|---|---|---|---|
| GLiNER | 195 M | DeBERTa-v3 | 512 | Apache-2.0 |
| GLiNER2 base / large / multi | 205 / 340 / ~205 M | DeBERTa-v3, mDeBERTa | 2048 | Apache-2.0 |
| GLiNER2.5 small / base / multi | 74 / 194 / 287 M | DeBERTa-v3-xsmall/base, mDeBERTa | — | Apache-2.0 |
| GLiFormer base / large | 264.2 / 575.6 M | DeBERTa | — | Apache-2.0 |

GLiNER2 (Zaratiana et al., **EMNLP 2025 System Demonstrations**,
[paper](https://aclanthology.org/2025.emnlp-demos.10/),
[arXiv:2507.18546](https://arxiv.org/abs/2507.18546), P): CrossNER
zero-shot F1 average **0.590** against GPT-4o's 0.599; zero-shot
classification average 0.72 against GPT-4o's 0.84. CPU latency in ms at
5/10/20/50 labels: **130 / 132 / 163 / 208**, against DeBERTa-v3's 1,714 /
3,404 / 6,758 / 16,897 and GPT-4o's 358 / 382 / 425 / 463 — **2.62×
faster than GPT-4o while running CPU-only**. Trained on 254,334 examples
(135,698 real documents plus 118,636 GPT-4o-generated).

**Three things decide whether this family is usable here.**

1. **The span-width ceiling disqualifies classic GLiNER.** It enumerates
   candidate spans up to `max_width` — **default 12 tokens**, and some
   shipped checkpoints use 8. Spans longer than that are *undetectable,
   not merely inaccurate* (P). A citation's author block is routinely
   20–100+ tokens. **GLiNER2.5 fixes exactly this** by replacing span
   enumeration with **boundary prediction**: "any span length within the
   encoded window is representable" (P). If this family is used, use 2.5.
2. **The structured-JSON feature has no published benchmark.** The paper
   states that hierarchical structure extraction "was not evaluated due
   to the absence of established zero-shot benchmarks for this task
   type" (P). `extract_json()` with `field::type::description` schemas
   exists and works; its accuracy is unmeasured.
3. **Licence trap on the v1 checkpoints.** `urchade/gliner_base` and
   `urchade/gliner_multi` are **CC-BY-NC-4.0**; the v2/v2.1 line and
   everything under `fastino/` are Apache-2.0 (P). Check each checkpoint,
   not the repository.

**The genuine advantage over every CRF parser** is nesting. GLiNER scores
candidate spans and decodes through a priority queue in O(n log n), with
a nested mode permitting fully nested spans while rejecting partial
overlaps (P). BIO/CRF taggers structurally cannot — one label per token.
That is why AnyStyle emits the whole author block as a single field, and
why it scored badly in a study whose field mapping expected individual
authors. Nesting is *wanted* here: an `authors` span containing
individual `author` spans, which is what a hierarchical schema expresses
and a CRF cannot.

GLiNER2.5 quality: macro-average F1 56.17 (multi) against GLiNER2's
56.09; base 54.87 against 53.34; Few-NERD base 55.14 against 47.22 (P).
**GLiFormer** (Knowledgator, September 2026,
[card](https://huggingface.co/knowledgator/gliformer-large-v1), P) reports
"multilevel structuring" **order-free, boundary-tolerant JSON F1 91.10**
(large) / 87.20 (base) — on **500 examples** with a deliberately forgiving
metric, and its own card concedes "fixed record anchors and the
configured span width constrain extraction capacity". Promising, not
established. Its NER numbers are more sober: 26 datasets, mean strict
entity F1 **50.91**.

**Rust inference is mature.** `gline-rs`
([repository](https://github.com/fbilhaut/gline-rs), **Apache-2.0**,
v1.0.0 marked production-ready, ONNX Runtime via `ort`) benchmarks
**6.67 sequences/s on a Core i9 CPU** against 1.61 for the Python
original, and **248.75 sequences/s on an RTX 4080** (P). *Estimate,
method: divide.* At 248.75/s, 1.15 M strings is 4,623 s ≈ **1.3 hours**
on that card; on CPU ≈ 48 hours. It does not support GLiNER2; for that
there are `gliner2-rs` and `gliner25-rs` (ONNX) and `gliner-rs` (candle),
none on crates.io.

Fine-tuning cost in GPU-hours: **none published** (U). LoRA is documented
as "2–3× faster than full fine-tuning". For scale calibration, ReLiK at
216 M parameters trained on **a single RTX 4090** — retriever 4 h, reader
3–13 h (P).

## 9. Late interaction: ColBERT and kin

**"ColBERTa" is not a real system** (P, searched several ways). The
intended target is almost certainly **ColBERTer**, which is the most
relevant variant here: Hofstätter, Khattab, Althammer, Sertkan & Hanbury,
[arXiv:2203.13088](https://arxiv.org/abs/2203.13088),
[code](https://github.com/sebastian-hofstaetter/colberter), **Apache-2.0**.
Its "Bag of Whole-Words" learns **whole-word** representations rather than
wordpiece and prunes non-essential word vectors; the **Uni-ColBERTer**
variant adds an explicit **exact-matching component** where BOW2 matches
only exact word matches between query and document. Storage: **"reduce the
storage footprint by up to 2.5×"**, and the smallest configuration reaches
**"index storage parity with the plaintext size"** at one dimension per
token (P). Whole-word units plus an exact-match channel is precisely the
fix for wordpiece fragmenting `Phys. Rev. Lett.` and `112:1169`. Other
nearby names, none of which is "ColBERTa": ColBERTv2, Jina-ColBERT-v2,
ColPali, ColQwen, ColBERTSaR, IBM's ColBERT-practical — and an unrelated
humour-detection paper also called ColBERT, a pure name collision.

**ColBERTv2** (Santhanam et al., NAACL 2022,
[arXiv:2112.01488](https://arxiv.org/abs/2112.01488), P), in its §5.3 and Tables
4–5: MS MARCO index **154 GiB (vanilla ColBERT) → 16 GiB at 1-bit / 25
GiB at 2-bit**, 6–10× compression including 4.5 GiB of inverted list —
**~20 bytes per vector at 1-bit, ~36 at 2-bit**. MS MARCO dev MRR@10
**39.7**, R@50 86.8, R@1k 98.4. Query latency **50–250 ms**; PLAID
([arXiv:2205.09707](https://arxiv.org/abs/2205.09707)) brings that to
**35–100 ms on GPU, 150–300 ms on CPU at 140 M passages** (P). Token
pooling (Clavié, Chaffin & Adams,
[arXiv:2409.14683](https://arxiv.org/abs/2409.14683)): **50 % vector
reduction with virtually no degradation, 66–75 % with under 5 %
degradation on most datasets**, drop-in at index time, no retraining (P).

**Estimated index size for this corpus.** *Method: bytes-per-vector from
the measured ColBERTv2 figures above, × tokens per record × records. A
citation string or work record tokenizes to roughly 60–80 wordpiece
tokens — abbreviations and numerals fragment badly, so 80 is used.*

| corpus | 2-bit, no pooling | 2-bit + 66 % token pooling |
|---|---|---|
| 10 M works × 80 tokens | ~29 GB | ~10 GB |
| 100 M works × 80 tokens | **~288 GB** | **~96 GB** |

At 100 M works a plain ColBERTv2 index does not fit the 128 GB box, and
that box is shared with the search engine. §13.2 draws the conclusion.

## 10. Small models and constrained decoding

**Constrained decoding libraries.** **XGrammar**
([repository](https://github.com/mlc-ai/xgrammar), **Apache-2.0**,
[arXiv:2411.15100](https://arxiv.org/abs/2411.15100), P): **under 40 µs**
per-token mask generation. The honest headline for a JSON-schema workload
is **~3–3.5× mask-generation speedup over Outlines, llama.cpp and
lm-format-enforcer** — the widely-quoted ">100×" is the **CFG** row, a
different measurement. Batch-1 overhead is **TPOT 6.3 ms constrained
against 6.2 ms unconstrained, ~1.6 %**. XGrammar-2 (2026-05-04) adds
cross-grammar caching and claims up to 80× compile speedup, one schema
going 534 ms → 5.37 ms. **llguidance**
([repository](https://github.com/guidance-ai/llguidance), **MIT**):
Earley parser over a regex-derivative lexer, **~50 µs single-core CPU per
token for a 128 k-token tokenizer**, no significant startup cost (P).
**Outlines** (**Apache-2.0**) front-loads an FSM index over the whole
vocabulary; JSONSchemaBench
([arXiv:2501.10868](https://arxiv.org/abs/2501.10868), P) measured
**compilation 3.48–12.77 s and TPOT 30–81 ms**, against XGrammar's
0.12–0.30 s compile and Guidance's 6.37–9.47 ms TPOT. Wrong shape for
this job.

**The accuracy debate, both sides.** Tam et al., **EMNLP 2024 Industry**
([paper](https://aclanthology.org/2024.emnlp-industry.91/), P): reasoning
tasks degrade sharply under JSON mode — GSM8K on GPT-3.5 **76.60 % →
49.25 %** — but **classification tasks improve**, DDXPlus on
Gemini-1.5-flash **41.59 % → 60.36 %**. Their own conclusion is that the
effect is task-dependent. dottxt's rebuttal
([post](https://blog.dottxt.ai/say-what-you-mean.html), P) shows the two
arms used different prompts, that the structured prompt never mentioned
JSON or gave a schema, and that the real variable was an AI-based parser;
their re-run has structured ≥ unstructured throughout. Third-party
JSONSchemaBench finds constrained decoding *improves* downstream accuracy
by up to 4 %.

**For this task it resolves cleanly.** Citation parsing is span
transcription, not multi-step reasoning — the regime where every source
shows constraint is neutral to helpful. And SqueezeBits measured **schema
correctness 61–72 % unconstrained against 96–100 % with guided decoding**
(S, charts only). At 1.15 M records that dominates any plausible
few-point accuracy effect, because invalid output costs a full retry.

There is also an under-appreciated upside: JSONSchemaBench measured
**Guidance at 6.37–9.47 ms TPOT against a ~15–16 ms unconstrained
baseline** — constrained decoding can be **~2× faster**, because every
field name, quote, colon and brace is grammar-determined and can be
fast-forwarded. A reference-parsing schema is perhaps 30–50 %
structurally forced tokens.

**Models.** Apache-2.0 and clean: **Qwen3-0.6B / 1.7B / 4B**,
**SmolLM3-3B**, **NuExtract3** (4 B, on Qwen3.5-4B), GLiNER2 / 2.5,
GLiFormer. Encumbered: **Qwen2.5-3B** and **NuExtract-2.0-4B** (Qwen
Research Licence, non-commercial), **Gemma 3** (custom `gemma` licence,
not Apache-2.0), **Llama 3.2** (community licence with an MAU cap). The
sharpest datapoint on specialisation: **NuExtract3 scores 0.651 ± 0.019
on NuMind's ~600-document extraction benchmark against 0.417 for its own
Qwen3.5-4B base** — a +0.234 gain at identical size (P). NuExtract 2.0's
numeric table is **U**; the blog 404s and the model cards use images.

**Throughput.** Anchor benchmark, RTX 4090 24 GB, vLLM offline, 100 input
/ 600 output tokens, 300 requests (S, vendor benchmark): Qwen2.5-3B
**10.31 req/s, 7,214 total tok/s**; a 1.5 B **13.85 req/s, 9,696 tok/s**;
gemma-2-9b collapses to 0.68 req/s in 24 GB, so stay at 4 B or below.

*Estimate, method: hold aggregate token throughput constant and rescale to
this workload's ~100 in / ~100 out = 200 tokens per request. Prefill is
cheaper per token than decode, so this errs conservative.*

```
3B:    7,214 tok/s ÷ 200 = 36.1 req/s → 1,150,000 ÷ 36.1 = 31,882 s =  8.9 h
1.5B:  9,696 tok/s ÷ 200 = 48.5 req/s → 1,150,000 ÷ 48.5 = 23,721 s =  6.6 h
```

With a ×1.25 guided-decoding haircut: **11.1 h (3 B) / 8.2 h (1.5 B)**.
Honest range **7–31 hours**, most likely 9–12. The spread is dominated by
actual output length and by the masking penalty at batch, neither of
which is published.

Two operational notes. **Grammar compilation is cached per unique
schema** — one schema used 1.15 M times makes compilation a non-issue;
the real cost is CPU-side per-token masking, and SqueezeBits observed
**vLLM degrading significantly at batch while SGLang barely did**,
because SGLang overlaps mask generation with the GPU step (S, charts
only, magnitude **U**). Benchmark both before committing; it is a bigger
lever than model choice. And **do not quantize weights** at 1.5–3 B on
24 GB — FP16 is only 3.6–6.2 GB, INT4 can be slower at high batch, and
the task is precision-critical on exactly the tokens quantization hurts
(DOIs, page numbers, volumes). Use an FP8 KV cache for concurrency
instead, and enable prefix caching, since the system prompt is identical
across all 1.15 M requests.

## 11. CSL: synthetic data and style identification

**Counts, verified twice** — GitHub trees API at master commit
`81c3eb86` (2026-09-18, `truncated: false`), cross-checked against
Zotero's `styles.json`, which agrees to the unit (M):

- **2,861 independent styles**, **8,000 dependent**, **10,861 total**.
- A **dependent style contains no formatting at all** — metadata plus
  `<link rel="independent-parent">` — so it renders byte-identically to
  its parent. **For style identification the label space is 2,861, not
  10,861.**
- Format split among independent styles: 1,344 author-date, 957 numeric,
  **547 note**, 10 author, 3 label. Note-format styles may produce no
  bibliography entry at all; in a 120-style sample, 4 returned "does not
  contain instructions to format bibliographies" (M).
- **`renamed-styles.json` holds 611 old→new mappings**, and three of
  Crossref's 17 classifier style names no longer exist upstream. Any
  style vocabulary needs this map or it silently fragments.
- `styles-distribution` is **deprecated** (README says so; last commit
  2020-11-23, roughly 1,000 styles stale).

**Licences:** styles **CC BY-SA 3.0 Unported**; locales the same;
specification and documentation **CC BY-SA 4.0**; schema **MIT** (P).
There is **no LICENSE file in the styles repository** — the licence is
asserted in the README and in a per-file `<rights>` element. On whether
ShareAlike reaches rendered output, two real-world precedents rather than
a legal conclusion: **GIANT**, ~1 billion strings rendered through those
styles, is published **CC BY 4.0**; **hayagriva** segregates explicitly,
code MIT OR Apache-2.0 with the bundled style collection under CC BY-SA
3.0. Both treat the styles as an input work. The clearer obligation is
around redistributing the style *files* — attribution, and keeping
contributor listings intact.

**Inversion is the wrong frame, and GIANT shows why.** They did not
invert citeproc. They **instrumented the style XML**, rewriting
`<text variable="publisher"/>` as
`<text variable="publisher" prefix="&lt;publisher&gt;" suffix="&lt;/publisher&gt;"/>`.
Because `prefix` and `suffix` are spec features that pass through
verbatim, **any conforming processor becomes a labelled-data generator
with no code changes** (P). Their repository holds 1,568 original and
1,747 instrumented `.csl` files, MIT. This is the most transferable
finding in this section: labelled-data generation is a **style-rewriting**
problem, not a processor problem.

**Processors — the Rust news is bad, then adequate.** **citeproc-rs is
archived** (2026-08-13), was **never published to crates.io**, and Zotero
removed it in commit `e679473db1` (2026-08-31) with the message
"citeproc-rs is no longer maintained, and people who had enabled the
hidden pref were hitting errors"; the proximate cause was that the WASM
build was slower than citeproc-js (P). The Rust answer is **hayagriva**
(MIT OR Apache-2.0, Typst project) — but its CSL conformance is
**511/860 test fixtures = 59.4 %** (M, from its checked-in pass list),
and the failing categories include ones that touch bibliography output
directly: `textcase` 39 %, `flipflop` 5 %, `affix` 22 %, `name` 64 %.
**jgm/citeproc** (Haskell, BSD-2-Clause) is the best permissive fidelity
at **~795/858 ≈ 92 %**. Since generation is a one-time offline job,
**citeproc-js under Node is the defensible choice** — it is what GIANT
used, and no throughput figure is published by any processor (U).

**The inverse problem is already solved, by Crossref, and is directly
reusable.** [citation_style_classifier](https://gitlab.com/crossref/citation_style_classifier),
**MIT, © 2018 Crossref**, `pip install styleclass` (P). Logistic
regression on **TF-IDF of 2/3/4-grams of *word types*** — capitalized-
word, lowercase-word, punctuation, year, deliberately **not** the words
themselves, so it is content-invariant. Trained on **5,000 Crossref
records × 17 styles = 85,000 strings**; train 87,808 / test 87,625;
**94.7 % accuracy** over 17 styles plus "unknown". And the style list
includes **`elsevier-without-titles`** and `american-chemical-society`
(the without-titles form) alongside their `-with-titles` siblings —
**Crossref's own classifier models title-less variants as first-class**.
A larger attempt exists — Kopan et al., ICDMW 2023, "6 million
bibliographic records spanning 91 citation styles", claiming to
significantly outperform it — but it is paywalled and its methods,
accuracy and dataset availability are **U**.

**Does synthetic transfer? The decisive experiment is Grennan & Beel,
WOSP 2020** ([paper](https://aclanthology.org/2020.wosp-1.4/), P). GROBID
trained on 5,460 GIANT strings against 5,460 real strings, macro
field-level F1:

| trained on | Eval_Grobid | Eval_GIANT | Eval_Cora | **Eval_WebPDF (real, unbiased, 300 refs)** |
|---|---|---|---|---|
| real | **0.93** | 0.69 | 0.74 | **0.74** |
| synthetic (GIANT) | 0.69 | **0.91** | 0.62 | **0.74** |

**On real PDF-extracted references, synthetic and real training data
perform identically.** Three caveats travel with that, and §13.4 takes
them up. Per-field on the real set: date 0.97, pages 0.94, volume 0.94,
author 0.84, journal 0.79, **title only 0.70**, publisher 0.46, **book
title 0.16** — books are essentially unparsed by either.

**One temporal trap, measured by reading the XML** (M): the current
`american-physics-society.csl` and `american-institute-of-physics.csl`
**now render titles**. The title-less APS form survives only as the
explicit `-without-titles` variant. **A CSL corpus generated today will
be less title-less than the historical literature it must match
against.** §16 says what to do about it.

## 12. What could not be verified

Kept as its own section because the gaps are part of the finding.

1. **Needle 3's extraction F1 numbers** — rendered only inside an SVG plot.
2. **Jev's parameter count, context limits, maximum Choice options, rate
   limits and licence** — none disclosed.
3. **GLiNER and GLiNER2 fine-tuning cost in GPU-hours** — published by
   nobody.
4. **GLiNER2's hierarchical JSON extraction accuracy** — the paper states
   it was not evaluated.
5. **NumColBERT's benchmark scores** — none in the abstract.
6. **ParsCit's original per-field F1** (hosts unreachable) and **Neural
   ParsCit's numbers** (paywalled). The circulating "0.96" for Neural
   ParsCit is **misattributed** — it is a GIANT→Cora figure.
7. **CERMINE's field-level exact-match numbers** — published only as a
   bar chart. The headline 95.12 is **token-level**, the most forgiving
   unit anyone in this survey publishes.
8. **Published throughput for AnyStyle, CERMINE, refextract,
   science-parse and ParsCit** — none exists.
9. **Europe PMC's matching algorithm and any precision or recall** — no
   primary source found anywhere.
10. **OpenAlex's matching algorithm beyond what the code shows**, and any
    precision or recall figure — none published.
11. **Crossref's current Marple-era strategy metrics**, and what
    `reference-cs-combined` actually does — "no code is available".
12. **Guenci et al.'s exact P/R/F1** as prose versus table — the two
    disagree; the table was used.
13. **CORA's licence** — none stated anywhere. **CiteSeerX** — data page
    unreachable; treat as dead.
14. **Absolute throughput penalty of guided decoding in vLLM versus
    SGLang** — charts published, numbers withheld. And no benchmark at
    all for a 1–4 B model *with* constrained decoding at the
    ~100-in/100-out shape.
15. **AWQ/GPTQ against FP16 batched throughput** for a 1–3 B model on a
    consumer GPU, and **any RTX 3090 figure**.
16. **ICDMW 2023 style classification** (6 M records, 91 styles) —
    paywalled.
17. **TransParsCit's F1** — PDF 403s; its abstract ("up to 100 % on key
    fields") conflicts with Grennan & Beel's GIANT→Cora 0.62.
18. **hayagriva's skip-adjusted CSL pass rate** — 511/860 is the
    checked-in absolute; its harness skips whole classes.
19. **Whether CC BY-SA 3.0 ShareAlike attaches to CSL-rendered output** —
    precedents reported, not a legal conclusion.
20. **A clean 100 M-document BM25 benchmark for Tantivy.**
21. **Any published work applying ColBERT or late interaction to
    bibliographic record linkage.** This is a genuine gap in the
    literature, not a search failure.

Two numeric corrections worth carrying out of this document: **GIANT used
1,564 styles, not the widely-cited "1,500"**, and its input was **633,895
unique records, not 677,000** (P, from the paper body against its own
abstract).

## 13. Corrections to the earlier architecture note

Four things in the note that preceded this survey were wrong or
overstated. They are kept as corrections rather than quietly fixed,
because the reasoning that produced them is the reasoning to avoid.

### 13.1 The matched set is not in-distribution with the unmatched target

The note treated the millions of references carrying **both** citation
text and a confirmed identifier as "a free, large, in-distribution
evaluation and training set". The first two adjectives hold. **The third
does not.**

References that already carry a confirmed identifier are, by
construction, the ones an existing matcher already succeeded on. They
skew toward recent, DOI-bearing, well-formed, English, journal-article
references from publishers that deposit reference lists. The ~1.15 M
without identifiers are the residue those matchers failed on: older,
title-less, non-English, books and reports, malformed, abbreviated in
ways no table lists. **Training on the matched set and evaluating on the
matched set will overstate performance on the unmatched set, and the
overstatement will be largest exactly where it matters.**

The literature supports the size of the gap. Grennan & Beel measured a
~30 % F1 swing between matched and mismatched train/test distributions
(§11). Default GROBID goes from 0.87 on PMC to **0.36** on FOSSIL (§5).
NIH-OCC goes from recall 0.98 same-journal to **0.78–0.89** on
heterogeneous sources (§6.6). §14.1 gives the causal account of why the
split exists at all.

### 13.2 ColBERT is the wrong tool here

The note treated late interaction as a serious candidate for candidate
generation. Three measurements say otherwise, and they point the same
way.

- **BM25 ties or beats it on the most relevant benchmark.** ColBERTv2's
  own BEIR table gives **SCIDOCS: BM25 15.8, ColBERTv2 15.4** — the
  scholarly short-text dataset, where BM25 ties or beats every neural
  system (P).
- **Dense retrieval collapses on rare entities**, which is what a
  reference string is. Sciavolino et al., EMNLP 2021
  ([paper](https://aclanthology.org/2021.emnlp-main.496/)): on
  EntityQuestions, **BM25 beats DPR by 49.9 points absolute top-20
  accuracy** on the worst pattern, dense retrievers show popularity bias,
  and data augmentation does not fix it (P).
- **The index does not fit.** §9's estimate puts a plain ColBERTv2 index
  at ~288 GB at 100 M works, on a 128 GB box shared with the search
  engine.

Add that subword tokenization does not model magnitude — "2,000" and
"20,000" can be nearly indistinguishable — which is live enough research
that NumColBERT ([arXiv:2605.10109](https://arxiv.org/abs/2605.10109),
May 2026) exists specifically to patch numerical gating into late
interaction. **The correct role for late interaction here, if any, is as
a reranker over BM25 candidates at rung 5, and ColBERTer's whole-word
plus exact-match formulation rather than vanilla ColBERT.**

### 13.3 CSL inversion is already done, and done better

The note proposed inverting CSL style definitions to identify which style
produced a string, and to generate training data. **Both halves already
exist and both were done better.** Style identification is Crossref's
`styleclass` — MIT, 94.7 %, and its features are n-grams of *word types*
rather than words, which is a cleverer idea than inversion (§11).
Generation is GIANT's **instrumented styles** — adding `prefix`/`suffix`
attributes so a standard processor emits labels — which needs no
inversion machinery at all and works with any conforming citeproc (§11).
The work to do is not to build either; it is to **re-run GIANT's
instrumentation over the 50 title-less styles** and to extend
`styleclass` past 17 labels.

### 13.4 Synthetic-at-scale is unproven, not proven

The note read Grennan & Beel's "synthetic equals real" result as a green
light for training at GIANT's scale. **That is not what they measured.**
Their own conclusions:

- **Returns to scale die at ~10,000 instances.** 1 k → 10 k gave +6 %;
  **20 k and 40 k gave no improvement, sometimes a decline** (P). They
  write: "there seems to be little benefit in using synthetic data
  (i.e. GIANT) for training traditional machine learning models."
- **That is a CRF result.** They explicitly did not train a deep model
  and say the deep case "has yet to be empirically shown". No published
  work has since done it successfully (U, searched).
- **Richer label sets helped more than more data did:** adding
  institution, note and pubPlace labels gave **+13.5 % F1** on fields not
  even in the evaluation set (P).

So the honest position is: synthetic data of the right *kind* is as good
as real; synthetic data at a billion strings is **untested for the
architecture actually under consideration**, and the one measured lever —
labelling more field types — is cheap and orthogonal to volume. §14.2
proposes the kind that would be worth generating.

## 14. Three contributions from the design discussion

Not from the literature. These came out of discussion with the user
during the survey and are recorded as design reasoning, not findings.

### 14.1 Why the distribution shift exists

The causal account behind §13.1, and it is simpler than it looks.
**Publishers solved PDF handling at different times, and the corpus
remembers.** A post-2010 Elsevier paper is well-formed because that
publisher industrialised its production pipeline long ago — consistent
typography, embedded text, structured reference deposits. A pre-1990
paper from a small society is none of those things: scanned, OCR'd,
idiosyncratically abbreviated, with no deposit behind it.

The consequence is uncomfortable and worth stating plainly: **the good
data is definitionally the data we already have, and the data we lack is
the data that is hard to get.** The two sets are not a random split of
one population; they are the output of a filter whose criterion is
roughly "was this easy to process". Any evaluation that samples
uniformly from what is already matched is sampling from the wrong side of
that filter.

### 14.2 A degradation model for synthetic data

Rendering clean Crossref metadata through CSL styles produces clean
strings, and §11's temporal trap means it produces *cleaner* strings than
the historical corpus even before noise. The proposal is to stop treating
generation as the whole job and **learn the corruption distribution from
real hard examples, then apply it to clean records**:

- **OCR confusions** — the character-level substitutions a given scanning
  era actually produces, learned rather than assumed.
- **Ligature and hyphenation damage** — `fi`/`fl` losses, line-break
  hyphens surviving into the extracted string.
- **`et al.` truncation at varying depths** — not just "after three
  authors", but the distribution of depths actually observed.
- **Inconsistent and unofficial abbreviations** — see §14.3.
- **Dropped and reordered fields** — the volume that is missing, the year
  that migrated to the end.

Crossref's own noise measurement is the motivation and the sizing (§3.2):
**APA plus noise loses about 2 points (97.6 → 95.2); AIP plus the same
noise loses about 15 (82.0 → 66.9).** Noise is not a uniform tax — it is
cheap on title-bearing strings and expensive on title-less ones, which
means a training corpus that models noise and titlelessness *jointly* is
measuring something a corpus that models neither cannot see. This is also
the natural answer to §13.4: the useful axis is corruption realism, not
volume.

### 14.3 Mining abbreviation variants from the corpus

The NLM title abbreviation table gives **official** forms only. A large
part of why this problem is hard is the long tail of *unofficial*
spellings that real authors and real house styles produce — PNAS alone
appears under several, and that is a journal everyone knows.

The observation: **every already-resolved reference that contains a venue
string is a `(variant → journal)` observation, and the corpus is full of
them and has never harvested them.** A reference whose target is known
and whose text contains `Proc. Natl. Acad. Sci. U.S.A.` is a labelled
example, for free, in whatever quantity the resolved set provides. The
result is an empirical abbreviation table derived from usage rather than
from a standards body — which is exactly the resource `inTitle` needs,
given that it is GROBID's weakest field at 71.65 strict F1 (§5) and the
hardest field for every system on RenoBench (best source recall 73.3,
§7).

This is also one of the few places where the selection bias of §13.1
works *in our favour*: the matched set is biased toward well-formed
strings, but venue abbreviations vary by house style rather than by
scan quality, so the variants harvested there transfer.

## 15. A PDF arrives with no metadata

Worth stating as a first-class use case rather than a side effect: **a
user uploads a PDF and wants its references back, resolved.** Not a
corpus job, not a batch — one document, interactively, and it must be
robust across three quite different inputs:

- **A bad scan** — OCR noise, skew, ligature damage. This is where §14.2's
  degradation model earns its keep, and where the rule-based tools fail
  hardest: rule/regex parsers average **recall 0.22** against ML's 0.66
  (§5).
- **A highlighted or annotated copy** — extraction artifacts from
  annotation layers, text ordering disrupted.
- **A publisher-clean PDF** — where GROBID is already excellent and
  anything slower is waste.

The same cascade serves all three, which is the argument for building it
as a cascade rather than as a single model: rung 3 routes by document
class, and both 2026 benchmarks reached "route, don't pick" independently
(§7). The interactive case also changes the latency budget — 0.067 s per
reference (§17) against a 30-reference bibliography is two seconds, which
is fine; a 3 B model at rung 6 on all thirty is not.

## 16. Evaluating this, and the opening

**Nobody publishes matcher precision adjudicated against *which work* was
matched, rather than whether an identifier was emitted.** That is the
opening, and it is not a small one.

The evidence that it is genuinely open:

- Crossref's 2,000-string set is the only honestly-constructed evaluation
  in the area, and it is **eight years old** (§6.1).
- OpenCitations' 100 % precision **cannot detect wrong-entity matches by
  construction** — a false positive is defined as a DOI not in Meta, so
  matching to the wrong work that happens to be in Meta scores as a true
  positive (§6.4).
- S2ORC's 1.00 is measured only over links it accepted (§6.3).
- NIH-OCC found that **over 90 % of its own false positives were
  actually false negatives in the gold standards** (§6.6).
- RenoBench's authors disown their own precision column because manual
  review found the annotation wrong 72–92 % of the time on flagged
  errors (§7).

Every one of those is the same failure in a different costume: the
evaluation measures whether the system emitted *an* answer, not whether
it emitted *the right* answer, and the ground truth is often the weaker
party.

**The design fix**, and the parts of it are all available:

1. **Hold out by style class and by title presence**, not uniformly. A
   single aggregate number will hide the §3.2 collapse completely.
2. **Use Crossref's MIT harness** —
   `github.com/CrossRef/reference-matching-evaluation` already generates
   per-style reference sets and releases match results — **plus the 50
   title-less CSL styles** identified in §3.1, which are exactly the
   generator input needed.
3. **Adjudicate the identity of the match, not the presence of one.** A
   wrong edge and a missing edge are different errors with different
   costs, and only the first is silent.
4. **Report per-rung.** The cascade of §4 has seven places to be wrong
   and they fail differently; one number averages them into
   uninterpretability.

**One trap to design around**, measured in §11: the current
`american-physics-society.csl` and `american-institute-of-physics.csl`
now render titles. The title-less form of those styles survives only as
the explicit `-without-titles` variants. **A test corpus generated today
from default style names will under-represent the historical title-less
form** — which is to say, it will quietly reproduce the bias of §13.1 in
the evaluation set as well as the training set. The title-less styles
must be sampled deliberately and over-weighted relative to their 1.8 %
share of the style repository.

## 17. biblio-glutton: the floor to beat

[repository](https://github.com/kermitt2/biblio-glutton), **Apache-2.0**,
Elasticsearch plus LMDB, by GROBID's author and used by GROBID for
consolidation. `THRESHOLD_MATCHING = 0.7` in
`LookupEngine.areMetadataMatching`, made configurable in PR #120, merged
**2026-09-15** (P).

Benchmarked on **17,015 reference/DOI pairs from the PMC 1943 sample**
(P):

| | P | R | F1 | DOIs found | sec/ref |
|---|---|---|---|---|---|
| **biblio-glutton (BiLSTM-CRF)** | **97.34 %** | **95.83 %** | **96.58** | 16,699/17,015 | **0.067** |
| biblio-glutton (CRF) | 97.33 % | 95.52 % | 96.42 | — | — |
| Crossref REST `query.bibliographic` | 97.19 % | 94.26 % | 95.69 | 16,502/17,015 | 0.180 |

Note that the Crossref row is the **naive public search API, not SBMV** —
so it is not the §6.1 system and should not be read as such.

This is the most directly reusable open matcher in the survey,
permissively licensed, from the same author as the parser already in
scope, and it is **the floor**. Anything built here has to beat F1 96.58
at 0.067 s per reference to justify itself. *Estimate, method: divide.*
At 0.067 s/ref single-threaded, 1.15 M references is 77,050 s ≈ **21.4
hours**, which is the same order as the §10 GPU estimate for a 1.5–3 B
model — so the interesting comparison is not speed, it is what each does
with the title-less residue, and neither has been measured on it.

## 18. What this implies for how we treat OpenAlex data

Two findings landed on the same day and point the same way.

**First, from this survey:** OpenAlex's reference matcher treats
**absence as agreement**. Its author check returns true when *either*
side has no author metadata — "no record_author_json, so not trying to
match" — and its scoring has **no minimum**, so a candidate agreeing on
nothing but an exactly-normalised title is still returned (§6.2). This is
the precise inverse of the rule this project adopted on 2026-09-18; see
[merge plan](../merge-plan.md) §8 for what that rule says and why. The
consequences are visible in OpenAlex's own repository: the `oxjob #880`
defect, where "digits are identity … stripping them collapsed thousands
of distinct records onto one work", shipped with an audited un-merge tool
requiring individual human sign-off for works cited 100 times or more.
That is what absence-as-agreement costs at scale, reported by the people
who paid it.

**Second, observed in this project rather than in the literature:** the
OpenAlex bulk snapshot contains records whose abstract belongs to a
different paper.

Neither is a reason to stop using OpenAlex; it remains the broadest open
source available. Both are reasons to treat its **derived** fields
differently from its **asserted** ones. A DOI OpenAlex received is
evidence. A link OpenAlex *inferred* is a claim produced by the matcher
described in §6.2, and it carries that matcher's error profile —
including, on the §3 evidence, a systematic hole wherever the reference
had no title to normalise. The same applies to abstracts: a field that
can belong to another paper is not a field to promote on OpenAlex's
authority alone.

Two practical consequences. **Re-derive rather than inherit** wherever
this project's own cascade can reach the same conclusion — OpenAlex's
40.6 % NULL rate on `cited_work_id` means it discards the residue anyway
(§6.2), so there is little to inherit on exactly the population that
matters. And **do not assume merged ids are traceable**: the pre-Walden
`merged_ids/` directory is gone, and the OpenAlex docs now contradict
each other, one page saying "old IDs never break" and another saying a
merged-away id returns 404 with no redirect (P). Only `deleted_ids.csv`
remains. Any snapshot-diff logic that assumed a merged-id → survivor
mapping no longer has one.
