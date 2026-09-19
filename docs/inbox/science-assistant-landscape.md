# AI as science assistant: the category, September 2026

**Status:** notes, 2026-09-17 — four parallel web surveys and a follow-up
the same day (searches and page reads only; nothing registered or
downloaded), recorded at the user's request. Context, not a plan: who is
building AI assistants for science, what the evidence says works, and
where the money is. Companion to [the industry around the
corpus](industry-landscape.md), [the retrieval
frontier](retrieval-frontier.md) and [product shapes](product-shapes.md),
whose funhouse-mirror disclaimer applies to every reading here. Facts are
marked **P** (primary: the organisation's own page, paper, filing or
repository), **S** (secondary: press, analysts, aggregators, search
summaries) or **U** (could not verify). Companies' own figures are
self-reported whatever the mark. Readings are marked as readings; the
user's positions as theirs.

## 1. The question

The user's framing: the category is contested and nobody really knows
what works yet — partly because nobody has predicted LLM capability well
(the labs' bets looking, from outside, closer to "models will become
infinitely capable", itself an iffy bet), partly because nobody knows how
scientists will settle into adopting AI. There is a large graveyard of
half-baked versions; a "Cursor for science" would run close to direct
competition with the labs and their spinoffs; and funding reportedly
dried up because everyone ran into the expense of physical reality — you
cannot know things in science without doing experiments outside the
computer.

## 2. The user's reactions, 2026-09-17

- **Clarivate's full-text Web of Science reader:** they could have just
  bought DeepDyve.
- **Undermind being tiny:** it is not always the good products that win.
- **scite at about $21 million:** not small for this space; a decent exit.
- **Periodic Labs at $7.5 billion and Lila at $8.5 billion** in talks:
  surprise.
- **The Erdős episode** that turned out to be literature search: which
  shows literature search is valuable.
- **Benchmarks:** very interesting — but numbers cannot be read without
  asking how hamstrung the model was by the test setup: with a harness,
  or the model alone (§8).
- **Altman and Amodei** are the least credible people to quote on this;
  though Amodei openly unsure about scientific discovery is a wet blanket
  beside his statement, as the user recalls it, that AI should solve
  cancer to regain public trust (U).
- **OpenAI dissolving its science unit:** seismic, and news to the user.
- **"7 % use it regularly" against "77 % of PubMed Central papers":**
  something does not add up (§4.3).
- **Adoption** being slow sounds right.
- **Funding:** dried up for new entrants, generally. The money has placed
  its bets and is concentrating around the survivors, worthwhile products
  or not; and investment has other competition — treasuries, energy,
  defence, memory and GPUs.
- **Cloud labs closing:** is Emerald Cloud Lab still around? (§3.6)
- **The interface going to the labs:** the labs genuinely have a hard
  time with user experience, so there is still space; distribution will
  be hard to compete with; the token subsidies are the real deal-breaker.
- **OpenAlex's 1.5 billion API calls a month:** about 600 queries a
  second on average, peaks likely much higher — a very informative number
  to keep track of (§6). Under a million in annual recurring revenue is
  informative too, but OpenAlex is not in it for the money; it needs to
  keep the lights on.

## 3. The map

### 3.1 The model labs

- **Anthropic.** Claude for Life Sciences, 2025-10-20, with connectors to
  Benchling, 10x Genomics, PubMed, BioRender, Synapse.org and Wiley's
  gateway (P); extended 2026-01-11 with Medidata, ClinicalTrials.gov,
  bioRxiv and medRxiv, Open Targets, ChEMBL and Owkin (P). Claude Science,
  a desktop research workbench in beta, 2026-06-30: sixty-plus scientific
  skills, a reviewer agent, NVIDIA BioNeMo and Modal integrations, credits
  for up to fifty projects at $30,000 each (P). Ten thousand free or $15
  seats for academic principal investigators, 2026-08-27 (P). John Jumper
  announced his move from DeepMind to Anthropic on 2026-06-19 (P).
- **OpenAI.** OpenAI for Science announced by Kevin Weil on 2025-09-02
  (P). Prism, a free LaTeX writing workspace on GPT-5.2, built on the
  acquired Crixet, 2026-01-27 (S). GPT-Rosalind for life sciences through
  trusted access, about 2026-04-16 (S), and a Rosalind Workbench in
  research preview (P). On 2026-04-17 Weil left and OpenAI for Science
  was "decentralized" into other research teams (P). Altman's internal
  goals of 2025-10-29 — an automated AI research intern by September 2026,
  an automated AI researcher by March 2028, with "may totally fail" (P);
  in September 2026 OpenAI declared the intern goal met, self-graded, with
  86 % success on tasks under fifteen minutes and more than half of four-
  to eight-hour tasks needing a human (S).
- **Google and DeepMind.** AI co-scientist announced 2025-02-19 (P),
  published in Nature 2026-05-19: leukaemia drug repurposing in cell lines
  only, two of three liver-fibrosis targets working in organoids, a
  bacterial gene-transfer mechanism matching a lab's unpublished result
  (P). Gemini for Science at I/O, May 2026 — hypothesis generation,
  computational discovery on AlphaEvolve, literature insights, skills —
  rolling out gradually (P). A closed-loop version running lab equipment
  reported in August 2026 with a fabrication rate of 4 % with safeguards
  and 46 % without (S).
- **Microsoft Discovery.** Announced at Build 2025-05-19 (P); generally
  available 2026-06-02 with Yale, Georgia Tech, PNNL, Ginkgo, Causaly,
  Wiley, BHP, GSK and others named (P); its non-PFAS coolant claim not
  peer reviewed (P).
- **Others.** NVIDIA and Lilly, a co-innovation lab of up to $1 billion
  over five years, 2026-01-12 (P); Meta's OMol25 and UMA atom models, May
  2025, and no 2026 science assistant found (U); xAI's discovery claims
  unverified (U).

### 3.2 Incumbents that own the data

- **Benchling.** Benchling AI, 2025-10-07: deep research over notebook
  data and literature, drafting entries from files and notes, structuring
  messy data, experiment optimisation, with AlphaFold, Chai-1 and Boltz-2
  bundled (P). Partnership with Anthropic the day Claude for Life Sciences
  launched, Claude reaching Benchling data over MCP with permissions and
  audit kept (P). AI Connectors on MCP, 2026-04-16 (P). AI on credits in
  every subscription; free for verified academics (P). Last priced round
  in 2021 at a $6.1 billion valuation (S); about $210 million annual
  recurring revenue in May 2024 (S).
- **Siemens and Dotmatics.** $5.1 billion, announced 2025-04-02, closed
  2025-07-01, including GraphPad Prism, SnapGene, Geneious and LabArchives
  (P, S); Luma Agent, 2026-05-13 (P).
- **Other notebooks and informatics.** Revvity Signals AI, 2026-06-22,
  select capabilities live (P); Sapio ELaiN, 2025-09-16, and its partner
  ecosystem (P); Labguru and Titian rebranded Cenevo, with agents at SLAS
  2026 (P, S); SciSure formed 2025, buying Labfolder and Labregister (P);
  Danaher bought Genedata, 2024 (P); Certara bought Chemaxon, $96.4
  million, 2024 (P).
- **Knowledge databases.** CAS's new SciFinder, 2025-08-04, and agents
  announced 2025-10-06 (P); Elsevier's Scopus AI 2024, ScienceDirect AI
  2025, Reaxys AI Search 2025, LeapSpace 2025-11-19 with agent features
  2026-06-25 (P); Clarivate's Web of Science Research Assistant 2024 (P),
  Web of Science DeepeR — a full-text reader starting with ProQuest and
  open-access content — announced 2026-09-16 (S), and its Life Sciences &
  Healthcare unit sold to Altaris for $600 million per a July 2026 filing
  (P); Digital Science's Dimensions Research GPT 2024 (P), Overleaf AI
  Assist 2025-06-24 (P) and Papers AI 2026-08-04 (P).
- **protocols.io.** Bought by Springer Nature, announced 2023-07-26 (P);
  AI features 2025-12-17 (P); no later sale found (U) — the user's
  recollection of a recent acquisition may be this one.

### 3.3 Literature and research tools

- **Elicit.** $22 million Series A at $100 million, 2025-02-26 (P); API
  and MCP 2026-07-15, a Research Agent 2026-08-05, collaboration
  2026-09-01 (P); Pro at $49 a month (P).
- **Consensus.** §7 of [the industry around the
  corpus](industry-landscape.md); $30 million Series B, May 2026 (P).
- **Undermind.** Y Combinator, summer 2024; about $500,000 pre-seed (S);
  no later round found (U).
- **SciSpace.** Consolidated into one "SciSpace Agent" (P); its own user
  figures disagree (S).
- **scite.** Bought by Research Solutions, announced 2023-11-27 (P), for
  about $20.9 million — $7.2 million cash, $6.3 million stock and an
  earnout (S); MCP server 2026-02-26 and Claude connector 2026-04-29;
  Research Solutions at $21.8 million annual recurring revenue in fiscal
  Q2 2026 (S).
- **Ai2's Asta.** 2025-08-26: agents, AstaBench, an MCP server over
  Semantic Scholar's 200 million-plus papers (P).
- **Litmaps and ResearchRabbit.** Litmaps bought ResearchRabbit
  2025-05-08, raising $1 million and claiming about $1 million annual
  recurring revenue and two million combined users (P).
- **Others.** Google Scholar Labs, 2025-11-18, limited and English only
  (P); Iris.ai now selling RSpace to corporate R&D (P); Scholarcy bought by
  Texthelp, October 2024 (S); Perplexity with Wiley content for
  universities, May 2025 (P).

### 3.4 AI scientists and autonomous labs

- **Periodic Labs** (Liam Fedus from OpenAI, Ekin Dogus Cubuk from Google
  Brain). $300 million seed led by a16z, September 2025, at a reported
  $1.3 billion (S); talks at about $7 billion (March 2026) and $500 million
  at $7.5 billion (May 2026), not confirmed closed (S); Periodic Neon, a
  trillion-parameter model for X-ray diffraction data from its own labs,
  2026-09-15, with no named discoveries (P).
- **Lila Sciences** (Flagship). $200 million seed (March 2025) and $350
  million Series A with NVIDIA (October 2025) (P); talks for about $2
  billion at $8.5 billion (June 2026), not confirmed closed (S); claims of
  ultra-stable mRNA and a CAR-T therapy without peer-reviewed data (P).
- **FutureHouse and Edison Scientific.** Edison spun out 2025-11-05 (P);
  Kosmos the same day at $200 a run, 79.4 % of its statements judged
  accurate by scientists Edison recruited but 57.9 % of its
  interpretations (P); $70 million seed, 2025-12-18 (P), at about $250
  million (S); an Incyte collaboration, May 2026 (P). Robin's ripasudil for
  dry age-related macular degeneration in Nature 2026-05-19: humans ran the
  lab work, in vitro only, and Robin's analysis (1.89-fold) differed from
  the human re-analysis (1.75-fold) (P).
- **Others.** Sakana AI's Series B of about $135 million at $2.65 billion
  (P) and its AI Scientist in Nature 2026-03-26, naive ideas and wrong
  citations admitted (P); Autoscience $14 million, 2026-03-18 (P);
  Intology's Zochi with an ACL 2025 main-conference paper (P); Axiom $200
  million at $1.6 billion (S); Harmonic $120 million at $1.45 billion (P);
  Isomorphic Labs $2.1 billion (S); AI biotechs over $6 billion in 2026 so
  far (S).

### 3.5 Open source

Stars as GitHub showed them on 2026-09-17 (P).

| repository | stars | last activity | state |
|---|---|---|---|
| gpt-researcher | 29.5k | 2026-06 | maintained, slowing |
| stanford-oval/storm | 31.4k | 2025-09 | dormant |
| Future-House/paper-qa | 9.2k | 2026-08 | maintained |
| langchain-ai/open_deep_research | 12.7k | archived 2026-08-21 | abandoned |
| smolagents open_deep_research | parent 29.4k | 2025-12 | demo |
| AgentLaboratory | 5.9k | 2025-08 | dormant |
| SakanaAI/AI-Scientist | 14.6k | licence changed 2025-12 to a RAIL-derived one | dormant |
| HKUDS/AI-Researcher | 5.7k | 2025-10 | dormant, no licence file seen |
| snap-stanford/Biomni | 3.9k | 2026-01 | slowing |
| mims-harvard/ToolUniverse | 1.7k | daily, 1,009 commits | very active |
| bytedance/deer-flow | 82.6k | daily; v2 a general agent harness | very active |
| K-Dense-AI scientific-agent-skills | 45.3k | 2026-09-14 | active, funnels to a paid platform |

The generic deep-research repositories peaked in 2025; most "AI
scientist" repositories released with papers were never maintained; the
energy moved to general agent harnesses and libraries of scientific tools
and skills.

### 3.6 Graveyard and consolidation

- **Meta's Galactica,** public demo pulled two days after its 2022-11-15
  launch for confident fabrication (S). **Papers with Code** shut
  2025-07-24, redirecting to Hugging Face (P, S).
- **Cloud labs.** Strateos stopped remote-access labs in April 2023 for
  on-site builds (P); Multiply Labs reportedly bought it (U); Lilly's
  remote lab reportedly sold to Arctoris (S). One practitioner's
  post-mortem: about $5 million of equipment per automated pipeline,
  samples still moved by hand, remote users unable to debug, tacit lab
  knowledge that will not go into protocols (S). Emerald Cloud Lab: §8.
- **AI drug discovery.** Zymergen bankrupt 2023 after Ginkgo's purchase
  (P); Exscientia merged into Recursion 2024, which then cut about a fifth
  of staff (P, S); BenevolentAI delisted 2025 (P).
- **No literature-tool startup hard failure found;** TechCrunch's running
  AI graveyard of 2026-09-15 has no science entries (S). They are absorbed
  at small prices instead.
- **Consolidation:** Springer Nature bought Slimmer AI's science division
  (2023) and protocols.io (2023); Research Solutions scite (2023);
  Clarivate most of MotionHall (2024); Texthelp Scholarcy (2024); Litmaps
  ResearchRabbit (2025); Digital Science metaphacts (2023) and Ontopic
  (2026), both knowledge-graph companies (P, S).

### 3.7 Catalogues of AI tools for biology

Added at the user's request: they recall a dashboard of tools for
DNA-adjacent work — mostly an aggregator, linking ordinary alignment
tools beside some AI ones — kept by Johns Hopkins, a large cancer research
group, or perhaps St. Jude. Nothing matching that description exactly was
found (U); what was found:

- **St. Jude Cloud** (P), with Microsoft and DNAnexus: an aggregator of
  St. Jude's own tools rather than of others' — a Genomics Platform with
  controlled-access harmonised data and end-to-end analysis workflows,
  Model Systems (xenograft and cell-line data), PeCan (a curated pediatric
  cancer variant knowledgebase), and a Visualization Community with
  ProteinPaint, GenomePaint and Disco. No AI tools named on its front page.
  St. Jude's Lead Discovery Informatics team lists its software, classic
  tools beside ADMET Predictor, Chemprop and DeepChem, but for chemistry,
  not DNA (P).
- **Aggregators of the kind remembered,** classic and AI side by side:
  Expasy, SIB's portal to more than 160 resources, with alignment tools
  and an AI and machine learning category (P); ELIXIR's bio.tools
  registry, tens of thousands of tools and databases, figures varying by
  source (S); and commercial no-code platforms — Neurosnap, offering
  MAFFT and MMseqs2 alignments, Foldseek, AlphaFold2, Chai-1, Boltz,
  RFdiffusion, DiffDock and GROMACS, and Tamarind Bio, with multiple
  sequence alignment beside AlphaFold variants (P).

- **NCI's ITCR tool registry** (P, updated 2026-07-24). The Informatics
  Technology for Cancer Research programme's list of about a hundred
  free, open-source tools in eleven categories — clinical informatics,
  digital pathology, epigenetics, genomics and variant interpretation,
  imaging, immuno-oncology, platforms, network biology, proteomics,
  transcriptomics, visualisation. A table, not a dashboard; AI tools not
  flagged as such. A connectivity map of the projects is kept on NDEx by
  UC San Diego.
- **The ITCR Training Network** (S). Courses built around ITCR tools by
  the Johns Hopkins and Fred Hutch data science labs — the likeliest Johns
  Hopkins connection.
- **NCI's Cancer Research Data Commons** (S). The Imaging Data Commons
  added image-similarity search over embeddings of more than 65,000
  pathology slides from Google Health AI (June 2026 update); the Genomic
  Data Commons keeps its own analysis tools.
- **Epoch AI's database of biological AI models** (P, report 2026-02-20).
  A public Airtable of 1,196 models released after September 2024 in nine
  categories — protein engineering, small-molecule design, viral vectors,
  genetic modification, immune modelling and vaccines, pathogen
  prediction, general-purpose biology, research agents, desk-research
  agents — with creator, date, parameters, training compute, datasets,
  weights and code availability and safeguards; 72 notable models checked
  by hand. 3.2 % of all models document safeguards, against 95 % of
  frontier language models.
- **RAND Europe and CLTR's Global Risk Index for AI-enabled Biological
  Tools** (S). 57 tools assessed on capability, misuse potential,
  accessibility and maturity, 13 rated red; Epoch's categories are adapted
  from it. Its successor, the **Risk Index Observatory** (2026 project, S),
  is described as a continuously updated, automation-assisted platform with
  dashboards and early warnings for developers.
- **Johns Hopkins Center for Health Security's AIxBio work** (P). Policy
  on language models and biological design tools; no tracker found (U).
- **The BioContextAI Registry** (P, Nature Biotechnology). A community
  catalogue of MCP servers for biomedical research, with a knowledgebase
  server reaching UniProt, Open Targets, Reactome, STRING, OmniPath and
  ClinicalTrials.gov.
- **ToolUniverse and K-Dense's skills** (§3.5): catalogues an agent loads
  rather than a person reads.

Reading: the catalogues split in two — trackers of what exists and how
dangerous it is (Epoch, RAND and CLTR), and registries of what an agent
can call (BioContextAI, ToolUniverse). A literature-verification server
would be listed in the second kind; BioContextAI is the natural one for
biomedicine.

## 4. The thesis against the evidence

### 4.1 Predicting capability

- **Underestimated:** in a 2022 tournament, superforecasters gave AI gold
  at the International Mathematical Olympiad 2.3 % and domain experts
  8.6 %; it happened in July 2025 (S).
- **Overclaimed:** MIT disavowed the preprint claiming AI raised
  materials-science productivity, 2025-05-16 (S); OpenAI's October 2025
  claim of solved Erdős problems was literature found (S); an outside
  review found little evidence that GNoME's materials are new, credible
  and useful (P).
- **Validated where checking is cheap:** an internal OpenAI model
  disproved Erdős's 1946 unit-distance conjecture, with a human-verified
  version by Alon, Bloom, Gowers and others, May 2026 (P). Tao's wiki
  counts about fifty standalone AI contributions to Erdős problems against
  more than 150 with human collaborators and warns of selection bias (P);
  he put AlphaEvolve at the level of an expert with optimisation software,
  at far larger scale, and found it games its scoring (P).
- **Biology stops before the organism:** the co-scientist and Robin
  results end in cell lines and organoids, humans doing the lab work (P).
- **Benchmarks** (setups in §8): LABBench2, frontier models 26–46 % lower
  than on LAB-Bench, weakest at finding the right source and figure,
  querying databases, handling exact sequences (P); PaperBench, the best
  agent 21.0 % of paper results reproduced against 41.4 % for ML PhDs on a
  subset (P); CORE-Bench declared solved at 95 % in December 2025, with a
  tool switch moving 42 % to 78 % and hand regrading adding 17 points (P);
  MLE-bench top medal rate 64.4 % (P); BixBench 17 % at launch and 84–98 %
  on a cleaned subset in 2026 (P, S); ScienceAgentBench about 33 % (S);
  SciReplicate-Bench 39 % (S); DiscoveryWorld about 20 % on hard tasks
  against about 70 % for human scientists (S); agents found the solution
  in 79–81 % of runs and used it in 37–50 % (P).
- **AI-written science:** Agents4Science 2025 accepted 48 of about 315;
  reviewers called the work technically correct but weak on scientific
  judgment, with made-up references (S). Sakana's workshop paper passed a
  venue accepting 60–70 %, humans having chosen which to submit (P, S).

### 4.2 The labs' stated bets

Quoted for what the labs say, not as credible forecasts (the user's
point of §2). Altman's goals carry "may totally fail" (P). Amodei's
October 2024 essay predicts fifty to a hundred years of biology in five
to ten, and names physical experiments, clinical trials and diminishing
returns to intelligence as limits (P); in February 2026 he was less sure
of scientific discovery than of coding and mathematics, where answers can
be checked (P). The labs diverge: OpenAI decentralised its science unit;
Anthropic built a workbench and hired Jumper.

### 4.3 Adoption

- **Wiley ExplanAItions 2025** (2,430 researchers, released 2025-10-07):
  84 % use AI at work, up from 57 %; 62 % for research or publishing
  tasks, up from 45 %; AI believed better than humans in 30 % of use cases,
  down from over half; worry about hallucination 51 % to 64 %; 80 % use
  general tools; on average 11 % have heard of the specialised ones; 57 %
  held back by missing guidance (P).
- **Elsevier, Researcher of the Future** (3,200-plus, 2025-11-04): 58 % use
  AI for work, up from 37 %; finding and summarising research 61 %,
  literature reviews 51 %, grant writing 41 %, data analysis 38 %; 22 %
  call AI tools trustworthy (P).
- **Oxford University Press:** 76 % using an AI tool and 8 % trusting AI
  companies with their data, 2024 (S); 20 % trusting AI outputs, 74 %
  unsure what to disclose, 35 % of arts and humanities researchers never
  using AI against 15 % in science and medicine, 2026 (S).
- **Ithaka S+R** (US academic biomedical faculty, October 2024): 63 % had
  tried generative AI, 7 % use it regularly, 7 % for hypotheses, 74 %
  seeing accuracy as a barrier (P).
- **Frontiers:** 53 % of peer reviewers use AI, 2025 (P). **Scientists who
  program:** perceived productivity tracks how much generated code is
  accepted at once, not testing (P).
- **A text-detection preprint** (August 2026, not peer reviewed): LLM help
  in an estimated 77 % of 2025 PubMed Central papers, against 52 % in 2024
  (S).
- **Why 7 % and 77 % need not conflict** — a reading. Different
  populations (US faculty against all PubMed Central authors worldwide,
  students and postdocs who draft included), different measures (regular
  use for research against any detectable LLM touch on a paper's text,
  polishing and translation included), different years (2024 against
  2025, in a year Wiley's figures moved fast), and many authors per paper,
  so one author's editing marks the whole paper. And the detection method
  is disputed: a May 2026 comment on a similar study in Science argues its
  effect comes from how papers are flagged (P).

### 4.4 Funding

- **In aggregate:** AI took about $202 billion of venture money in 2025,
  roughly half of the global total, and $242 billion in Q1 2026, 80 % of it
  (S). No figures for AI-for-science as its own category (U); no reporting
  that funding dried up (U).
- **Concentrated:** autonomous labs, AI drug discovery and verifiable
  mathematics raise hundreds of millions to billions (§3.4); literature and
  workbench tools raise $14–70 million (§3.3); their exits are small.
- **The user's refinement:** dried up for new entrants; bets concentrating
  around survivors whatever their products; capital competing with
  treasuries, energy, defence, memory and GPUs.

### 4.5 Physical reality

The hard failures sit where AI meets the lab or clinic (§3.6); no
literature tool failed outright. The labs' own leaders name experiments as
the limit, and investors are paying to build labs rather than walking
away (§3.4). Periodic's launch post concedes experiments take days and
give ambiguous results (P).

## 5. What seems to work, tentatively

- Tasks checked quickly: proofs, code, reproducing a paper when its code
  is supplied.
- Writing and literature assistance, where adoption is.
- Agents reaching data the customer already owns through connectors
  (Claude into Benchling).
- Not yet: end-to-end AI scientists producing important work; discovery
  without physical validation.

## 6. OpenAlex's numbers

The user asked that these be tracked.

- **Traffic:** about 1.5 billion API calls a month (P, OpenAlex's review of
  2025) — about 580 calls a second on average by arithmetic, peaks
  unknown and likely much higher.
- **Money:** about $800,000 annual recurring revenue and a $3.5 million
  Wellcome grant (P). Not a business: enough to keep the lights on.
- **API terms:** keys required for every call since February 2026, $1 of
  free use a day, then usage pricing; new endpoints for semantic search
  (beta), full-text PDF and XML for 60 million open-access works, and a
  bulk-download command-line tool; the CC0 snapshot stays free (P).
- **A conflict with this repository's own observation:** on 2026-09-15,
  keyless requests were served under a limit of 1,000 a day (ingestion
  plan §8, item 8). The announced key requirement may not be enforced as
  described, or keyless use survives at a lower tier (U).
- **Walden,** the rewrite live in November 2025, added 190 million works
  (P).

## 7. Readings for this repository

2026-09-17, with the user's correction taken in.

- **The interface is contested, and the labs hold distribution and token
  subsidies.** Prism is free, Claude Science hands out seats, Gemini for
  Science is rolling out. The user: the labs are genuinely bad at user
  experience, so there is space, but distribution is hard to compete with
  and the token subsidies are the real deal-breaker. The tools that
  survive are becoming servers those interfaces call — the finder, not
  the holder, again.
- **Trust is the scarce thing, and verification has buyers.** Use rises
  while trust falls (§4.3). A detector vendor, with a stake, found
  fabricated citations in 50 of 300 sampled ICLR 2026 submissions (P);
  ICLR desk-rejects them (P); arXiv's computer-science section bans
  unchecked AI content (S).
- **Bulk-first sidesteps the data layer's new prices.** Tools built on
  OpenAlex's API pay per use now (§6); this repository reads the free
  snapshot.
- **The clearest paying niche is the check before an experiment.** Whoever
  spends on physical experiments — Periodic, Lila, Edison's pharma
  customers, pharma generally — has money and a reason to pay for "has this
  been done, and what does it rest on". Literature search is valuable (the
  user, on the Erdős episode).
- **Unknown:** whether narrow agent gains become physically validated
  discoveries before the lab bets run out of capital; whether scientists
  settle on general assistants or specialised tools; whether OpenAI's
  retreat or Anthropic's push is the better bet.

## 8. Benchmark setups, and Emerald Cloud Lab

Asked by the user: with a harness, or the model alone. From a follow-up
survey the same day.

**Mostly harnesses, and the harness moves the score.** Model alone
against harness, or one harness against another, on the same test (P):
CORE-Bench, Opus 4.5 from 42 % to 78 % by switching CORE-Agent for Claude
Code; PaperBench, o1 from 13.2 % to 24.4 % with IterativeAgent in place of
BasicAgent, 26.0 % at 36 hours; ScienceAgentBench, Claude 3.5 Sonnet from
16.7 % prompted directly to 32.4 % with self-debug, which beat OpenHands by
10.8 points at a seventeenth of the cost; MLE-bench, GPT-4o at 8.7 % with
AIDE, 4.4 % with OpenHands and 0.8 % with MLAB, and o1-preview from 16.9 %
at one attempt to 34.1 % at eight; LABBench2, table questions needing
retrieval at about 0.1 without tools and 0.6–0.7 with them (read off
figures).

| benchmark | headline | setup | budget and attempts | grading |
|---|---|---|---|---|
| LABBench2 | 26–46 % below LAB-Bench | models without tools, and with web search, code execution and fetch through Edison's harness | not stated | open answers; custom verifiers for sequence and cloning tasks, others not described |
| PaperBench | 21.0 % (Claude 3.5 Sonnet) | BasicAgent, a ReAct loop with bash, Python, browser, file reader | 12 h, one A10 GPU, 3 runs a paper | o3-mini judge against rubrics |
| CORE-Bench Hard | 77.8 % automatic, 95.5 % after manual validation | Claude Code, submitted by Nicholas Carlini | about $87 for the run | exact match, numbers within intervals from three manual runs, every question in a task right |
| MLE-bench | 64.4 % any medal | Famou-Agent 2.0 on Gemini 3 Pro Preview (Baidu AI Cloud) | 24 h a competition, 3 seeds | Kaggle medal cutoffs; self-reported |
| BixBench | 17 % at launch (21 % Claude 3.5 Sonnet in v2) | Aviary agent in a Jupyter notebook | 5 runs a capsule | Claude 3.5 Sonnet judge on open answers |
| ScienceAgentBench | 32.4 % | Claude 3.5 Sonnet with self-debug | best of 3 | task scripts; GPT-4o on figures |
| DiscoveryWorld | 38 % easy, 18 % challenge (ReAct) | GPT-4o agents: ReAct, Plan+Execute, Hypothesizer | 100 or 1,000 steps, 5 seeds | automatic scorecards; GPT-4o on knowledge |
| AstaBench | Asta v0 53.0 %, ReAct with GPT-5 43.3 % | 57 agents in 22 classes; standard date-restricted literature tools and a sandbox | cost fixed to one price snapshot | per task |

Notes (P unless marked):

- **LABBench2's comparison** looks, by inference from its figures, like
  multiple choice with the content supplied against open answers needing
  retrieval without tools; the paper does not say (U).
- **PaperBench's humans:** eight ML PhDs on a three-paper subset, best of
  three after 48 hours, 41.4 % against o1's 26.6 % on the same papers.
- **CORE-Bench:** automatic grading covered about four fifths, the rest by
  hand; a follow-up found Opus 4.5 at 82.1 % on both CORE-Agent and
  OpenCode but disagreeing on 31 % of tasks, with 15 task errors and 20
  exploitable shortcuts in the benchmark.
- **MLE-bench** has stopped taking submissions while fairness is worked on.
- **BixBench's 84–98 %** (April 2026) is not the launch test: a curated
  50-question subset with ambiguous questions and wrong keys fixed,
  multiple choice graded by letter, one run each, frontier models with
  scientific skills and in one case web access. On original against
  curated questions, Biomni Lab went from 52.2 % to 88.7 % and Edison from
  42.4 % to 78.0 %; plain Claude Code scored 65.3 %.
- **DiscoveryWorld's humans:** eleven MSc and PhD scientists on sixteen
  tasks, one seed, 66 % completion — against agents averaged over five
  seeds, so the human gap compares slightly different setups.
- **AstaBench labels every entry** by openness (open weights to closed
  interface) and tooling (standard, equivalent custom, custom), plots cost
  against accuracy, and requires logs.
- **"Explore but ignore"** used Terminus and SWE-agent scaffolds with bash
  or an editor tool, ten attempts: the editor raised pass rates but cut use
  of the planted solution by 13.7–48.3 %; high reasoning raised use from
  11 % to 37 %.

Reading: a headline number without its harness, tools, budget, attempts
and grader is not a measure of the model. AstaBench's labelling is the
practice to copy for this repository's own scoreboard ([benchmarks for
search and research](retrieval-benchmarks.md)).

**A counterweight, recorded the same day.** HarnessTax (Pan, Zaharia and
others, September 2026) ran seven models in Claude Code, Codex CLI and Pi
on SWE-bench Lite and Terminal-Bench 2.0: success within about ±2 % and
±5 % across harnesses, cost up to 5× apart, the four-tool Pi on the
frontier, and another harness than the model's own best in nine of twelve
comparisons (P). Reading: below the edge of capability the harness decides
what is paid, at the edge whether the task is solved — the gaps above are
edge cases, or bare models against agents. The framing is a synthesis,
not the study's finding, and has caveats — a ceiling effect, unlike
contrasts, equal scores on different tasks. Detail, caveats and
consequences for this repository in [configuration per query, and the
harness tax](future-ambitions-configuration.md) §3.

**Emerald Cloud Lab** appears to be operating — its site carries © 2026
and one Austin facility (P) — but no 2025–2026 news was found. It moved
from South San Francisco to 105,000 square feet in Austin, announced
2023-02-28 (P); a March 2026 post says it serves a small number of
customers on contracts reportedly above $250,000 a year (S). Carnegie
Mellon's $40 million cloud lab with ECL was announced in 2021 (P) and
opened in 2024 (S); two US senators proposed a national network of cloud
labs in December 2025 (S). Strateos folded after its 2023 shift, per a
2024 PNAS feature (S); Arctoris bought Lilly's lab and moved it to Oxford
in September 2024 (S); Ginkgo launched a cloud lab in March 2026 (S).

## 9. Open questions

1. Where adoption actually settles: general assistants, specialised tools,
   or connectors between them (§4.3, §7).
2. Whether OpenAlex's key requirement is enforced, and what its traffic
   peaks are (§6).
3. Which of the well-funded labs buy literature grounding rather than
   build it (§3.4, §7).
4. How much of each benchmark score is the harness (§8).

## Sources

The labs: [Claude for Life Sciences](https://www.anthropic.com/news/claude-for-life-sciences);
[healthcare and life sciences expansion](https://www.anthropic.com/news/healthcare-life-sciences);
[Claude Science](https://www.anthropic.com/news/claude-science-ai-workbench);
[expanding support for scientists](https://www.anthropic.com/news/expanding-support-for-scientists);
[John Jumper](https://x.com/JohnJumperSci/status/2068001285173834106);
[Kevin Weil, OpenAI for Science](https://x.com/kevinweil/status/1962938974260904421);
[Kevin Weil, decentralised](https://x.com/kevinweil/status/2045230426210648348);
[TechCrunch on OpenAI's side quests](https://techcrunch.com/2026/04/17/kevin-weil-and-bill-peebles-exit-openai-as-company-continues-to-shed-side-quests/);
[Prism](https://openai.com/index/introducing-prism/);
[GPT-Rosalind](https://openai.com/index/introducing-gpt-rosalind/);
[Rosalind Workbench](https://developers.openai.com/blog/rosalind-workbench);
[Altman's goals](https://x.com/sama/status/1983584366547829073);
[Engadget on the research intern](https://www.engadget.com/2251859/openai-says-it-reached-its-goal-of-creating-an-automated-research-intern/);
[The Decoder on research interns](https://the-decoder.com/openai-reports-ai-research-interns-and-warns-about-its-own-pace-at-the-same-time/);
[GPT-5 science experiments](https://arxiv.org/abs/2511.16072);
[TechCrunch on the Erdős episode](https://techcrunch.com/2025/10/19/openais-embarrassing-math/);
[unit-distance disproof](https://arxiv.org/abs/2605.20695);
[AI co-scientist](https://research.google/blog/accelerating-scientific-breakthroughs-with-an-ai-co-scientist/);
[co-scientist in Nature](https://pmc.ncbi.nlm.nih.gov/articles/PMC13345910/);
[Gemini for Science](https://blog.google/innovation-and-ai/technology/research/gemini-for-science-io-2026/);
[closed-loop co-scientist](https://the-decoder.com/google-deepminds-ai-co-scientist-now-plans-experiments-runs-lab-equipment-and-writes-scientific-papers/);
[Tao on AlphaEvolve](https://terrytao.wordpress.com/2025/11/05/mathematical-exploration-and-discovery-at-scale/);
[Tao's Erdős wiki](https://github.com/teorth/erdosproblems/wiki/AI-contributions-to-Erd%C5%91s-problems);
[Microsoft Discovery GA](https://azure.microsoft.com/en-us/blog/announcing-microsoft-discovery-general-availability-and-microsoft-discovery-app-preview/);
[NVIDIA and Lilly](https://nvidianews.nvidia.com/news/nvidia-and-lilly-announce-co-innovation-lab-to-reinvent-drug-discovery-in-the-age-of-ai).

Incumbents: [Benchling AI](https://www.benchling.com/blog/introducing-benchling-ai);
[Benchling and Anthropic](https://www.benchling.com/blog/benchling-partners-with-anthropic-to-build-a-bridge-between-science-and-ai);
[Benchling AI Connectors](https://www.prnewswire.com/news-releases/benchling-launches-ai-connectors-to-power-the-data-ecosystem-for-rd-302743911.html);
[Sacra on Benchling](https://sacra.com/c/benchling/);
[Siemens and Dotmatics](https://press.siemens.com/global/en/pressrelease/siemens-completes-acquisition-dotmatics);
[Luma Agent](https://www.dotmatics.com/news/introducing-luma-agent);
[Revvity Signals AI](https://news.revvity.com/press-announcements/press-releases/press-release-details/2026/Revvity-Launches-Signals-AI-a-Native-Agentic-Framework-for-Accelerating-Scientific-RD/default.aspx);
[Sapio ELaiN partners](https://www.sapiosciences.com/news/sapio-sciences-announces-partner-ecosystem-for-elain-its-third-generation-ai-lab-notebook/);
[Cenevo](https://www.cenevo.com/press/labguru-and-titian-software-rebrand-as-cenevo);
[CAS agents](https://www.cas.org/press-releases/cas-agentic-ai-research-workflows);
[LeapSpace](https://www.elsevier.com/about/press-releases/elsevier-launches-leapspace-an-ai-assisted-workspace-to-accelerate-research-and-discovery);
[LeapSpace agents](https://www.prnewswire.com/news-releases/elsevier-expands-leapspace-with-new-agentic-capabilities-for-tasks-across-the-complete-research-workflow-302809180.html);
[Web of Science DeepeR](https://www.stm-publishing.com/clarivate-unveils-web-of-science-deeper-for-ai-driven-full-text-research/);
[Clarivate divestiture filing](https://www.sec.gov/Archives/edgar/data/0001764046/000176404626000085/ex992lshdivestitureinves.htm);
[Overleaf AI Assist](https://www.digital-science.com/press-releases/digital-science-launches-new-cutting-edge-ai-writing-tools-for-20-million-overleaf-users/);
[Papers AI](https://www.newswise.com/articles/digital-science-launches-papers-ai-an-ai-native-researcher-workspace-for-writing-data-and-code);
[protocols.io acquisition](https://group.springernature.com/gp/group/media/press-releases/publisher-acquires-protocolsio-driving-open-research/25844774);
[protocols.io AI](https://group.springernature.com/gp/group/media/press-releases/new-ai-capabilities-on-protocolsio/27837624).

Tools, open source, graveyard: [Elicit Series A](https://elicit.com/blog/series-a/);
[Elicit changelog](https://support.elicit.com/en/articles/14823097-changelog);
[Elicit pricing](https://elicit.com/pricing);
[Undermind](https://www.ycombinator.com/companies/undermind);
[scite acquisition](https://researchsolutions.investorroom.com/2023-11-27-Research-Solutions-Announces-Acquisition-of-scite);
[scite price](https://www.marketscreener.com/quote/stock/RESEARCH-SOLUTIONS-INC-15295099/news/Research-Solutions-Inc-completed-the-acquisition-of-Scite-Inc-for-20-9-million-45517866/);
[Asta](https://allenai.org/blog/asta);
[Litmaps and ResearchRabbit](https://www.scoop.co.nz/stories/BU2505/S00127/nz-startup-litmaps-acquires-us-rival-and-raises-1m-to-accelerate-ai-driven-research-worldwide.htm);
[Google Scholar Labs](https://scholar.googleblog.com/2025/11/scholar-labs-ai-powered-scholar-search.html);
[Galactica](https://www.technologyreview.com/2022/11/18/1063487/meta-large-language-model-ai-only-survived-three-days-gpt-3-science/);
[Strateos shift](https://www.businesswire.com/news/home/20230418006219/en/Strateos-Announces-Strategic-Shift-to-Focus-on-Customer-Demand-for-On-Site-Cloud-Labs);
[what happened to cloud labs](https://stefangolas.substack.com/p/what-happened-to-cloud-labs);
[Zymergen filing](https://www.sec.gov/Archives/edgar/data/1830214/000162828023033551/dna-20231003.htm);
[Recursion and Exscientia](https://ir.recursion.com/news-releases/news-release-details/recursion-and-exscientia-two-leaders-ai-drug-discovery-space);
[BenevolentAI delisting](https://live.euronext.com/en/products/equities/company-news/2025-03-12-result-extraordinary-general-meeting-and-delisting);
[TechCrunch AI graveyard](https://techcrunch.com/2026/09/15/the-ai-graveyard-a-running-list-of-projects-and-startups-that-didnt-make-it/).

Autonomous labs and funding: [Periodic Labs seed](https://techcrunch.com/2025/09/30/former-openai-and-deepmind-researchers-raise-whopping-300m-seed-to-automate-science/);
[Periodic Labs talks](https://www.bloomberg.com/news/articles/2026-03-25/ai-science-startup-periodic-labs-is-in-deal-talks-at-about-7-billion-valuation);
[Periodic Neon](https://periodic.com/news/building-labs-that-learn);
[Lila Series A](https://www.lila.ai/news/announcing-the-close-of-our-series-a);
[Lila talks](https://www.bloomberg.com/news/articles/2026-06-03/lila-sciences-said-in-talks-for-funds-at-8-5-billion-valuation);
[Edison spinout](https://www.futurehouse.org/news/announcing-edison-scientific);
[Kosmos](https://advances.edisonscientific.com/research/announcing-kosmos/);
[Kosmos paper](https://arxiv.org/abs/2511.02824);
[Edison $70M](https://edisonscientific.com/news/we-raised-70m-to-accelerate-science);
[Incyte and Edison](https://investor.incyte.com/news-releases/news-release-details/incyte-and-edison-scientific-announce-strategic-collaboration);
[Robin in Nature](https://pmc.ncbi.nlm.nih.gov/articles/PMC13346116/);
[Sakana AI Scientist in Nature](https://sakana.ai/ai-scientist-nature/);
[Sakana Series B](https://sakana.ai/series-b/);
[Autoscience](https://www.businesswire.com/news/home/20260318760407/en/Autoscience-Raises-$14M-to-Build-the-Worlds-First-Automated-AI-Research-Lab);
[Crunchbase AI funding 2025](https://news.crunchbase.com/ai/big-funding-trends-charts-eoy-2025/);
[Crunchbase biotech 2026](https://news.crunchbase.com/health-wellness-biotech/startup-investment-exits-steady-ai-2026/).

Evidence: [Wiley ExplanAItions 2025](https://newsroom.wiley.com/press-releases/press-release-details/2025/AI-Adoption-Jumps-to-84-Among-Researchers-as-Expectations-Undergo-Significant-Reality-Check/default.aspx);
[Elsevier Researcher of the Future](https://www.elsevier.com/insights/confidence-in-research/researcher-of-the-future);
[OUP 2024](https://www.insidehighered.com/news/quick-takes/2024/05/24/report-most-researchers-use-ai-tools-despite-distrusting-it);
[OUP 2026](https://www.infodocket.com/2026/09/10/disclosing-ai-use-in-academic-research-a-new-global-study-from-oxford-university-press/);
[Ithaka S+R](https://sr.ithaka.org/publications/adoption-of-generative-ai-by-academic-biomedical-researchers/);
[Frontiers](https://www.frontiersin.org/news/2025/12/15/most-peer-reviewers-now-use-ai-and-publishing-policy-must-keep-pace);
[scientists who program](https://arxiv.org/abs/2512.19644);
[PubMed Central detection preprint](https://phys.org/news/2026-08-marker-words-sharp-ai-biomedical.html);
[Science preprint study](https://gwern.net/doc/science/2025-kusumegi.pdf);
[comment on flagging](https://arxiv.org/abs/2605.17979);
[Nature, AI and scientists' output](https://www.nature.com/articles/s41586-025-09922-y);
[LABBench2](https://arxiv.org/abs/2604.09554);
[AstaBench](https://arxiv.org/abs/2510.21652);
[PaperBench](https://openai.com/index/paperbench/);
[CORE-Bench solved](https://x.com/sayashk/status/1996334941832089732);
[MLE-bench](https://github.com/openai/mle-bench);
[BixBench](https://arxiv.org/abs/2503.00096);
[explore but ignore](https://arxiv.org/abs/2604.17609);
[MIT disavowal](https://techcrunch.com/2025/05/17/mit-disavows-doctoral-students-paper-on-ai-productivity-benefits);
[GNoME review](https://pubs.acs.org/doi/10.1021/acs.chemmater.4c00643);
[Agents4Science](https://www.sciencenews.org/article/science-conference-test-ai-agents);
[Agents4Science analysis](https://arxiv.org/abs/2511.15534);
[GPTZero on ICLR 2026](https://gptzero.me/news/iclr-2026/);
[ICLR 2026 policy](https://blog.iclr.cc/2025/11/19/iclr-2026-response-to-llm-generated-papers-and-reviews/);
[Amodei, Machines of Loving Grace](https://darioamodei.com/machines-of-loving-grace);
[Amodei with Dwarkesh Patel](https://www.dwarkesh.com/p/dario-amodei-2);
[Narayanan and Kapoor](https://www.normaltech.ai/p/could-ai-slow-science);
[Epoch AI on R&D](https://epoch.ai/gradient-updates/most-ai-value-will-come-from-broad-automation-not-from-r-d);
[FRI on the IMO forecast](https://x.com/Research_FRI/status/1962834279689265402).

Catalogues: [St. Jude Cloud](https://www.stjude.cloud/);
[St. Jude Cloud docs](https://docs.stjude.cloud/overview);
[St. Jude Cloud in Cancer Discovery](https://aacrjournals.org/cancerdiscovery/article/11/5/1082/666431/St-Jude-Cloud-A-Pediatric-Cancer-Genomic-Data);
[St. Jude LDI software](https://www.stjude.org/research/departments/chemical-biology-therapeutics/lead-discovery-informatics/software-tools.html);
[Expasy](https://www.expasy.org/);
[bio.tools](https://bio.tools/);
[Neurosnap services](https://neurosnap.ai/services);
[Tamarind Bio MSA](https://app.tamarind.bio/tools/msa);
[NCI ITCR informatics tools](https://www.cancer.gov/about-nci/organization/cssi/resources/informatics-tools);
[ITCR](https://itcr.cancer.gov/);
[ITCR Training Network](https://www.itcrtraining.org/resources/blog);
[CRDC updates, June 2026](https://datacommons.cancer.gov/news/crdc-components-updates-june-2026);
[Epoch AI on biological AI models](https://epoch.ai/blog/expanding-our-analysis-of-biological-ai-models);
[RAND–CLTR Global Risk Index](https://www.longtermresilience.org/reports/global-risk-index-for-ai-enabled-biological-tools/);
[RAND Risk Index Observatory](https://www.rand.org/randeurope/research/projects/2026/risk-index-observatory-for-ai-biological-tools.html);
[Center for Health Security AIxBio](https://centerforhealthsecurity.org/our-work/aixbio);
[BioContextAI in Nature Biotechnology](https://www.nature.com/articles/s41587-025-02900-9);
[BioContextAI registry](https://github.com/biocontext-ai/registry).

Benchmark setups: [PaperBench paper](https://arxiv.org/abs/2504.01848);
[CORE-Bench leaderboard](https://hal.cs.princeton.edu/corebench_hard);
[Kapoor on CORE-Bench grading](https://substack.com/@sayash/note/c-183913356);
[CORE-Bench follow-up](https://arxiv.org/abs/2606.26158);
[labbench2 harness](https://github.com/EdisonScientific/labbench2);
[BixBench curated subset](https://www.biorxiv.org/content/10.64898/2026.04.28.721523v1);
[Phylo on biology agents](https://phylo.bio/blog/evaluating-ai-agents-in-biology);
[ScienceAgentBench](https://arxiv.org/abs/2410.05080);
[DiscoveryWorld](https://arxiv.org/abs/2406.06769);
[AstaBench repository](https://github.com/allenai/asta-bench);
[AstaBench post](https://allenai.org/blog/astabench).

Cloud labs: [Emerald Cloud Lab](https://www.emeraldcloudlab.com/);
[ECL to Austin](https://www.prnewswire.com/news-releases/emerald-cloud-lab-to-relocate-state-of-the-art-facility-from-south-san-francisco-to-austin-301758704.html);
[on cloud-lab numbers, March 2026](https://lennartjusten.substack.com/p/no-there-are-not-hundreds-of-cloud);
[CMU cloud lab](https://www.cmu.edu/news/stories/archives/2021/august/first-academic-cloud-lab.html);
[programmable cloud labs explained](https://www.wesanews.org/health-science-tech/2025-12-22/programmable-cloud-labs-explained);
[PNAS on cloud labs](https://www.pnas.org/doi/10.1073/pnas.2406320121);
[Arctoris and Lilly's lab](https://pharma-industry-review.com/arctoris-acquires-eli-lillys-life-science-studio-drug-discovery-automation-lab).

OpenAlex: [2025 in review](https://blog.openalex.org/openalex-2025-in-review/);
[API keys and pricing](https://blog.openalex.org/openalex-api-new-features-and-usage-based-pricing/);
[Walden](https://blog.openalex.org/openalex-rewrite-walden-launch/).
