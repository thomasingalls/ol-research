# Benchmarks for search and research

**Status:** notes, 2026-09-16 — two web surveys made the same day
(searches and page reads only; no dataset downloaded, nothing registered),
recorded at the user's request. Speculative, not a plan: which published
benchmarks the retrieval experiments of [future
ambitions](future-ambitions.md) §5 and the [tracer's
engine](future-ambitions-tracer-engine.md) could run against. Companion
to the [retrieval frontier](retrieval-frontier.md). Every fact is marked
**P** (primary: the paper, repository, dataset card or track page),
**S** (secondary only) or **U** (could not verify).

**Since, 2026-09-26:** a third survey, of more judged sets and of the
starter set's practical facts, is §2.4; it corrects §2.2's TREC Precision
Medicine topic counts and RELISH's BM25 figures, which are marked where
they stand. The sets are chosen for use in the [evaluation
plan](../evaluation-plan.md).

## 1. Two caveats for every run

- **Judged pools.** Relevance judgments cover only the documents someone
  looked at. Over a whole corpus, every relevant work nobody judged
  counts as a miss, and scores fall. Measures that tolerate unjudged
  documents (bpref, condensed lists) soften it.
- **Snapshots.** Most benchmarks were built on an older corpus — a PubMed
  baseline of December 2018, the MEDLINE baseline of 2019, CORD-19 of
  July 2020 — so a corpus of 2026 holds years of works nobody judged, and
  some judged PMIDs may since have been deleted.

Hence two runs each:

- **Restricted** to the benchmark's documents, or to works published
  before its snapshot: comparable with published figures.
- **Open** over everything: the corpus as it is used, read as a lower
  bound.

## 2. Search quality alone

### 2.1 A starter set

Suggested, weighted toward biomedicine, where the corpus is strongest:

| benchmark | tests | gold ids | access | published BM25 |
|---|---|---|---|---|
| TREC-COVID | question → works, biomedical | CORD-19 `cord_uid`, mapping to DOI, PMID, PMC | CORD-19 licence | nDCG@10 0.656 (BEIR) |
| TREC Precision Medicine 2019–2020 | question → abstracts, biomedical | PMID | open (S) | none found |
| RELISH | work → similar works | PMID | CC0 | MRR 0.878 |
| CLEF eHealth TAR 2017–2019 | everything for a Cochrane review | PMID | MIT | none gathered |
| unarXive citation recommendation | citing sentence → cited work | OpenAlex `W` id | CC BY-SA 4.0 | none gathered |
| LitSearch | CS literature question → works | Semantic Scholar `corpusid` | MIT (repository) | recall@5 38.5–62.6 |

- **TREC-COVID:** the deepest graded judgments, and the most-quoted BM25
  anchor.
- **TREC Precision Medicine:** native PMIDs over a MEDLINE baseline.
- **RELISH:** "more like this" — the expansion from a seed set. Its
  candidates were pooled by lexical methods, which flatters BM25.
- **CLEF TAR:** the standard test of recall, which is what a concept
  trace needs: whether all of it was found, not whether the top hit is
  good.
- **unarXive:** the trace's own mechanism — from the sentence around a
  citation, the cited work — with gold already in OpenAlex ids; arXiv's
  fields (computer science, physics, mathematics), so it depends on how
  much of arXiv arrives through OpenAlex.
- **LitSearch:** the one everyone cites, and the only CS question set
  here; its ids need mapping, and the corpus may be thin on ACL and ICLR.

### 2.2 The candidates, one by one

1. **LitSearch** (Princeton, EMNLP 2024). 597 questions, 1.00–1.21
   target works each, binary relevance (P). Corpus: 64,183 ACL Anthology
   and ICLR papers from S2ORC, fixed, on Hugging Face (P); ids are
   Semantic Scholar `corpusid` (P), mapping to DOI or arXiv id a Semantic
   Scholar lookup (S). BM25 recall@5 38.5 on inline-citation specific
   questions and 62.6 on author-written ones; the best dense retriever
   24.8 points higher (P). Repository MIT; the dataset card states no
   licence (P).
   [repository](https://github.com/princeton-nlp/LitSearch),
   [dataset](https://huggingface.co/datasets/princeton-nlp/LitSearch)
2. **BEIR's scientific subsets** (P,
   [repository](https://github.com/beir-cellar/beir)); BM25 as nDCG@10.

   | subset | task | queries | corpus | judgments | ids | licence | BM25 |
   |---|---|---|---|---|---|---|---|
   | TREC-COVID | question → works | 50 | 171,332 | three grades, about 493 relevant a query | `cord_uid` | CORD-19 licence | 0.656 |
   | NFCorpus | lay question → works, nutrition | 323 | 3,633 | three grades, 38.2 a query | PubMed documents; mapping to PMID U | academic use | 0.325 |
   | BioASQ | question → works | 500 | 14.9 M | binary, 4.7 a query | PMID | "Copyright" | 0.465 |
   | SCIDOCS | title → cited works | 1,000 | 25,657 | binary; 5 cited and 25 uncited | S2 SHA ids | GPL v3 | 0.158 |
   | SciFact | claim → abstracts | 300 | 5,183 | binary, 1.1 a query | S2ORC ids | CC BY-NC 2.0 | 0.665 |

   BioASQ is not shipped: the repository marks it not public and gives
   instructions to rebuild it, since BioASQ needs an account (its own
   licence CC BY 2.5) (P). Only TREC-COVID and BioASQ map cleanly to
   PubMed; SCIDOCS and SciFact are small closed pools on Semantic Scholar
   ids.
3. **TREC-COVID, full** (P,
   [data](https://ir.nist.gov/trec-covid/data.html)). 50 topics over five
   rounds, the final corpus CORD-19 of 2020-07-16; 69,318 judgments on
   three grades, round 5 over 192,509 documents (P, via ir_datasets).
   CORD-19's `metadata.csv` gives `cord_uid`, DOI, PMC id, PMID, S2 id,
   arXiv id and a licence per work (P). Maps onto DOIs and PMIDs, and the
   deep graded pools carry over.
4. **TREC Precision Medicine, scientific abstracts, 2017–2020**
   ([2019](https://www.trec-cds.org/2019.html),
   [data](https://trec.nist.gov/data/precmed2019.html)). Topics: 30 in
   2017 and 50 in 2018 (P, via ir_datasets), 40 in 2019 (S; the track
   page suggests 30), 30 in 2020 (P). *Corrected 2026-09-26 (§2.4): 40
   in 2019 and 40 in 2020 (P), the 2020 phase-1 qrels covering 31.* Three grades — not, possibly,
   definitely — in 2017–2018, about 22,000–23,000 judgments a year (P).
   2017–2018: a PubMed snapshot plus AACR and ASCO conference abstracts
   without PMIDs; 2019–2020: the MEDLINE 2019 baseline (P). PMIDs (P);
   qrels open on NIST's site (S). No BM25 figure found.
5. **TREC Clinical Trials, 2021–2023.** Documents are ClinicalTrials.gov
   records by NCT id (2023: 30 topics, about 2.1 million trials, three
   grades) (P). Not a literature benchmark; relevant to future ambitions
   §3's clinical-trials corpus.
6. **CLEF eHealth TAR, 2017–2019** (P,
   [repository](https://github.com/CLEF-TAR/tar)). Cochrane reviews: 50
   in 2017, 30 in 2018, 49 in 2019. Screening prioritisation of what a
   Boolean query retrieved; in 2018–2019 also search from the protocol
   without the query. Topics carry title, objective, protocol and Boolean
   query; the collection is the PubMed baseline of December 2018; PMIDs;
   qrels at abstract level (`qrel_abs`) (P) and content level (S). MIT.
7. **CSMeD** (NeurIPS 2023, P,
   [repository](https://github.com/WojciechKusa/systematic-review-datasets)).
   325 reviews from nine collections, medicine and some CS, and CSMeD-FT
   for full-text screening; loaders Apache 2.0. Building it needs a
   Cochrane cookie, Semantic Scholar and CORE API keys, GROBID and Entrez.
8. **RELISH** (Database 2019, P,
   [figshare](https://figshare.com/projects/RELISH-DB/60095)). 3,017 seed
   articles with 60 candidates each, 181,020 pairs on three grades —
   relevant, somewhat, irrelevant — judged largely by the seed articles'
   own authors. Candidates pooled from BM25, TF-IDF and PubMed's related
   articles, which favours lexical methods. PMIDs; CC0. BM25, relevant
   against not: MRR 0.878, P@5 0.642. *The paper's own figures, on its
   2,233 analysed seeds, are MRR 0.859 and P@5 0.622 (P, §2.4).*
9. **SciRepEval** (Ai2, 2022, P,
   [repository](https://github.com/allenai/scirepeval),
   [test sets](https://huggingface.co/datasets/allenai/scirepeval_test)).
   Search: 2,585 test queries with ten candidates each from Semantic
   Scholar's click logs, graded 0–10; candidate ids appear to be S2 corpus
   ids (U); no BM25 figure. Proximity tasks: RELISH (3,190 queries,
   191,245 candidates), SCIDOCS cite, co-cite, co-view and co-read, highly
   influential citations, same author, paper-reviewer matching. ODC-BY.
   The search task reranks ten candidates, so it never tests a first
   stage; the proximity tasks repeat RELISH and SCIDOCS.
10. **DORIS-MAE** (NeurIPS 2023, P,
    [Zenodo](https://doi.org/10.5281/zenodo.8299749)). Complex multi-aspect
    CS questions — 100 in the paper, 50 in the README — over 363,133
    arXiv CS abstracts of 2011–2021, about 100 candidates a query,
    relevance 0/1/2 per aspect annotated by GPT-3.5 and summed; metadata
    with URL and S2 citation data; arXiv id as key (U). CC BY-NC. BM25:
    R@5 8.47, R@20 30.50, NDCG_exp@10% 13.23. Labels written by a model on
    a small pool.
11. **CSFCube** (2021, P,
    [repository](https://github.com/iesl/CSFCube)). Query by example
    with a facet (background, method, result): 50 query papers from CL and
    ML venues, graded pools from about 800,000 S2ORC abstracts; the split
    across facets U. CC BY-NC 4.0. No BM25 figure. Niche: the facets need
    sentence labels.
12. **BRIGHT** (2024, P). Reasoning-heavy questions over web passages: 12
    subsets, 1,384 queries; the science-flavoured ones Biology 103, Earth
    Science 116, Economics 103, Psychology 101, Sustainable Living 108.
    Documents are chunks of web pages linked from StackExchange answers,
    with no DOI or PMID. CC BY 4.0. BM25 nDCG@10 on those five: 18.9,
    27.2, 14.9, 12.5, 15.0. Cannot be mapped onto works.
13. **Local citation recommendation** (Gu et al., ECIR 2022, P,
    [repository](https://github.com/nianlonggu/Local-Citation-Recommendation)).

    | dataset | test contexts | papers | BM25 R@10 | BM25 MRR |
    |---|---|---|---|---|
    | ACL-200 | 9,585 | 19,776 | 0.263 | 0.138 |
    | FullTextPeerRead | 6,814 | 4,837 | 0.328 | 0.185 |
    | RefSeer | 126,593 | 624,957 | 0.189 | 0.099 |
    | arXiv | 104,401 | 1.66 M | 0.222 | 0.118 |

    ACL-200 uses ACL Anthology ids (P); RefSeer's ids and the licences U.
    **unarXive 2022** (P): 1.9 million arXiv full texts, 28 of 63 million
    references linked to OpenAlex, MIT; its `saier/unarxive_citrec` split
    has 225,348 test contexts with the cited work as an OpenAlex `W` id,
    CC BY-SA 4.0. The best of the group for this corpus. Successors:
    CiteME (2024; 130 CS excerpts answered by a title) (S); CiteBART (EMNLP
    2025) reuses the classic sets (P).
14. **TREC BioGen, 2024–2025** (P). 2024: 65 topics over 20,727,695
    PMIDs of the 2023 PubMed baseline, cited PMIDs judged supports,
    contradicts, neutral or not relevant. 2025: task A 40 questions and 194
    sentences, task B 30 topics, 26.8 million PMIDs; a BM25 baseline for
    task A at 51.03 % precision on "supports". Whether the qrels are
    published is U. Judged per cited sentence, not as ranked pools, so
    weak for search.
15. **BioASQ Task B** (P,
    [participants' area](https://participants-area.bioasq.org)). Task
    13b: 5,389 training questions averaging 9.74 gold PMIDs; test 4 × 85
    questions; up to ten documents an answer, scored by MAP; expert gold
    not exhaustive. Registration; CC BY 2.5. A good PMID question set with
    under-judged recall.
16. **LongEval-Sci** (CLEF 2025, continued 2026, P,
    [data](https://researchdata.tuwien.ac.at/records/v8phe-g8911)). Real
    queries from CORE's search. 2025: training 393 queries, 4,262
    click-model qrels, about 2.01 million documents; test 492 new and 99
    held-out queries over more than a million. Documents by CORE id (S),
    a DOI field U. CC BY 4.0. The best 2025 runs were BM25 at nDCG@10
    about 0.29 on the 2025-01 snapshot; 2026 has three snapshots of 2025
    and an official BM25 over title and abstract falling from 0.270 to
    0.140. Real queries, shallow click labels, CORE ids to match to DOIs.
17. **PaperFindingBench** (AstaBench, ICLR 2026, P). 267 test and 66
    validation CS queries — 48 navigational, 43 metadata, 242 semantic;
    true gold for the first two, the semantic ones judged by a model
    against criteria with recall estimated. The Asta Scientific Corpus
    (Semantic Scholar), cutoff 2025-06-01; gated on Hugging Face (no
    redistribution, no training on test). Needs Semantic Scholar's APIs
    and a model judge.
18. **Others found.**
    - **SR4CS** (2026, P, [Zenodo](https://doi.org/10.5281/zenodo.17163932)):
      1,212 CS systematic reviews with expert Boolean queries, 104,316
      references resolved by DOI first; CC BY 4.0; BM25 MAP 0.175, R@100
      0.262.
    - **PMC-Patients ReCDS-PAR** (P): a patient summary → PMIDs, about
      6,000 test queries with about 19 relevant each, grades 1–2; CC
      BY-NC-SA 4.0; BM25 nDCG@10 15.28, R@1k 30.64 over the arXiv
      version's 1.4 million articles, the current release 11.7 million.
    - **The Seed Collection** (SIGIR 2022, P,
      [repository](https://github.com/ielab/sysrev-seed-collection)): 40
      medical reviews with seed, included and retrieved studies as PMIDs,
      and Boolean queries — search that starts from a few known works.
    - **ScholarQuest** (2026, P): 1,111 CS queries, arXiv ids, judged by
      a model on three grades, a fixed corpus of about a million papers.
    - **PaSa RealScholarQuery** (2025, S): 50 AI queries.
    - **LitQA2-FullText-Search** (P): 75 biology questions, one target
      paper each, recall@30.

### 2.3 Poor fits

BRIGHT (web pages, not works); SCIDOCS and SciFact (small pools on
Semantic Scholar ids); PaperFindingBench (hosted tools and a model
judge); DORIS-MAE (labels written by a model); SciRepEval's search task
(reranks ten candidates); TREC BioGen (per-sentence judgments).

### 2.4 More sets, surveyed 2026-09-26

A third survey (page reads, web searches and public APIs; no dataset
downloaded, nothing registered, a few small PDFs and one qrels file read),
made when the user asked, of the [evaluation plan](../evaluation-plan.md),
whether more sets like TREC's and RELISH are "worth our time" — breadth
across fields mattering, since the user is "opposed to optimizing
exclusively around" any one narrow set. Kinds: T topical, K known-item, D
query by document, R recall (systematic reviews), C clicks.

**The starter set, checked.**

- **TREC Precision Medicine.** Topics, trec_eval qrels and judgment CSVs
  are open on trec.nist.gov with no login (P, fetched); no licence stated
  (P, by absence). 2017: 30 topics, 22,642 judgments; 2018: 50, 22,429
  (P, ir_datasets); both over 26.76 million MEDLINE abstracts plus AACR
  and ASCO abstracts without PMIDs (P). **2019: 40 topics** — 30 cancer
  from MD Anderson, 10 genetic disease (P, overview) — over the "MEDLINE
  2019 baseline", 29,138,916 abstracts of mid-December 2018 (P); the
  overview's per-topic tables sum to 18,316 judgments (2,571 definitely,
  2,973 partially, 12,772 not relevant), its text repeating 2018's 22,429
  (P); topics 5 and 6 have nothing definitely relevant. **2020: 40
  topics** over the same corpus, but the phase-1 qrels cover **31**, nine
  dropped for two or fewer relevant (P, NIST's page); phase 2 grades
  evidence strength (P). The "sample" qrels are for inferred measures
  through NIST's `sample_eval.pl`; the trec_eval qrels are the ones for
  this plan. No numeric BM25 figure in the 2019 overview (P); a run named
  BM25 sits near 0.45 median infNDCG on a box plot (U). Every year is
  precision oncology, 2019's genetic diseases aside: narrow by itself.
- **RELISH.** One figshare item, `RELISH_v1` (DOI
  10.6084/m9.figshare.7722905.v1), one file, `RELISH_v1.json.gz`, 978,490
  bytes, MD5 `64e93f32da691bc756a25a3fa8e44451`, CC0 (P, figshare API; its
  web pages refuse automated reads). JSON records: `uid`, `pmid` (the
  seed), `experience`, `is_anonymous`, `response` with candidate PMIDs
  under relevant, partial, irrelevant (P). Seeds: 3,278 by the file's
  description, 3,017 annotated and 2,233 analysed by the paper (P). The
  60 candidates are the top 20 each of BM25, TF-IDF and PubMed's related
  articles, round-robin, over 18,345,070 PubMed articles of 2018-07-03
  (P); 91% of seeds judged by one of their own authors (P). The paper's
  BM25 on the 2,233 seeds is MRR 0.859 and P@5 0.622 (P), not §2.2's
  0.878 and 0.642, whose subset is unknown. The paper's second copy,
  relishdb.ict.griffith.edu.au, no longer resolves: figshare's is the only
  one.
- **TREC-COVID.** The collection to use is "TREC-COVID Complete",
  `qrels-covid_d5_j0.5-5.txt`, with `topics-rnd5.xml` and
  `docids-rnd5.txt` (P, ir.nist.gov); no submission was scored on it, and
  NIST warns against training on one round's qrels and testing on a
  later's (P). 69,318 judgments on three grades (P). Per-round qrels over
  five CORD-19 releases make a small time-slice test (P). CORD-19 of
  2020-07-16: 192,509 documents (P), `metadata.csv` with DOI, PMC id and
  PMID per document (P), under a licence "for text and data mining only"
  (P); a crosswalk of ids reads as within it (a reading, not advice).

**New candidates.**

| set | kind · field | queries · judgments | judged by | ids | access | BM25 | verdict |
|---|---|---|---|---|---|---|---|
| TREC Genomics 2004–05 ad hoc | T · genomics | 50 + 50; about 975 judged a topic in 2004, pool top 75 of 27 runs; three grades (P) | pooled, biologists (P) | PMID (P) | open, no password (P) | TF·IDF MAP 0.2343 in 2004 (P) | now |
| TREC Clinical Decision Support 2014–16 | T · clinical | 30 a year; about 37,800 qrels a year; three grades (P) | pooled clinicians (P) | PMC id (P), to PMID through PubMed (S) | open (P) | U | now; judged on full text, so abstracts score low |
| MeSH as query (Kim et al., BioNLP 2018) | T, pseudo-judged · biomedicine | 1,735 multi-word MeSH headings (P) | indexing: a work carrying the heading is relevant (P) | PMID (P) | the paper's files are gone (404, P); rebuilt from our own PubMed records | Solr BM25 MAP 0.305 over PubMed (P) | now, built in-house |
| SYNERGY (ASReview) | R · medicine, psychology, CS, biology, chemistry, mathematics | 26 reviews, 169,288 records, 2,834 included (P) | the reviews' own inclusions (P) | OpenAlex id and DOI (P) | CC0 (P) | none | now for medicine by DOI; whole with OpenAlex |
| OAG-QA (OAG-Bench) | K-ish · 22 fields, mathematics to economics | 17,948 questions, 87 topics, one gold paper typical (P) | the paper cited in a StackExchange or Zhihu answer (P) | OAG/AMiner, by title or DOI (U) | mirrors (S), licence U | U | later |
| iSearch / iSearch++ (2026) | T · physics | 65 topics by 23 physicists (S), four grades (S) | the topic's author (S); ++ adds model re-judgments (P) | arXiv id map (P) | assessments on Zenodo, licence unstated (P); texts by emailed request (P) | U | later |
| Reference strings to DOIs (Crossref's method) | K · every field | 2,000 real strings and 7,374 in 11 styles (P) | algorithms and hand checks (P) | DOI | framework MIT (P), data U | n/a | later, built from Crossref's own references |
| TripClick / TripJudge | C and T · health | 3 × 1,175 test queries; TripJudge 1,136 queries, 12,590 pairs, four grades (P) | clicks; students, κ 0.63 (P) | Trip ids with PubMed URL and DOI (P) | form emailed to Trip, non-commercial, no sharing (P) | nDCG@10 0.199–0.267 click, 0.570 judged (P) | later, the user's call |
| SPARBench (2025) | T · CS, medicine | 50 queries, 556 relevant (P) | a model, then experts (P) | S2 and arXiv (P) | MIT (P) | U | later |

Waiting on full text, not dropped: TREC Genomics 2006–07 (judged on
passages of full text; aspect judgements, U). The user, 2026-09-28:
"full text is a goal to include, it's just not here yet. full text is the
gold standard!" A set judged on full text is kept for when full text is
in the corpus, never dropped for being full text.

Kept, not dropped for their language; what stands in the way is access,
the user's call: CLEF Domain-Specific (German and Russian; a fee, no
DOIs); NTCIR-1/2 (Japanese records; a mailed agreement). The user,
2026-09-28: "'it is not in english' is no reason to exclude something of
use". The index already facets works by language (P, `/language/` in
`crates/search/src/schema.rs`).

Kept, not dropped for their field: CLEF SimpleText (computer science;
a registration); ACM-CR, MAPLE, SciNet (recorded as "tiny or one field":
one field is no reason, so each is to be re-checked for size alone). The
user, 2026-09-28: "'it is not biomed' is no reason to exclude something
of use. biomed has a broad potential customer base, but academics study
all kinds of things, and it's not just pharmas that need support", and
"pubmed was just a convenient place to start". A set outside biomedicine
waits on the corpus reaching its field (Crossref, OpenAlex, arXiv), not
on its field.

Not worth it, with the reason: OHSUMED (no MEDLINE-UI-to-PMID crosswalk found, shallow pools,
research use only); TREC-CHEM and
CLEF-IP (patents); NASA-IR (paragraphs as gold); pathfinder
(unreleased); ChemRxiv Retrieval (queries written by a model); TREC Fair
Ranking (reranking); Multi-XScience (the citation scoreboard does the
same at scale); SciFact-Open (claims on S2 ids); CheckThat 2025 4b (COVID
only, a pool of 7,718); PubMed's search logs (NLM: "Due to privacy
concerns, we are not able to release the PubMed search logs", P).

**The gap.** No judged set over papers in the humanities, law,
engineering, materials or earth science turned up, beyond OAG-QA's
topics and BRIGHT's web pages. Breadth there comes from sets built out
of the corpus: citations, reference strings, and indexers' vocabularies
as queries — MeSH now; ERIC and zbMATH Open are ideas not checked (U).

**MeSH as query, and one condition.** The normalized PubMed records keep
their MeSH headings (`source-pubmed`'s `mesh`), and the index does not
search them (`crates/search/src/schema.rs`), so a suite of headings as
queries asks whether text finds what indexers tagged, without the answer
in the index. Indexing MeSH as text — one of serving plan §11 item 11's
open questions — would put the answer key in the index and end the
suite's use.

## 3. Deep research: what can be reproduced

### 3.1 The split

Mostly not as published, but along a useful line: **finding the right
works usually can be reproduced; writing the answer usually cannot.**
Answers are graded by commercial models that change, and agents search
the live web, which drifts; but several benchmarks publish their gold
works by id, and whether they were found is arithmetic.

### 3.2 The benchmarks, one by one

1. **AutoResearchBench** (P: [paper](https://arxiv.org/html/2604.25256),
   [repository](https://github.com/CherYou/AutoResearchBench),
   [DeepXiv SDK](https://github.com/DeepXiv/deepxiv_sdk)). 1,000 CS
   queries with gold, Apache 2.0 on Hugging Face, lightly obfuscated and
   decrypted by a script: 600 Deep (one paper to find; 10 % with no
   answer) and 400 Wide (about 9.2 gold papers each). Gold as arXiv ids;
   Deep scored by exact match, Wide by overlap (IoU), both deterministic;
   models and human checks only built the Wide gold. Retrieval through
   DeepXiv, a hosted arXiv search synced daily with a free token of 1,000
   requests a day, or Jina web search; the corpus through April 2026 per
   the paper, no cutoff found enforced in the tool (U). Best:
   Claude-Opus-4.6 9.39 % on Deep, Gemini-3.1-Pro 9.31 % IoU on Wide.
   **Reproducible with caveats:** the date can be pinned by the arXiv id's
   year and month (up to 2604); its field is computer science, and arXiv
   is not ingested yet.
2. **LAB-Bench's LitQA2** (P:
   [dataset](https://huggingface.co/datasets/futurehouse/lab-bench)). 248
   multiple-choice questions with an "insufficient information" option,
   scored by accuracy and precision; about 80 % public (199 questions,
   CC BY-SA 4.0), 20 % held out against contamination. Each public item
   carries DOI links and a key passage; the questions are built so
   abstracts cannot answer them. Human experts: 70 % accuracy.
   **Reproducible with caveats:** answering needs full text and a fifth is
   hidden; finding the work by DOI is scoreable on this corpus.
3. **LABBench2** (P: [paper](https://arxiv.org/abs/2604.09554),
   [harness](https://github.com/EdisonScientific/labbench2)). About 1,900
   tasks, gated, CC BY-SA 4.0; literature tasks include litqa3 (168
   open-response questions), sourcequality and patentqa; no private split
   mentioned. Open responses graded by a model judge defaulting to Claude
   Sonnet 4.5, prompts in the harness; models bring their own tools,
   mainly live web search. DOIs appear for SourceQualQA (P, weak);
   whether litqa3 has DOIs, and best scores in text, U. **Reproducible
   with caveats:** a closed judge and the live web.
4. **AstaBench** (P: [paper](https://arxiv.org/abs/2510.21652),
   [repository](https://github.com/allenai/asta-bench)). Literature tasks
   (test/validation, cutoff): PaperFindingBench 267/66, CS, 2025-06-01;
   LitQA2-FullText and LitQA2-FullText-Search 75/10 each, biology,
   2024-10-17; ScholarQA-CS2 100/100, 2025-05-01; ArxivDIGESTables-Clean
   100/70. The date pinning is a filter on a hosted tool service backed by
   Semantic Scholar, behind a key requested by form; no downloadable
   snapshot found (U). Gated dataset, no redistribution or training on
   test; the scorer downloads the test rubrics, so test gold is not
   hidden. Paper-finding gold as S2 CorpusIDs. Judges: gpt-4o-2024-11-20
   for relevance; the repository's default for ScholarQA-CS2 now
   google/gemini-3-flash-preview. Best: Asta Scholar QA, Elicit and
   SciSpace at about 85 % or more on ScholarQA-CS2; Asta Paper Finder more
   than double ReAct on PaperFindingBench. **Reproducible with caveats:**
   hosted tools behind a key, closed judges that have changed, CorpusIDs
   to map.
5. **ScholarQABench** (P:
   [repository](https://github.com/AkariAsai/ScholarQABench)). Seven sets,
   ODC-BY: SciFact 208, PubMedQA 843 and QASA 1,375 (single-paper);
   ScholarQA-CS 100 with expert rubrics; ScholarQA-Multi 108 with expert
   answers; ScholarQA-Bio 1,451 and ScholarQA-Neuro 1,308 with questions
   only. Graded by an OpenAI judge on the CS rubrics (the model string U),
   Prometheus v2 for organisation and coverage, citation precision and
   recall, accuracy or ROUGE for the single-paper sets, and 16 experts.
   No fixed retrieval: OpenScholar used its downloadable datastore (45
   million papers to October 2024, about 744 GB) with the live Semantic
   Scholar API and You.com. OpenScholar-8B beats GPT-4o by 5 % and
   PaperQA2 by 7 % on correctness. **Reproducible with caveats:** the
   biomedical multi-paper questions have no gold.
6. **SciArena** (P: [paper](https://arxiv.org/abs/2507.01001),
   [data](https://huggingface.co/datasets/yale-nlp/SciArena-with-paperbank)).
   A live arena: researchers ask, a ScholarQA-style pipeline retrieves from
   Semantic Scholar, two models answer, humans vote, Elo ranks. MIT: about
   13,200 votes with both responses, a 2,000-item SciArena-Eval for
   testing judges, and a version with the retrieved papers. o3 is the best
   judge at 65.1 % agreement with humans and led at launch. **The arena is
   not reproducible;** SciArena-Eval is, and tests judges, not retrieval.
7. **BrowseComp and BrowseComp-Plus** (P:
   [BrowseComp-Plus](https://github.com/texttron/BrowseComp-Plus)).
   BrowseComp: 1,266 short-answer questions released encrypted in OpenAI's
   simple-evals with a canary; the live web; an LLM checks equivalence to
   the reference (prompt in the appendix; simple-evals uses
   gpt-4.1-2025-04-14); Deep Research 51.5 % at release (P), over 90 %
   now per aggregators (S). **Not reproducible.** BrowseComp-Plus: 830 of
   the queries over a fixed corpus of 100,195 web documents, MIT, not
   gated; evidence, gold and hard-negative ids per query; BM25 and
   Qwen3-Embedding indexes prebuilt; Qwen3-32B as an open-weights judge.
   GPT-5 with Qwen3-Embedding-8B 70.1 %; Search-R1 with BM25 3.86 %.
   **Reproducible as published,** but general web trivia with no scholarly
   ids.
8. **DeepResearch Bench** (P: [paper](https://arxiv.org/abs/2506.11763)).
   100 doctoral-level report tasks, 50 Chinese and 50 English; queries,
   reference reports and criteria public; the live web. Report quality
   (RACE) by a model judge the README says moved from Gemini-2.5-Pro to
   GPT-5.5 in May 2026, so older and newer scores do not compare; citation
   checking (FACT) fetches the cited pages through Jina. **Reproducible
   with caveats:** the live web and a changed closed judge.
9. **DeepResearchGym** (P: [paper](https://arxiv.org/abs/2505.19253)).
   1,000 Researchy Questions; a free search API over ClueWeb22 (licensed)
   and FineWeb (more than 180 million documents), with code to host it;
   judge gpt-4.1-mini-2025-04-14, checked against humans; Perplexity
   sonar-deepresearch best at 72.5 key-point recall. **Reproducible with
   caveats:** a closed judge, ClueWeb's licence, web pages not papers.
10. **FutureSearch's Deep Research Bench** (P:
    [paper](https://arxiv.org/abs/2506.06287)). 89 tasks, private but for 8
    examples; the RetroSearch frozen web snapshot internal; o3 best at 0.51.
    **Not reproducible.**
11. **ResearchQA** (P: [paper](https://arxiv.org/abs/2509.00496)). 21,400
    queries and about 160,000 rubric items mined from 54,000 surveys (found
    through Crossref, Semantic Scholar and S2ORC) across 75 fields, 7,500
    in the health sciences; MIT on Hugging Face; test 3,750 queries,
    test_mini 776. A gpt-4.1-mini judge on rubric coverage; a pairwise
    judge agrees with experts 74 % of the time; systems bring their own
    retrieval; no gold paper ids found (U). Perplexity deep research best
    at 75.29 % coverage. **Reproducible with caveats:** a closed judge, and
    it scores answers, not retrieval.
12. **SAGE** (P: [paper](https://arxiv.org/abs/2602.05975)). 1,200
    queries across CS, natural science, healthcare and humanities over a
    corpus of 200,000 open-access papers, scored by exact match and
    weighted recall; BM25 81.2 % exact match on short questions. Whether
    the corpus text and its ids are released is U.
13. **ScholarGym** (P: [paper](https://arxiv.org/abs/2601.21654)). 2,536
    queries over 570,000 arXiv papers, deterministic recall and precision;
    CS, physics and mathematics, over arXiv, which is not ingested yet.
14. **BioASQ Task B and Synergy** (P:
    [Task 14b](https://participants-area.bioasq.org/general_information/Task14b/),
    [overview](https://pmc.ncbi.nlm.nih.gov/articles/PMC10042099/)). Expert
    questions — yes/no, factoid, list, summary. Phase A returns up to ten
    documents and snippets from the 2026 PubMed baseline; phase B answers
    from the gold documents. Documents as PubMed URLs, so PMIDs. MAP and
    F-measure for documents and snippets; macro-F1, MRR and F1 for the
    answer types; ROUGE and manual scores of 1–5 for summaries. Training
    questions (5,389) need a free registration; BioASQ-QA (4,721
    questions) is on Zenodo under CC BY 4.0. Gold built by pooling, so
    unfound relevant PMIDs count against a system. Synergy runs rounds
    against designated snapshots with experts judging only what was
    submitted. **Task B reproducible with caveats** (incomplete pools,
    manual summary scores); **Synergy not reproducible** offline.

### 3.3 A shortlist for this corpus

1. **BioASQ Task B:** the only one built on PubMed, with PMID gold, a named
   baseline, deterministic document metrics and an openly licensed copy.
2. **LitQA2's public split:** DOI gold makes finding the work scoreable on
   Crossref and OpenAlex, dated by AstaBench's cutoff of 2024-10-17;
   answering still needs full text.
3. **ResearchQA:** large, MIT, heavy in the health sciences; a closed
   judge, and answer quality rather than retrieval.
4. **AutoResearchBench:** deterministic, pinnable by arXiv id; useful only
   if OpenAlex brings arXiv CS.
5. **BrowseComp-Plus:** fully frozen with an open judge — for checking an
   agent loop and BM25 against published figures, not the corpus.

Not reproducible: BrowseComp, FutureSearch's bench, SciArena's arena,
BioASQ Synergy. Only with a commercial judge and live search: DeepResearch
Bench, DeepResearchGym, ResearchQA, LABBench2's literature tasks,
AstaBench.

## 4. In practice

A reading, 2026-09-16, not findings.

- **The sample cannot run these meaningfully.** It holds 82,744 held
  works; most gold works are not among them.
- **The full PubMed baseline alone unlocks most of the starter set.**
  TREC-COVID (through its PMIDs), TREC Precision Medicine, RELISH, CLEF
  TAR and BioASQ are PMID-based. PubMed is the smallest of the three
  dumps, which makes it a natural first full-size run that gives a
  scoreboard at the same time.
- **Each benchmark pinned to its snapshot** (§1): filtered by publication
  date to its baseline for comparable runs, deletions since noted.
- **Beside the citation scoreboard** of future ambitions §5.2: the
  benchmarks give figures comparable with others; the scoreboard measures
  the corpus as it is.
- **Reported as a frontier.** Cost against quality, repeated runs,
  bootstrap intervals, a fixed price list and a labelled harness, not one
  accuracy figure; and the citation labels as a nearly free way to profile
  retrieval configurations for a per-query router ([configuration per
  query](future-ambitions-configuration.md) §4.3, §4.5).

## 5. Open questions

1. Whether BioASQ's account and its training set are worth it over the
   Zenodo copy.
2. How much of arXiv arrives through OpenAlex, which decides unarXive,
   LitSearch and AutoResearchBench.
3. How LitSearch's `corpusid` would be mapped without Semantic Scholar's
   key, if at all.
4. Which measure tolerating unjudged documents to report beside the
   standard ones (§1).
