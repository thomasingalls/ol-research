# The industry around the corpus

**Status:** notes, 2026-09-16 and 2026-09-17 — web research made during
one conversation (searches and page reads only; nothing registered,
nothing downloaded but one public PDF), recorded at the user's request
"alongside relevant supporting research", because "the sellers' fears
are important to keep track of". Context, not a plan: who gives access to
scholarly full text and on what terms, what publishers and their vendors
fear, and readings of what that means for this repository. Companion to
[product shapes](product-shapes.md), whose funhouse-mirror disclaimer
applies to every reading here, to the [retrieval
frontier](retrieval-frontier.md), and to [AI as science
assistant](science-assistant-landscape.md). Facts are marked **P** (primary: the
organisation's own page, post or filing, read), **S** (secondary:
press, blogs, or search summaries of pages that refused the reader) or
**U** (could not verify); **from memory** where nothing was checked.
Readings are marked as readings; the user's positions as theirs.

## 1. Why this note

The user wants a wedge and something to grow into a moat, and does not
want to be at the mercy of publishers, "who are fickle and defend their
moats rather ruthlessly". The questions behind the research: whether the
repository's technical efficiency is an edge in late 2026; how anyone
gets agents or people at paywalled text; who has deals with whom; and
what the publishing industry itself says is happening.

## 2. The user's positions, 2026-09-17

- **"The AI labs could build this" is not a reason not to try:** they
  could enter any vertical they choose.
- **The judgment layer is not much of a moat:** Semantic Scholar and
  OpenAlex both have a good one.
- **Serving agent volume needs more than one box,** and real numbers —
  end-to-end latency, queries a second, concurrent users — before "higher
  limits for less" can be claimed. The incumbents' rate limits exist to
  push would-be scrapers to the bulk downloads, which is cheaper for
  everyone.
- **Concept trails are derivative work in the vein of LLM training
  data:** wholly new, perhaps defensible as a legal theory, not a plan.
  They could lead searchers to the exact paywalled article faster than
  trawling.
- **Rental as a route to text:** DeepDyve, Reprints Desk and perhaps CCC
  might give agents or people found text at lower cost. The user's
  assessments: scite was doing smart things, and Research Solutions'
  leadership is a worry; DeepDyve has stayed afloat through cunning,
  cost-effective product choices but is slow, with some eighteen years
  of technical debt on a small team; CCC is the behemoth holding
  everyone's access data, with the worst user experience in the history
  of user experience, trying but not getting there.
- **Private deployment** of the search as an on-premises module:
  interesting, probably not enough as it is.
- **Consensus:** never understood the appeal; early into OpenAI's GPT
  store, it looked like a low-sophistication operation riding a market
  change — but with publisher deals it has to be taken seriously.
- **Publishers going straight to the AI platforms** is irritating and
  obvious: they cut out third parties that might eat their margins,
  whether the losses are real or hypothetical — publishers simply do not
  serve a great many people because of their pricing.
- **Paid access at query time:** the user was mostly unaware such deals
  were happening; publishers tend to be averse to giving out their
  corpora at any price.
- **"Zero-click"** is funny: one-click rental was a scary, revolutionary
  idea not long ago.
- **The industry is unbelievably out of touch with its customers** — the
  JAMA story (§8.2) prompted it, and the user has a long list of reasons.
- **Licensing snippets of papers** (§6) would drag the industry back to
  something like the time when everything that mattered had to be done
  in Latin, a language nobody spoke.
- **Peter Webster is right** (§11); anyone paying attention would agree.
- **Reference lists:** doubt that publishers' closed terms legitimately
  cover the references at the end of a paywalled article. A tangent, not
  actionable.

## 3. Edge and moat, as discussed

Readings of 2026-09-17, with the user's corrections of §2 taken in.

- **Efficiency is an edge, not a moat.** Code is cheap now and the stack
  is open parts, so it can be copied; jevlike copied Jev's shape in a day
  ([tracer's engine](future-ambitions-tracer-engine.md)). What it buys:
  pricing and freshness others cannot afford, independence from outside
  money on a small fixed cost, and a faster rate of corpus-wide fixes,
  since a whole rebuild is hours.
- **Not a data moat.** Generic judgments are commoditised (§2); judgments
  per question are only a moat if usage builds them, and concept trails
  are a legal theory.
- **What survived:** being the finder, not the holder — verification and
  resolution against a merged, fresh, provenance-keeping corpus;
  discovery that routes people and agents to the text; reading through
  the customer's own entitlements; private deployment where queries are
  confidential (pharma and biotech, as understood rather than verified).
  The moat, if any, looks like position rather than data.

## 4. Full text: who gives access, and how

### 4.1 Intermediaries

- **Research Solutions** (Reprints Desk's parent). Acquired scite in
  December 2023 (P). Launched the Article Galaxy MCP on 2026-04-07: AI
  agents and developer tools search the literature, check access rights
  and pricing, and acquire articles in one workflow; one of four pieces
  of an agentic toolkit with the Scite API, the Scite MCP and the Article
  Galaxy API (P). The one company already holding discovery, citation
  statements, rights and purchase for agents.
- **DeepDyve.** A rental service with over 35 million full-text streaming
  papers as of 2026-01-01; publisher partners include Springer, Wiley and
  Oxford University Press; Elsevier is not in its collection; it runs its
  own AI chat over the collection on OpenAI and Anthropic models (S,
  from its own pages through search summaries).
- **CCC.** RightFind shows per article whether AI/ML use is licensed (S,
  a customer's integration docs); an AI Systems Training License for
  organisations (P); announced 2026-03-03, internal-only AI reuse rights
  for US academic customers from 2026-07-01 and transactional AI
  licensing (S, the press release through a news site).

### 4.2 The British Library

Rhysida ransomware struck in October 2023 (S). An update to the UK
interlending forum, given November 2025 and posted January 2026 (P): the
Library is moving to Alma and Primo (catalogue live 8 December) and to
Rapido for document supply, with a phased launch aimed at spring 2026
(whether it happened: U); worldwide location searches will not return;
the copyright-fee-paid service is being reworked, the Library having had
over 80 separate publisher licences plus CLA and CCC agreements, which it
calls an administrative nightmare to avoid in future; EThOS gets an
interim platform linking out to university repositories, file downloads
to be restored from 2026. A service being rebuilt for libraries, not a
supply channel for agents.

### 4.3 The newer players

- **Consensus: direct publisher deals, and LibKey.** §7.
- **Elicit: the customer's entitlements.** Its browser extension uses the
  user's active logins to download papers their institution gives them;
  credentials stay local and are not stored by Elicit (P). Uploaded PDFs
  stay private and are not added to its corpus (S). No publisher
  agreement mentioned.

### 4.4 Publishers direct to the platforms

Wiley's AI Gateway, launched October 2025, puts Wiley content into
Anthropic's Claude, Perplexity, Mistral's Le Chat and AWS Marketplace;
Sage and ASM joining (S, press coverage of Wiley's release).

### 4.5 Routing plumbing

Secondhand throughout, mostly Aaron Tay's writing (S): LibKey (Third
Iron) sends a user to their library's subscribed copy and is integrated
by Consensus, scite and Litmaps; GetFTR (publisher-run) resolves
entitlement and is reportedly positioning an MCP server for agents;
Semantic Scholar tries GetFTR, then LibKey; scite's MCP returns access
options — proxy links, interlibrary loan, purchase through Article
Galaxy.

### 4.6 Three strategies

| strategy | who | cost to a newcomer |
|---|---|---|
| direct publisher deals | Consensus; publishers themselves through Wiley's AI Gateway | dependence per publisher; less leverage as publishers sell to the platforms directly |
| the customer's own entitlements | Elicit; LibKey and GetFTR as routing | none owed to publishers; access as good as the customer's subscriptions |
| purchase or rental per article | Article Galaxy MCP; DeepDyve | an intermediary's margin; machine use is a separate right, priced by publishers |

Reading: the second row fits not being at publishers' mercy — find,
verify, hand the reader to their own access, with purchase from an
intermediary as the fallback; the text is never held or licensed.

## 5. Publishers and open metadata

- **References in Crossref are open.** The board voted on 2022-03-10 to
  remove the reference distribution policy; from 2022-06-03 every
  deposited reference is open metadata and no member can restrict it (P).
  The user's doubt of §2 stands only for references never deposited.
  In the US facts are not copyrightable (Feist, from memory); the EU's
  database right is the complication. Not legal advice.
- **Abstracts in OpenAlex are shrinking.** Since November 2024 Elsevier
  and Springer Nature have restricted abstracts of non-open-access
  articles; coverage reportedly fell from about 80 % to below 40 %, and
  to 22.5 % for Elsevier articles of 2022–2024 (S: Library Learning
  Space, 2025-09-30, citing Aaron Tay). For this repository: OpenAlex
  gave 26,422 of the sample's abstracts; PubMed is the steadier source
  for biomedicine; coverage by publisher is to be measured on the corpus.

## 6. Licensing deals

- **A tracker** of deals licensing scholarly content to LLM companies:
  Wiley, Taylor & Francis, Oxford, Cambridge and Sage; Elsevier absent
  (P: Schonfeld, Dayan and Ruediger in the Scholarly Kitchen,
  2024-10-15).
- **Figures.** Wiley's $23 million deal (P: Carpenter in the Kitchen,
  2024-09-04); Wiley's Josh Jarrett reporting about $40 million so far,
  mostly from books (P: Carpenter's interview, 2024-10-31); Taylor &
  Francis with Microsoft reported at about $10 million in its first year
  (S; from memory as an Informa disclosure). One secondary site credits
  a $23 million figure to Springer Nature and Google; the Wiley
  attribution is the one with a named source.
- **The shift the Kitchen describes:** from one-off training deals toward
  paid access at the moment of a question — retrieval through MCP and
  subscriptions — with the divide between content baked into a model's
  weights and content fetched at query time (§8.2).
- **Snippet licensing.** Josh Nicholson of Research Solutions and scite
  proposes licensing citation snippets for retrieval instead of full
  text, with usage reporting in the manner of COUNTER (P: Kitchen guest
  post, 2025-08-07). The user's reaction is in §2.

## 7. Consensus, the company

- **Who.** Founded 2021 by Eric Olson (chief executive) and Christian
  Salem (chief product officer), Northwestern football teammates; public
  launch 2022; about 26 people on its About page, which says it moved to
  San Francisco (P). Olson from data science at DraftKings, Salem from
  product at the NFL (U).
- **Money raised,** about $45 million: a $3 million seed in April 2023
  led by Draper Associates (S); an $11.5 million Series A on 2024-08-14
  led by Union Square Ventures with Nat Friedman and Daniel Gross (P); a
  $30 million Series B on 2026-05-11 led by GreatPoint Ventures, DJ Patil
  joining the board (P).
- **Traction, self-reported.** About 200,000 registered users in April
  2023 (S); a featured GPT when OpenAI's store opened on 2024-01-10 (S);
  400,000 monthly active users and nearly $2 million annualised revenue
  in August 2024 — the only revenue figure (P); 2.5 million monthly
  active users in May 2026 (P); "10M+ users" and 12,500 universities
  represented now (P); over 100 university customers (S).
- **Model.** Freemium: Pro at $20 a month (P, via search); a Deep tier
  at more (S); teams from April 2024 (P); campus licences quoted
  individually (P); an API at $0.05 a call beyond plan credits, aimed at
  R&D teams, with an MCP server (P); running inside ChatGPT, Claude and
  Microsoft 365 Copilot from 2026-09-14 (P).
- **Product.** Deep Search 2025-07-09 (P); a GPT-5 Scholar Agent around
  October 2025 (S); a launch week 2026-05-11 to 15 with agent mode, MCP
  and API, a reference manager with Zotero import, a citation graph and
  publisher full text (P).
- **Retrieval, as it describes it** (P, June 2025): Semantic Scholar,
  OpenAlex, its own crawl and publisher full text; embeddings with BM25;
  the top 1,500 reranked by recency, citations and journal reputation; a
  stronger model on the top 20. "Over 400M scholarly sources" claimed in
  September 2026 (P).
- **Publisher deals.** ACS, November 2025 — ACS's own post calls it a
  pilot over a curated selection (P); Sage, 2026-01-13 — full text only
  for relevance and short cited previews, no training, announced by
  Consensus only (P); Taylor & Francis, 2026-05-15, with click-throughs
  claimed to have more than doubled (P, Consensus only); De Gruyter
  Brill, 2026-08-05, journals only (P). Wiley, APA, AAAS and OUP are
  named in its 2026 posts with a claim of seven of the ten largest
  publishers; no dated announcement or publisher confirmation found (U).
  Its pitch: it drives discovery of publishers' content rather than
  training on it (P).
- **Criticism.** Aaron Tay (S, 2025-11-15): among the top tools for its
  interface, but its consensus meter is vote-counting, journal scores
  are a shaky quality signal, study-type labels can be wrong, paywalled
  papers are mostly analysed from title and abstract, results are hard to
  reproduce, and it suits undergraduates best. A Cureus review (July
  2025) found little empirical evidence about it (S). Its benchmarks
  against Google Scholar and frontier models are its own (P).
- **Reading.** A distribution-first company: early reach through the GPT
  store, students and clinicians, now sold to publishers as referral
  traffic — a low-risk pilot for a publisher (previews, no training,
  clicks back) and exactly what a publisher losing clicks to AI
  summaries wants to hear. Publishers partner with its audience, not its
  technology. Weak on method, strong on distribution, and competing with
  ChatGPT and Claude for the same users.

## 8. The Scholarly Kitchen, 2024 to September 2026

Every post below was read (P); summaries are paraphrase.

### 8.1 Orientation

The Society for Scholarly Publishing's blog, written for the industry.
From 2025 most of its writing on AI and discovery is guest posts by
companies selling into the space — Silverchair, Wiley and Atypon,
Research Solutions and scite, LibLynx, Cashmere. The editor, David
Crotty, is openly sceptical of AI. Chefs writing on these themes include
Todd Carpenter, Roy Kaufman (CCC), Lisa Janicke Hinchliffe, Roger
Schonfeld, Charlie Rapple, Stephanie Lovegrove Hansen, Hong Zhou, Roohi
Ghosh, Alison Mudditt, Phill Jones and Angela Cochran. The industry
talking to itself: what sellers fear, not what buyers want.

### 8.2 Themes

- **Discovery has moved into AI tools,** taken as settled: students start
  there; keyword search persists beside AI search; content must be
  "AI-ready" — structured, with persistent ids and clear licence signals
  (Zhou and Bishtawi, 2024-04-30, 2024-06-04, 2026-01-06; Jones,
  2025-10-13; Jiang, 2026-09-01; Harvey and Reid of Wiley, 2026-09-03,
  citing Consensus at 2.5 million monthly users).
- **Zero-click.** Click-through falls by up to about two thirds where AI
  Overviews appear and AI answers strip journal branding (Kaube and
  Smith, 2025-07-07); appearances in search rise while clicks fall, OUP
  reporting 19 % fewer click-throughs to reference content, which weakens
  the usage evidence behind renewals; calls for new usage and indexing
  standards and joint work through SSP, STM and NISO (Rapple,
  2026-02-04); COUNTER counts only human use on the publisher's own
  platform, so "AI reads" and "AI citations" are proposed (Urberg of
  LibLynx and Bendall of Research Solutions, 2026-02-12); membership and
  trust as what societies can defend (Kaube and Smith, 2026-03-10);
  worries about comprehension, opaque gatekeeping and the Global South
  (Ghosh, 2026-05-06).
- **Licensing.** Where a licensing market exists, fair use is harder to
  claim (Kaufman, 2024-05-14; on *Thomson Reuters v. Ross*, 2025-02-20);
  attribution should be a mandatory term (Carpenter, 2024-09-04); a model
  licence framework is wanted (Carpenter, 2025-02-26); content can be
  licensed in parts and exposed through MCP (Showers, Kotula and Hlava,
  2026-02-24). From fourteen confidential interviews with executives
  (Toler and Cochran, 2026-03-11): the divide is content in a model's
  weights against content fetched at query time; large retrieval
  platforms such as OpenEvidence turn publishers into suppliers; no
  pricing norms; pharma buying, academia absent; publishers understaffed;
  some betting on authority rather than content. Subscription access at
  query time preferred to one-off training deals, and a warning that
  every publisher building its own AI leaves a fragmented field
  (Hinchliffe interviewing Toler, 2026-06-10).
- **Machines as readers.** About half of Silverchair's platform traffic
  is automated; training crawlers should be treated apart from agents
  acting for a user; JAMA Network gained by letting selected agents in;
  9 % of one webinar's attendees were evaluating MCP (Hansen,
  2026-05-27). Platform strategy (Leitch of Silverchair, 2025-12-19);
  headless content and MCP endpoints with explicit licences, attribution,
  payment and governance still open (Reid of Wiley and Atypon,
  2026-08-21); publishers already lost control of aggregation, then of
  discovery, and need shared standards in the manner of Crossref for
  authenticated users, licensed retrieval, attribution and reporting
  (Queen of JHU Press and Woahn of Cashmere, 2026-07-30). Bots: filter,
  do not block, with separate and some paid routes (Lloyd of LibLynx,
  2026-05-07); bots knocking collections offline (Hinchliffe with
  Weinberg, 2025-06-23); open content and harvesting (Dohe, 2025-10-07).
- **Metadata.** AI search tools lean on OpenAlex, Crossref, Semantic
  Scholar, PubMed and arXiv; publishers cutting abstracts moves the wrong
  way; metadata derived from full text and shared as knowledge graphs or
  embeddings would protect paywalled text (Webster, 2026-05-29; §11). AI
  using openly licensed content welcomed, with machine-readable
  provenance and better funding for Crossref asked (Mudditt of PLOS,
  2026-03-31); metadata gaps blamed on production workflows rather than
  withholding (Johnson, 2026-06-11).

### 8.3 Disagreements

- **Zero-click as threat or filter:** Rapple, Kaube and Smith, and Ghosh
  see a threat; Hansen sees a filter leaving higher-intent traffic, to
  be measured by engagement and owned channels (2026-05-18), and a
  commenter replies that libraries still renew on COUNTER.
- **Licensing or fair use:** Carpenter and Kaufman back licensing; Rick
  Anderson holds training is fair use, and Haseeb Irfanullah compares it
  to academia's own reliance on fair use (Ask the Chefs, 2026-09-10).

## 9. The sellers' fears

As the Kitchen's authors state them, in the industry's own framing:

1. **Lost referrals.** AI answers keep readers off publisher platforms.
2. **Invisible usage.** COUNTER does not see AI use, and renewals are
   argued on COUNTER.
3. **Lost brand.** AI answers strip the journal's name from the finding.
4. **A third loss of control.** After aggregation and then discovery,
   how AI takes part in scholarship.
5. **Becoming a supplier.** Large retrieval platforms set the terms.
6. **No pricing norms,** and too few staff to build or negotiate.
7. **Fragmentation.** Every publisher its own AI, none of them used.
8. **Fair use undercutting licensing,** if courts or the market go that
   way.
9. **Bots.** Crawlers loading collections, and no settled way to tell a
   trainer from an agent acting for a reader.
10. **Small and society publishers** unable to respond alone.
11. **Academia absent as a buyer;** pharma the one buying.

## 10. The Kitchen's blind spots

- Almost nothing on Undermind, Ai2's Asta or the deep-research agents;
  Perplexity only in passing.
- GetFTR, LibKey and Seamless Access barely appear in an AI context; the
  plumbing is discussed as MCP, bot filtering and usage counts.
- Nothing on article rental, DeepDyve or CCC's RightFind in an AI
  context.
- Publishers restricting abstracts in open indexes: one passing mention
  in a guest post, no debate, no defence.
- Readers who are not served because of price — the user's point of §2 —
  are absent; the problem is framed as lost usage evidence, not unserved
  readers.
- Buyers' voices are rare: the writing is sellers and their vendors.

## 11. Peter Webster

Asked by the user: who he is and how he reached the conclusion of
§8.2.

- **Who** (P: the post's biography; Saint Mary's University news,
  2026-05-25). Joined Saint Mary's University, Halifax, in 1993 as its
  information technology services librarian; associate university
  librarian 2010–2023; led technology and digital library development,
  electronic resource management, and regional and national licensing
  through the consortia Novanet and CAAL-CPUA, the Canadian Research
  Knowledge Network and the Digital Research Alliance of Canada; more than
  35 articles and chapters on library systems and information access;
  named the university's first librarian emeritus in May 2026. An MLS
  from Dalhousie, a BA from Alberta.
- **The earlier piece** (P: InfoToday Europe, 2024-05-07, read through a
  summary). On whether AI search can turn keywords into concepts: a real
  study's search needed dozens of connected keywords, far beyond what
  users type; his testing of Semantic Scholar found a large share of
  records with no abstract, leaving the AI only titles; well-curated open
  sources such as PubMed and ERIC skew results because their metadata is
  more complete; controlled vocabularies are mostly absent. Recommended
  interoperable metadata standards, automated enrichment and exchange
  between platforms.
- **The Kitchen post** (P, 2026-05-29). AI search tools miss important
  articles and misread concepts because they work from titles and
  abstracts; research shows abstract-only search lacks sensitivity; then
  Tay's report that Elsevier and Springer Nature are cutting the
  abstracts they share. His answer: metadata derived from full text,
  shared as knowledge graphs and embeddings — Semantic Scholar's S2AG and
  SPECTER as examples — so paywalled text stays protected; publishers,
  OpenAlex, Crossref and NISO to agree cross-publisher standards; the
  publishers' concerns — protection, attribution, revenue — named as
  what cooperation must answer.
- **How he got there** — a reading: three decades on both sides of the
  supply chain, negotiating licences and running the systems readers
  search through. His 2024 tests showed concept search is limited by
  metadata; publishers then cut the abstracts; once abstracts are no
  longer the source, derived structure from full text is the one route
  that does not ask publishers to release the text itself. A buyer's
  voice in a sellers' venue, and his 2024 question — concepts from
  keywords — is the concept tracer's problem.

## 12. Readings

2026-09-17; the product-shapes disclaimer applies.

- **Publishers' pain is lost referrals and invisible usage.** Consensus
  earns its deals by selling referrals. A finder that sends attributable,
  measurable traffic to the exact article, and reports it in the terms
  publishers already count, speaks their language without needing their
  permission.
- **Nobody in their conversation is routing agents to the customer's own
  entitlements.** Either an opening, or a sign nobody pays for it yet;
  worth finding out which.
- **Derived layers are gaining legitimacy inside the industry.** Webster's
  proposal is the shape concept trails would take: built from facts and
  structure, not reproduced text.
- **Remember whose voice this is.** The Kitchen tells what sellers fear,
  not what buyers want.
- **Rental and machine use.** Renting a paper for a person to read is one
  right; letting an agent read it is another, licensed and priced by
  publishers — so renting text for agents puts a newcomer back in
  publishers' reach through the intermediaries.

## 13. Open questions

1. Whether anyone pays for routing agents to their own entitlements, and
   who (§4.5, §12).
2. Which publishers would welcome attributable referrals from a finder,
   and in what reporting form — COUNTER, or the "AI reads" proposed
   (§8.2).
3. OpenAlex's abstract coverage by publisher, measured on the corpus
   (§5).
4. Whether private deployment has buyers, and what it would need to be
   enough (§2, §3).
5. What the user's long list of ways the industry is out of touch with
   its customers says about where a product should stand (§2).

## Sources

Access and intermediaries:
[Research Solutions: Article Galaxy MCP (2026-04-07)](https://researchsolutions.investorroom.com/2026-04-07-Research-Solutions-Announces-New-Article-Galaxy-MCP,-Giving-AI-Agents-The-Ability-To-Find,-License,-Access-Scientific-Literature);
[Research Solutions: spring 2026 updates](https://www.researchsolutions.com/resources/release-notes/spring-2026-updates);
[Research Solutions: acquisition of scite](https://www.researchsolutions.com/resources/press-releases/research-solutions-announces-acquisition-of-scite);
[DeepDyve: publisher partners](https://www.deepdyve.com/corp/partners/publishers);
[DeepDyve: pricing](https://www.deepdyve.com/pricing);
[CCC: AI Systems Training License](https://www.copyright.com/solutions-ai-systems-training-license/);
[CCC: AI re-use rights for US academic customers](https://www.morningstar.com/news/business-wire/20260303630177/ccc-launching-new-ai-content-re-use-rights-for-us-academic-customers-and-transactional-licensing-capabilities-for-ai);
[Nested Knowledge: RightFind usage rights](https://about.nested-knowledge.com/docs/rightfind-integration/);
[British Library update, Forum for Interlending](http://forumforinterlending.org.uk/wp-content/uploads/2026/01/BL-Update-Nov.pdf);
[British Library: cyber-attack recovery](https://www.bl.uk/about/cyber-attack);
[Elicit: browser extension](https://support.elicit.com/en/articles/14759113-elicit-browser-extension);
[Elicit: privacy for uploaded papers](https://support.elicit.com/en/articles/14758043-privacy-for-uploaded-papers);
[Wiley AI Gateway launch](https://secure.businesswire.com/news/home/20251013329806/en/Wiley-Launches-Interoperable-Platform-to-Power-Scientific-Discovery-in-Worlds-Leading-AI-Technologies);
[Library Technology Guides: Wiley AI Gateway](https://librarytechnology.org/pr/31824);
[Third Iron: LibKey integration](https://thirdiron.com/libkey-integration/);
[Aaron Tay: GetFTR, LibKey and linking](https://aarontay.substack.com/p/a-relook-at-getftr-libkey-exlibris);
[Scite MCP](https://scite.ai/blog/introducing-scite-mcp).

Metadata and licensing:
[Crossref: open reference distribution](https://www.crossref.org/blog/amendments-to-membership-terms-to-open-reference-distribution-and-include-uk-jurisdiction/);
[Crossref: distributing references](https://www.crossref.org/blog/distributing-references-via-crossref/);
[Library Learning Space: publishers close access to abstracts](https://librarylearningspace.com/publishers-close-access-to-scholarly-content-such-as-abstracts-due-to-ai-incentives/);
[CASRAI: publisher AI licensing deals, 2026 (secondary, conflicting)](https://casrai.org/news/scholarly-publisher-ai-licensing-deals-2026).

Consensus:
[About](https://consensus.app/home/about-us/);
[Series B](https://consensus.app/home/blog/30m-in-new-funding-to-reach-the-next-10m-researchers/);
[Series A](https://www.prweb.com/releases/consensus-raises-11-5-million-in-series-a-funding-to-make-the-worlds-scientific-research-discoverable-and-consumable-302222612.html);
[USV](https://blog.usv.com/consensus);
[seed, VentureBeat](https://venturebeat.com/ai/consensus-raises-3m-partners-with-openai-revolutionize-scientific-web-search);
[how Consensus works](https://consensus.app/home/blog/how-consensus-works/);
[API](https://consensus.app/home/api/);
[launch week](https://consensus.app/home/launch-week/);
[Consensus everywhere](https://consensus.app/home/blog/consensus-everywhere/);
[ACS partnership](https://consensus.app/home/blog/consensus-partners-with-acs-to-bring-trusted-research-into-the-ai-era/);
[ACS Axial](https://axial.acs.org/publishing/acs-publications-se-asocia-con-consensus-para-hacer-la-literatura-cientifica-mas-accesible);
[Sage partnership](https://consensus.app/home/blog/sage-partners-with-consensus-to-bring/);
[Daily Northwestern](https://dailynorthwestern.com/2024/04/16/lateststories/northwestern-football-alumni-create-ai-research-start-up-consensus/);
[Aaron Tay: Consensus deep dive](https://aarontay.substack.com/p/a-2025-deep-dive-of-consensus-promises);
[Cureus review](https://pmc.ncbi.nlm.nih.gov/articles/PMC12318603/).

The Scholarly Kitchen:
[AI use in the Kitchen](https://scholarlykitchen.sspnet.org/2026/08/27/ai-use-in-the-scholarly-kitchen/);
[year in review 2025](https://scholarlykitchen.sspnet.org/2026/01/05/the-year-in-review-2025-in-the-scholarly-kitchen/);
[impact of AI on discovery](https://scholarlykitchen.sspnet.org/2024/04/30/the-impact-of-ai-on-information-discovery-from-information-gathering-to-knowledge-application/);
[conversational discovery](https://scholarlykitchen.sspnet.org/2024/06/04/towards-conversational-discovery-new-discovery-applications-for-scholarly-information-in-the-era-of-generative-artificial-intelligence/);
[keywords are not dead](https://scholarlykitchen.sspnet.org/2026/01/06/keywords-are-not-dead-but-discovery-is-no-longer-just-search/);
[three years after ChatGPT](https://scholarlykitchen.sspnet.org/2025/10/13/three-years-after-the-launch-of-chatgpt-do-we-know-where-this-is-heading/);
[trends beyond scholarly comms](https://scholarlykitchen.sspnet.org/2026/09/03/guest-post-a-year-of-trends-beyond-scholarly-comms-what-you-need-to-know/);
[the human layer](https://scholarlykitchen.sspnet.org/2026/09/01/guest-post-the-human-layer-why-ai-makes-academic-libraries-more-essential-not-less-2/);
[licensing and exclusive rights, AI edition](https://scholarlykitchen.sspnet.org/2024/05/14/the-interplay-between-copyright-licensing-and-exclusive-rights-ai-edition/);
[copyright's big win](https://scholarlykitchen.sspnet.org/2025/02/20/copyrights-big-win-in-the-first-decided-us-artificial-intelligence-case/);
[attribution in AI licensing](https://scholarlykitchen.sspnet.org/2024/09/04/make-attribution-mandatory-in-ai-licensing/);
[tracking licensing to LLMs](https://scholarlykitchen.sspnet.org/2024/10/15/licensing-scholarly-content-llms/);
[Wiley leans into AI](https://scholarlykitchen.sspnet.org/2024/10/31/wileys-josh-jarret-interview-about-impact-of-ai/);
[model licensing framework](https://scholarlykitchen.sspnet.org/2025/02/26/we-could-use-a-model-licensing-framework-for-ai-tools/);
[licensing articles for AI](https://scholarlykitchen.sspnet.org/2025/08/07/guest-post-a-smarter-way-to-license-research-articles-for-ai/);
[AI readiness](https://scholarlykitchen.sspnet.org/2026/02/24/guest-post-ai-readiness-and-the-new-value-equation-in-scholarly-publishing/);
[leaders off panels](https://scholarlykitchen.sspnet.org/2026/03/11/what-publishing-leaders-say-about-ai-when-theyre-not-on-panels/);
[scholar-ready AI](https://scholarlykitchen.sspnet.org/2026/06/10/building-scholar-ready-ai/);
[Ask the Chefs: AI and copyright licensing](https://scholarlykitchen.sspnet.org/2026/09/10/ask-the-chefs-ai-and-copyright-licensing/);
[AI search shortcomings and metadata](https://scholarlykitchen.sspnet.org/2026/05/29/guest-post-scholarly-ai-search-shortcomings-and-the-need-for-better-metadata/);
[what AI asks of open access](https://scholarlykitchen.sspnet.org/2026/03/31/what-ai-asks-of-open-access/);
[leaky metadata pipeline](https://scholarlykitchen.sspnet.org/2026/06/11/guest-post-fixing-the-leaky-metadata-pipeline-a-conversation-with-the-creator-of-research-nexus-score/);
[the user has changed](https://scholarlykitchen.sspnet.org/2026/05/27/the-user-has-changed-has-scholarly-publishing/);
[platform strategy](https://scholarlykitchen.sspnet.org/2025/12/19/guest-post-how-ai-is-transforming-platform-strategy-beyond-the-hype/);
[next reader is a machine](https://scholarlykitchen.sspnet.org/2026/08/21/guest-post-your-next-reader-is-a-machine-and-we-are-ready/);
[third time is not the charm](https://scholarlykitchen.sspnet.org/2026/07/30/guest-post-the-third-time-is-not-the-charm-who-will-own-how-ai-participates-in-scholarship/);
[AI bots and digital collections](https://scholarlykitchen.sspnet.org/2025/06/23/are-ai-bots-knocking-digital-collections-offline/);
[proved you're human](https://scholarlykitchen.sspnet.org/2025/10/07/guest-post-have-you-proved-youre-human-today-open-content-and-web-harvesting-in-the-ai-era/);
[love, death and robots](https://scholarlykitchen.sspnet.org/2026/05/07/guest-post-love-death-robots-scholarly-edition/);
[when the front door moves](https://scholarlykitchen.sspnet.org/2025/07/07/guest-post-when-the-front-door-moves-how-ai-threatens-scholarly-communities-and-what-publishers-can-do/);
[taming the crocodile](https://scholarlykitchen.sspnet.org/2026/02/04/responding-to-the-threat-of-zero-click-search-and-ai-summaries-how-do-we-tame-the-crocodile/);
[the elephant in usage reports](https://scholarlykitchen.sspnet.org/2026/02/12/guest-post-theres-an-elephant-in-the-room-but-not-in-your-usage-reports/);
[societies 2030](https://scholarlykitchen.sspnet.org/2026/03/10/guest-post-societies-2030-the-community-advantage-in-an-ai-first-world/);
[zero-click readership](https://scholarlykitchen.sspnet.org/2026/05/06/zero-click-readership-are-ai-overviews-changing-the-way-we-discover-research/);
[filter, not funnel](https://scholarlykitchen.sspnet.org/2026/05/18/filter-not-funnel-what-zero-click-means-for-brand-and-engagement/).

Peter Webster:
[Saint Mary's University news](https://news.smu.ca/news/2026/5/25/from-card-catalogues-to-ai-peter-webster-named-saint-marys-universitys-first-librarian-emeritus);
[faculty profile](https://www.smu.ca/research/profiles/faculty/Webster00.html);
[InfoToday Europe, 2024](https://www.infotoday.eu/Articles/Editorial/Featured-Articles/AI-and-the-problem-of-identifying-concepts-from-keywords-in-scholarly-article-search-163892.aspx).
