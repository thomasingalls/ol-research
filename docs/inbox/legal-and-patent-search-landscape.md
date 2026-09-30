# The legal and patent-search landscape

**Too early; hold loosely.** The user asked for this sweep while the
product has no users, no chosen shape and nothing built beyond search,
and labelled the research too early and its findings as ones to hold
loosely. Read every figure and every reading here as a snapshot to
bring to a conversation, not a basis for a decision; prices, valuations
and positions in this field move month to month.

**Status:** note, 2026-09-28, written at the user's request ("do both
sweeps, write them up as inbox notes"). It maps the landscape around the
conversation with a relative's intellectual-property law firm
([first customers and first conversations](first-customers-and-conversations.md)
§2.1). Speculative, not a plan; nothing in it amends the plans of
record. Its facts come from one research report, kept verbatim in
[market-sweeps-agent-reports/](market-sweeps-agent-reports/01-legal-and-patent-search.md);
the bracketed tags are the report's own: **[P]** a primary page or full
text read, **[A]** an abstract or landing page only, **[S]** a third
party's write-up (LawNext, Wikipedia). Web search was unavailable to the
researcher, so it read about 75 primary pages directly and made about 30
API requests; what it could not reach is marked "not found". Related:
[industry landscape](industry-landscape.md), [competition list](competition-list.md),
[retrieval benchmarks](retrieval-benchmarks.md), the tools of
[the set builder](recall-accounted-set-builder.md) and
[teach it your method](teach-it-your-method.md).

## 1. Why this sweep

The user, after the first-customers note:

> but the point stands, there's no way nobody noticed ip law. and the
> offer as written seems thin compared to the likes of [big ai law],
> what legora, harvey, anthropic and its skills, the vast (i assume)
> competitive landscape of incumbents from the pre-ai era ...

The sweep asks who the players are, what they offer and cost, how prior-
art search is done and judged, and where academic literature meets IP
work.

## 2. Legal AI platforms

| player | what it is | scale, money | patents |
|---|---|---|---|
| Harvey | agents, a document vault, research, contracts | $550M at a $15.5B valuation, Sep 2026 ([P] title, [S] date); 2,400+ organisations, 75+ AmLaw 100 firms ([P]); an alliance with LexisNexis ([S]) | no patent search on its site |
| Legora (Stockholm) | legal AI platform; an "AI-native citator" | 100,000+ professionals at 1,500+ firms ([P]); $550M at $5.55B, Mar 2026, talks at ≥$10B, Aug 2026 ([S]); a Singapore Management University partnership, Sep 2026 ([P]) | not stated |
| Thomson Reuters CoCounsel | deep research, drafting, review of up to 10,000 documents | from the Casetext purchase ($650M, 2023, [S]); runs inside Claude through its connector ([P]) | not stated |
| LexisNexis, Lexis+ with Protégé | research and drafting | AI "90% of new business" (Jul 2026, [P]); Mistral models added in France ([P]) | its IP line is separate (§3) |
| vLex Vincent, now Clio | research over 1B+ documents in 100+ countries ([P]) | Clio bought vLex for $1B, closed Nov 2025 ([S]) | not stated |
| Spellbook; Luminance | contract drafting and review; contract intelligence | 5,000 legal teams; 1,000+ enterprises ([P]) | no |
| Anthropic, `claude-for-legal` | 12 practice plugins on GitHub, among them `ip-legal`: trademark clearance, freedom-to-operate triage, infringement triage, invention intake ([P]) | Thomson Reuters maintains a plugin there; connectors include CourtListener and USPTO trademark status ([P]) | its freedom-to-operate skill runs a patent search "if a connector is available (Solve Intelligence Patents, or other patent-research MCP)": it ships no corpus, and search is an open slot ([P]) |

**How they are judged: on accuracy, not completeness.** Stanford found
the legal research tools of Lexis+ AI and Westlaw hallucinating "between
17% and 33% of the time" (Magesh et al., May 2024, [A]); an independent
evaluation (Vals, Feb 2025, [P]) tested Harvey, CoCounsel, Vincent and
others on document tasks. No platform read publishes the recall of a
research set.

## 3. Patent and prior-art search tools

| tool | scale, and non-patent literature (NPL) | price, owner |
|---|---|---|
| Derwent Patent Search | 178M patent records; 73M NPL items from 13,600 Web of Science journals ([P]) | Clarivate |
| Questel Orbit | 100M patents, 150M NPL; the Sophia AI agent ([P]) | Paris; about 1,500 staff, 20,000 clients; owned by Eurazeo, RAISE and IK Partners since 2020 ([S]) |
| PatSnap | 214M patents, 256M papers, 18,000 customers ([P]) | its Eureka plan from $200 a month ([P]) |
| LexisNexis IP | TotalPatent One, PatentSight+, IPlytics and others; 155M+ patents ([P]) | LexisNexis |
| PatBase | 190M publications, NPL included ([P]) | Minesoft |
| IPRally; Amplified | 120M and 140M patents; NPL not stated ([P]) | prices not public |
| PQAI | open source, over USPTO, EPO and DOAJ data ([P]) | free; $20 a month; an API from $700 a month ([P]) |
| Google Patents | 120M+ publications; NPL from Google Scholar, Books and the Prior Art Archive ([P]) | free |
| The Lens | 200M+ scholarly records, citations mined between patents and papers ([P]) | sold by Cambia to a new Australian non-profit, The Lens Limited, 1 June 2026 ([P]) |

**Newer entrants:** Solve Intelligence (drafting, prosecution, prior art
"across patents, literature, and case law"; 700+ teams; backed by
Thomson Reuters, Microsoft and Y Combinator, [P]); Patlytics (130M
patents and 300M papers, $40M Series B, [P]); DeepIP (Paris; 400+ firms;
bought PatentMaker, Jun 2026, [S]); Stilta (Stockholm; patents, papers
and the Wayback Machine; claims "roughly three times the recall" of
general models; $10.5M seed, May 2026, [S]).

**Vendors' own benchmarks:** PatSnap's PatentBench (Jul 2026, [P]): 340
cases with the examiners' most relevant citations as the answer key;
PatSnap finds at least one of them in 85 % of cases, but recall within
its top 100 is 37 %, and a general model scored 71.76 % and 30.51 %.
Questel benchmarks on its own 10,000-query set, not yet public ([S]).

**Patent offices:** the USPTO's pilot AI search (Oct 2025 – Apr 2026, at
least 1,600 applications) returns up to 10 ranked documents from patent
collections only, with no non-patent literature listed ([P]); WIPO's
PATENTSCOPE added AI-assisted search including NPL on 2 July 2026 ([P]).

## 4. How prior-art search is done and judged

- **Completeness is acknowledged as impossible, and stopping is a
  judgement of probability.** The EPO's Guidelines: a search "cannot
  always be 100% complete… all information retrieval systems… are
  inevitably imperfect", and missing less relevant art "is acceptable"
  (B-III 2.1); the examiner stops "once the probability of discovering
  any more relevant prior art becomes very low in comparison to the work
  involved" (B-IV 2.6) ([P]).
- **Quality as measured:** 83.0 % of EPO search reports cite a document
  that destroys novelty or inventive step; its PreSearch tool gained
  model-generated queries and "additional non-patent literature" in
  2025; 2.1 % of granted patents are opposed (EPO Quality Report 2025,
  [P]).
- **What a miss can cost:** at the USPTO's appeal board, fiscal 2026 to
  30 June, 1,096 petitions; 197 of 332 final decisions found all claims
  unpatentable ([P]).
- **What searchers want:** 81 patent analysts showed a "continuing
  preference" for boolean queries, the "importance of recall and
  coverage", and trust and explainability as open problems
  (Russell-Rose, Chamberlain and Azzopardi, IP&M 2018, [A]). Relevance
  feedback cut review effort by 17.85–59.04 % at a fixed recall target
  (Kats et al., 2023, [A]). PRES (Magdy and Jones, SIGIR 2010) is the
  recall-oriented measure for patent search.

## 5. Where academic literature meets IP work

- **Non-patent literature is a large part of prior art in some fields.**
  "Just over 28%" of EPO search reports cite it, most in life sciences
  and chemistry; the EPO's own NPL database holds 208M documents, 44M in
  full text ([P]).
- **Citations to papers sit mostly outside the patent's text:** "only 24%
  of front page citations to academic articles are in the patent text"
  (Bryan, Ozcan and Sampat, NBER w25742, [A]). An open dataset of
  worldwide patent-to-paper citations exists, under a non-commercial
  licence (Marx and Fuegi, 2020, [A]).
- **A pain point, suggested but not measured:** Google Patents calls NPL
  search "critical in many fields" ([P]); the USPTO pilot leaves it out;
  the EPO added it to PreSearch only in 2025. No survey quantifying it
  was found.

## 6. Europe and Singapore

The Unified Patent Court has sat since 1 June 2023 across 18 states, its
central division's seat in Paris; 883 cases by 31 May 2025, 369 of them
revocation actions or counterclaims ([S]). IPOS Singapore has been an
international search and examination authority since 2015 ([S]). French
players: Questel, DeepIP, Doctrine (case law, no IP, [P]), and
LexisNexis running Mistral in France. **Market data on IP firms'
spending on search tools: not found.**

## 7. A reading of the user's worry

Held loosely, as the label says.

- **The user was right about the crowd.** IP work is noticed, crowded and
  well funded. Semantic search over patents and hundreds of millions of
  papers is table stakes (PatSnap, Orbit, Derwent, Patlytics, Stilta,
  Solve). The general legal platforms (Harvey, Legora, CoCounsel) do not
  search patents. And this project's corpus holds no patents, so for an
  IP firm it could only sit beside the patent tools, not replace them.
- **Where it is not thin (the researcher's reading, and the note's):**
  - Recall is reported only as benchmark averages, and the best vendor's
    own is 37 % at 100; nobody measures, per search, the probability the
    EPO's stopping rule asks the examiner to judge. That is the set
    builder's `missing()`.
  - Classification codes do not reach papers, so the boolean-and-codes
    workflow is weakest exactly on the non-patent-literature side, the
    side this corpus covers.
  - Searchers ask for recall, coverage and explainability; a method taught
    on the searcher's marks, measured and rerunnable, answers the last.
  - Anthropic's IP plugin leaves an empty slot for a patent or literature
    search connector. One product shape worth keeping in view: the
    literature connector others plug in, rather than a platform.
- **Point tools do sell in patent practice:** Solve (700+ teams), DeepIP
  (400+ firms), IPRally (4,600+ professionals), while general legal work
  consolidates into platforms (Clio with vLex, Harvey with LexisNexis).

**Questions for the conversation with the relative's firm:** how they
search non-patent literature today, and with what; how they decide a
search is done, in practice; whether a measured estimate of what is
missing would change their work or their client's risk; whether a
literature connector inside tools they already use would be welcome.

## 8. Corrections to the repository's notes

From the report ([P] each):

- **The Lens** changed owners on 1 June 2026 (§3); the
  [competition list](competition-list.md) now says so.
- **CLEF-IP** is patents only, about 1.3M EP and 400K WO documents
  published before 2001, in English, French and German, under CC
  BY-NC-SA 3.0: non-commercial, which matters for a commercial product.
- **TREC-CHEM 2009** held about 59,000 Royal Society of Chemistry articles
  beside 1.18M patent files, but its prior-art task was judged only by
  existing patent citations.

The last two are added to [retrieval benchmarks](retrieval-benchmarks.md)'
line on the patent sets.

## 9. Open

1. Prices for Harvey, Legora, CoCounsel, Lexis, Derwent and Orbit: not
   public.
2. IP firms' spending on search tools: not found.
3. Whether CLEF-IP's non-commercial licence allows its use to evaluate a
   commercial product: a question for the licence, and the user.
4. Whether a literature connector for other people's tools is a product
   shape to pursue (§7).
