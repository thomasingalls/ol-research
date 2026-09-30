# Research report 2: teaching a method, the evidence outside

**What this is:** the final report of a web-research subagent, delivered
on 2026-09-28 to the conversation that wrote
[teach it your method](../teach-it-your-method.md), and kept verbatim,
following the practice the user asked for with
[searching within a result set](../search-within-results.md)'s reports.
It is model output, not checked line by line. It used 112 tool calls
over about 18 minutes. Its web-search budget was already spent when it
began, so it fetched primary pages directly and queried the Europe PMC
and arXiv APIs; its "not found" is weaker than usual. Its tags are its
own: [P] it read the primary text, docs or results; [A] abstract or
landing page only; [K] its prior knowledge, not re-checked. One line in
§2 is its own arithmetic ("My arithmetic"), an estimate the note leaves
out. The prompt it was given follows the report.

---

**How I searched.** The WebSearch budget for this session was already used up (200 of 200) when I started. Everything below comes from fetching primary URLs I knew directly, plus the Europe PMC and arXiv APIs. So a "not found" means not found in those sources; a full web search might turn something up. Tags: [P] = I read the primary text, docs or results; [A] = abstract or landing page only; [K] = my prior knowledge, not re-checked this session. No [S] tags appear because no search-engine summaries were used.

## 1. Existing tools

- **Elicit Systematic Review** (elicit.com/blog/how-we-evaluated-elicit-systematic-review, Mar 2025; /blog/evaluating-elicit-slr, May 2026; support.elicit.com/en/articles/14759154) [P, vendor's own evaluation]
  - **How it works:** an LLM drafts screening criteria from the research question. The user can edit, disable or add criteria, and override decisions. Overrides are recorded, but no document says they train anything, so it follows instructions rather than learning from marks.
  - **Scope:** it searches 138 M papers, but screening is capped per review at 5k / 20k / 40k papers depending on plan.
  - **Numbers:** 96.9% sensitivity and 92.5% specificity against 994 Cochrane reviews (2026). The 2025 figures were 93.6% / 62.8%, and about half the misses were traced to badly generated criteria.
  - **Reproducibility:** "Keyword searches are 100% reproducible." Nothing is said about whether LLM screening gives the same result on a rerun, about versioning criteria, or about alerts.
- **ASReview** (van de Schoot et al., arXiv 2006.12166; published in Nature Machine Intelligence 2021 [K]) [P]
  - Active learning on the user's labels, over an imported set. It supports updating a review: the included papers from the first run train the model before the updated set is screened.
  - It has a simulation mode and is open source and runs locally.
  - Across 4 datasets (one from software engineering), average work saved at 95% recall was 83% (range 67–92%), i.e. 95% of relevant studies found after screening 8–33% of records.
- **Rayyan** (Ouzzani et al., Systematic Reviews 2016) [A]
  - A prediction model starts making suggestions after 5 included studies, working on an imported set.
  - The current site advertises "zero-shot relevance ratings" and an "AI Reviewer" but does not describe the mechanism [P].
- **Covidence** (covidence.org blog) [P]
  - "Most relevant" sorting is learned from reviewers' votes. It needs at least 25 decisions, with at least 2 includes and 2 excludes.
  - It "does not remove any studies" from screening.
  - I could not verify whether it integrates Cochrane's RCT classifier.
- **DistillerSR** (distillersr.com) [P, vendor]
  - Features listed: AI Re-Rank learned from team decisions ("up to 70%"), AI classifiers, generative-AI "Smart Screening", and an audit log "with version control".
  - Hamel et al., BMC Med Res Methodol 2020 [A]: median 47.1% less screening at 95% recall across 10 reviews.
  - Gates et al., Systematic Reviews 2019 [A]: fully automated mode saved more work but "came with increased risk of missing relevant records".
- **EPPI-Reviewer** (eppi.ioe.ac.uk/cms/er4/Help/…) [P]
  - Priority screening (active learning), plus six built-in classifiers: Cochrane RCT (99% recall), systematic review, economic evaluation, COVID-19, Long COVID, and PubMed study type.
  - **OpenAlex auto-update:** machine learning finds new records "similar to the ones already included" across OpenAlex (260 M+ records), each time OpenAlex publishes its roughly monthly snapshot of changes. Users can then apply "their own, or pre-built classifiers". Of everything I found, this is the closest to "run my method over the whole corpus and each new batch".
  - Shemilt et al., Wellcome Open Research 2021/2024 [A]: the Microsoft Academic Graph plus machine-learning workflow for a living COVID map found 469 more eligible articles at one month and cost £3,179 per week less than Boolean searches of Medline and Embase.
  - LLM coding: "evaluation purposes only… not recommend its use in real reviews" without formal accuracy testing. It records model versions (e.g. GPT-4o 2024-08-06).
- **Nested Knowledge** (about.nested-knowledge.com/docs/…) [P] offers both approaches side by side:
  - A **Screening Model**: gradient-boosted trees on metadata, abstract, citations and OpenAI embeddings, trained on the review's own decisions once it has 50 screened and 10 included records. Its model card reports AUC 0.88, recall 0.76 and precision 0.40 across hundreds of reviews. It powers a "Robot Screener" that acts as the second reviewer.
  - A separate **Smart Screener**: an LLM answers yes/no for each user criterion "without prior training", capped at 5,000 abstracts.
- **Laser AI** [P, marketing]: "AI-driven prioritization", "up to 45%" less screening time; no mechanism described.
- **Semantic Scholar Research Feeds** (semanticscholar.org/faq) [P]
  - A contrastively trained paper-embedding model. Papers saved in a Library folder are the positives, and "not relevant" ratings improve the feed.
  - Refreshed daily from papers published in the last 3 months, across all fields.
  - No measurement or versioning is exposed to the user.
- **arxiv-sanity-lite** (github.com/karpathy/arxiv-sanity-lite) [P]: one SVM per user tag over tf-idf of abstracts, with a daily email. arXiv only; the live instance holds about 30k papers.
- **Google Scholar alerts** (help page) [P]: alerts on a query, on new citations, or on new articles by an author. Nothing learned from the user's marks is documented.
- **Iris.ai** [P]: now an enterprise "AI Harnessing" product. I found no user-trainable criteria.
- **Relativity:** its documentation URLs returned 404, so I have not verified how its active learning works.
  - Grossman & Cormack, Richmond Journal of Law & Technology 17(3), 2011 [P, abstract and introduction]: technology-assisted review at the TREC 2009 Legal Track gave recall and precision better than exhaustive manual review.

## 2. Prompted LLMs vs trained small classifiers

- **Bucher & Martini**, arXiv 2406.08660 (2024) [A]: fine-tuned BERT-style models "significantly outperform" zero-shot GPT-3.5, GPT-4 and Claude Opus across sentiment, stance and similar tasks.
- **TnT-LLM** (Wan et al., Microsoft), arXiv 2403.12173 (2024; I think it appeared at KDD 2024, unverified) [P]
  - GPT-4 labels were used to train logistic regression, LightGBM and MLP classifiers on ada2 or Instructor embeddings.
  - On a human-labelled test set, ada2 + logistic regression scored 0.658 accuracy against GPT-4's 0.655.
  - On multilabel tasks the small models had higher precision and lower recall than GPT-4.
- **SetFit** (Tunstall et al., arXiv 2209.11055; huggingface.co/blog/setfit) [P]
  - With 8 labelled examples per class it is competitive with RoBERTa-large fine-tuned on the full 3k training set (Customer Reviews dataset).
  - Training took 30 s on a V100, about $0.025, against about $0.70 for T-Few 3B.
- **Sensitivity to prompt wording and format:**
  - Sclar et al., ICLR 2024 [A]: formatting alone moved accuracy by up to 76 points (LLaMA-2-13B).
  - Reiss, arXiv 2304.11085 (2023) [A]: ChatGPT's classifications were inconsistent across repeats and across wording changes; unsupervised use "not recommended".
  - Dennstädt et al., Systematic Reviews 2024 [A]: sensitivity 81.9–97.6%, specificity 19.1–75.2%, and "minor adaption" of the setup mattered.
  - Delgado-Chaves et al., PNAS, Jan 2025 [A]: 18 LLMs on 3 reviews cut one reviewer's workload by 33–93%; performance depended on how clearly the criteria were written.
- **Systematic-review screening numbers:**
  - Tran et al., Annals of Internal Medicine 2024 [A]: GPT-3.5 on 22,665 citations from 5 reviews; sensitivity 81.1–96.5%, specificity 25.8–80.4%.
  - Oami et al., JMIR Medical Informatics 2025 [A]: GPT-4 Turbo specificity 0.98 against GPT-3.5's 0.51, with similar sensitivity (about 0.83–0.85). Changing the model generation changes the result.
  - Vivekanantha et al., Current Reviews in Musculoskeletal Medicine 2025 [A]: GPT-5 cost $0.002–0.0036 per abstract. My arithmetic: at 30 M works that is $60k–108k for each full pass.
  - Wang et al., ECIR 2024 (arXiv 2401.06320) [P]: zero-shot LLMs calibrated to a recall target and ensembled with BioBERT reached about 93% recall at a 95% target, saving 50–54% of screening work, against 26–34% for BioBERT alone. They estimated GPT-4 would have cost about $80,000 for their experiments.
  - Lieberum et al., J Clin Epidemiol 2025 scoping review [A]: 37 studies; "fully established or validated applications are often lacking".
- **Not found:** distilling LLM labels or using SetFit specifically for systematic-review screening, in my Europe PMC and arXiv queries.

## 3. Drift and nondeterminism

- **Chen, Zaharia & Zou**, arXiv 2307.09009 [A]: GPT-4 accuracy at identifying primes fell from 84% to 51% between the March and June 2023 versions. I believe it was later published in Harvard Data Science Review in 2024, unverified.
- **Staudinger et al.**, SIGIR-AP 2024 [P]: they could not reproduce ChatGPT Boolean-query results for systematic reviews. Results were "neither reproducible nor reliable when rerunning the same prompts with different seeds", and they "do not consider any of the obtained results reliable enough" for systematic reviews.
- **Atil et al.**, arXiv 2408.04667 [A]: 5 LLMs, 8 tasks, 10 runs each at "deterministic" settings. Accuracy varied by up to 15%, and the gap between best and worst possible runs was up to 70%.
- **Horace He / Thinking Machines**, Sept 2025 [P]: nondeterminism at temperature 0 comes from kernels that are not batch-invariant. Qwen3-235B gave 80 distinct completions in 1,000 runs; with batch-invariant kernels it gave 1.
- **Not found:** any study measuring run-to-run or model-version drift specifically for screening.

## 4. Reproducibility and updating searches

- **PRISMA-S** (Rethlefsen et al., Systematic Reviews, Jan 2021) [P]: 16 items. Item 8: search strategies "copied and pasted exactly as run". Item 10: cite any published filters used. Item 11: cite prior reviews whose strategies were reused. Item 12: report update methods ("rerunning searches, email alerts"). Item 13: date of the last search.
- **Living reviews:**
  - Elliott et al., J Clin Epidemiol, Nov 2017 [A]: reviews "continually updated, incorporating relevant new evidence as it becomes available".
  - Synnot et al. (CENTER-TBI) [A]: automated search updates every three months.
- **Cochrane RCT classifier** (Thomas et al., J Clin Epidemiol 2021, PMC8168828) [P]
  - An ensemble of two SVMs with Platt scaling, trained on 280,620 Embase records and calibrated to 99% recall.
  - It retrieved 43,783 of 44,007 RCTs (99.5%) that were included in Cochrane reviews.
  - It discards 58% of records automatically on the Clinical Hedges set; in 2018 it spared Cochrane Crowd 185,039 of 449,480 records.
  - The pipeline runs classifier, then Crowd, with Screen4Me for individual reviews.
- **Related evaluations** [A]:
  - Screen4Me: 100% sensitivity, 80.71% specificity (J Clin Epidemiol 2021).
  - Centralised Search Service: captured 634 of 650 included references, 97.5% (J Clin Epidemiol 2020).
  - COVID register classifier: 98.9% recall with a 24.1% net workload reduction (Systematic Reviews 2022).

## 5. Search filters as shareable artifacts

- **ISSG Search Filters Resource** (sites.google.com/a/york.ac.uk/issg-search-filters-resource) [P]
  - Filters organised by study design and by other characteristics, with monthly update searches, an appraisal checklist and published appraisals.
  - Listing a filter is "not an endorsement of its validity".
- **Hedges** (validated against hand-searched journals as the gold standard) [A]:
  - Haynes et al., BMJ 2005 (treatment studies): 99.3% sensitivity at 70.4% specificity.
  - Montori et al., BMJ 2005 (systematic reviews): 49,028 articles from 161 journals; 99.9% sensitivity at 52% specificity.
  - Wilczynski & Haynes 2002: strategies built in 1991 still worked in 2000.
  - That PubMed Clinical Queries implements the Hedges filters is [K]; the Hedges site was unreachable.

## 6. Trust

- **O'Connor et al.**, "A question of trust", Systematic Reviews, June 2019 [P]
  - The barriers are trust and setup.
  - Reviewers want the tool shown "non-inferior or superior in accuracy to current practice", measured as precision and recall against human reviewers.
  - They fear grants or papers being rejected, and want respected teams or funders to lead adoption.
  - They want evaluations reported reproducibly: "no study provided all the criteria for reproducible reporting".
- **Scott et al.**, J Clin Epidemiol 2021 [A]: 253 respondents; 89% had used automation tools, 79% of them for screening; lack of knowledge was the top barrier (51%).
- **Joint position statement from Cochrane, Campbell, JBI and the Collaboration for Environmental Evidence** (Flemyng et al.; Environmental Evidence Oct 2025, JBI Evidence Synthesis Nov 2025, Campbell Systematic Reviews Dec 2025) [P]
  - Reviewers stay responsible, and AI must be used with human oversight.
  - They may need to "pilot (or calibrate)" a tool within their own review.
  - They must report the tool's "name(s)… version(s) and date(s) used".
  - It endorses RAISE, which has three parts: recommendations for practice; building and evaluating tools; selecting and using tools.
- **Not found:** surveys from 2023–2026 on reviewers' trust in LLM tools.

## 7. Outside biomedicine

- **Software engineering:**
  - FASTREAD (Yu, Kraft & Menzies, Empirical Software Engineering 2018, arXiv 1612.03224) [A]: finds 95% of relevant studies after reviewing "an order of magnitude fewer papers".
  - ASReview's software-engineering dataset and its user tests with a public-administration team [P].
- **Public health and behavioural economics:** Shemilt et al., Research Synthesis Methods 2014 [A]: text mining over 800k and over 1 M records cut screening by 88–90%.
- **Environment:** Nykvist et al., Environmental Evidence 2025 [A]: GPT-4 on about 12k records reached 100% recall and saved 50% of screening time.
- **Public policy:** Rubinstein et al., Cochrane Evidence Synthesis and Methods 2025 [A]: GPT-4 screened 43,480 articles.
- **Law:** Grossman & Cormack (see section 1).
- **Humanities and education-specific tools:** not found.

## What this means for the pitch

- The tools that learn from a researcher's marks (ASReview, Covidence, DistillerSR, Rayyan, Nested Knowledge) all work inside one review's imported set.
- The only ones I found that run over a whole corpus and keep up with new works are:
  - EPPI-Reviewer's OpenAlex auto-update, roughly monthly;
  - Semantic Scholar feeds, daily but with no measurement shown to the user;
  - Cochrane's single fixed RCT classifier.
- I found no tool that treats a user's trained classifier as a versioned, shareable artifact measured against held-back marks. The nearest precedent is validated search filters, which are shared with published sensitivity and specificity figures.

---

## The prompt it was given

> Web research task (read-only; do not create or modify any files). Today is 2026-09-28. Context: a user building a scholarly-literature search engine (tens of millions of works, all fields; PubMed was just a starting point) is writing up a product pitch for researchers: "you can teach the search engine your method / your criteria and it will apply it faithfully. you can refine it over time, and even have more than one methodology. and you can start from the subset of journals/researchers/whatever that interest you. you already know how to do it; this thing does what you need how you like and doesn't get tired doing it." Mechanism in mind: the researcher's marks (on-topic / off-topic judgements) plus a plain-language description of their criteria train a cheap classifier (e.g. logistic regression on embeddings, SetFit, GLiNER2 fine-tuned with LoRA), bootstrapped by a strong model labelling a few hundred works, measured against held-back marks, versioned, and run over the whole corpus and over each day's new works. The user wants to distinguish this from "skills files" (plain-language instructions for an LLM agent, e.g. Anthropic Agent Skills / SKILL.md), because search over very large corpora needs trained ML, not only prompts.
>
> Find, with sources:
> 1. Existing tools that let researchers encode or teach their inclusion criteria or interests, and how: Elicit (screening against editable criteria), ASReview, Rayyan, Covidence, DistillerSR, EPPI-Reviewer (its classifiers, e.g. the RCT classifier), Laser AI, Nested Knowledge, Semantic Scholar Research Feeds (recommendations learned from the user's ratings), arXiv-sanity-like tools, Google Scholar alerts, Iris.ai, e-discovery active learning (Relativity). For each: does it learn from the user's marks or only follow instructions; does it run over a whole corpus or only an imported result set; can a method be saved, versioned, shared, rerun; any measured performance.
> 2. Prompted LLM classifiers versus trained small classifiers for screening or topic classification: accuracy, consistency across runs, sensitivity to prompt wording, cost per document; SetFit (few-shot sentence-transformer fine-tuning) results; distillation of LLM labels into small models (e.g. TnT-LLM); any systematic-review-specific comparisons 2023–2026.
> 3. Drift: effects of hosted-model version changes on LLM screening or classification results (reproducibility of LLM-based systematic-review steps across model versions); nondeterminism at temperature 0.
> 4. Reproducibility and updating of literature searches: PRISMA-S (reporting literature searches), how search strategies are published for reproducibility; living systematic reviews (definition, e.g. Elliott et al. 2017, Cochrane's living reviews) and how new evidence is found for them (surveillance searches, classifiers such as Cochrane's RCT classifier / Screen4Me).
> 5. Search filters as shareable artifacts: validated methodological search filters (e.g. the ISSG / InterTASC Search Filters Resource, Hedges / PubMed Clinical Queries) — how they are validated and shared.
> 6. Researcher trust in automation for evidence synthesis: surveys or studies (e.g. O'Connor et al., "A question of trust", Systematic Reviews 2019, and later), what validation researchers want before relying on a tool; any guidance from Cochrane/Campbell or RAISE (Responsible AI in Evidence Synthesis) on AI screening.
> 7. Outside biomedicine: tools or studies of teaching criteria for literature in computer science, social science, education, law or the humanities.
>
> Return a report under 1,400 words: findings with URL, date/venue, what exactly the tool or method does, key numbers if any, and whether you read the primary source or only a search summary (tag [P] primary full text/docs/result tables, [A] abstract or landing page only, [S] search-engine summary only). Say "not found" plainly where you searched and found nothing. Do not invent systems, numbers or citations; flag anything uncertain.
