# Research report 1: teaching the system, in the project's own notes

**What this is:** the final report of a read-only research subagent (a
repository explorer, no web access used), delivered on 2026-09-28 to the
conversation that wrote [teach it your method](../teach-it-your-method.md),
and kept verbatim, following the practice the user asked for with
[searching within a result set](../search-within-results.md)'s reports.
It is model output, not checked line by line. It used 45 tool calls over
about 4.5 minutes. Its `file:line` citations point into the repository
as it stood on branch `worktree-search-within-results` that day. The
prompt it was given follows the report.

---

Paths are relative to /Users/thomasingalls/Code/ol-rht/.claude/worktrees/search-within-results/. Short names: SWR = docs/inbox/search-within-results.md, RASB = docs/inbox/recall-accounted-set-builder.md, FA = docs/inbox/future-ambitions.md, SP = docs/serving-plan.md, PS = docs/inbox/product-shapes.md, CL = docs/inbox/competition-list.md.

**1. Users teaching the system**
- None of it is built. The closest match to the pitch is SWR §10 (SWR:326-337). The user describes a category in a sentence or marks "a couple of dozen examples". A stronger model labels a few hundred works. A cheap model over vectors then applies the boundary to every work ("one multiply-and-add per work"). The user checks what was kept, what was dropped and what sits near the boundary, and "it learns again". Some marks are held back: "the evaluation plan's frozen half again, inside the product". GLiNER2 can be LoRA-tuned from marks (SWR:366-368).
- On labels: "never pre-compute *your* labels; keep theirs, with who said them" (SWR:79-86, 310-312). The versioning hook is the signed annotation `(work, label, process, version, evidence)` (SWR:314-319).
- A custom facet is "more than a saved boolean string: measured where a reference exists, extended where none does" (SWR:242-254).
- Still open: whether custom facets are "saved, named and reused" and shared (SWR:955-957). A step "can be reproduced from its query and the epoch" (SWR:928-935).
- RASB proposes `label(set, marks)` and `missing(set)` (RASB:56-63). Marks come from a person or a strong model, and who marks is open (RASB:49-50, 269-270). Scoring works outside the edge means a cheap classifier over much of the corpus (RASB:272-274).
- Tracer: a rubric per trace plus a SetFit-style tiny classifier (docs/inbox/future-ambitions-tracer-engine.md:212-228). The user's hand-labelled concepts are its evaluation (PS:97; FA:289-292).
- Saved searches and alerts for users are not in the notes. "Feeds" and "origins (SP6)" mean publisher feeds as a way for new records to come in, not user feeds (SP:51, 165-172, 613-615, 829-835, 1349-1351). PS:99-102 lists them among what the tracer would not need first.

**2. Competitors**
- ASReview: a reviewer marks include/exclude and a model reorders the rest; it "matches §10's loop... used to rank" (SWR:533-540). Legal predictive coding is the same loop (SWR:541-550). "Marks exist where the job makes them" (SWR:562-567). No tool combining these for general literature search was found (SWR:588-594).
- Elicit: "model screening against editable criteria" over up to 500 papers (SWR:661-662). Also alerts, libraries and API/MCP (CL:40, 164). Pro costs $49 a month (docs/inbox/science-assistant-landscape.md:142-144).
- Undermind: GPT-4 classifies about 150 full texts per search and fits a discovery curve (SWR:653-657). The notes record direct overlap with the product (CL:201-222). The user: "it is not always the good products that win" (science-assistant-landscape.md:34).
- Consensus: the agent applies filters such as study type (SWR:658-660). Criticised because "study-type labels can be wrong" and results are "hard to reproduce" (docs/inbox/industry-landscape.md:249-253).
- scite has monitoring (CL:65, 189). The user: "scite was doing smart things" (industry-landscape.md:44-45).
- Semantic Scholar has research-feed alerts (CL:191). The notes do not say whether they learn from ratings.
- Iris.ai offers monitoring and concept extraction for filtering (CL:168). AnswerThis offers screening (docs/inbox/undermind-alternatives.md:38-39).
- Jev: a hosted classifier with options the caller defines. Using it sends abstracts off the box (SWR:385-400).
- Rayyan, Covidence, DistillerSR and EPPI-Reviewer are not in the notes. Rayyan appears only as a paper co-cited with PRISMA (SP:730-731).

**3. Freshness**
- Target, decided 2026-09-15: under ten minutes from disk to answer, hoped under a second. Sources lag on their own: Crossref about an hour, PubMed and OpenAlex daily (SP:55-62).
- The clock is the batch, one line in the journal (SP:128-145). Each night a full merge rebuilds everything and publishes a new epoch (SP:147-159).
- SP5 landed: a batch of 4,989 records merged in 2.7 s and was answerable 1.3 s later (SP:578-597).
- Automatic intake of new records (origins, SP6) is not built (SP:613-615; docs/ingestion-plan.md:1388-1392). "Nothing runs fetch, normalize, merge and publish on a clock" (FA:39-42). At corpus scale the nightly rebuild takes hours and freshness pauses during it (SP:1501-1505).
- Running something over each day's new works: the only mention is retraining a query router "on the clock the freshness work already keeps" (docs/inbox/future-ambitions-configuration.md:206-210). A classifier run per batch is not in the notes.
- Why freshness, in the user's words: the first to have a paper indexed "getting the sign-up"; "freshness serves acquisition" (PS:104-115). Efficiency buys "pricing and freshness others cannot afford" (industry-landscape.md:79-83).

**4. Starting from a subset**
- Exists: facets year, form, genre, language, status, held_by and access. Venue and authors are searchable text only (crates/search/src/schema.rs:20-21; SWR:130-142). Facets are counted but cannot be used as filters yet; that waits on item 36 (SWR:133-135).
- Planned: facets including venue, "later MeSH, topics and entity ids" (SP:335-338). A venue facet comes "once it has an id" (SP:1447-1448).
- Deferred: a registry per kind (PersonId, InstitutionId, FunderId, VenueId) (docs/inbox/ambitions-beyond-citations.md:88-96, 133). Slicing by person, institution, funder or venue "waits on a registry per kind" (FA:46-48). None exist (PS:65).
- ORCID, ROR and similar ids are kept as claims. Ids for institutions, authors and funders are "coming one day... not this pass" (ingestion-plan.md:1905-1920).
- MeSH never reaches the index (SWR:143-146).

**5. Product shape**
- Speculative, not a plan (PS:3-19). Four or five candidate shapes, none chosen (PS:21-26, 58-66). The open question is who the first user is (PS:126-131).
- The wedge: academic literature, for academics and researchers, in the short term (SWR:8-14; RASB:8-11). Europe-first is a stated market preference (PS:152-156). The stated "dream" is a workbench (PS:242-268).
- Positioning: the claimed edge (PS:43-51) was qualified on 2026-09-25 (PS:53-56). Provenance and a designed interface are "not unclaimed" (PS:264-268). "Judgments per question are only a moat if usage builds them"; the moat, if any, is "position rather than data" (industry-landscape.md:85-93).
- Cost: "One box, no cluster" (FA:27-28). The box is 128 GB and shared with the search engine (SP:85-93). Serving agents "needs more than one box" and real numbers (industry-landscape.md:34-38). Pricing for this project is not in the notes.
- Reproducibility: every answer carries its epoch (SP:344-345). It is a dimension to compare against competitors, not an advantage (CL:218-220; undermind-alternatives.md:79-81). Published methods are not in the notes.
- RASB: the measurable win is cost, and "Nothing here is measured" (RASB:212-216).

**6. docs/inbox/classification-feedback.md**
- The question (2026-09-19): can the system discover its own misclassifications? Answer: only where someone built a detector by hand, "no general machinery" (1-17).
- `--survey` catches crashes, not wrong classifications (19-33).
- The one detector that worked: `PublisherIds::off_list`. It found 53 registrants, two of them misfiled (35-48). Why it worked: it counts cases just outside a rule, prints every run, and names the dimension of the near-miss (50-60).
- Other detectors (62-111): cross-source field disputes, "the only classification... with a standing, quantitative wrongness signal" (66-74); `kind_conflicts`; `collided`; confidence labels; resolution disagreement; churn, "the only longitudinal signal" (102-105); invariants, which show consistency, not correctness (107-111).
- Eight gaps (113-163), including refusals nobody counts, single-source genres, stubs, and search with "no relevance measurement whatsoever" (161-163).
- Three kinds of detector exist (165-171). The blind spot is a classification "uniformly, confidently wrong across all sources" (176-179). Closing it needs "a labelled sample... or a human reading", and none is wired in (181-183).
- Pattern to repeat: near-miss counters, "one counter and no rule change" (185-202).
- Dashboard ambition, "not exactly actionable": reports are within-run, so the first question is where the series is stored (204-226).

**7. Rules the pitch must respect**
- Answer keys independent of what is tested (docs/evaluation-plan.md:205-224; SWR:251-254; RASB:216-220). Marks used to train cannot also measure (SWR:335-337).
- No napkin estimates: FA:8-10; docs/graph-plan.md:564-566; real numbers before "higher limits for less" (industry-landscape.md:34-36). Vendor figures are untested (SWR:394-396).
- Compromises ledger: each compromise with its cost and what lifts it (evaluation-plan.md:682-686; docs/merge-plan.md:2056-2060).
- "Abstracts are the risk" (SP:422-428). Third-party classifiers raise it (SWR:389-393); on-box GLiNER2 answers it (SWR:359-365).
- Full text is never served (SP:35-36; PS:119-124).
- An honest interval: "a confident wrong 'you have 95 %' is worse than no number" (RASB:193-195). Two identical model screening runs differed on 94 records (RASB:167-170).

---

## The prompt it was given

> Read-only research in the repository at /Users/thomasingalls/Code/ol-rht/.claude/worktrees/search-within-results (do not modify anything). The project is a scholarly-literature search engine (BM25 over titles/abstracts via Tantivy, a citation graph, facets counted over every match, daily batches with an incremental merge). A product pitch is being written up: "teach the search engine your method / your criteria and it will apply it faithfully; refine it over time; have more than one methodology; start from the subset of journals/researchers/whatever interests you; it doesn't get tired." Mechanism: a researcher's marks plus a description train a cheap classifier, measured against held-back marks, versioned, run over the whole corpus and each day's new works.
>
> Read (medium thoroughness; skim for relevant parts): docs/inbox/product-shapes.md, docs/inbox/competition-list.md, docs/inbox/industry-landscape.md, docs/inbox/science-assistant-landscape.md, docs/inbox/undermind-alternatives.md, docs/inbox/future-ambitions.md, docs/inbox/future-ambitions-tracer-engine.md, docs/inbox/future-ambitions-configuration.md, docs/inbox/classification-feedback.md, docs/inbox/search-within-results.md (§9, §10, §12), docs/inbox/recall-accounted-set-builder.md, docs/serving-plan.md (search for freshness, batch, epoch, feed, origin, alert, venue, entity), docs/ingestion-plan.md (search for freshness, daily, entity, venue, funder, ORCID), docs/evaluation-plan.md (§3.4).
>
> Report, citing file:line for every claim:
> 1. Anything in the notes about users teaching, training, labelling or giving feedback to the system (classification feedback, marks, personal classifiers, saved searches, alerts, feeds, "origins and feeds (SP6)"), and what they say.
> 2. Competitors in the notes that let users encode criteria or learn from ratings (Elicit, ASReview, Rayyan, Covidence, DistillerSR, EPPI-Reviewer, Semantic Scholar feeds, Undermind, Consensus, scite, Iris.ai...), and what the notes say about each.
> 3. The freshness path: how new works arrive (daily batch, incremental merge, epochs, the clock), how quickly, and anything about running something over each day's new works; the user's stated reason for freshness (acquisition).
> 4. What "start from a subset of journals/researchers" would need: venue ids, entity ids for authors/institutions/funders, facets — what exists, what is planned, what is deferred.
> 5. Product-shape statements: target users, the wedge, positioning against competitors, any pricing or cost-to-serve constraints ("cheap to serve", one box), and anything about reproducibility or published methods.
> 6. The classification-feedback note (docs/inbox/classification-feedback.md): summarize it fully, with line numbers.
> 7. Any rule the pitch must respect (independent answer keys, no napkin estimates, compromises ledger, abstracts' terms risk, full text never served).
>
> Keep the report under 1,100 words. Quote short phrases where precision matters. Do not speculate beyond the notes; say "not in the notes" where absent.
