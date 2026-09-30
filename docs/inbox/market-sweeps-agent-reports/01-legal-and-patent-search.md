# Research report 1: legal AI and patent search, the landscape

**What this is:** the final report of a web-research subagent, delivered
on 2026-09-28 to the conversation that wrote
[the legal and patent-search landscape](../legal-and-patent-search-landscape.md),
and kept verbatim, following the practice the user asked for with
[searching within a result set](../search-within-results.md)'s reports.
**Too early, held loosely:** the user labelled this research as too
early and its findings as ones to hold loosely; the label is the user's
and the note's, and was not given to the researcher. It is model output,
not checked line by line. It used 157 tool calls over about 19 minutes;
web search was unavailable (the session's budget was spent), so it read
primary pages directly and made about 30 API requests, which it lists.
Its tags are its own: [P] primary page or full text; [A] abstract or
landing page only; [S] a third party's write-up (LawNext, Wikipedia).
The prompt it was given follows the report.

---

## Legal AI and patent/prior-art search: landscape report (2026-09-28)

**How I searched.** WebSearch was unavailable: the session's budget was already spent, so I ran zero searches. Instead I read about 75 primary pages directly, plus LawNext (LawSites) and Wikipedia where company pages refused me. I made about 30 API requests, one at a time: GitHub 9, Crossref 8, Federal Register/govinfo 4, arXiv 2, Semantic Scholar 3 (2 were rate-limited), OpenAlex 2 (both rate-limited), TU Wien repository 1, Zenodo 1. I sent no email or key to any service. I created or changed no repository files.

**Tags.** [P] = primary page or full text read. [A] = abstract or landing page only. [S] = third-party write-up only (LawNext, Wikipedia).

**Against the repo notes.** The three notes barely touch this field. `competition-list.md` has only The Lens. `retrieval-benchmarks.md` names TREC-CHEM and CLEF-IP as "kept" without details. `industry-landscape.md` mentions *Thomson Reuters v. Ross*. Everything below is new, with three corrections or additions:
- **The Lens has a new owner.** Cambia sold it to a new Australian non-profit, The Lens Limited, on 1 June 2026 [P about.lens.org].
- **CLEF-IP holds patents only and is non-commercial.** It is about 1.3M EP documents plus about 400K WO documents, all published before 2001, in English, French and German, licensed CC BY-NC-SA 3.0 [P researchdata.tuwien.ac.at]. The NC licence matters for a commercial product.
- **TREC-CHEM 2009 did include papers.** Besides 1.18M patent files it had about 59,000 Royal Society of Chemistry articles. But its Prior Art task (1,000 topics) was judged only by existing patent citations; 18 Technical Survey topics were judged by students and then experts [P, TREC 2009 overview PDF].

### 1. Legal AI platforms
- **Harvey.**
  - Raised $550M at a $15.5B valuation, September 2026 (blog title [P], date [S]). Earlier rounds: $8B in December 2025, $11B in March 2026 [S Wikipedia]. 2025 revenue $190M [S].
  - Reports 2,400+ organisations, 200,000+ professionals, 75+ AmLaw 100 firms, 70+ countries [P harvey.ai].
  - Products: Agents, Vault, Knowledge (research), contract intelligence. No patent search on the site.
  - Alliance with LexisNexis (primary law, Shepard's), 18 June 2025 [S].
- **Legora** (Stockholm).
  - 100,000+ professionals at 1,500+ firms in 50+ markets [P newsroom].
  - $550M at $5.55B, March 2026; $100M ARR, April 2026; talks at ≥$10B, August 2026 [S].
  - Singapore Management University partnership, 9 September 2026; "AI-native citator" rebuild [P].
- **Thomson Reuters CoCounsel Legal.**
  - Deep Research, drafting, tabular review of up to 10,000 documents × 100 questions. Pitched on "linked citations for verification", not completeness [P].
  - Its MCP runs inside Claude [P]. Pricing not public. Came from the Casetext purchase ($650M, 2023) [S].
- **LexisNexis (Lexis+ with Protégé).** Says AI products are "90% of new business" (23 July 2026) and added Mistral models in France (9 July 2026) [P press]. Pricing not public.
- **vLex Vincent.** Now "part of Clio"; 1B+ documents across 100+ countries [P]. Clio bought vLex for $1B (announced June 2025, closed 10 November 2025) and raised a $500M Series G at $5B [S LawNext].
- **Spellbook.** Contract drafting and review in Word; "5,000 legal teams"; per-seat custom pricing; nothing on IP [P].
- **Luminance.** Contract intelligence; "1,000+ enterprises" [P]. $40M Series B, April 2024 [S]. Later rounds not verified.
- **Anthropic.** GitHub `anthropics/claude-for-legal` (created 2026-04-21, about 9.5k stars) [P].
  - 12 practice plugins, among them `ip-legal`: trademark clearance, FTO triage, infringement triage, invention intake, portfolio. `litigation-legal` has a Claim Chart Builder.
  - Thomson Reuters maintains a CoCounsel/Westlaw plugin there. Connectors include CourtListener, USPTO TSDR, iManage and Everlaw.
  - The FTO skill says: "Run a preliminary patent search if a connector is available (Solve Intelligence Patents, or other patent-research MCP)." So Anthropic ships no patent corpus; search is an open integration slot.
  - claude.com/solutions/legal names Thomson Reuters, Harvey, Legora, Freshfields and DocuSign [P].
- **How completeness is judged.** Evaluations measure answer accuracy, not recall.
  - Stanford (Magesh et al., arXiv 2405.20362, May 2024): Lexis+ AI and Westlaw tools "hallucinate between 17% and 33% of the time" [A].
  - Vals VLAIR (27 February 2025) tested Harvey, CoCounsel, Vincent and Oliver on document tasks; Lexis withdrew [P].
  - No platform I read publishes recall of a research set.

### 2. Patent and prior-art tools

| Tool | Scale and NPL (non-patent literature) | Price and ownership |
|---|---|---|
| **Derwent Patent Search** (formerly Derwent Innovation) | 178M patent records; 73M NPL items from 13,600 Web of Science journals [P] | Clarivate |
| **Questel Orbit** | 100M patents, 150M NPL (clinical trials, science) [P]; Sophia AI agent | Paris; about 1,500 staff and 20,000 clients; owned by Eurazeo, RAISE and IK Partners after a 2020 buyout [S fr.wikipedia] |
| **PatSnap** | 214M patents, 256M papers, 18,000 customers [P] | Eureka from $200/month [P] |
| **LexisNexis IP** | TotalPatent One, PatentSight+ (with Protégé), IPlytics, PatentAdvisor, PatentOptimizer, Lex Machina; 155M+ patents [P] | LexisNexis |
| **PatBase** | 190M publications, "NPL, patents & design patents" [P] | Minesoft |
| **IPRally** | 120M patents; 6 patent offices and 4,600+ professionals use it; NPL not stated [P] | Price not public |
| **Amplified** | 140M patents; no NPL mentioned [P] | Price not public |
| **PQAI** | Open-source; data from USPTO, EPO and DOAJ [P] | Free; $20/month; commercial API from $700/month [P] |
| **Google Patents** | 120M+ publications; NPL from Google Scholar, Google Books and the Prior Art Archive [P] | Free |
| **The Lens** | 200M+ scholarly records plus patent-to-paper citation mining [P] | See ownership change above |

Vendor benchmarks:
- **Questel QaECTER** (April 2026): benchmarked on its own Sophia-Bench of 10,000 queries graded against examiner-cited art; the benchmark is not public yet [S].
- **PatSnap PatentBench** (July 2026, its own): 340 cases with examiner X-citations as the answer key. PatSnap finds at least one X document in 85% of cases, but **only 37% recall within the top 100**. ChatGPT 5.6 scored 71.76% / 30.51% [P]. PatSnap's landing page misstates the 85% as "prior art found in top 100".

Public offices:
- **USPTO ASAP pilot** (petitions 20 October 2025 to 20 April 2026, at least 1,600 applications): an internal AI tool uses CPC plus the specification and returns up to 10 ranked documents. It searches US patents, pre-grant publications and foreign patents (FIT) only, with **no NPL listed** [P Federal Register 2025-19493].
- **WIPO PATENTSCOPE**: "AI-Assisted Search" launched 2 July 2026; includes NPL [P summary].
- **Espacenet**: I could not confirm public NPL coverage.

Newer AI entrants:
- **Solve Intelligence**: drafting, prosecution, prior art "across patents, literature, and case law"; 700+ teams; backed by Thomson Reuters, Microsoft and Y Combinator [P].
- **Patlytics**: 130M patents plus 300M literature; $40M Series B [P].
- **DeepIP** (Paris): $40M+; 400+ firms; bought PatentMaker, used by nearly half of Germany's top IP firms, June 2026 [S].
- **Stilta** (Stockholm): searches 180M patents, 250M papers and the Wayback Machine; claims "roughly three times the recall" of general models; $10.5M seed led by a16z, May 2026 [S].

### 3. How professional search is done and judged
- **EPO Guidelines** [P] state the limits of completeness plainly.
  - B-III 2.1: a search "cannot always be 100% complete… all information retrieval systems… are inevitably imperfect"; missing less relevant art "is acceptable".
  - B-IV 2.6: stop "once the probability of discovering any more relevant prior art becomes very low in comparison to the work involved".
- **EPO Quality Report 2025** [P].
  - 83.0% of search reports cite a document that destroys novelty or inventive step.
  - The PreSearch AI tool gained LLM query generation and "additional non-patent literature" in 2025.
  - 2.1% of granted patents are opposed (Agri-food 8.5%, Digital 0.5%).
- **What a miss can cost (USPTO PTAB, FY26 to 30 June 2026)** [P]: 1,096 petitions; 50% denied institution; 197 final decisions found all claims unpatentable, out of 332 final decisions.
- **Searcher studies.** Russell-Rose, Chamberlain and Azzopardi (IP&M 2018; 81 patent analysts) found a "continuing preference" for Boolean queries, "importance of recall and coverage", and trust and explainability as open challenges [A].
- **Recent papers** [A]:
  - PANORAMA (October 2025, 8,143 USPTO examination records): LLMs retrieve prior art fairly well but "struggle" with novelty and obviousness.
  - Sparse Coverage (July 2026) evaluates on CLEF-IP 2013.
  - Kats et al. (2023): relevance feedback cut review effort 17.85–59.04% at a fixed recall target.
- **Recall metric.** PRES (Magdy & Jones, SIGIR 2010) is the recall-oriented metric for patent search; existence verified only.

### 4. NPL in prior art
- **EPO, 2025** [P]: "just over 28%" of search reports cite NPL, most in life sciences and chemistry. The NPL database holds 208M documents, 44M of them full text. Selected articles are stored as "XP" documents (Guidelines B-IX 3.1).
- **Bryan, Ozcan and Sampat** (NBER w25742): "Only 24% of front page citations to academic articles are in the patent text" [A].
- **Marx & Fuegi** (SMJ 2020): an open dataset (CC BY-NC) of worldwide patent-to-paper citations, with confidence scores [A].
- **Callaert et al.** (Scientometrics 2006) exists, but I could not read its abstract; no numbers from it.
- **Pain point.** Google Patents says NPL search is "critical in many fields" [P]. The USPTO pilot omits NPL. The EPO added NPL to PreSearch only in 2025. I found no survey that quantifies NPL search as a pain point.

### 5. Europe and Singapore
- **UPC.** In force 1 June 2023, 18 states, central division seat in Paris. By 31 May 2025: 883 cases, including 304 counterclaims for revocation and 65 standalone revocation actions [S Wikipedia]. The court's own site refused access.
- **IPOS Singapore.** A PCT search and examination authority (ISA/IPEA) since 9 October 2015 [S]. The IPOS International page refused access.
- **French and European players.** Questel, DeepIP, Doctrine (French case-law research, no IP [P]), and LexisNexis with Mistral in France.
- **Market data on IP firms' spending on search tools: not found.**

### 6. Where a set-oriented tool fits
- **Redundant.** Semantic search over patents plus 150–300M papers is table stakes: PatSnap, Orbit, Derwent, Patlytics, Stilta, Solve. The general legal platforms (Harvey, Legora, CoCounsel) do not search patents.
- **Possible gap** (my reading, not a finding):
  - The EPO's stopping rule is a probability judgment that nobody measures per search.
  - Vendors report recall only as benchmark averages; PatSnap's own is 37% at 100.
  - Classification codes (CPC) do not apply to papers, so the Boolean-plus-classification workflow is weakest on exactly the NPL side.
  - "What the set is still missing" and a classifier trained on the searcher's marks address this. A reproducible "method" matches the searchers' demand for explainability.
  - Anthropic's FTO skill leaves an empty patent/literature-search connector slot.
- **Point tools versus platforms.** Patent practitioners do buy point tools: Solve (700+ teams), DeepIP (400 firms), IPRally (4,600 professionals). General legal work is consolidating into platforms: Clio–vLex, Harvey–LexisNexis, Legora–American Law Institute.
- **The corpus's limit.** It has no patents, so for an IP firm it would complement the firm's patent tools rather than replace them.

**Uncertain or not found:**
- Pricing for Harvey, Legora, CoCounsel, Lexis, Derwent and Orbit (none public).
- Luminance rounds after 2024.
- PQAI's claimed "#2" independent ranking: source not found.
- Clarivate IP divestiture: nothing found.

---

## The prompt it was given

> Web research task (read-only; do not create or modify any files). Today is 2026-09-28. Context: a user building a scholarly-literature search engine (tens of millions of works; academic literature is its first market) may talk with a relative who runs an intellectual-property law firm based in Paris with an office in Singapore. The user wants to know the landscape that conversation sits in: legal AI and patent / prior-art search, who the players are, what they offer, what they cost, and where academic literature meets IP work (non-patent literature in prior-art search). The product idea being compared: set-oriented search with measured recall (counts, samples of what a query kept and dropped, a classifier trained on the user's own marks, an estimate of what the set is still missing), a "method" a professional can teach once and rerun reproducibly.
>
> Before searching the web, read these notes in the repository at /Users/thomasingalls/Code/ol-rht/.claude/worktrees/search-within-results (read-only): docs/inbox/industry-landscape.md, docs/inbox/competition-list.md, docs/inbox/retrieval-benchmarks.md (TREC-CHEM, CLEF-IP). Say where your findings add to or differ from them.
>
> Find, with sources:
> 1. Legal AI platforms: Harvey, Legora, Thomson Reuters CoCounsel / Westlaw AI, Lexis+ AI / Protégé, vLex Vincent (and its acquisition), Luminance, Spellbook, others you find; and what Anthropic offers for legal work as of 2026 (Claude skills, integrations, partnerships). For each: what it does (drafting, review, research, search), whether IP / patent work is covered, funding or valuation, pricing where public, customers (firm size), and anything on how it handles completeness or recall of research.
> 2. Patent and prior-art search tools: Clarivate Derwent Innovation, PatSnap, Questel Orbit, LexisNexis TotalPatent One / PatentSight / IPlytics, Minesoft PatBase, IPRally, Amplified, PQAI, Google Patents, The Lens, EPO Espacenet, WIPO Patentscope, USPTO search, and AI entrants 2023–2026. What they offer (boolean with CPC/IPC classification codes, citation search, semantic search, AI summaries), which cover non-patent literature (NPL) and how, pricing where public, and ownership (who bought whom).
> 3. How professional prior-art search is done and judged: the searcher's workflow (boolean line-by-line with classification codes, citation chasing, semantic search), how completeness is assessed, what a missed reference costs (invalidity, opposition), and any published evaluation of tools (CLEF-IP, TREC-CHEM, patent-search user studies, EPO or USPTO studies of AI search tools).
> 4. Non-patent literature in prior art: how much of cited prior art is academic literature (studies of examiner citations to NPL, by field), which databases examiners and firms use for NPL, and whether NPL search is described as a pain point.
> 5. The European and Singapore context: the EPO, the Unified Patent Court (since 2023), IPOS Singapore; any market data on IP law firms' spending on search tools; French or European legal-tech players.
> 6. Where a narrow, set-oriented search tool could fit or would be redundant among these; any sign that law firms buy point tools versus platforms.
>
> If the WebSearch tool is unavailable or its budget is spent, fetch primary pages directly (company sites, pricing pages, documentation, papers) and use public APIs (arXiv, Europe PMC, OpenAlex) gently: one request at a time, a modest number in total; say in the report how you searched and how many API requests you made. Return a report under 1,400 words: findings with URL, date/venue, what exactly each player or study does, key numbers, and whether you read the primary source or only a summary (tag [P] primary full text/docs/pricing page, [A] abstract or landing page only, [S] search-engine summary or third-party write-up only). Say "not found" plainly where you searched and found nothing. Do not invent companies, numbers or citations; flag anything uncertain.
