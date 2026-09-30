# Research report 2: what academics and institutions pay

**What this is:** the final report of a web-research subagent, delivered
on 2026-09-28 to the conversation that wrote
[what academics pay](../what-academics-pay.md), and kept verbatim,
following the practice the user asked for with
[searching within a result set](../search-within-results.md)'s reports.
**Too early, held loosely:** the user labelled this research as too
early and its findings as ones to hold loosely; the label is the user's
and the note's, and was not given to the researcher. It is model output,
not checked line by line. It used 150 tool calls over about 20 minutes;
web search was unavailable (the session's budget was spent), so it read
primary pages directly and made 6 API requests, which it lists. Its tags
are its own: [P] primary page; [A] abstract only. Prices are as shown on
2026-09-28, in US dollars unless marked. The prompt it was given follows
the report.

---

**Report: what academics and institutions pay for literature search, discovery and screening (2026-09-28)**

**How I searched.** The session's WebSearch budget was already used up (200 of 200), so I ran no web searches. I read about 70 primary pages through WebFetch and plain curl, and extracted text from three PDFs. I made 6 public API requests: 1 to OpenAlex, 3 to Europe PMC and 2 to the French ministry's open-data portal. I did not touch the repository. I did create one temporary file in /tmp, then deleted it. Prices are USD unless marked, as shown on 2026-09-28. Tags: [P] primary page, [A] abstract only.

**Where this adds to or differs from the notes**
- **Elicit:** $49 is confirmed, but it is the annual-billing rate.
- **Consensus:** the notes' $20/month Pro could not be re-verified. Prices load in the browser and the help centre refused the request.
- **"Academia absent as a buyer" (industry-landscape §9, citing the Kitchen):** this holds for AI licensing, not for discovery. French institutions spent €37.6M on databases in 2024.
- **Screening tools:** no prices were recorded in the notes before. All below are new.
- **New:** France's 2026–2033 open-science strategy tells institutions to fund OpenAlex.
- **New, relevant to the extension notes:** Paris-Saclay's library says its Lean Library extension stops working on 2027-01-01.

**1. AI research tools, individual plans [P]**
- **Elicit:** Basic free. Pro $49/mo billed annually ($588/yr). Scale $169/mo annually ($2,028). Enterprise custom. No academic discount shown.
- **Undermind:** Free. Pro $16/mo annually. Team $15/person/mo. Enterprise custom, with sitewide login. No academic discount.
- **scite:**
  - Connect is free (25 MCP credits).
  - Basic is $14/mo and Pro $35/mo, both billed yearly. The "Save $67" and "Save $84" figures imply about $20 and $42 when paid monthly.
  - The page shows a 30% fall sale running to 2026-09-30, and it is unclear whether these prices already include it.
  - Academic discount only if you recommend scite to your institution.
- **SciSpace** (read from price data embedded in the page, which was served as country "FR"):
  - Premium $20/mo, or $12/mo billed annually.
  - Advanced $90/mo, or $70/mo annually.
  - Teams $25/user/mo, or $15/user/mo annually, for 2–300 users.
- **Litmaps:** Pro $10/mo or $120/yr. Education pricing with an academic email. Discounts for classrooms, labs, institutions and some countries.
- **ResearchRabbit:** RR+ $10/mo on the annual plan, $12.50 monthly. Parity discounts in 100+ countries. The Institution plan includes LibKey and is priced by quote.
- **Keenious (Europe):** Plus from €10/mo annually. Team €20/user/mo. Students get 30% off. Institutional plans are custom and include a data processing agreement.
- **scienceOS (EU servers):** Angel €7.25/mo. Team €6/user/mo, minimum 6 users. These prices are "valid only for academic, non-profit, and personal use".
- **Claude:** Pro $17/mo annually or $20 monthly. Max from $100. Team standard seat $20/$25. Education plan on request.
- **Semantic Scholar:** free, run by the nonprofit Ai2.
- **Not verified** (the page refused the reader or showed prices only in the browser): Consensus, Paperpal, R Discovery, Perplexity, ChatGPT.

**2. Systematic-review and screening tools [P]**
- **Covidence:**
  - $339/yr for one review, $907/yr for three, with unlimited collaborators.
  - Departmental or organisation-wide licences with unlimited reviews are priced by quote. Reduced rates for low- and middle-income countries.
  - Owned by the non-profit Future Evidence Foundation. Claims 800,000+ researchers at 500+ institutions; Cochrane's standard platform since 2015.
  - I could not confirm on a university library's page that libraries hold these licences.
- **Rayyan:**
  - Free for 3 active reviews.
  - Essential $4.99/mo and Advanced $8.33/mo, billed annually.
  - Business/Academic $41.67 per licence per month, minimum 5 licences ($2,500/yr), "discounts available for Academics". Enterprise custom.
- **DistillerSR:** Student $19.95/mo. Faculty $94/mo, billed annually. Research Project (1 faculty + 4 students) $194/mo. Department, institution and corporate prices by quote only.
- **EPPI-Reviewer:** £10 per user per month, plus £35 per shareable review per month. Site licences by quote. Free for Cochrane and Campbell authors.
- **Nested Knowledge:** Academic $19.95/user/mo, Academic+ $69.95. Corporate $295 and $695/user/mo. First review free.
- **Laser AI:** the page returned "$3,000" for per-project, per-user and team tiers alike. That looks like a rendering artefact, so treat it as unverified. No academic tier.
- **ASReview:** free and open source, coordinated at Utrecht University, with a €1.5M NWO Vici grant.
- **What a review costs, as a frame for screening savings:**
  - Michelson & Reuter (Contemp Clin Trials Commun 2019) [A]: about $141,194.80 per review. The ten largest academic institutions publish about 132 a year, roughly $18.7M each.
  - Borah et al. (BMJ Open 2017) [A]: mean 67.3 weeks and 5 authors, across 195 PROSPERO reviews.

**3. Institutional spending and how it is bought**
- **France, Couperin's electronic-resources survey for 2024** ([BilanERE_2020_2024_VF.pdf](https://www.couperin.org/wp-content/uploads/2023/09/BilanERE_2020_2024_VF.pdf)) [P]:
  - 311 institutions spent €130.4M excluding tax, up 1% on 2023.
  - Periodicals €78.7M (60%), databases €37.6M (29%), e-books €9.7M (8%).
  - 79% of spending went through Couperin negotiations. CNRS is the largest single spender at €10.0M.
- **Group purchases, carried 89% by ABES:**

  | | 2020 | 2022 | 2023 | 2024 |
  |---|---|---|---|---|
  | Web of Science + InCites | €3.7M | €4.6M | €4.7M | €4.6M |
  | Scopus + SciVal | €2.1M | €2.2M | €2.2M | €1.7M |

  The 2024 Scopus drop coincides with CNRS leaving; that link is my reading.
- **Couperin's current negotiations list** [P]: offers for Web of Science + InCites and Scopus + SciVal for 2027–2029 are marked as distributed to members, updated 2026-09-24 and 25.
- **How a vendor gets in** [P]:
  - Propose in February–March of the year before.
  - A negotiation is "pointless" without at least about a dozen interested institutions.
  - Vendors must return Couperin's GDPR and AI questionnaires (the AI one was updated 2026-09-18), plus KBART data, COUNTER 5 statistics and the standard licence.
  - Members get trials of 1–3 months.
  - Couperin is a non-profit funded by member dues and subsidised by the ministry.
- **Europe, EUA Big Deals Survey (2018, data 2016–17, 28 consortia)** [P]:
  - The top three database deals per country came to €33.3M a year, against €383.6M for periodicals; €421.0M in total.
  - Purchase models: 68% bought by the consortium and recharged to institutions, 18% paid centrally, 15% framework contracts.
  - The pie chart's text suggests Web of Science about 50% and Elsevier (Scopus + Reaxys) about 29%, but the label order is uncertain.
- **Clarivate Pulse of the Library 2026 (vendor survey, 1,876 librarians)** [P]: 3% of libraries report active AI implementation; 30% have no plans.
- **Not found:** UK freedom-of-information cost figures (the site refused the reader). The Jisc and Projekt DEAL purchasing routes were not researched.

**4. The move to open infrastructure**
- **Sorbonne** [P]: announced 2023-12-08 that it would not renew Web of Science or Clarivate's bibliometric tools for 2024, and would build a partnership with OpenAlex. No cost disclosed.
- **CNRS** [P]: announced 2024-01-11 that its Scopus subscription ended on 2023-12-31. It kept Web of Science, citing higher use, and points researchers to OpenAlex, Crossref, Dimensions and HAL. No euro figure.
- **French Stratégie de science ouverte 2026–2033**, launched 2026-09-16 ([PDF](https://doi.org/10.52949/93)) [P]:
  - Measure OS4.A1.M1: "Promouvoir le développement d'outils comme OpenAlex… Encourager les institutions utilisatrices à contribuer à son modèle économique" (promote tools like OpenAlex and have user institutions help fund it).
  - It also encourages the Barcelona Declaration.
  - One of its indicators counts French institutions that support OpenAlex.
- **Barcelona Declaration signatory file** [P]: 146 signatories and 57 supporters.
  - France has 18 signatories, including Université Paris-Saclay, Sorbonne, Udice, ANR, the national open-science committee and Couperin (joined 2026-03-09).
  - CNRS is not listed.
- **Reading:** the policy is to move money to open infrastructure, yet national renewals of the proprietary databases continue into 2029. A newcomer built on open data will be judged against "OpenAlex plus a membership fee".

**5. Willingness to pay**
- **Wiley ExplanAItions 2025** [P, press release]:
  - 70% use a freely available tool, even though 48% also have access to a paid one.
  - 40% say their organisation provides AI tools; 58% in the corporate sector.
  - 80% use general tools against 25% using AI research assistants. On average 11% have heard of the specialised tools surveyed.
- **Elsevier 2025** [P, landing page]: 58% use AI; 27% say they are adequately trained. No data on payment.
- **Researchers paying out of pocket or from grants:** not found.

**6. Corporate versus academic pricing**
- Nested Knowledge charges about 15 times more for corporate seats ($295 against $19.95), and 10 times more at the top tier.
- DistillerSR publishes academic prices only; corporate and government buyers get quotes.
- scienceOS's list prices exclude commercial use.
- Consensus sells to research organisations separately, with enterprise terms above 200 users.
- Elicit, Undermind and Laser AI show no academic tier.
- **Reading:** this looks like price discrimination, with academia as the funnel and companies paying the margin. Evidence that academia is a loss leader is indirect: Claude's $15 seats for principal investigators and Benchling free for academics, both already in the notes.

**7. Europe-specific factors**
- **EU procurement thresholds for 2026–2027** (Delegated Regulation (EU) 2025/2152) [P]:
  - Supplies and services: €140,000 for central government, €216,000 for other public bodies.
  - Works: €5,404,000.
- **France's own lower threshold** for buying without advertising or competition: not verified, because the official pages refused the reader.
- **Data sovereignty:** Couperin's GDPR and AI questionnaires, and the strategy's framing of infrastructure as "souveraineté". scienceOS, Keenious (data processing agreement) and ASReview ("GDPR compliant") all sell on data protection.
- **How European tools are funded:** Open Knowledge Maps is a charitable non-profit living on memberships of €480–€10,800 a year from about 30 members. ASReview runs on an NWO grant. Keenious and scienceOS are commercial.
- **Lean Library at Paris-Saclay:** the library's page says the extension will "no longer be operational from 1 January 2027". Lean Library's own site shows no shutdown, and Couperin lists a 2027 Lean Library negotiation as in progress. So this is probably a local non-renewal, which is my reading.

---

## The prompt it was given

> Web research task (read-only; do not create or modify any files). Today is 2026-09-28. Context: a user building a scholarly-literature search engine (tens of millions of works across all fields; academics and researchers are its first users; it would sell into Europe first, and the user lives beside Université Paris-Saclay) wants to know what academics and their institutions actually pay for literature search, discovery and screening tools, and through whom the money flows (individual researchers, labs and grants, libraries, institutions, national consortia).
>
> Before searching the web, read these notes in the repository at /Users/thomasingalls/Code/ol-rht/.claude/worktrees/search-within-results (read-only): docs/inbox/industry-landscape.md, docs/inbox/science-assistant-landscape.md, docs/inbox/competition-list.md, docs/inbox/product-shapes.md. Say where your findings add to or differ from them (for example, prices already recorded).
>
> Find, with sources:
> 1. Individual pricing for AI research tools, current in 2026: Elicit, Consensus, scite, Undermind, SciSpace, Paperpal/R Discovery, ResearchRabbit and Litmaps, Semantic Scholar (free), Google Scholar (free), Perplexity / ChatGPT / Claude plans used for research; free tiers, student and academic discounts, institutional or team plans.
> 2. Systematic-review and screening tools: Covidence, DistillerSR, Rayyan, EPPI-Reviewer, Nested Knowledge, Laser AI, ASReview (open source): individual versus institutional pricing, and how institutions license them (e.g. Covidence institutional licences held by libraries).
> 3. Institutional and library spending on discovery: Web of Science and Scopus subscription costs to universities (published figures, FOI or consortium data), Dimensions; library budgets and the share spent on databases versus journals; how purchases are made (library acquisitions, national consortia such as Couperin in France, Jisc in the UK, Projekt DEAL in Germany).
> 4. The move to open infrastructure: universities or research organisations that cancelled Web of Science or Scopus in favour of OpenAlex or other open sources (e.g. Sorbonne University, CNRS, or others — verify what happened and when), the French national open-science plan, the Barcelona Declaration on Open Research Information, and what this says about willingness to pay for proprietary discovery.
> 5. Willingness to pay: surveys or studies of researchers paying for AI tools out of pocket or from grants, adoption rates of AI research tools among academics (e.g. Wiley, Elsevier, Nature or Oxford University Press surveys 2024–2026), and what researchers say they would pay for; the cost of a systematic review (published estimates) as a frame for what screening tools save.
> 6. Pharma and corporate pricing versus academic pricing for the same tools, where visible; whether vendors treat academics as a loss leader or a funnel.
> 7. Europe-specific factors: public procurement thresholds, GDPR and data-sovereignty preferences in tool choice, and any European or French academic search or AI tools and how they are funded.
>
> If the WebSearch tool is unavailable or its budget is spent, fetch primary pages directly (vendor pricing pages, library pages, consortium and ministry pages, survey reports, papers) and use public APIs (Europe PMC, arXiv, OpenAlex) gently: one request at a time, a modest number in total; say in the report how you searched and how many API requests you made. Return a report under 1,400 words: findings with URL, date/venue, exact prices or figures with their currency and date, and whether you read the primary source or only a summary (tag [P] primary page/document, [A] abstract or landing page only, [S] search-engine summary or third-party write-up only). Say "not found" plainly where you searched and found nothing. Do not invent prices, numbers or citations; flag anything uncertain.
