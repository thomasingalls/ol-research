# Ingestion pipeline — plan

**Status:** draft 4 — source facts verified against current documentation on
2026-09-14 (§12). Nothing here is implemented.
**Inputs:** the architecture summary of 2026-09-14, folded 2026-09-15 into
the [graph plan](graph-plan.md) and the
[candidate-management note](inbox/candidate-management.md); decisions from
three rounds of discussion on 2026-09-14, logged in §10.

## 1. Three layers; this plan covers one

```text
raw         source-original bytes on disk. Source of truth. Immutable, append-only.
normalized  per-source records in one strict model. No merging. A pure function of
            (raw, model version): rebuildable at will, never re-fetched.
merged      "our" product-facing schema, one record per work across sources.
            A separate design problem with its own plan. Not covered here.
```

Ingestion is raw → normalized. It is dumb, lossless and re-runnable. The merged
layer needs a global view — identity resolution, conflict rules — which is
exactly why it is a different stage: anything that needs cross-record state is
out of scope here.

Named so they are not forgotten, all downstream: identity resolution and
`WorkId(u32)` minting; matching unstructured references to works; graph
construction; full text. One note for the merge plan: OpenAlex and Semantic
Scholar are aggregators that have already done identity resolution (their
`ids` / `externalids` maps). Those assertions are an input to merging, and a
yardstick for it.

Standing, 2026-09-15: for the POC this pipeline is an aggregator over
aggregators. Nothing comes from a publisher directly (publisher RSS feeds
are a possible later origin; whether anyone still publishes them is
unchecked). The interesting data is where the aggregators *disagree* about
a record. A policy that resolves those disagreements sensibly is a product
feature, not plumbing — for instance, preserving matchability against the
whole smear of representations a work has while promoting the corrected
form for display and use. That policy belongs to the merge plan; what
ingestion owes it is every representation kept intact and typed, so that
the disagreement is visible rather than papered over at the source. This
is roughly what Dimensions does; the aim is to do it better.

The merge plan is [merge-plan.md](merge-plan.md): draft 1, 2026-09-15,
written against the overlapping sample (§7) and a census of it.

## 2. Sources

Five. Summary first; per-source facts follow. The bullets mix documented
facts with *expected* warts from prior experience of these sources; every
wart is a claim the POC run confirms or refutes, not a commitment.

| Source           | Native ID  | Nodes | Edges                                 | Full-scale channel                          | POC channel                              |
|------------------|------------|-------|---------------------------------------|---------------------------------------------|------------------------------------------|
| Crossref         | DOI        | yes   | yes — `reference[]`                   | annual file: 35,908 `.jsonl.gz` shards, 223 GB, torrent | the official 31 MB random-sample torrent |
| PubMed           | PMID       | yes   | partial — `ReferenceList`, mostly PMC | baseline + daily update files, HTTPS        | one late-numbered baseline file          |
| arXiv            | arXiv ID   | yes   | **no**                                | OAI-PMH harvest                             | OAI-PMH, ~10 pages                       |
| OpenAlex         | `W…`       | yes   | yes — `referenced_works[]`            | quarterly S3 snapshot over HTTPS, 666 GB    | one manifest-chosen part: 10,873 records, 20 MB |
| Semantic Scholar | corpus ID  | yes   | yes — `citations` dataset             | Datasets API, 30 shards per dataset, API key | **paused** — needs a new API key        |

### Crossref

- Nearly 1.8 × 10^8 records in the March 2026 file. Distributed as 35,908
  gzipped JSON-lines shards, `0.jsonl.gz` to `35907.jsonl.gz`, flat in one
  directory, one record per line — 223,053,729,528 bytes, and no tar (the
  torrent's metafile, read 2026-09-25) — the pre-2024 `{"items":[…]}`
  layout is gone. Beside them is `a.py`, 666 bytes: Crossref's own
  conversion script, which sorts a shard's records by DOI and writes each
  back with its keys sorted, so every line starts `{"DOI": …`. Via
  Academic Torrents (doi 10.13003/nggf-vt1j) or AWS requester-pays
  (~$18). Records are the REST API's item objects: confirmed on the
  sample, whose 10,000 records normalize with no failure on model 8
  (2026-09-25); the reader tells an API page from a record line by its
  first bytes.
- Crossref also publishes a 31 MB **random sample** as its own torrent: 100
  `.jsonl.gz` files. That is the POC input — no selection from the full
  file needed, and a random draw is the best schema-diversity sample available.
  Caveat from first use: the sample torrent has no web seed and few peers — it
  fetched zero bytes in a session while the full file showed 34 seeders.
  Fallbacks in §7.
- The 2026 file includes retractions from the Retraction Watch database.
- `reference[]` entries carry a `DOI` (resolved edge) or only `unstructured`
  text / partial fields (author, year, journal-title, volume, first-page).
  Coverage depends on publisher deposits.
- `title` is an array: usually one, sometimes zero, sometimes several.
- `issued.date-parts` is `[[2020,3,15]]`, `[[2020,3]]`, `[[2020]]` or `[[null]]`.
- Organisational authors use `name` instead of `given`/`family`.
- `abstract` is a JATS XML fragment, present for a minority.
- `type` is a finite vocabulary (`journal-article`, `posted-content`, …).
- Retractions/corrections are `update-to[]` on the *updating* record — the
  notice points at the original. Marking the original retracted means
  inverting the relation: a corpus-level pass, so it belongs to the merge
  stage; ingestion just types the relation.
- `subject` is deprecated. `relation` (`has-preprint`, …) and
  `alternative-id` are cross-links. arXiv is not a Crossref member.
- Observed in the 1,000-work API sample (355 distinct JSON paths):
  `api_format.md` is behind the data — it does not know `role[]`,
  `award-info`, `funder.id[]`, `affiliation.id[]`, `resource`, `project`,
  `indexed.version`, `stm-asf`, `syndication` or `updated-by`, and it calls
  `issn-type` values `pissn`/`eissn` where the data says
  `print`/`electronic`. `date-parts` is always one inner array of one to
  three integers, or `[[null]]` (34 works, all `issued`). Names are absent
  on some affiliations, funders and institutions; `contributor[].role` is
  `[null]`. Three vocabularies have an authoritative source (`/types`, the
  Crossmark update types, the deposit schema's identifier types); the rest
  of the enums are exactly what was observed.
- Value level, same sample: every DOI, ORCID (447), ISSN (1,230) and ISBN
  (186) passes its checksum; every timestamp's `date-parts`, `date-time`
  and `timestamp` agree; `URL` is exactly `https://doi.org/` + DOI;
  `references-count` equals the array; `published` equals `issued`. Those
  are strict types and enforced invariants in the normalizer — but for
  the ISSN and, since the whole March 2026 file was surveyed, the ISBN and
  the ORCID, which a few publishers get wrong and which are claims (§10).
  `prefix` is
  the DOI's *current owner*, not its registrant — they differ on 67 works,
  because DOIs move between publishers. 47 reference DOIs (0.2 %) carry a
  Unicode hyphen (U+2010) for an ASCII one: the `Claim<Doi>` category
  again. In the bulk sample (2026-09-25) it is 13,305 of 262,499 reference
  DOIs, every one in Wiley's records, and it is read as `-` since model
  version 9 (§10). `updated-by` appears on affected works, so retraction status needs
  no inversion pass after all — §2's earlier claim about `update-to` stands
  for the notice side only.
- Institutions (model 10). An author's affiliation and a work's own
  `institution[]` are one element of the deposit schema, `<institution>`:
  a name or an id, up to six each of acronyms, places and departments, and
  ids of three kinds, ROR, ISNI and Wikidata (`institution_id@type`, the
  same from 5.3.1 to 5.5.0). All of it is kept, each id with who asserted
  it; a ROR and an ISNI are claims, because a few records' fail their
  checks. An empty one, `{}`, says nothing and is set aside, and so is an
  author with nothing in it but its `sequence`.
- Types (model 10). A record may have no `type` — 2,257 among those the
  survey stopped at, EMBL-EBI's dataset pages first — and its kind is then
  unknown: no form, no genre but a notice's. Crossmark's update type was
  free text before deposit schema 4.3.5 (October 2014), and old records
  carry their publishers' words: each is read for its meaning and kept
  beside it (`vocab::UpdateWord`), a word that is no update (`print`, a
  section's name) is set aside and counted, and a test deposit's word
  keeps its record out through `source_crossref::exclusions`. Crossref
  copies a work's `updated-by` into its `relation` under the same word.
  The reference, funding, peer-review and relation-id lists are the
  deposit schema's whole lists.
- Dates (model 10). A license's `start` may be absent: Crossref writes
  the work's `issued`, padded to its first day, where a deposit gave no
  start, and nothing where the work has no `issued` to fill it from —
  dissertations and standards above all. A start is read as from
  publication (absent, or on the padded `issued`), as its own day, or,
  for the one `0001-01-01`, as written beside why; `delay-in-days` is
  checked as `max(0, start − issued)` and dropped. The dates beside the
  publication dates — `posted`, `accepted`, `approved`,
  `published-other`, `content-created`, `content-updated` — are kept, each
  with what it is.
- Fields new in the March 2026 file (model 10): a work's `version`
  (deposit schema 5.4.0's `version_info`: a label, kept as written, and
  its language, read and not kept) and `part-number` (a set's part, kept
  as written). A grant's projects carry more than the reader knew — a
  funder's share of the money, co-lead investigators, investigators'
  other names and dates, planned dates, funders named by a ROR alone,
  amounts with no currency — all read, none kept.
- Known gaps, kept on purpose and listed so they stay known: editors,
  chairs and translators are dropped, and their affiliations with them,
  though those carry the same ids (the user, 2026-09-26: fine "so long as
  we know we're missing important data"). A reference's `type` is read
  and not kept (its own pass, for citation matching). Crossmark policy
  pages — about 260 among the records the survey stopped at, typed or
  not — are not works and are normalized as if they were, until books and
  their kin are taken on after articles. Posted content's `status`
  (withdrawn, removed) is kept and the merge does not read it yet. Records
  on Crossref's test prefix, `10.5555`, are normalized as works unless
  one stops the run and joins the exclusion list. Grants are read and not kept;
  a grant's `issued` is the earliest of its award starts, and is kept as
  its publication date meanwhile. A conference's dates and a journal
  issue's are read and not kept. Crossref's live API writes `subtype:
  blog` on Rogue Scholar's posts (2026-09-26), where the March 2026 file
  has them as `other`: the next file will stop there.
- Terms: metadata is free to reuse and not subject to copyright; abstracts
  carry some limitations.

Records looked up through E-utilities (2026-09-15), older than the 2026
baseline files, showed forms the baseline never did — `pmc` reference ids
with the prefix, `NumberOfReferences`, month ranges and slashes in
`Season`, repeated `ELocationID`s, `OtherID`, `medline` and `sici` ids,
`GeneSymbolList`, more manuscript-id prefixes, `PersonalNameSubjectList`,
`GeneralNote` — all modelled (§9 M5).

### PubMed

- ~3.7 × 10^7 records. Baseline: ~1,300 `pubmedNNnNNNN.xml.gz` files over
  HTTPS, roughly PMID-ordered, so late-numbered files hold recent records
  (DOIs, references, structured abstracts); early ones hold 1960s records
  with neither. Daily update files add, revise and — via `DeleteCitation` —
  delete. E-utilities `efetch` returns the same XML.
- `PubDate` is `Year`/`Month`/`Day`, or `Season`, or `MedlineDate` free text
  ("1998 Dec-1999 Jan"); `Month` is a name or a number.
- Authors are `LastName`/`ForeName`/`Initials` or `CollectiveName`; ORCID as
  an `Identifier` on some.
- `Abstract` may be several labelled `AbstractText` sections.
- `ArticleIdList` carries `doi`, `pmc`, `pii`.
- `ReferenceList` in `PubmedData`, mostly PMC-derived; each reference has a
  `Citation` string plus optional `pubmed` / `doi` IDs.
- Retraction is on the affected record: `PublicationType` "Retracted
  Publication", `CommentsCorrectionsList` `RefType="RetractionIn"`.
- MeSH headings: richest topic signal of the five.
- Observed in `pubmed26n1334.xml.gz` (4,989 records, 250 distinct element
  paths): `<PMID Version="1">` on every record. `IdType="pmc"` and
  `IdType="pmcid"` are one identifier in two serializations by role — the
  record's own PMC ID is `pmc` with a `PMC` prefix (`PMC12834818`), a
  reference's is `pmcid` as bare digits (`5349499`); never both on one
  record. A `PmcId` type needs two entry points, like `Pmid`.
- The 2026 baseline still declares the 2025 DTD (`pubmed_250101.dtd`) over
  plain `http://`. `AbstractText`, `ArticleTitle`, `Citation`, `Keyword`,
  `CoiStatement` are mixed content — inline `b`/`i`/`sub`/`sup` and MathML
  trees up to six deep appear in real abstracts. `Language` occurs twice on
  one record (`Language+` in the DTD). `Grant/Country` is present but empty
  on 92 % of grants.
- The DTD is large. Strict modelling of it is the biggest single modelling
  surface in this plan; the POC file will show how much of it is exercised.
- Terms: NLM provides the data freely; some abstracts may be under copyright
  *(confirm the current NLM data terms before product use)*.

The REST API's works for the PubMed seed (2026-09-15) carried forms the
1,000-work sample lacked — CRediT roles and reviewers, ISNI affiliation
ids, Retraction Watch as an update source, a reference-type vocabulary of
its own, award ids — every one now a closed enum or a field (§9 M5).

### arXiv

- ~2.5–3 × 10^6 papers. Channel: OAI-PMH (`metadataPrefix=arXiv`, 1,000
  records per page, resumption tokens, throttled with `Retry-After`).
  Sanctioned bulk path, supports `from`/`until` and `set` for selective
  harvest, one schema for POC, full scale and incremental. This replaces the
  Kaggle snapshot (account required, ~4.5 GB) and the Atom API (different
  schema, 3 s per request).
- No reference lists in any channel.
- `id` is old style `archive[.subject]/YYMMNNN` (`hep-th/9901001`) or new
  style `YYMM.NNNN` (2007–2014) / `YYMM.NNNNN` (2015+); versions are `vN`.
- OAI `arXiv` format gives structured authors (`keyname`, `forenames`,
  `suffix`, `affiliation`), `created` (v1 date), `updated`, `categories`
  (space-separated), `journal-ref` free text, `doi` (the journal DOI, when
  authors supplied it — a minority), `license`, `comments`.
- Every paper also has a DataCite DOI `10.48550/arXiv.<id>` since 2022 —
  DataCite, not Crossref, so it resolves in none of our other sources except
  via OpenAlex/S2 `ids`.
- Withdrawal is a new version whose comment says "withdrawn".
- Terms: metadata is CC0 *(confirm against arXiv's bulk-data terms)*.

### OpenAlex

- Snapshot at `s3://openalex/data/jsonl/works/updated_date=YYYY-MM-DD/part_XXXX.gz`,
  free, no AWS account or key, fetchable over plain HTTPS
  (`https://openalex.s3.amazonaws.com/` + path). `data/jsonl/works/manifest.json`
  lists every part with `url`, `content_length`, `record_count`. Quarterly.
  The June 2026 snapshot: 510,372,821 works, 666 GB gzipped JSONL. A Parquet
  twin exists; not used — raw JSON is what the model mirrors (§5 rule 8).
- A part file is one `updated_date`, so any single part is biased toward
  whatever was touched that day. POC part: `updated_date=2025-07-25/part_0000.gz`,
  10,873 records, 20 MB. Take a second from another date if diversity matters.
- The REST API requires a key since 2026-02-13 (free tier exists). Irrelevant
  to the snapshot path; it returns the same object schema.
- Widest top-level schema of the five (50+ fields) and it evolves between
  snapshots (`concepts` deprecated for `topics`, fields added). Fail-fast
  will fire on snapshot upgrades; that is the intended alarm.
- `ids` (openalex, doi, pmid, pmcid, mag) is the richest cross-ID map, but
  `doi` and `pmid` are URL-form (`https://doi.org/…`,
  `https://pubmed.ncbi.nlm.nih.gov/…`).
- `abstract_inverted_index`: word → positions; must be reconstructed.
- `referenced_works[]` are OpenAlex `W…` IDs — edges in OpenAlex's namespace.
- Also: `is_retracted`, `open_access{}`, `language`, `type` and
  `type_crossref`, `mesh[]` on PubMed-derived works, `topics[]`,
  `authorships[]` with institutions and `raw_author_name`, `indexed_in[]`.
- `is_authors_truncated`: the API truncates very long author lists; check
  whether the snapshot does.
- Terms: CC0.
- Unpaywall is not a separate dataset any more: since OurResearch's rewrite
  it is a legacy-compatible format served from the same OpenAlex data, and
  "the OA facts in an Unpaywall record and in an OpenAlex work's
  `open_access` object come from the same pipeline". `open_access`,
  `best_oa_location` and each `locations[]` entry's `is_oa` / `license` /
  `version` / `pdf_url` are that data. No separate sample is needed
  (checked 2026-09-15).
- Observed in the 10,873-work part file (249 JSON paths): the Work object
  documentation has moved and its new home 404s, so the census is the
  document. `best_oa_location` is never null but is an object of nulls when
  there is no OA copy (6,869); `institutions` is always an empty array;
  `is_xpac` is undocumented; `author.id` is null on 1,321 of 30,825
  authorships; `apc_list` holds integers where the one `apc_paid` holds
  floats; `abstract_inverted_index` is present on 8,831 with 951k words and
  1.5M positions, all sorted. `referenced_works` are `W…` ids: 50,724 edges
  in the file, 2,800 works with any.
- Value-level census (2026-09-15, `openalex_values.py`): every ORCID (12,555
  resolved, 6,931 raw), every ROR (8,366) and every MeSH UI passed its
  check; 15 `doi` values are the bare `https://doi.org/`; 4 source ISSNs
  fail their check digit; `language` is `sh` once and `ceb` once.
  Derivable, and now checked rather than stored: `ids.openalex`, `ids.doi`,
  `display_name`, `publication_year`, `authors_count`,
  `corresponding_author_ids`, `corresponding_institution_ids`,
  `countries_distinct_count` (from the institutions' codes, not the
  authorships'), `institutions_distinct_count`, `locations_count`,
  `primary_location`, `best_oa_location` (always the first `is_oa`
  location), `primary_topic`, `open_access.is_oa`, `license_id`,
  `is_accepted`, `is_published`, `is_paratext`, `referenced_works_count`,
  each award's `funder_display_name`, each source's `host_organization`.
  Not derivable, kept: `author_position` (30 single-author works say
  `last`), `raw_orcid` (differs from the resolved one on 5; each occurs
  without the other), authorship `countries` (a superset of the
  institutions'), `ids.mag` (not the `W` number on 2), `oa_url` (absent on
  172 open works), `has_fulltext` (independent of `has_content` on 1,993).
  A location's `id` is typed by its `provenance`: a MAG id (9,973; not this
  work's on 43), a PMID (409; not this work's on 181), a bare DOI (4,960;
  not this work's on 992) or the repository's own string (5,957). Scores
  are not float32-exact except the SDGs', so they stay `f64`. Two works
  cite themselves.

The API (2026-09) is a newer serializer than the June snapshot: §9 M5
lists every difference the first 35,000 looked-up works showed, all
modelled, each with a fixture.

The snapshot of 2026-09-23 (2,040 parts, 476,196,327 works; 1,581 parts
and 403.6 M works in 2026 partitions) differs again. Three of its
smallest 2026 parts (2026-03-23, 2026-03-30, 2026-06-29; 9,071 works),
enumerated path by path against the model on 2026-09-25, while the 17
June parts show no difference under the same script:

- `source.listed_in`, new, on every source object: an array of the lists
  that index the source (`erih-plus`, `cwts-core`, `doaj`, `medline`,
  `scielo`, `latindex`, national lists such as `jufo-1`…`3`, `norway-1`,
  `2`, `ki-jl-1`…`3`, `abdc-*`), `[]` when none; absent from the API's
  answers. Model 8 refuses it at the first work of every part.
- A work `type` of `software`: 25 works, all Zenodo DataCite records.
- `updated_date` before `created_date`: 2 works, both created
  2026-08-07.
- `language` codes `enc` and `idb`: kept as malformed claims, not failures.

These parts are small and mostly DataCite's, so the large 2026
partitions may hold more; the snapshot is surveyed whole before it is
normalized (§10, 2026-09-25).

### Semantic Scholar — paused

- Paused: the datasets need an API key, and the previous one was invalidated
  in S2's key reset. Everything below stays valid; resume when a new key
  arrives.
- Datasets API `/datasets/v1/release/<id>/dataset/<name>` returns pre-signed
  shard URLs. Current release `2026-09-09`; 30 shards per dataset: `papers`
  200M records in 30 × 1.5 GB, `abstracts` 100M in 30 × 1.8 GB, `citations`
  2.4B in 30 × 8.5 GB; also `authors`, `paper-ids` (450M), `publication-venues`,
  `tldrs`, `s2orc`, embeddings. **Requires an API key** (free, requested via
  the form at semanticscholar.org/product/api; you obtain it, I never handle
  it). Introductory limit 1 request/s.
- One paper's data is split across datasets joined on `corpusid`: `papers`
  (metadata, `externalids`: DOI, ArXiv, PubMed, PubMedCentral, MAG, DBLP,
  ACL), `abstracts`, `citations` (`citingcorpusid`, `citedcorpusid`,
  `isinfluential`, `intents`, `contexts`), plus `paper-ids`,
  `publication-venues`.
- `papers` fields: `corpusid`, `externalids`, `title`, `authors[]`
  (`authorId`, `name` — a single string), `venue`, `publicationvenueid`,
  `year`, `publicationdate`, `journal{name,volume,pages}`,
  `publicationtypes[]`, `s2fieldsofstudy[]`, `isopenaccess`,
  `referencecount`, `citationcount`, `influentialcitationcount`.
- Terms: ODC-BY with attribution to Kinney et al. 2023 (arXiv:2301.10140) —
  a product-facing obligation, not just a download formality.

### Long tail — assessed and mostly wanted, 2026-09-15

Scoped to articles and their edges, books excluded. Because the product
lives on the disagreements between aggregators (§1), overlap with what is
already ingested is not a reason to skip a source. In rough order of
expected value:

- **DataCite** public data file: DOIs for datasets, preprints and theses;
  a source of record where OpenAlex and Crossref disagree with it. Wanted.
- **NIH Open Citation Collection** (iCite): CC0, PMID → PMID edges from
  full text; the biomedical edge set that PubMed's own reference lists
  only partly carry. Wanted.
- **OpenCitations Index**: CC0, bulk CSV dumps, ~2 × 10^9 citing-DOI →
  cited-DOI edges built from Crossref, DataCite and NIH-OCC. The most
  direct edge source and an independent check on Crossref's reference
  DOIs. Wanted.
- **Europe PMC** (and PMC's own OA subset): bulk full-text XML with parsed
  reference lists carrying PMIDs and DOIs, plus preprints. Metadata
  overlaps PubMed and OpenAlex; the value is at the full-text stage.
- **CORE**, **BASE**: aggregators over the repositories OpenAlex harvests,
  which is exactly the point — a third and fourth opinion on the same
  records, and CORE's full-text corpus. Both need an application for bulk
  access. Wanted.
- **Retraction Watch**: carried free by Crossref since 2023, also a CSV;
  the funnel's retraction filter wants it. Wanted.
- **DBLP**: CC0 dump, computer science, no citations; clean venues and
  authors for one field. Wanted.
- **Papers With Code**: the site was retired in 2025; its CC BY-SA dumps
  (papers, code links) survive on Hugging Face as of last look, unverified
  here. Wanted if the dumps are still reachable.
- **Papers We Love**: a curated list, not a database — no schema, no ids
  beyond links. Useful as a seed set, not as a source.
- **JSTOR**, **Dimensions**, **Google Scholar**: closed. JSTOR's article
  metadata is in Crossref under its prefix; Dimensions has no bulk access
  and a proprietary graph; Scholar has no API and forbids scraping.
- **OAI-PMH** is a protocol, not a source: the route to arXiv, and how the
  aggregators above are harvested.

Each wanted source is a crate implementing `Source` (§3.2) when its turn
comes; the order above is a guess to be revised by what the merge stage
finds it needs.

### Consequences

Edges now come from three overlapping producers — Crossref (publisher
deposits), OpenAlex (Crossref + MAG + PubMed derived), Semantic Scholar (own
extraction plus the same upstreams) — and partially from PubMed. Overlap and
disagreement between them is the merge stage's problem. For ingestion it
means `Reference` targets live in three namespaces (DOI, `W…`, corpus ID) and
the target type is an enum over namespaces, not a DOI.

For the architecture doc: Semantic Scholar's `citations` dataset alone is
2.4 × 10^9 rows, and Crossref and OpenAlex are of the same order. The 2 × 10^10
its storage budget assumes is ~10× above what any source exposes. Out of
scope here; it changes that arithmetic. Taken up 2026-09-15: the graph
plan's §2 plans on 6 × 10^8 works and 3 × 10^9 edges.

## 3. Architecture

### 3.1 Stages

```text
acquire    list → fetch: bytes to <store>/raw/, byte-identical, verified, a receipt per file (§3.5) — or ingest, streamed through the stages below
parse      bytes → source-native typed record, borrowed from the buffer   (strict, §6)
normalize  native record → normalized records, pushed to the sink          (no copies, §5)
sink       normalized output, tagged with the model version, plus a stats report
```

Raw retention is a requirement. The premise of strict typing is that the
model *will* change on contact with real data; re-normalizing must never
require re-fetching.

### 3.2 What "pluggable" means here

- **Compile-time plugins.** Each source is a crate implementing a small set of
  traits from the core crate. Adding a source = a crate plus one line of
  registration in the binary. No dynamic loading, no config-defined
  transforms.
- **Origin is separate from normalization.** A normalizer takes a native
  record and does not know whether it came from a dump file or an API page.
  Every source has one raw schema (§2 chose channels to make this true).
- **Sinks are pluggable.** First sink is the binary layout in §5; the merge
  stage and the graph builder get their own later.
- **API origins are a last resort.** Used only where no download exists (POC
  gaps, later incremental updates); always rate-limited, always identified
  (`mailto`, keys), and every response is written to raw so nothing is
  fetched twice.

Trait shapes, as landed on 2026-09-15 (`crates/ingest/src/source.rs`,
factored from the three normalizers that came first). The lifetimes are
the point (§5):

```text
Source    NAME; Native<'a>, Id<'n>, Work<'n>, Reference<'n>; ParseError, NormalizeError
          records(impl BufRead, each: FnMut(&Native<'_>) → ControlFlow)   frames the stream; strict (structural)
          normalize(&'n Native, &mut impl Sink<'n>) → Result<()>  pushes by reference; strict (value-level)
Sink<'n>  work(&Work<'n>)   reference(&Id<'n>, &Reference<'n>)   deletion(&Id<'n>)
Runner    ingest::normalize::<S>(paths, limit, &mut sink) → Report, or
          Failure { source, path, record ordinal, stage, the source's error }
```

`records` streams (landed 2026-09-15, the second shape of this method):
the source cuts the raw byte stream into one record's frame at a time — a
line for JSON lines, one `PubmedArticle`/`DeleteCitation` element cut at
its own end tag for PubMed, a whole API page for Crossref's envelope —
parses the frame from a buffer reused for the next, and hands the record
to the runner's callback; `Break` ends the read before the next record is
parsed. A worker therefore holds one record's text, never a file's, and
the same code runs over a file on disk, an inflating decoder, or a
download in flight. The first shape took the whole inflated file as one
`&str`; on OpenAlex's largest parts (1.1 GB gzipped, 6.6 GB inflated)
that was one worker's memory, not a laptop's.

Parser and normalizer are two methods of one `Source` trait: they share
the native type and nothing else needed a boundary between them. Each
source crate exports a marker type (`Pubmed`, `Crossref`, `OpenAlex`)
implementing it; the binary registers each in one line. `'n` is the
lifetime of the native record; the runner requires a sink for every `'n`,
so a sink can keep nothing it is handed — it serializes immediately, as §5
rule 3 wants. Three record kinds so far — work, edge, deletion — because
that is what three sources emit; S2's abstracts will add a fourth when a
normalizer produces one.

A source may emit several record streams (S2 ships papers, abstracts and
citations as separate datasets, so it emits separate streams). It never joins
them.

### 3.3 Layout

```text
crates/
  model/              canonical types, strict newtypes — no IO, no source knowledge
  ingest/             the traits, runner (parallel over files), fetch, manifests, stats
  source-crossref/    native structs + normalizer, one crate per source
  source-pubmed/      (streaming XML)
  source-arxiv/
  source-openalex/
  source-s2/
  merge/              the merged layer (merge-plan.md): names, identity, the registry, the merged record, edges, the report; the journal, the public-id log, the assembled document
  graph/              the citation graph (graph-plan.md): both orientations over the edges set, a label an arc, the report
  serve/              the serving layer (serving-plan.md): the names lookup, the works index, the epoch, the query core
  search/             the lexical index behind one shape, Tantivy as a module; a second engine when wanted
  ol-rht-serve/       the transport binary: the query core over gRPC, public ids on the wire
  ol-rht/             CLI:  <source> list | fetch <listing> | ingest <listing> | check | normalize [--survey] | dump | keys; merge identity | merge [--public-key] | merge --batch <sets> | merge dump; graph build | graph query | graph check; search build | search query; names build | works build | epoch publish | epoch check; query work | query search | query expand
<store>/                  the store of record (§3.5): one root, `--store`, never committed
  raw/<source>/<path>     bytes as served, write-once, at the source's own path; `<path>.part` while in flight
  listings/<source>/<stamp>.jsonl   what the source offered, one entry per line, beside the index document it came from
  receipts/<source>.jsonl what was fetched: one verified file per line, append-only — the completion marker
  normalized/v<model>/<source>/<path minus extensions>.{works,references,deletions,symbols}.zst   rebuildable
  merged/registry.jsonl   the WorkId registry (merge plan §4): of record, append-only
  merged/v<model>.<merge>/<shard>.{works,edges,names,relations,unresolved,symbols}.zst   rebuildable
  graph/v<model>.<merge>.<graph>/{manifest.json,perm.u32,inv.u32,forward.*,transpose.*}   the citation graph (graph plan §8): rebuildable
  merged/journal.jsonl    one line per merge run: the clock every artifact is positioned by (serving plan §4): of record
  merged/public.jsonl     the public-id log beside the registry (serving plan §6): of record
  merged/v<model>.<merge>/delta/{works,edges,relations,unresolved}.jsonl   what incremental merges append (serving plan §5): rebuildable
  merged/merge.lock       while a merge runs, full or incremental: one at a time
  search/<engine>/v<model>.<merge>.<search>/   the lexical index (serving plan §3): rebuildable
  names/v<model>.<merge>.<serve>/   the names lookup (serving plan §3): rebuildable
  works/v<model>.<merge>.<serve>/   the works index (serving plan §3): rebuildable
  epochs/<n>.json  epochs/current   the epoch manifests and the switch (serving plan §4)
```

Each source crate carries `fixtures/`: single raw records that once failed to
normalize, each with a test asserting they now do (§6).

`hello` is removed when the first real crate lands.

### 3.4 Concurrency

Bulk inputs are already sharded into many files. Parallelism is a work
queue over files — `std::thread::scope`, no `rayon` — one worker per core,
each taking the next raw file from a shared index, sequential within a
file, and one sink per worker so nothing mutable is shared. A sink is told
where each raw file begins and ends, and the binary sink writes one output
set per raw file, named by mirroring the raw path below the inputs' common
directory, so a rebuild of one raw file replaces one set. A record limit
is claimed through one shared counter and is exact across workers; the
first failure raises a stop flag every worker checks between records.
Every worker's sink comes back to the caller either way, to be finished
or discarded — a failed run still leaves nothing. Landed 2026-09-15.
Within a file the runner claims the record limit one record ahead, so the
record after the limit is never parsed, and returns the spare claim when
the file ends; a claim that finds the limit reached waits while other
workers still hold claims that may come back, so the limit stays exact
across workers (a race the test suite caught once in forty runs). API mode is
rate-limited per source.

### 3.5 Store of record

Decided 2026-09-15. Everything the pipeline produces or depends on lives
under one store root, including its bookkeeping; git holds code and
fixtures only. The lesson is HDFS's: it stayed the store of record at a
previous shop because the bookkeeping lived in the same namespace as the
bytes — a file at a path was the fact, a `_SUCCESS` marker meant a stage
had finished, and every job spoke the same root. Committing per-file
manifests to git while the bytes sit gitignored splits that; the copy that
survives would be the pointers, which is backwards. The three manifests
committed under `manifests/` on 2026-09-14 move into the store when
`fetch` lands.

- One root, `--store`, the way `fs.defaultFS` was. Paths under it are the
  addresses the pipeline uses; the code speaks root plus relative path
  and nothing else.
- Raw bytes are write-once under `raw/<source>/` at the source's own
  paths. What a source offered and what was fetched are objects under the
  same root, beside what they describe, immutable once written; the
  record of a fetch is the completion marker for its raw file.
- Normalized output is a cache under a model-versioned prefix,
  rebuildable, never of record. Of the merge output, only the registry
  that mints `WorkId`s is of record — append-only, like receipts; the
  merged sets are a cache under `merged/v<model>.<merge>/` (merge plan
  §4, 2026-09-15).
- Nothing derived from data is committed.

Backing: a directory on this laptop for the POC; a box, not yet ready, for
the whole corpus (~1.3 TB raw across the five sources, §7), with plain
files on a checksumming filesystem or an S3-compatible store (MinIO,
Garage) on it. S3-compatible is today's HDFS — one API every tool speaks,
write-once objects, listing as the catalog — and the target once compute
moves next to the data. Filesystem paths now, the S3 API later; in
between, the layout must sync losslessly to a bucket. Record-level
streaming (§3.2, landed) is what makes reading straight from a bucket one
pass with no local copy. Not wanted: lakeFS, DVC, table
formats — they version mutable data, and raw here is immutable, with the
model version naming every derived layer.

Landed 2026-09-15. `list` asks a source what it offers — OpenAlex's works
manifest, NCBI's two directory indexes, Crossref's one torrent — reads the
index strictly (it fired on first contact: NCBI's update index carries a
`_stats.html` page per file) and writes a *listing*, one JSON line per
file with the source's own path, byte and record counts and sidecar URL
where the source gives them, beside the index document it came from.
Selection is a grep of the listing. `fetch` takes a listing and brings in
every entry without a matching receipt: into `<path>.part`, hashed on the
way, resumed by `Range` from a partial file, then checked against
everything promised — `Content-Length`, the listing's bytes, NCBI's `.md5`
sidecar, S3's `ETag` when it is an MD5 — synced, renamed, and only then
receipted. A file is its own transaction: a run stops at its first
failure with the files before it complete and the one in flight
resumable. It refuses torrents (printing the `aria2c` line) and any raw
file that exists without a receipt. `ingest` does the same while
normalizing: one worker owns one file from first byte to receipt, the
body tee'd to disk and the hashers, inflated, framed, parsed, normalized
and written on that thread, nothing handed between threads; on a resume
the partial file is replayed through the framer first. A transfer the
run stops inside — the record limit, another worker's failure — gets no
receipt and no output set. Three parts, 15 MB, on three workers: 1.3 s
end to end at 40 MB peak resident. The three POC files moved to their
sources' own paths and their manifests became receipts in the store;
`manifests/` is gone from the repository.

Lookups landed 2026-09-15 (§8, item 8; §9, M5), the seeding-only kind.
`keys` prints the identifiers a normalized set holds — a work's own DOI
or PMID, the ids its references point at — canonical and distinct, one
per line. `list --seed <file> --by <key>` chunks that seed into the
requests that answer it (50 DOIs to OpenAlex or Crossref, 200 PMIDs to
NCBI's `efetch`), every URL known before a byte moves, as entries of a
third kind at `lookup/<stamp>/<key>-<n>.<ext>`, the seed kept beside the
listing the way an index document is. `fetch` takes that listing like any
other: one request in flight, paced per source (one a second at OpenAlex
and NCBI, one every two at Crossref), one wait for what a 429 or 503's
`Retry-After` asks up to five minutes and a stop at anything else, the
page kept whole and never resumed, receipted with the `Content-Length`
check when the response carries one; a half-done run resumes through the
receipts as files do. `--mailto` reaches an API as the source's own
contact parameter, only when given, and never lands in a listing or a
receipt. `ingest` refuses a lookup listing: a page is fetched, then
normalized, so one the parser rejects stays on disk as the evidence.

## 4. Model — what is committed

Only this much is known well enough to commit to:

- Each source has a native unique identifier, and every normalized record
  carries it. These five types and the rules they enforce are the model:

| Type          | Rule                                                                                        |
|---------------|---------------------------------------------------------------------------------------------|
| `Doi`         | `10.<4–9 digits>/<suffix>`; lowercased (DOIs are case-insensitive); URL forms stripped      |
| `Pmid`        | positive integer that fits `u32`; URL form stripped                                         |
| `ArxivId`     | old style `archive[.subject]/YYMMNNN` or new style `YYMM.NNNN[N]`; version parsed off, kept |
| `OpenAlexId`  | `W` followed by digits; URL form stripped                                                   |
| `S2CorpusId`  | positive integer; width fixed from the observed maximum                                     |

- The normalized layer is per-source records, one stream per record kind the
  source emits, keyed by these IDs. Its remaining fields are an *output* of
  the discovery in §4a, not an input to this plan.

Everything else about the normalized shape — a candidate field list, edge and
abstract streams, non-ID newtypes — is speculation written before any record
was parsed. It lives in Appendix A, marked as such. The merged schema is a
separate plan altogether (§1).

### 4a. Schema discovery

For each source one of two things is true, and the first job of its crate is
to find out which:

1. **The source publishes a normative description of what its serializer
   emits.** Then the native struct mirrors that document field for field, and
   the document is the first fixture. Candidates to check — none yet
   confirmed complete or current:
   - PubMed: the yearly XML DTD (`pubmed_YYMMDD.dtd`). Genuinely normative.
   - arXiv: the OAI `arXiv` metadata XSD.
   - Crossref: `api_format.md` in the REST API docs. Descriptive, not a schema.
   - OpenAlex: the "Work object" pages at docs.openalex.org. Descriptive and
     evolving. The Parquet twin of the snapshot carries a machine-readable
     schema of the same serializer's output; worth diffing against the JSONL.
   - Semantic Scholar: README field descriptions only.
2. **It does not, or the document is incomplete.** Then the native struct is
   built from observed records, and `deny_unknown_fields` (§6) is the
   discovery mechanism: every field the source emits that is not modelled
   stops the run and gets modelled.

In both cases reality is discovered along the way. Serializers have bugs,
APIs have bugs, and sometimes the data itself is the bug; a normative
document says what the source *meant* to emit. The fixture corpus (§3.3) is
where the gap between meant and did accumulates, one record per divergence.

## 5. Type design for throughput

The scale is awkward: too much for a laptop to be careless with, far too
little to justify a cluster. The failure mode is not algorithmic. It is a few
billion small allocations, copies and serialization round-trips that each
cost nothing and together cost hours — the same pathology that made a Hadoop
job take hours over a handful of records. The strict types are the mechanism
against it: the trait boundaries in §3.2 are shaped so that the cheap path is
the only path.

**Envelope** (full scale, this machine): ~3 × 10^8 records, ~10^9–10^10 edges,
~1 TB raw gzipped. Normalization is one streaming pass — decompress, parse,
normalize, write — per file, per worker, with nothing alive beyond one record
and one file's buffers. Illustratively: at 20k records/s/core that is ~4
core-hours; at 2k/s it is two days. The difference is entirely the rules
below. M2 replaces the illustration with a measured number, and that number
becomes a budget checked on every change.

First measurement, 2026-09-14 — PubMed, first cut of `source-pubmed`,
release build, one core, Apple Silicon: **28.8k records/s, 393 MiB/s** of
decompressed XML through the strict zero-copy parser, with gzip inflation
running at the same rate ahead of it. Extrapolated, the full 37M-record
baseline is ~22 core-minutes. Allocations per record are not yet counted.

Second measurement, 2026-09-15 — all three sources through one runner,
end to end (inflate, parse, normalize, and write the binary sink at zstd
level 3), release build, one core, Apple Silicon:

| source   | records | without sink | with sink   | allocations / record | output vs raw          |
|----------|--------:|-------------:|------------:|---------------------:|------------------------|
| PubMed   |   4,989 | 14.3k rec/s  |  8.9k rec/s | 74 parse only, 94 all | 11.3 MiB vs 13 MB gz  |
| Crossref |   1,000 | 28.5k rec/s* | 14.4k rec/s | 195 parse only, 207 all | 1.8 MiB vs 11 MB JSON |
| OpenAlex |  10,873 | 22.5k rec/s  | 15.2k rec/s | 162 parse only, 184 all | 13.2 MiB vs 20 MB gz  |

\* uncompressed input; the others include gzip inflation.

Third measurement, 2026-09-15, after record framing (§3.2) replaced the
whole-file `&str` — same machine, same files, one core, peak resident set
from `time -l`:

| source   | parse only          | with sink           | peak RSS before → after | bytes allocated / record |
|----------|--------------------:|--------------------:|------------------------:|-------------------------:|
| PubMed   | 14.9k → 13.0k rec/s |  6.6k → 8.2k rec/s  | 77 MB → 4.9 MB          | 66.5 KB → 13.6 KB        |
| Crossref | 26.4k → 24.4k rec/s | 14.5k → 15.3k rec/s |  5 MB → 7.3 MB          | 48.6 KB → 66.8 KB        |
| OpenAlex | 20.5k → 25.4k rec/s | 15.4k → 16.0k rec/s | 123 MB → 5.3 MB         | 37.7 KB → 13.2 KB        |

Throughput is within noise either way (the OpenAlex gain is the giant
string leaving the cache); memory is now one record plus buffers, which
is what lets eight workers take the largest OpenAlex parts. Crossref's
API pages are one document each and are still held whole, as they must
be; its public data file is JSON lines and streams. Output is
byte-identical to the second measurement's on all 36 files. Three
findings from the second measurement stood until the fourth measurement.

Fourth measurement, 2026-09-15, the allocation work (rule 1) — same
machine, one core, parse only, then with the stats sink and the binary
sink; output byte-identical on all 36 files:

| source   | allocations / record before → after | bytes allocated / record | parse only        | with sink        |
|----------|------------------------------------:|-------------------------:|------------------:|-----------------:|
| PubMed   | 87 → 7.5 (8.8 with sinks)           | 13.8 KB → 7.3 KB         | 14.1k → 13.9k rec/s | 8.5k → 9.1k rec/s |
| Crossref | 195 → 50 (57 with sinks)            | 66.8 KB → 54.9 KB        | 23.3k → 21.9k rec/s | 12.5k → 14.0k rec/s |
| OpenAlex | 162 → 2.1 (3.1 with sinks)          | 13.2 KB → 2.6 KB         | 25.4k → 22.4k rec/s | 14.5k → 16.8k rec/s |

Two causes, neither of them the `Vec`s the second measurement blamed
first. (a) serde's derive borrows a bare `Cow<str>` field marked
`#[serde(borrow)]` and copies one inside an `Option` or a list — `Cow`'s
own `Deserialize` yields the owned form — so most of a JSON record's
strings were a `String` each; `ingest::Text` borrows wherever it sits and
took OpenAlex from 162 to 2. (b) quick-xml's attribute iterator keeps a
heap-allocated list of seen keys for its duplicate check, one allocation
per element with attributes; PubMed does that check inline now. The lists
were real but smaller: every list field is a `smallvec` sized at about
the 90th percentile of its observed length (`Few<T, N>`), so only a long
list allocates. What remains: Crossref's API escapes every `/` (8,376 in
one page of 100 works), and each escaped string must be unescaped into
its own allocation — its public data file may not; PubMed's last six are
quick-xml's per-frame open-tag bookkeeping, kept whatever the
configuration (its issue #514), and a handful of long lists; OpenAlex's
two are lists past their inline length and the 10 % of records with an
escape. Throughput is within noise: the allocations were never the
dominant cost, as the second measurement said, but rule 1 is now met in
spirit — steady state is a few allocations per record, not a hundred.

Fifth measurement, 2026-09-15, symbol tables (rule 4) — layout version 2:
every `model::Interned` value in a set's records is a `u32` into the
set's fourth file, `<prefix>.symbols.zst`, one string per symbol; the
sink interns through a table installed on its thread while a set is open,
the reader loads the table beside the records and resolves through it,
and `dump` prints the text. Interned: journal title, abbreviation,
MedlineTA, NLM id and country, MeSH descriptor and qualifier names,
publication types, chemical names and registry numbers, grant agencies
and countries, data-bank names (PubMed); publisher, container title,
affiliation name, license URL, relation kind (Crossref); source, host
organization and lineage names, institution name, topic, subfield, field
and domain names, keyword slug and name, concept name, MeSH descriptor
and qualifier names, funder name, SDG name, raw source name and type
(OpenAlex). Chosen from a repetition survey: those repeat enough that
52–100 % of their bytes within a file are copies; titles, author names,
URLs and citation text do not and stay text, as the rule says.
Affiliations were tried and taken out: half of them are unique within a
file (116k distinct of 247k in one PubMed file), so the table held most
of the bytes the records saved. Every JSON dump is byte-identical to the
old layout's, on all 36 files.

| source   | symbols per set       | works stream | whole set | with sink                                    |
|----------|----------------------:|-------------:|----------:|---------------------------------------------:|
| PubMed   | 34,216                | −2.3 %       | −0.5 %    | 9.1k → 8.8k rec/s, 8.8 → 10.0 allocs/record  |
| Crossref | 235 per 100-work page | −11.6 %      | +0.2 %    | 14.0k → 13.1k rec/s                          |
| OpenAlex | 42,246                | −7.2 %       | −3.8 %    | 16.8k → 16.0k rec/s, 3.1 → 7.0 allocs/record |

The honest reading: at per-file granularity, bytes barely move — zstd
already folds a journal name repeated within a window, and a table per
set is paid for by every set — and the sink pays about 5 % in throughput
for the hashing, one allocation per distinct string per file. What the
tables buy is structural, and downstream: the merge stage and the graph
builder receive categorical values as small integers with a per-file
dictionary, not ten million forty-byte strings to hash and compare
(rule 4's trap). Whether that is worth the 5 % is a question for the
merge stage to answer with a number; the field set is the knob, and the
whole thing is one type in one place if the answer is no.
(1) The sink costs 30–45 % of the throughput at zstd level 3; the level
and the frame size are knobs not yet turned. (2) Rule 1 is not met: 75 to
200 allocations per record, nearly all of them the `Vec`s inside the
native records — authors, references, MeSH headings; quick-xml and serde
build them per record — and their counterparts in the normalized ones;
the sink adds under twenty. At ~3 × 10^6 allocations per second that is a
tax of perhaps a tenth of the time, not the dominant cost; the fix, when
it is worth it, is per-worker arenas or reused native records. This is
the first budget number. (3) The layout is about the size of the gzipped
raw, not smaller: OpenAlex's abstract is kept as raw JSON text and every
name and affiliation is a string. Symbol tables (rule 4) and a compact
abstract form are the levers, not the compressor.

Napkin extrapolation, same date — whole corpora at the measured end-to-end
rates with the sink, one core: PubMed, ~4 × 10^7 citations (1,334
baseline files of 30,000; the 3.7 × 10^7 in §2 was an estimate) → ~75
min; Crossref, ~1.8 × 10^8 works → ~3.5 h, call it 4 with inflation;
OpenAlex, 5.1 × 10^8 works → ~9.5 h. About 14 core-hours, 9 without the
sink. On eight cores, once the runner is parallel over files, ~2 h of wall
time, and the disk keeps up (0.9 TB of gzipped raw reads in ~15 min from
NVMe). The binding constraints are not the machinery: the raw is 0.9 TB
gzipped, this layout writes roughly as much again, and downloading it
takes longer than running it. The POC stays at 10^4 records per source.

The rules are committed. The specific types named inside them —
`PartialDate`, `WorkKind`, interned venue names — are Appendix A
illustrations of a rule, not commitments.

Rules, each with the trap it closes:

1. **Borrow from the raw buffer; own only at the sink.** Native structs are
   `Native<'a>` with `Cow<'a, str>` fields (`#[serde(borrow)]`; quick-xml text
   events likewise). A field needing no unescaping costs zero allocations. A
   worker reads one record into a reusable buffer, parses borrowing from it,
   normalizes, writes, clears. Steady-state allocation per record approaches
   zero. *Trap: a `String` per field per record — 60 fields × 3 × 10^8.*
2. **No DOM.** Never deserialize into `serde_json::Value` or an XML tree and
   walk it. Typed structs straight from bytes. JSONL sources (Crossref,
   OpenAlex, S2) are read a line at a time into the reusable buffer; XML
   sources (PubMed, arXiv OAI) through a streaming event parser. *Trap: parse
   twice, allocate everything, keep it all alive.*
3. **Normalize by consuming, emit by pushing.** `normalize(native, &mut emit)`
   consumes the native record and pushes `Work<'a>` / `Reference<'a>` /
   `Abstract<'a>` to the sink by reference; the sink serializes immediately.
   No `Vec<Reference>` per record, no clone from native to normalized.
   *Trap: intermediate collections and copies between stages.*
4. **Categorical values are integers.** Closed vocabularies (`WorkKind`,
   subject schemes, languages, statuses) are `u8`/`u16` enums. Repeated open
   values — venue names, publishers, MeSH descriptors, arXiv categories, S2
   fields — are interned to `u32` symbols, one table per output file so
   workers share nothing. Free text (titles, abstracts, author names,
   unstructured references) is not interned. *Trap: the same 40-byte journal
   name stored and hashed 10^7 times.*
5. **IDs are as small as their namespace allows.** `Pmid(u32)`,
   `OpenAlexId(u64)`, `S2CorpusId` sized from the observed maximum at M1,
   `ArxivId` packed into a `u64` (archive index, yymm, number, version). DOI
   is the one ID that cannot be packed: stored lowercased as a compact
   `Box<str>`, and ingestion never holds all of them in memory — it has no
   reason to. *Trap: every ID a heap string, every comparison a `strcmp`.*
6. **Dates and flags are packed.** `PartialDate` is a `u32` (year, month,
   day, precision). `Access` and `WorkStatus` fit in a byte. *Trap: a struct
   of `Option<u32>`s with padding, or a date crate's full timestamp for
   "2020".*
7. **Streaming gzip, always multi-member.** Some dump files are multi-member
   gzip; a single-member decoder stops silently after the first member and
   the run "succeeds" on a fraction of the data. Always `MultiGzDecoder`, and
   the stats report reconciles record counts against the manifest where the
   source publishes them (OpenAlex does). *Trap: silent truncation.*
8. **One pass, one binary, no framework.** Stages are fused in-process; the
   raw and normalized layers are on disk, nothing between them is. No
   dataframe or Arrow layer in the ingestion path — a columnar conversion
   mid-pipeline is exactly the copy this section exists to avoid. Parquet can
   be a terminal sink later. *Trap: the cluster pathology — serialize,
   shuffle, deserialize, per stage.*
9. **Joins are not ingestion.** S2's `papers`, `abstracts` and `citations`
   are three streams keyed by corpus ID; ingestion emits three normalized
   streams and never joins them. Joining needs sorted-merge or an index and
   belongs to the merge stage with everything else that needs cross-record
   state. *Trap: a hash join on 10^8 keys inside the ingest loop.*
10. **Nothing per record that is O(syscall) or O(log line).** MB-sized
    `BufWriter`s, no `tracing` events below file granularity, progress
    counters aggregated per file. *Trap: 10^8 `write(2)`s; 10^8 log lines.*
11. **Memory ceiling is per worker, not per run.** One decompressed file
    (≤ ~100 MB) plus output buffers per worker; eight workers under 1 GB.
    The only global state is the symbol tables, bounded by vocabulary size,
    not corpus size. *Trap: collect, then process.*
12. **Normalized output is binary and zstd-compressed.** Length-prefixed
    `postcard` (or `bincode`) records with a per-file symbol table; `dump
    --json` renders any file for eyes. Gzip compression is ~10× slower than
    zstd and NDJSON re-inflates what rule 4 just shrank. *Trap: writing a
    terabyte of text.*
13. **Measure before trusting.** Records/s per source, allocations per
    record (a counting allocator in tests), peak RSS. Each is a number in
    the M2 report and a regression check afterwards. *Trap: believing the
    rules were followed.*

Where borrowing meets fail-fast: a parse failure under rule 1 still has the
raw bytes in the buffer, which is exactly what the failure report in §6
needs. The two disciplines reinforce each other.

## 6. Fail fast

The contract: **a run either normalizes 100 % of its input or produces
nothing** — non-zero exit, no partial output committed. There is no lenient
mode and no threshold. Every mismatch between a source and the model is fixed
in code before any output exists.

What fails a run:

- an unknown field in a native record (`deny_unknown_fields` on every native
  struct; explicit element whitelists in the PubMed parser);
- a field that is present but does not parse into its strict type;
- a value outside a closed vocabulary;
- a missing required field (native ID);
- an internally inconsistent record (a day without a month, a version suffix
  on a versionless ID).

What does not: a field the source's own schema makes optional, absent. That is
`None`, and the stats report counts it per field and source. Every `Option`
in the model must be justified by the source schema, not by convenience.

One more thing that does not fail the run, decided on the first normalizer
(2026-09-14) and open to veto: a value in an identifier-typed field that is
not that identifier, when the category is known, counted and modelled. PubMed
puts an e-location string or the literal word `DOI` in one DOI slot per ~500
records, and typos or URLs in one reference DOI per ~20,000 — 0.2 %, which
at full scale is ~70k records, a data file rather than an exclusion list. The
strict `Doi` type never holds such a value; the record holds a
`Claim<Doi> = Valid(Doi) | Malformed { raw, error }`, so downstream code has
to match on it and the stats report counts it. That is not an `Other` on a
closed vocabulary — the DOI syntax is fixed by the DOI Handbook and cannot be
extended to admit `DOI` — it is the data itself being the bug, kept visible.

The failure report is precise enough to act on without re-running: source,
raw file, record ordinal and byte offset, native ID if recoverable, field path
(JSON pointer / XPath), raw value excerpt, reason, and the command that
reproduces it against that one record.

The fix loop, every time:

1. Decide what the value *is*: extend the model (new variant, new field), add
   an explicit per-source mapping, or add the native ID to a checked-in
   exclusion list with a reason — itself a modelled decision.
2. Save the raw record as a fixture in the source crate with a test.
3. Re-run.

Modelled warts are not failures: `MedlineDate` becomes `PartialDate::Range`
via a real parser of its documented forms; a value matching no documented form
is a failure. The escape hatch is always the model, never the data.

Normalized output is tagged with a model version; any model change
invalidates it. Rebuilding is CPU-bound and parallel, and cheap relative to
fetching.

Two ergonomic details that do not weaken the contract: `--limit N` for fast
iteration, and opt-in `--survey`, which scans everything, writes no output,
and reports every distinct failure signature with counts and samples, so one
run surfaces many warts instead of one. Default is stop-at-first. A survey
surveys parse failures too (2026-09-23). In a lines file or an API page
(Crossref, OpenAlex) a record is framed — its line or its item cut out —
before it is parsed, so one that does not parse costs that record and the
survey reads the next (2026-09-26). In PubMed's XML the reader's place in
the byte stream is no longer a record boundary, so the file is abandoned
and the next taken. Every abandoned file and the record it was abandoned
at is named in the report, because a survey that quietly skipped most of
a file would be worse than one that stopped.

## 7. POC acquisition

Target ~10k records per source, disk-conservative, downloads over APIs.
93 GiB free on this machine; the POC needs well under 1 GB. Software is
installed and run by you, not by me; the plan gives commands.

| Source           | What to fetch                                                                    | ≈ size     | Notes                                                              |
|------------------|----------------------------------------------------------------------------------|------------|--------------------------------------------------------------------|
| Crossref         | **arrived via the API fallback**: 10 × `sample=100` (5 plain, 5 `has-references:true`) — 1,000 works, 733 with references, 19 types, 71 top-level keys | 9 MB | the sample torrent is unreachable: `No route to host` to both seeders; see below |
| PubMed           | two baseline files: `pubmed26n1333` (66 MB, 30,000 records) and `pubmed26n1334` (the last, 4,989) | 79 MB | `ol-rht pubmed list` then `fetch` of a grep'd listing; md5 sidecars verified |
| arXiv            | OAI-PMH `ListRecords`, ~10 pages, one date window or `set`                        | 10–20 MB   | each page saved verbatim                                           |
| OpenAlex         | 17 parts across 2016–2025 `updated_date`s, 31,353 works: the POC part plus 16 through `fetch` and `ingest` | 65 MB | `ol-rht openalex list` then `fetch`/`ingest`; ETag-as-md5 verified |
| Semantic Scholar | **paused** — first ~10k lines each of one `papers`, `abstracts`, `citations` shard, streamed, once a key exists | ~10 MB | not byte-identical files — manifest records release, shard, line count. The three prefixes will not share corpus IDs; fine for schema coverage |

Sampling goal for the POC is *schema diversity*, not a connected subgraph.
Connectedness is the graph experiment's concern and can be assembled later
from OpenAlex `referenced_works` by seed-and-expand.

Measured 2026-09-15, once three corpora were normalized: the samples are
exactly as disjoint as that goal implies. Joined on every cross-ID they
carry (DOI, PMID, OpenAlex id), no work appears in two sources, and the
edges point almost entirely outside the samples — 280 of 131,074 OpenAlex
targets, 144 of 511,361 PubMed, 6 of 24,344 Crossref. The merge stage
would have nothing to merge and the graph experiment no subgraph, so an
*overlapping sample* comes next (§9, M5): the same works seen from every
source, seeded from the PubMed files (34,859 of 34,989 records carry a
DOI) and fetched from OpenAlex and Crossref by ID lookup, then a bounded
one-hop expansion along PubMed's cited PMIDs for the graph experiment
(195,435 distinct across the two files; 2,000 seeds reach 14,409).
Verified the same day with one small request each: OpenAlex answers a
batch of 50 DOIs per request (`filter=doi:a|b|…`, page size 50; the API
work carries one key the snapshot does not, `content_urls`, which is §6
doing its job), Crossref answers 50 repeated `doi:` filters in one
`work-list` envelope (nothing new to parse), and NCBI's `efetch` returns
the baseline files' own `PubmedArticleSet` XML. The whole sample is about
700 requests to each of OpenAlex and Crossref and none to NCBI.

Fetched and normalized the same day, all of it under `lookup/<stamp>/` in
the store, raw and normalized alike: the seed at OpenAlex (698 pages of
50 DOIs, 42 of them refetched at a larger page, §8 item 8) and at
Crossref (698 pages); the expansion's 15,432 cited PMIDs at NCBI (78
pages) and those works again at Crossref by DOI (304 pages) and at
OpenAlex by PMID (200 of 309 pages, the rest on a later day). About
2,000 requests in all, at one a second or slower, none refused. What the
store then holds, distinct by native id:

| source   | works  | of which the sample's lookups | edges     | edge targets inside the sample |
|----------|-------:|------------------------------:|----------:|-------------------------------:|
| PubMed   | 50,404 | 15,432 cited works            | 1,231,899 | 43,684 (3.5 %)                 |
| OpenAlex | 76,142 | 44,797                        | 2,262,555 | 39,426 (1.7 %)                 |
| Crossref | 50,620 | 49,637                        | 2,744,441 | 64,862 (2.4 %)                 |

Of the 50,007 PubMed works with a DOI, 49,620 are also at Crossref and
44,564 at OpenAlex; 44,245 are seen from all three, 68 from PubMed
alone. That is the overlapping sample the merge plan is written against.
The first disagreement is already countable: on 61 works OpenAlex's PMID
names a PubMed record whose DOI is not the one OpenAlex gives. The graph
experiment has its subgraph: the seed and its cited works with the edges
between them resolved.

Full-scale raw for all five is over a terabyte (Crossref 208 GB, OpenAlex
works 666 GB, S2 papers + abstracts + citations ~350 GB, PubMed ~40 GB). That
is beyond this machine and is a planning item for M9, not a surprise for
later.

### POC fetch commands

Historical: how the first three files arrived on 2026-09-14, run by you.
Since 2026-09-15 `ol-rht <source> list`, `fetch <listing>` and
`ingest <listing>` do this (§3.5); the files below now live at their
sources' own paths under the store, with receipts.

```bash
brew install aria2
```

Crossref sample (a torrent; aria2 downloads the `.torrent` and follows it):

```bash
aria2c --file-allocation=none --seed-time=0 -d data/raw/crossref \
  https://academictorrents.com/download/f692802461d1277d14551526ecb292a9637af254.torrent
```

Drop `--seed-time=0` to seed back for a while; it is 31 MB.

It stalled, and the reason is now exact (aria2 info log, 2026-09-14): both
trackers answer, exactly two peers exist — residential addresses — and
connecting to either fails with `No route to host`. They are behind NAT; so
are we, with the macOS application firewall on; aria2 has no UPnP/NAT-PMP, so
neither side can accept a connection and no number of trackers changes that.
The torrent path needs one connectable side: a router port-forward to aria2's
listen port, or a client with UPnP (Transmission, qBittorrent) on a router
that allows it. Not worth it for a 31 MB sample; the API fallback below was
used instead, and the same fix applies to the 223 GB full file if that is
ever wanted by torrent. In order of preference the alternatives were:

1. Retry from the saved `.torrent` with extra public trackers; aria2 resumes
   from its control file. aria2 uses only the trackers inside the `.torrent`
   plus DHT and never adds public lists by itself; `--bt-tracker` does.
   A tracker scrape on 2026-09-14 showed 2 seeders and 11 completed
   downloads for the 2026 sample on opentrackr (none of the samples are
   registered with Academic Torrents' own tracker), so seeders exist and
   reachability is the problem; leave it running in the background.

   ```bash
   aria2c --file-allocation=none --seed-time=0 -d data/raw/crossref \
     --bt-tracker=udp://open.stealth.si:80/announce,udp://tracker.torrent.eu.org:451/announce \
     "data/raw/crossref/[SampleDataset]March2026PublicDataFilefromCrossref-f692802461d1277d14551526ecb292a9637af254.torrent"
   ```

2. The [March 2025 sample torrent](https://academictorrents.com/details/3e5545bb8d34b57a50181dfed0d80e88da066045):
   same JSONL format, a year older, possibly better seeded.
3. Ten polite API requests with `sample=100` and your own `mailto` — 1,000
   random works, same item schema, a few MB, zero abuse. Each response is
   an envelope (`message.items[]`) rather than JSONL; the Crossref parser
   then accepts two containers around one record schema, which §3.2 allows
   (origin is separate from normalization).

   ```bash
   for i in $(seq 1 10); do curl -fsS "https://api.crossref.org/works?sample=100&mailto=YOU@EXAMPLE.ORG" -o "data/raw/crossref/api-sample-$i.json"; sleep 2; done
   ```

PubMed, most recent baseline file (name from the listing in §12):

```bash
curl -fSL --create-dirs -o data/raw/pubmed/pubmed26n1334.xml.gz \
  https://ftp.ncbi.nlm.nih.gov/pubmed/baseline/pubmed26n1334.xml.gz
```

OpenAlex, one part file:

```bash
curl -fSL --create-dirs -o data/raw/openalex/updated_date=2025-07-25/part_0000.gz \
  https://openalex.s3.amazonaws.com/data/jsonl/works/updated_date=2025-07-25/part_0000.gz
```

arXiv waits for `ol-rht fetch` (resumption-token paging, `Retry-After`).
S2 is paused until a new key exists; when it does, it lives in `S2_API_KEY`
in your environment, never in the repo or in this conversation.

## 8. Open items

1. Which sources have a usable normative serialization document (§4a) —
   checked per crate as each is started, not up front.
2. NLM and arXiv data terms (§2) — gate product use, not the POC.
3. Semantic Scholar API key — paused; M6 resumes when one exists.
4. ~~`--survey` opt-in is assumed (§6).~~ Confirmed 2026-09-15: keep it,
   opt-in, default stop-at-first. Built the same day: `normalize --survey`
   tallies every distinct failure signature (field and kind, no values)
   with counts and three examples, writes nothing, exits non-zero if
   anything failed. First use found five warts in one PubMed file in one
   pass. ~~Parse failures still stop it.~~ Reversed 2026-09-23: they are
   surveyed too, costing the rest of their file, which the report names.
   The old rule was written for two sample files on this machine; over
   1,334 files on the cluster it charged a code change, a rebuild, an
   image push and a re-run per wart, and the first full-baseline survey
   died partway through the first of eight shares knowing one thing
   (`pubmed26n0573.xml.gz` record 11927, PMID 17669187, no
   `<PublicationType>`). Whether that element should be required at all
   is still open, and is what the survey now exists to answer.
5. Country (ISO 3166-1) and currency (ISO 4217) vocabularies: `CountryCode`
   and `CurrencyCode` check shape only (two or three uppercase letters). A
   generated table like `Lang`'s, from Debian iso-codes, would close them;
   small, and worth doing the first time a bad code shows.
6. `Claim<Lang>` for OpenAlex's deprecated `sh` (one work in 10,873) and
   `Claim<Issn>` for its four wrong check digits: the DOI-claim policy (§6)
   applied to two more categories, open to veto. Extended 2026-09-15 to
   PubMed's journal ISSNs (one bad check digit in 30,000 records) and to
   a PubMed reference's PMID and PMC id (a PMC id and a DOI arrived under
   `IdType="pubmed"`).
7. Snapshot diff — noted 2026-09-15, not designed. Consecutive corpora
   from one source overlap substantially (OpenAlex's June 2026 snapshot
   carries `updated_date` partitions back to 2016; PubMed rebuilds the
   baseline yearly and adds daily update files). A diff step that fetches
   and normalizes only what changed would cut both the download and the
   rebuild. It sounds simple and is not: identity across snapshots,
   records moving between partitions, deletions and merges,
   re-partitioning. The file-level half is cheap and falls out of the
   per-file fetch records (same URL, size and hash: skip); the
   record-level half is the hard part and is deliberately not sketched.
8. Paged queries — prioritized 2026-09-15 for the overlapping sample (§7,
   §9 M5); the shape is under discussion. Standing rule, decided the same
   day: a paged query is *for* seeding the POC (and, later, arXiv, whose
   sanctioned bulk path is a paged harvest); it is never the main path for
   a source that offers bulk files, because nobody who runs an API wants
   it paged until the client holds every record, and every source here
   publishes its bulk form. Gentle by construction: one request in flight,
   a fixed pace per provider well inside what it publishes, `Retry-After`
   honoured, a 429 stops the run (the receipts make the next run resume),
   nothing re-requested that a receipt covers. What the providers say, as
   of 2026-09-15: OpenAlex meters the API — a keyless client has 1,000
   requests a day (`X-RateLimit-Limit`, reset at midnight UTC), a free key
   ten times that, 100 requests a second and the daily budget both answer
   with 429, page size at most 100; a key would be yours to obtain and
   hold, like S2's (§2). Crossref's anonymous pool reports one request a
   second (`X-Rate-Limit-Limit: 1`, interval 1 s; the polite pool with a
   `mailto` is the documented way to more, and `--mailto` stays opt-in).
   NCBI asks for three requests a second without a key, `tool` and `email`
   registered before it will unblock an address, and large jobs on
   weekends or 21:00–05:00 Eastern; the FTP baseline for anything large.
   What running it taught, 2026-09-15: a page the size of the batch loses
   works — OpenAlex holds more than one work for some DOIs, and 42 of 698
   batches of 50 answered 51 to 53 — so a lookup's page is twice its
   batch, and both parsers refuse a page shorter than its own count
   (OpenAlex's `meta.count`; Crossref's `total-results` when it fits the
   page). The day's keyless budget at OpenAlex went on 944 requests: 698
   for the seed, 42 to refetch the short pages, 200 of the 309 the second
   hop needs, and four probes; the rest of that hop resumes through the
   receipts on a later day, `fetch` on the same listing.
9. ~~**Publisher ids with markup in their value — to fix soon.**~~
   Settled 2026-09-24, merge version 13. The count over the whole 2026
   baseline (`ol-rht-id-markup-v1`, 39,994,988 records): 388 values with
   an attribute leaked into them, every one in the article's own `pii`
   slot, of 17.9 M; none in a DOI slot, an ELocationID or a reference. By
   the registrant of the article's DOI: 10.12968 221, 10.5483 74, 10.17458
   38, 10.3126 28, none 13, 10.1111 10, 10.2174 4. Two shapes among the
   examples: a whole empty element, `<ARTICLE_ID IdType=""/>`, which the
   shape rule already refused for its space; and a prefix leaked in front
   of an id, `IdType="pii">JEU342`. How many were names before the rule is
   measured by the merge survey's dictionary, before and after: 91,074,034
   distinct names (v4) against 91,073,972 (v5), 62 values, which of the
   seven publishers not listed. The user: a malformed claim, never a name —
   "the id after idtype is almost certainly incomplete anyway". The
   normalized record keeps the value as written; `publisher::Scope::name`
   refuses it (`Refusal::Markup`), and not the registry's reading of
   names, so a name of that shape already written still reads and the
   next complete merge drops it. The same count found the angle brackets
   of 225 k SICI DOIs and PIIs, which are the identifiers, and eight
   values with a stray `"` (three PIIs, five reference DOIs), left as they
   are. Found
   2026-09-24 in the corpus merge survey (`ol-rht-merge-survey-v1`, among
   the components "resolved without a rule"): publisher-id names such as
   `reg1111:IdType="pii">JEU342`. The value is not ours to blame: NLM ships
   it so. PMID 19017063, fetched from `efetch` the same day, carries
   `<ArticleId IdType="pii">IdType="pii"&gt;JEU342</ArticleId>` — the
   element's text is the attribute and a `>` leaked in front of the real
   id, most likely in the publisher's deposit (19017056 likewise, `JEU335`,
   both Wiley, `J Eukaryot Microbiol`). The normalizer keeps `ids.pii` as
   written and the merge makes it a name, so today it is a publisher id
   that no reference will ever repeat, and a second spelling of a work's
   id where the clean one (`JEU342`) also arrives from elsewhere. The same
   report shows a second publisher id on that record,
   `reg1111:IdType="doi">10.1111/…`, which NLM's copy of today does not
   carry, so the baseline's copy differs from it and the leaked prefix is
   not always `pii`. Nothing it touches stops a run, which is why it would
   go unnoticed. What is not known yet: how many values carry it, in which
   slots (`pii`, `doi`, reference ids), and whether one publisher or many. To settle first by counting (a survey tally of id
   values containing `IdType=` or `>`), then decide with the user between
   the two honest answers: keep such a value as a malformed claim, never a
   name, as a `pmc` slot holding something else is kept (§6); or read the
   id after the leaked prefix as a modelled repair of a known shape. Not a
   silent strip.

## 9. Milestones

- **M0** — plan agreed; model v0 field set fixed.
- **M1** — `model` crate: the five native-ID newtypes (§4) with packed
  representations and property tests. The newtype toolkit grows as §4a
  discovers fields; nothing is added ahead of evidence. *In progress:
  `Pmid` and `OpenAlexId` landed against real files; `Doi` waits for
  Crossref data, `ArxivId` and `S2CorpusId` for theirs.*
- **M2** — `ingest` core: traits, runner, fetch with manifests, fail-fast
  reporting, binary sink, counting allocator. `source-crossref` on the POC
  sample. First throughput and allocation numbers; they become the budget.
  *Started 2026-09-14: `source-crossref` native types from the census,
  `deny_unknown_fields` everywhere, `Cow` strings, a strict `date-parts`
  deserializer, both containers (API envelope with per-item DOI context,
  JSON lines), seven real fixtures. `ingest` crate created with the shared
  gzip reader; the traits wait for a second normalizer to show the common
  shape. `serde`/`serde_json` are the third and fourth dependencies. All
  1,000 sample works parse with zero unknown fields at 22k works/s; the one
  divergence on the way was a modelling mistake (a grant's investigators
  reused the contributor type, which requires `sequence`), which is how the
  loop is supposed to feel.*
  *Normalizer landed 2026-09-15: `Registrant`, `Orcid`, `Issn`, `Isbn`,
  `DateTime` joined `model` (every one checksum- or shape-strict, because
  the sample earned it); the normalized `Work` carries the DOI, owner
  prefix, member, type, three instants, the partial dates, titles, abstract,
  serials, authors with ORCIDs and ROR affiliations, language, counts,
  licenses, updates both ways, relations and alternative ids; one
  `Reference` edge per entry with a `Claim<Doi>`. Invariants enforced:
  `URL` is the DOI's, each instant's three representations agree,
  `published` equals `issued`, `references-count` equals the array, typed
  serial lists equal the plain ones, an author has exactly one of
  `family`/`name`, a reference's DOI and `doi-asserted-by` come together.
  All 1,000 works normalize at 22.6k works/s. Title/abstract markup raw by
  decision. Next: the `ingest` traits, now that two normalizers exist.*
  *Traits landed 2026-09-15 (§3.2): `Source` and `Sink` in `ingest`, the
  three sources registered as marker types, one runner and one `Failure`
  shape for all three, a deletion stream for PubMed. Still here: fetch with
  manifests, the binary sink, the counting allocator, parallelism over
  files — and with them the first budget number.*
  *Continued 2026-09-15: counting allocator (rule 13) and allocations per
  record in every report; the binary sink v0 (rule 12: postcard values,
  length-prefixed, one zstd stream per record kind, a header with the
  model version) and its reader; `--out` and `dump`; write errors stop the
  run at the record they hit; a failed run leaves no output. serde on
  every normalized and model type, with canonical text in JSON and packed
  numbers on disk. First numbers in §5. Left: fetch with manifests,
  parallelism over files, symbol tables, the allocation work.*
  *Parallel runner landed 2026-09-15 (§3.4): `std::thread::scope`, a
  shared file index, one sink per worker, an exact shared record limit, a
  stop flag; the binary sink writes one output set per raw file, named by
  mirroring the raw path below the inputs' common directory; `--workers`
  (default one per core) on `check` and `normalize`; `--out` is now a
  directory. Ten Crossref files go from 25k to 41k records/s on ten
  workers, which says only that the machinery scales — 8.7 MiB is too
  little to measure; PubMed and OpenAlex have one POC file each, so their
  numbers are unchanged until a second file is fetched.*
  *Record framing landed 2026-09-15 (§3.2): `records` reads a byte stream
  and frames one record at a time — lines, PubMed elements cut at their
  end tags, one Crossref page — so peak memory fell from the inflated file
  (123 MB on the OpenAlex part) to one record (5 MB), throughput held,
  output byte-identical (§5, third measurement). The same reader now
  serves a file on disk or a download in flight.*
  *Fetch landed 2026-09-15 (§3.5): `list`, `fetch` and `ingest` with
  listings and receipts under the store root; `--survey`. Second files
  brought the first real warts — OpenAlex: institutions without ROR or
  type, an APC without a USD value, a paratext work with no title and a
  null retraction flag, 32 DOAJ works with no date; PubMed: `mid`
  manuscript ids (NIHMS and EMS), a `YYYY Mon-Mon DD` MedlineDate, a
  reference's PMID or PMC id that is a DOI or a PMC id, an ISSN with a
  bad check digit — each a fixture now, all 17 OpenAlex parts and both
  PubMed files survey-clean.*
  *Allocation work and symbol tables landed 2026-09-15 (§5, fourth and
  fifth measurements): 2–8 allocations per record for the JSON and XML
  sources (Crossref's API escaping keeps it at 50), and layout version 2
  with a symbol table per set. M2 is complete; the zstd level and frame
  size stay knobs not yet turned.*
- **M3** — `source-openalex` (widest schema, second edge source, cross-IDs).
  *Native struct landed 2026-09-15 from the census: every object
  `deny_unknown_fields`, closed enums for type / OA status / version /
  license / provenance / index / source and institution types / author
  position, the abstract's inverted index kept as a raw JSON slice, a
  separate all-nullable `BestOaLocation`. All 10,873 works parse at 22.5k
  works/s; seven real fixtures. Normalizer next.*
  *Normalizer landed 2026-09-15 from a value-level census: `model` gained
  `EntityId<KIND>` (one parser per OpenAlex entity letter; `OpenAlexId` is
  `EntityId<'W'>`, unchanged), `HostOrganization`, `Ror` (ISO 7064 MOD
  97-10; all 8,366 in the sample passed), `CountryCode` / `CurrencyCode`
  (shape only), `Date::parse_iso`, and millisecond `DateTime`s at no size
  cost. Locations carry an id typed by provenance. Twenty-one fields the
  census proved derivable are checked and dropped. Three `Claim`s: 15 DOIs
  that are the bare prefix, 4 ISSNs with a wrong check digit, 1 deprecated
  language code. All 10,873 works normalize at 22.9k works/s, 233 MiB/s —
  the parse-only rate — and every count reconciles with the census (30,825
  authorships, 21,299 locations, 16,110 sources, 50,724 edges, 2
  self-citations). Fourteen real fixtures. Next: the `ingest` traits.*
- **M4** — `source-pubmed` (streaming XML, the largest modelling surface).
  *First cut landed 2026-09-14: native types mirror the DTD field for field
  (everything under `PubmedArticle` and `DeleteCitation`; `PubmedBookArticle`
  is an explicit error), attribute vocabularies are closed enums, cardinality
  is enforced, child order is not yet. Mixed content is kept as the raw
  inner markup. The whole real file parses with zero failures; the DTD and
  five real records are the crate's fixtures.* *Normalizer complete for
  every field the file contains (2026-09-15).*
  *Normalizer first cut landed the same day: `Pmid`, `PmcId`, `Doi`,
  `Date`/`PartialDate` (with the `MedlineDate` grammar), `Lang` (ISO 639-2
  from Debian iso-codes), `MeshUi` — plus the record's identifiers, dates,
  languages, publication types, comments/corrections relations, and one
  `Reference` edge per reference-list entry, pushed to an `Emit`. The whole
  file normalizes; the stats report is the first real one. Title, abstract,
  authors, journal and MeSH headings next.*
- **M5** — paged queries and the overlapping sample. *Prioritized
  2026-09-15 (§7, §10): the three POC corpora share no work, so this
  comes before the remaining sources. A lookup entry kind in the listing
  (the requests for a seed list of ids are all known up front, so `list`
  can write them and `fetch` can bring them in, one page per request, a
  receipt each, resumable for free), `keys` to draw a seed from our own
  normalized sets, and the pacing the providers ask for. Then the sample:
  the PubMed files' DOIs looked up at OpenAlex and Crossref, and a bounded
  one-hop expansion for the graph experiment.*
  *Landed 2026-09-15 (§3.5, §10): `keys`, `list --seed --by`, the lookup
  entry kind, paced `fetch`, `ingest` refusing lookups; the sample
  fetched, every page surveyed, every wart modelled with a fixture (§10,
  "the sample's warts"), every cache rebuilt, the sample measured (§7).
  Left: 109 OpenAlex pages of the second hop, which `fetch` on the same
  listing resumes once the keyless day resets.*
- **M6** — `source-s2` (three streams). Paused until a key exists; the slot
  stays, and M7 can move ahead of it.
- **M7** — `source-arxiv` (OAI-PMH): the harvest kind of paged query —
  the next page known only from the last — and the one source whose
  sanctioned bulk path it is (§2).
- **M8** — full POC run across all five; the fix loop until it is green;
  review the stats report and the budget.
- **M9** — full-scale plan: storage, incremental origins (`DeleteCitation`,
  update files, OAI `from`), model-version invalidation; the zstd level and
  frame size, once the box says whether disk or CPU is the scarce one.
  The incremental origins are also the serving plan's SP6: a `watch`
  listing kind with a cursor per source, paced as lookups are.
- Then: the merge plan — [merge-plan.md](merge-plan.md), draft 1 of
  2026-09-15, written against the sample, with milestones MM1–MM5 of its
  own; the graph plan — [graph-plan.md](graph-plan.md), draft 1 of
  2026-09-15, milestones G1–G4, its G2 being the merge plan's MM4; and
  the serving plan — [serving-plan.md](serving-plan.md), draft 1 of
  2026-09-16, milestones SP1–SP6, its SP2 the search sinks below and its
  SP6 M9's incremental origins.
- **Search sinks**, noted 2026-09-15 so they are not forgotten: the
  metadata goes into **Meilisearch** and **Tantivy** at least, and probably
  two more for comparison (Quickwit, which is Tantivy at scale; Typesense or
  OpenSearch). The search strategy is the serving plan's SP2
  (2026-09-16): Tantivy first, embedded, then Meilisearch as the
  comparison, with the scale caveat in its §11. These index the *merged*
  layer, so they are sinks of the merge stage, not of ingestion; the
  `Sink` trait shape (§3.2) is what they will implement against, with a
  merged `Source` behind it. The citation graph remains the end goal; the
  search index is the funnel's cheap-scoring stage
  ([candidate management](inbox/candidate-management.md)).

Order after Crossref is adjustable; OpenAlex second because it exercises
cross-IDs and a second edge namespace early.

## 10. Decision log

- 2026-09-14, round 1 — pin stable Rust; workspace; no CLAUDE.md; no
  dependencies in the scaffold.
- 2026-09-14, round 2 — add OpenAlex and Semantic Scholar (five sources).
  Ingestion is raw → per-source normalized, no merging; the merged schema is
  a separate design problem. Fail fast, no lenient mode, no thresholds:
  upfront pain is the price of operational discipline. Crossref first;
  prefer downloads over API use; ~10k records per source; conservative disk.
- 2026-09-14, round 3 — "strict typing" is also a throughput discipline:
  no juggling of strings, copies and slices through the munging path (§5).
  Crossref via torrent files, `aria2c`, installed and run by the user.
  S2 via streamed shard prefixes. `--survey` opt-in assumed.
- 2026-09-14, verification — Crossref 2026 is JSONL in one 208 GB tar, with
  an official 31 MB random-sample torrent (POC input). OpenAlex snapshot is
  free with no account; path gained `jsonl/`; 510M works, 666 GB; API now
  needs a key. S2 release 2026-09-09, shard sizes, ODC-BY. Edge-count anchor:
  S2 `citations` = 2.4 × 10^9.
- 2026-09-14, round 4 — §5 accepted as the starting point. §4's field table
  was overclaiming: nothing beyond each source's native ID is known well
  enough to commit to. It moved to Appendix A as speculation; §4a (schema
  discovery — a normative serializer document where one exists, observed
  reality where not, and reality regardless) replaced it. S2 paused pending
  a new API key.
- 2026-09-14, M1 started — `model` crate with `Pmid` (`u32`) and
  `OpenAlexId` (`u64`, forced by the observed maximum 4,410,348,178);
  `hello` removed; `ol-rht id` runs one parser over stdin. Crossref sample
  torrent stalled at zero bytes; fallbacks recorded in §7.
- 2026-09-14, Crossref raw — torrent diagnosed as unreachable (NAT on both
  sides, `No route to host`); API fallback used with permission: 1,000
  random works via `sample=100`, public pool, no email. Raw files are API
  envelopes, so the Crossref parser accepts two containers around one record
  schema (§3.2). Manifests with sha256 written for all three sources under
  `manifests/`, backfilled from disk as §7 said they would be.
- 2026-09-14, Crossref native struct — pure-discovery path, 355 JSON paths
  from the census, `api_format.md` used as a hint and found stale in nine
  places. Three closed vocabularies with an authoritative source, the rest
  observed. `ingest` crate started with the shared gzip reader. First real
  run: 654 works, one modelling error, then all 1,000.
- 2026-09-15, Crossref normalizer — strict `Orcid`/`Issn`/`Isbn`/`Registrant`
  /`DateTime` types with checksums, justified by a sample in which every
  value passed. Stats: 18,345 valid reference DOIs, 47 malformed (all
  U+2010), 5,952 without a DOI, 52 empty references; 67 owner-prefix
  transfers; issued precision 466 day / 351 year-month / 149 year / 34 none;
  eight relation kinds; Crossref's own DOIs are already lowercase — zero
  lowercasing copies.
- 2026-09-15, ingest traits — `Source` (records + normalize) and
  `Sink<'n, S>` (work, reference, deletion), factored from the three
  per-source `Emit` traits, which are gone. One runner drives every source
  and replaces three hand-written loops in the binary; `--limit` works for
  all. A `Failure` names source, file, record ordinal and stage around the
  source's own error. PubMed's `DeleteCitation` becomes a deletion stream.
  The citing id travels beside the edge, not inside it, for every source.
  Long tail of sources assessed (§2): OpenCitations and Europe PMC worth a
  slot; the rest not.
- 2026-09-15, round 5 — aggregator over aggregators: disagreement between
  sources is the product's material, and the policy resolving it is a
  feature of the merge stage (§1). Long tail re-decided on that basis (§2):
  DataCite, NIH-OCC, OpenCitations, CORE, BASE, Retraction Watch, DBLP,
  Papers With Code wanted; Papers We Love a seed list; the closed ones
  skipped. Search sinks (Meilisearch, Tantivy, two more) recorded as a
  downstream goal (§9). M2 continues: allocation counting, the binary
  sink, `dump`.
- 2026-09-15, binary sink — postcard + zstd, length-prefixed, one stream
  per record kind, tagged with `model::VERSION` (one number for the model
  and every normalized type; bump it on any change). Sink methods stay
  infallible; a writing sink remembers its error and the runner asks after
  every record (`Sink::failed`), so a full disk stops the run where it hit
  and nothing partial survives. Packed types serialize as canonical text
  for JSON and as numbers for postcard, so `dump` reads well at no cost on
  disk. First measurements (§5): the sink halves throughput at zstd level
  3 (a knob), rule 1's allocation goal is far (75–200 per record, the
  native `Vec`s), and the layout is not smaller than gzipped raw (symbol
  tables and the abstract are the levers).
- 2026-09-15, parallel runner — files are the unit of work; workers pull
  from a shared index and own their sinks (§3.4). The sink contract grew
  `begin_file` / `end_file`, so one raw file is one output set and a
  rebuild of one raw file replaces one set; output names mirror the raw
  path below the inputs' common directory, and two raw files that would
  collide stop the run before it starts. A record limit is claimed
  through one counter and is exact across workers. Every worker's sink
  comes back to the caller, finished on success and discarded on failure,
  so "a failed run leaves nothing" holds with workers too. Stats sinks are
  merged by exhaustive destructuring, so a new counter cannot be forgotten
  silently. No `rayon`: `std::thread::scope` is enough for a file queue.
- 2026-09-14, normalizer first cut — value-level layer over the native
  record; the `Claim<Doi>` decision above (§6); `Lang` from ISO 639-2 via
  Debian iso-codes (NLM's own language page is retired); publication types
  carry MeSH UI + name, the MeSH vocabulary itself deferred. Whole file
  normalizes at 28.4k records/s, i.e. normalization is free next to parsing.
  First stats report: 3,338 day / 895 year-month / 709 year / 40 span /
  6 season / 1 quarter publication dates; 74 % of references carry no
  identifier at all; 7 % of DOIs need lowercasing.
- 2026-09-15, markup decision — mixed-content fields (title, abstract,
  keywords, affiliations, citation text, COI, suffix, collective names)
  are kept as the raw inner markup in the normalized layer too, borrowed,
  with no flattening method anywhere: the markup matters, and dealing with
  it is deferred until something needs to explicitly. Then the flattening
  can live close to the raw type. Observed inline vocabulary in PubMed:
  `b`, `i`, `sub`, `sup`, MathML; never `u` or `DispFormula`.
- 2026-09-15, PubMed normalizer, all fields — journal (ISSN with type,
  ISSN-linking, NLM id, country, pagination), title, abstract sections with
  labels and NLM categories, other abstracts, authors with ORCIDs in their
  three forms (2,873 bare, 1,995 `http://`, 2,739 `https://`; 17 truncated
  to fifteen characters → `Claim<Orcid>`), affiliations with ROR/GRID/ISNI/
  Ringgold ids, investigators, MeSH headings and qualifiers, supplementary
  concepts, chemicals, keywords, grants (empty `Country` → `None`), data
  banks, COI, history with local times, publication status. DTD content the
  file never contained fails on first sight. Whole file at 25.9k records/s;
  every count in the stats report reconciles with the census (34,978
  authors, 42,448 affiliations, 11,460 MeSH headings, 19,167 keywords,
  2,074 investigators, 9,603 abstract sections).
- 2026-09-15 — `--survey` confirmed as an opt-in diagnostic mode (§6);
  default remains stop-at-first, output never written on any failure.
- 2026-09-15, OpenAlex native struct — census-driven, documentation
  unavailable. The inverted-index abstract stays raw, consistent with the
  markup decision. Unpaywall found to be folded into OpenAlex; no separate
  source.
- 2026-09-15, OpenAlex normalizer — a field the census shows to be a
  function of others is an invariant, not a field: it is checked and
  dropped, and a record breaking it fails (twenty-one such fields, §2).
  Location ids are typed by `provenance`. The `Claim` policy extended, on
  the same rule (known, counted, modelled), to ISSNs with a wrong check
  digit and to a language code outside ISO 639-2 (`sh`); open to veto (§8).
  `language` is read as ISO 639-1, or ISO 639-2 for a language with no
  two-letter code (`ceb`) — the form OpenAlex actually writes; a three-letter
  code for a language that has two would fail. Country and currency codes
  shape-checked only until a vocabulary table exists (§8). Entity ids
  generalized over the letter with the committed `OpenAlexId` unchanged;
  the other letters live inside the OpenAlex record and are not native ids.
- 2026-09-14, M4 first cut — first external dependencies: `quick-xml`,
  `flate2`. Native layer is DTD-faithful text (dates, identifiers stay text;
  the normalizer applies `model` types), which keeps the two layers'
  strictness separate: structural here, value-level there. `ol-rht pubmed
  check` parses a file and reports throughput. `pmc`/`pmcid` resolved as one
  identifier in two serializations.
- 2026-09-15, store of record — one store root for bytes and bookkeeping,
  `--store`, never committed; filesystem paths now, S3-compatible later, a
  box to be pressed into service for the whole corpus (§3.5). Fetch design
  under discussion: a command that writes the list of URLs to disk,
  separate from the command that fetches them; a record per fetched file;
  streaming at file level with fetch, and at record level as its own step.
  Snapshot diff noted as a future optimization (§8, item 7).
- 2026-09-15, record framing — "streaming" meant record level all along:
  `Source::records` now takes a `BufRead` and a callback, each source
  frames its own records (a line; a PubMed element cut at its end tag,
  parsed with the frame's file offset so errors are unchanged; one
  Crossref page), and the runner claims the limit one record ahead. Peak
  memory is one record, not one file; output byte-identical; numbers in
  §5. Names settled: *listing* (what a source offers, written by `list`)
  and *receipt* (what was fetched, appended by `fetch`), both under the
  store root; "manifest" is OpenAlex's own file. HTTP via `ureq` over
  rustls, no transparent decompression; `sha2` and `md-5` for the digests
  the sources publish or imply (NCBI's `.md5` sidecars; S3's ETag is the
  object's md5 for single-part uploads, and matches the POC part).
- 2026-09-15, fetch landed — `list` per source (strict about the index;
  NCBI's `_stats.html` pages tripped it on first contact and are now a
  known kind), `fetch` (partial file, hash while writing, Range resume,
  four checks, one receipt per verified file, refuses torrents and
  unreceipted files), `ingest` (the download run straight through the
  framer on the worker that owns it; a resume replays the partial file
  first), `--survey` (§8, item 4). Measured: a 66 MB PubMed file in
  3.1 s with its md5 checked; an 11 MB part killed mid-transfer resumed
  at 6.0 MB and verified; three parts ingested on three workers in 1.3 s
  at 40 MB resident. The POC files moved to their sources' own paths;
  `manifests/` became `receipts/` in the store and left the repository.
- 2026-09-15, second files — five OpenAlex fields and four PubMed forms
  the first files never showed (§9, M2); the model gained `Option`s, two
  `Claim`s and one MedlineDate form, each with a fixture from the record
  that failed. None of it was a surprise in kind: this is the loop §6
  describes, and `--survey` made it one pass per file instead of one
  round per wart.
- 2026-09-15, allocation work — rule 1 met in spirit: OpenAlex 162 → 2
  allocations per record, PubMed 87 → 7.5, Crossref 195 → 50, output
  byte-identical (§5, fourth measurement). The two real causes were
  serde copying `Cow<str>` inside `Option`s and lists, fixed by
  `ingest::Text`, and quick-xml's heap-backed duplicate-attribute check,
  done inline now; the lists were the smaller part and are inline
  `smallvec`s at their 90th-percentile length. Stats sinks key their maps
  by enums, not by a `String` per record.
- 2026-09-15, symbol tables — rule 4 as written: `model::Interned`, one
  table per output set in a fourth file, thread-local interner while a
  set is open, resolution through the loaded table when reading; fields
  chosen from a repetition survey, affiliations tried and left as text.
  Dumps byte-identical on 36 files; bytes −0.5 % to −4 % per set, sink
  throughput −5 % (§5, fifth measurement). Kept for what it gives the
  merge stage — integers for categorical values — with the field set as
  the knob and the number still owed by that stage.
- 2026-09-15, overlapping sample first — three normalized corpora share
  no work and their edges point outside the samples (§7), so the merge
  plan and the graph experiment are both blocked on an overlapping sample,
  and it moves ahead of the remaining sources: M5 is now paged queries and
  the sample; S2, arXiv, the full run and the full-scale plan are M6–M9.
  Batched lookups verified at all three providers with one small request
  each; OpenAlex meters its API. Rule: paged queries are for POC seeding
  (and arXiv's harvest, its own bulk path), never the main path for a
  source with bulk files; gentle by construction (§8, item 8). zstd knobs
  not turned: nothing consumes the layout yet and downloading dominates
  running; they belong to M9. Design under discussion before anything is
  built.
- 2026-09-15, lookups — agreed and built the same day (§3.5): a seed of
  ids from our own normalized sets, chunked into requests known up front,
  so a lookup listing is an ordinary listing with a third entry kind and
  `fetch` resumes it for free; paced per source as a constant, not a
  flag; fetch then normalize, never `ingest`, so a rejected page stays;
  the full PubMed seed, not a slice; harvests (the next page known only
  from the last) left for arXiv. What the first 53 OpenAlex API works
  taught (§9, M5): the API is a newer serializer than the June snapshot,
  and §6 found every difference before a record was kept — one field
  added, two gone, a null for an object of nulls, location ids carrying
  their namespace instead of a `provenance` field, naive date-times, and
  one work whose corresponding institutions no author accounts for, now
  kept as written when it disagrees rather than a stop. Crossref's API
  answers were the envelope already modelled.
- 2026-09-15, the sample's warts — running the lookups through the
  normalizers found the drift §6 exists for, source by source, and every
  one is a fixture now (nine OpenAlex, seventeen PubMed, one Crossref).
  OpenAlex: five work types the snapshot never showed, `content_urls`,
  no `authors_count` or `ids.mag`, `best_oa_location` null, location ids
  with their namespace in front and no `provenance` (typed by the
  namespace, `mag:` included; a location with no id at all), naive
  date-times (`DateTime::parse_utc_naive`), an ISSN-L and an ORCID that
  became claims, and a best copy that spells unpublished as `false` where
  the location says null. PubMed, older records: `pmc` reference ids with
  the prefix (a PMID in that slot on 3 % of them is a malformed claim),
  `NumberOfReferences` down to zero, `Sep-Oct`, `Mar/Apr` and `Autumn` as
  seasons, two `ELocationID`s of one type, `OtherID`, `medline` and
  `sici` ids, `GeneSymbolList`, `UKMS`/`CAMS`/`HALMS` manuscript ids, an
  abstract language that is not a code, `PersonalNameSubjectList`,
  `GeneralNote`. Crossref: the CRediT roles and `reviewer`, `ISNI`
  (`model::Isni`, ORCID's checksum), `retraction-watch`, a reference-type
  vocabulary that is not the work types', award ids, `record-id`, an
  assertion group without a label. Four census-derived OpenAlex fields
  are not always derivable in live data and are now kept when they
  disagree rather than dropped or fatal (corresponding institutions and
  authors, institutions and countries distinct counts); `host_organization`
  is stored outright. One of our own checks was wrong once: a
  corresponding author listed twice counted twice. The normalized shapes
  changed under every source, so every cache was rebuilt from raw; the
  layout has no schema evolution by design (§3.5).
- 2026-09-15, the merge plan — drafted against the sample as
  `merge-plan.md`, its decisions logged there. What changes here: the
  registry that mints `WorkId`s is the merge's one thing of record and
  the merged sets are a cache (§3.5); the store gains `merged/` (§3.3);
  `ingest::Key` gains `Pmc` so a PMC id can be a name, which no source
  can look up by. The census behind it: 82,723 works in the sample's
  177,166 records, 83 conflict components in seven shapes, 4.2 million
  cited works the sample does not hold.
- 2026-09-15, one framing for both layers — the merged sets (merge plan
  §8, MM3) use the normalized layout's framing rather than a second one:
  `ingest::binary::Writer` is the set writer factored out of the sink,
  any kinds, a symbol table beside them; `Binary` is a `Writer` per raw
  file with the normalized four. `ingest::Kind` gained the merged layer's
  `edges`, `names`, `relations` and `unresolved`, appended, so every
  header written before still reads; the layout version stays 2.
- 2026-09-15, the graph plan — drafted from the architecture summary and
  the inbox notes on it as `graph-plan.md`; the summary retired, its
  funnel a note of its own. What changes here: the store gains `graph/`
  and the crate list a `graph` crate (§3.3, planned); §2's note on the
  summary's edge budget is settled there at 6 × 10^8 works and 3 × 10^9
  edges.
- 2026-09-16, the serving plan — drafted as `serving-plan.md` from the
  serving-shape decisions of 2026-09-15. What changes here: the store
  gains the journal, the public log, the delta files and the serving
  artifacts' prefixes (§3.3, planned); the search sinks of §9 are its
  SP2; M9's incremental origins are its SP6.
- 2026-09-16, the search sink landed (serving plan SP2) — the `serve`
  and `search` crates, `Contributor` gained `text`, the store gained
  `search/<engine>/`; Tantivy is the one new external dependency of
  weight, and the mapping it indexes is provisional by the user's
  decision.
- 2026-09-16, the batch shape landed (serving plan SP3) — the merge
  writes a journal line per complete run and mints public ids when given
  the key; the store gained `merged/journal.jsonl`, `merged/public.jsonl`,
  `names/`, `works/` and `epochs/`; `fst` and `blake3` joined the
  dependencies.
- 2026-09-16, the transport landed (serving plan SP4) — `ol-rht-serve`,
  gRPC on `tonic`, the protocol compiled by `protox` so no compiler is
  installed; `tokio` joined the dependencies with it.
- 2026-09-16, the delta path landed (serving plan SP5) — `ol-rht merge
  --batch <sets>` merges normalized sets incrementally against the live
  tables and appends to `merged/v<…>/delta/`; the server folds the
  journal every poll. Two things changed in this layer's ground: the
  merged layout is at version 2, because every record in a merged row now
  carries its assertions and its view (merge plan §5); and the full
  merge honours PubMed's deletions sets and takes the latest revision of
  a record delivered twice, which update files will do daily. A lock
  under `merged/` keeps merges from overlapping. No new dependency.
- 2026-09-17, PubMed's first update files — the user fetched
  `pubmed26n1335` to `pubmed26n1339` for a second sample harness (serving
  plan §5), and they normalized after one pass of §6's loop: 91,744
  citations, 2,300,205 references and 53 deletions, 195 MiB, beside the
  baseline under `normalized/v1/pubmed/updatefiles/`. The loop's findings,
  each with a test: `MedlineDate` gained four spellings of a range — ` - `,
  `/`, ` & ` and a full month name — plus a day range across years, a year
  alone and a month with a full stop (`model::date`); a versioned
  citation's PMC id carries its version (`PMC8459621.2`), which must agree
  with `PMID@Version` and is not kept; and seven records went on a
  checked-in exclusion list, the fix loop's third way, now built
  (`source_pubmed::exclusions`, counted by the stats report through the
  sink's `excluded`), each keyed to the revision it names so that a later
  one normalizes or stops on its own: four whose `ELocationID` and
  `ArticleId` DOIs disagree, one with two PIIs, one whose PMC slot holds
  `wst_2021_260`, and one reference identified by a PII. Each waits on a
  decision or a model change, not a parser. The merge's genre table gained
  seven publication types the files showed. No model version change: the
  layout is unchanged and every form that normalized before normalizes
  the same.
- 2026-09-18, the seven decided, and every id kept — model version 2. The
  user's call on all four shapes, after each was looked at in the data and,
  for the DOIs, at Crossref: a record may give two valid DOIs, and both are
  names (of the four, one has two registered DOIs — an early-online one and
  the issue's — two have a right `ArticleId` and a wrong `ELocationID`, and
  one the reverse, so no rule that trusts a slot works); a record may give
  more than one publisher item id, so `ids.pii` is a list; an `ArticleId
  IdType="pmc"` that is not a PMC id is a malformed claim, kept as written
  and never matched on, like a malformed DOI; and a reference may carry an
  id of any other kind the DTD allows, kept as written in
  `Reference::loose`. The user's rule for all of them: "keep as many ids as
  possible, resolve at matching". The exclusion list is empty again and its
  machinery stays for the next shape that needs a decision. How rare these
  are, over the 126,740 records and 2,811,661 references of the 2026
  baseline's last two files and the first five update files: 4 records with
  two valid DOIs, 1 with two publisher ids, 1 with a `pmc` slot holding a
  publisher's id, 1 reference carrying an id of another kind. The sample
  was normalized again under `normalized/v2/`, set for set the same 2,012
  sets as v1 at 727 MiB, and the seven records are in.
- 2026-09-24, dates — model version 6, from
  [the date note](inbox/date-policy.md) reviewed against evidence it did
  not have. The survey of the whole 2026 baseline on model version 5, run
  on the box, found 10,164 records it could not read, all but 106 of them
  (PMC ids given twice, handled apart in f0a93d7) on a date; its examples
  — three per signature per share, 24 per signature — showed most of them
  to be shapes, not false values. The grammar gained `MedlineDate`
  seasons in any case (`1976 WINTER`, 19 of the 24 examples of the 9,662
  "not a recognized form"), a start day with an end month only
  (`1992 Aug 15-Sep`), the end's year written again after it
  (`2016 Nov-Dec 2016`) and the first month's year written after it
  (`2017 Dec 2016 - Jan`); and more than two digits where a day goes is
  now reported as the shape it is, because the 24 examples of the 140
  "day is not 1..31" each put a year there, once a two-digit one, and
  none a day. Two readings of forms already accepted changed: a range
  whose end month comes before its start, with no second year written,
  ends the year after (`1990 Dec 21-Jan 10` ends in 1991), and a range
  that still ends before it starts is refused (`MedlineError::Backwards`,
  `1990 Dec 12-8`, which was being kept wrong; reading it as the 12th to
  the 18th waits on a source that states the convention). The user's
  decision: a PubMed history date or time that names none is kept as
  written beside why (`HistoryDate`, `HistoryTime`) and its record
  normalizes — 200 records with a day that never was, and all 12 of them
  fetched from `efetch` carry it on a date the publisher supplied
  (`received` 7, `accepted` 4, `revised` 1), never on one NLM stamps; and
  48 with `null` for an hour or a minute, now `DateError::TimeDigits`
  apart from a time out of range. The stats report counts both by
  `PubStatus`, and a structured `PubDate` whose day does not exist
  reports the whole date. Still refused, to be decided together once a
  survey on version 6 shows what is left: issue dates that cannot be true
  (`1956 Oct 7-Nov 31`, 8 records) and ones nothing reads
  (`1990 Nov C-Dec`, `1974 Apr 1974, 4 p 3`) — whether to keep a coarser
  date beside the raw text, or to list the records for exclusion. The
  142,172 PubMed records on this machine normalize under version 6 with
  none failing.
- 2026-09-24, later — model version 7. The survey on version 6 found
  9,503 records it could not read, all `MedlineDate`s: 9,248 not a
  recognized form, 245 day ranges whose end comes before their start, 8
  with a day no month has (`1956 Oct 7-Nov 31`), 2 with a day out of
  range (`1980 Aug 1-32`). Version 6's refusal of the backwards ranges
  was right to refuse and wrong in what it expected to find: they are
  not errors but a notation. The neighbouring issues of the same weekly
  journals, fetched from NLM, put `1995 Mar 22-8` (`Nurs Times` 91(12))
  in the week of the 22nd — issue 13 is `1995 Mar 29-Apr 4` — which is
  the end's tens digit dropped, as NLM drops it from page ranges on the
  same records (`14-5`); and the user's reading, the 22nd to 8 April, is
  what the notation would mean if it did not name a crossed month, which
  it does (`Mar 29-Apr 4`). Which a record means is not in the record,
  and in one issue of eight checked the dropped-digit end is a day past
  the next issue's start, so the end is not kept: a counting Job on the
  box measures the corpus against the neighbouring issues before any
  reading is encoded. Decided with the user: a `MedlineDate` the grammar
  does not read whole, that begins with a year, keeps what of it is
  certain — its year, or such a day range's first day — beside the text
  as written and why (`Published::Kept`), and its record normalizes; one
  that does not begin with a year still stops the run. The stats report
  lists the kept ones by text, commonest first, which is what the
  grammar is taught from. Two shapes are read outright: a month of a
  volume over two years (`1992-1993 Oct`, `VolumeMonth`, compared by the
  merge as its first year only) and a seasonal volume written with both
  years (`1986 Winter-1987 Spring`).
- 2026-09-24, the count (`ol-rht-date-ranges-v1` on the box, 39,994,988
  records in 1,334 files). Day ranges within one month that end before
  they start: 16 issues, 242 records, against 6,011 that run forwards and
  1,396 that cross a month and name it. Of the 16, 14 have a next issue
  in their volume, and in all 14 it begins before a next-month end would
  fall, which rules that reading out; in 12 it begins the day after the
  dropped-digit end, in one (`Nurs Times` 79(25), `Jun 22-9`) the day
  that end names, and in one six days after it. Of the other two, one's
  previous issue began one dropped-digit length before, and one has no
  neighbour. No range in the corpus is written so that only the
  next-month reading fits (`28-3`). `Nurs Stand` 5(12), `1990 Dec 12-8`,
  is followed by 5(13), `1990 Dec 19-1991 Jan 8`: a crossing, written
  with its month and year. Whether version 8 reads these as the
  dropped-digit week is the user's call; the merge and the index read
  only the first day either way.
- 2026-09-24, model version 8 — the user's call: read them. A day range
  within one month whose end has fewer digits than its start, and reads
  as before it, takes the start's leading digits: `1995 Mar 22-8` is the
  22nd to the 28th. Where the reading gives no later day of the month
  (`28-3`, `Feb 22-9` in 1995) the range is still refused and kept with
  its first day; the count found none such. The survey on version 7 had
  found one more thing, the only shape left that stopped a run: a season
  before its year (`Spring 2008`, `Winter 2017`), 3,044 records of 2006
  to 2017, every one of its 24 examples this shape. Version 8 reads it.
  That survey also listed what version 7 keeps rather than reads, 6,116
  records: a long tail with no dominant shape — `2016 Sept-Oct` (65),
  `2016 11-12` (59), `2000 Aug, 7`, `2023 Auguest`, `2021 [SEASON]`,
  `2020 COVID-19 SPECIAL ISSUE` — which is the case for keeping over
  cataloguing that version 7 was built on.
- 2026-09-25, model version 9 — U+2010 HYPHEN in a DOI's suffix is read
  as `-`. Crossref's March 2026 bulk sample carries it in 13,305 of its
  262,499 reference DOIs, every one from Wiley's records (1,248 of them),
  and the merge had been dropping those references with no row at all:
  a reference whose one name is malformed has nothing to land on. Read
  as `-`, 13,301 are DOIs, and the three tried resolve at doi.org where
  the U+2010 form of one answers 404. The DOI Handbook (2025 edition,
  §4.2, §4.3.4) allows any Unicode in a suffix and calls two names the
  same only when their code points are, but for ASCII case, so the two
  forms are different names and this is a reading, not the standard's
  equivalence. The user's first preference was to keep the character as
  written, then: "we should just normalize that character, what the hell
  wiley". Only U+2010 is read. The same sample's other 30 rejected
  reference DOIs stay malformed: an en dash (13), `×` for `x` in SAGE's
  (8), spaces inside SICI DOIs (3), a minus sign (2), zero-width
  characters (2), and two that mix U+2010 with another; the merge still
  drops those without a trace (merge plan §11).
- 2026-09-25, Crossref's bulk file is shards, not a tar — the 2026-09-14
  entry above had it as one tar. The torrent's metafile (read 2026-09-25)
  names 35,908 `N.jsonl.gz` shards and Crossref's conversion script
  `a.py`, 223,053,729,528 bytes, flat in one directory; the box fetched it
  by magnet link (homelab `job-fetch-crossref-v1`). A command naming
  every shard is about 2.3 MB of arguments, over Linux's 2 MiB, so the
  Jobs that read them take them in eight shares, listed without a glob.
- 2026-09-25, Crossref before OpenAlex. The OpenAlex snapshot of
  2026-09-23 is fetched to the box, but it waits until Crossref is live
  (the user: "there are a lot of practical considerations needed to take
  care of before taking this on"). Model 8 reads none of its 2026
  partitions — 403.6 M of its 476 M works — for a new field, a new work
  type and two updates dated before their creation (§2, OpenAlex). The
  user is interested in OpenAlex as a hold-out: a dataset that validates
  this pipeline's correctness, not an infallible oracle, but a
  disagreement with it needs a rigorous justification.
- 2026-09-25, a set's records can be sought (serving plan §11, item 35,
  the user's option (c)). A batch reads records back from the normalized
  layer — a re-delivered record's earlier copies, the citers of a stub it
  divides or of a name it moved — and a set was one zstd frame, read
  whole to find any record: about half a second a set (M). The works and
  references files now start a new frame every 256 KiB of records
  (`binary::FRAME`), which reads as one stream as before, and a set gains
  `<prefix>.index.zst`: each frame's offset and first record, and each
  work's native name with its first reference's ordinal, since a work's
  references follow it. A reader with the index finds a record by a seek
  and one frame; a set without one is scanned, as before. The index is
  keyed by the name the merge writes (`Source::native_key`), so the
  source crates fill it. On the sample the sets are about 7 % larger
  (M: the two baseline files 65.8 to 70.3 MiB) and an index is about
  65 KB for 30,000 works. Neither the model nor the layout version moves:
  every file reads as it did, and a set normalized before has no index.
- 2026-09-26, a survey reads past a line that does not parse — the
  user's call. The first survey of Crossref's whole March 2026 file
  (homelab `ol-rht-survey-crossref-v1`, model 9) read 132,819,641
  records and abandoned 24,545 of its 35,908 shards at their first parse
  failure (M), leaving an estimated 46 M records unread, and a record
  with two warts showed only the first: 1,052 of the licenses with no
  start sat behind a Wikidata id or an affiliation's acronym (M, over
  the records those shards stopped at, dumped by homelab
  `ol-rht-dump-crossref-v1`). The 2026-09-23 rule abandoned a file
  because the reader's place is lost, which is PubMed's XML and not a
  lines file: Crossref and OpenAlex cut a line, or a page's item, out
  before parsing it, so the next record starts where it did.
  `Source::records` now hands such a failure to the runner beside the
  records, and a survey tallies it and reads on; PubMed's still ends
  the read. Every run that is not a survey stops at it, as before. Over
  the dump's 33,476 records a survey now reads all of them and abandons
  nothing (M: 26,262 failed, 42 signatures).
- 2026-09-26, model version 10 — what the Crossref survey found, in four
  parts the user decided the same day, one version for all of them.
  Institutions first. 1,957 shards stopped at a Wikidata id
  (`id-type: wikidata`) and 892 at an acronym on an author's affiliation;
  the parser knew Wikidata nowhere and an acronym only on a work's own
  institution, though both are one schema element. An affiliation and a
  work's `institution[]` now normalize to one `Institution`: name,
  acronyms, places and departments as written, and a ROR, an ISNI and a
  Wikidata item (`model::Qid`, written `Q182973`: "keep the letters,
  they're part of the id format"), each with who asserted it. The work's
  own institutions were parsed and dropped before, and are kept now: they
  name a dissertation's university, and most Wikidata ids sit there. A
  ROR is a claim, as an ISSN is — one record carries `03npvkt06`, whose
  checksum fails (10.25189/2675-4916.2023.v4.n1.id679); an ISNI and a
  Wikidata item are strict. The schema's "a name or an id" is enforced,
  and so is its country on a grant investigator's affiliation only; a DOI
  as an institution id, a second id of one kind, or one Retraction Watch
  asserted stops the run, and none has been seen. Over the dumped
  records (M): 9,014 affiliations and 996 works' own institutions, 553
  and 243 with a Wikidata item, none failing, two RORs malformed (one
  record's two copies). The user sees ids for institutions, authors and
  funders coming one day, as works have them, and kept "a name or an id"
  with that in mind; not this pass.
- 2026-09-26, model version 10, types. A record with no `type` (2,243
  shards; EMBL-EBI's ProteomeXchange pages, ScienceOpen collections, SAGE
  videos, created 2006 to 2022) has an unknown kind: `Option<WorkType>`,
  no form, and no genre unless a notice gives one — not guessed as
  `database`, not kept out. `patent` (1,070 shards) and `journal` (64)
  were reference types, not work types, and the reference, funding,
  peer-review and relation-id lists take the deposit schema's whole lists
  (`short-doi` and `orcid` are the data's, not the schema's; a short DOI
  is read as a DOI, an ORCID names nothing). Crossmark's update words, the
  user's readings: capitals, `expression-of-concern` and the misspellings
  `retration`, `retracion` are the Crossmark word; Elsevier's `err` is an
  erratum (its label on every one); APS's `corrected` and AIP's
  `publisher-note` are corrections; `comment` and `correspondence` are a
  comment, a new update type the merge reads as `is-comment-on` /
  `has-comment`; `unknown`, `print`, `article` and three more section
  names are no update, set aside and counted (`Sink::set_aside`, new);
  and `sample_update` and two more are test deposits, whose three records
  among the dumped ones the new checked-in exclusion list keeps out. Each
  update keeps the word beside its reading. Two things the dump showed
  that the proposal had not: Cambridge's `corrected-article` names the
  work its notice corrects, not the notice — 32 of its 33 notices are
  titled "… – ERRATUM" or "… – CORRIGENDUM", 29 one page — so it is a
  correction, where the user had chosen a type of its own on the
  proposal's reading that the notice is the corrected article; and an
  update to the record itself (APS's `corrected`, 35, and a few
  `correction`, `retraction`, `new_edition`) marks a work updated in
  place, which now makes it no notice in the genre, though it still
  counts in its status. Over the dumped records (M): no vocabulary
  failure left, 1,070 updates read from an older word, 69 set aside, and
  the merge's view maps every relation of the 12,394 that normalize.
- 2026-09-26, model version 10, dates. A license with no `start` stopped
  3,272 shards: Crossref's deposit schema makes the start optional ("the
  license terms will apply for the article's lifetime") and its API docs
  call it required. What Crossref does (M, every local license and every
  dumped one): where a deposit gives no start it writes the work's
  `issued`, padded to its first day, and where there is no `issued`,
  nothing; `delay-in-days` is `max(0, start − issued)`, 0 when either is
  missing, and every start is at midnight. The user's readings: a start
  that is absent or on the padded `issued` means from publication (the
  fill and a publisher's start on that day cannot be told apart, and say
  the same), any other is its day, and the one `0001-01-01` (Frontiers,
  10.3389/fmicb.2019.02678) is kept as written beside why
  (`LicenseStart`); the delay is checked and dropped. Those rules are
  checked, and a license that breaks one stops the run. Since model 9
  the reader had been giving 40 % of the local sample's licenses a day
  and an hour their publishers never named. The dates Crossref gives
  beside the publication dates were parsed and dropped, and are kept now
  as (what it is, date) — the user's call, with a dissertation's or a
  standard's `approved` often its only date — among them `content-
  updated`, new, which stopped 9,641 shards: a dataset's last change, in
  no fixed order with `content-created`. A grant's own `award-start`
  (991 shards) and an investigator's role dates are read, not kept, as
  grants are not. Over the dumped records (M): no date failure left;
  9,525 starts from publication, 1,289 on a day; 11,060 `content-updated`
  and 1,115 `approved` kept.
- 2026-09-26, model version 10, fields. The file has four top-level
  fields the reader did not know, and the dumped records hold no fifth:
  `content-updated` (the dates entry above), `award-start` (likewise),
  `version` (193 shards: a label, `v1` on Rogue Scholar's posts, `1` on
  eLife's, kept as written since the schema allows 100 characters of
  free text; its language, `en` on 6 of 226, read and not kept) and
  `part-number` (100 shards, RSC's sets above all: `1`, `06`, `33-6`,
  kept as written). Behind `award-start` the grants held more: a
  funder's share of the money with its percentage (35 shards, 912
  dumped grants), co-lead investigators (13 shards), investigators'
  other names (a list, where the API docs say a string), planned project
  dates (seen on the live API, not yet in the file), funders named by a
  ROR alone, and a project amount with a `null` currency beside a real
  amount (89). All are read, typed, so that a new shape still stops the
  run, and none is kept: grants are the user's to decide as a pass of
  their own. Documented fields not seen in the data — `component-number`,
  `free-to-read`, `status` — are not modelled; the first to appear stops
  the run with its shape. Over the dumped records (M): every one of the
  1,197 grants reads; 226 version labels and 103 part numbers kept. What
  the dump still stops at is the next pass's: 3,009 authors with neither
  a family name nor a name, 1,951 records with no `member`, 1,117 `null`
  strings (a resource's URL, mostly), 270 journal issues with no number,
  150 records with no `resource`, and one language.
- 2026-09-26, the survey of the whole March 2026 file on model 10
  (homelab `ol-rht-survey-crossref-v2`, reading past a line that does not
  parse): 179,536,204 records from all 35,908 shards, none abandoned,
  about 12.5 minutes a share (M). 355,035 fail, 0.2 %, in 33 signatures:
  the next pass's — 303,155 authors with neither a family name nor a
  name, 22,268 `null` strings, 15,535 records with no `member`, 6,255
  journal issues with no number, 1,290 ISBNs, about 200 records whose own
  DOI is not ASCII, 303 three-letter language codes, 195 with no
  `resource`, 9 ORCIDs — and a few that model 10 itself caused or had not
  seen, settled the same day, still as version 10 since nothing had been
  normalized under it: 5,806 records carried an empty affiliation, `{}`
  (Maximum Academic Press above all), which the new "a name or an id"
  failed, and which is now set aside and counted; two ISNIs fail their
  check character, and an ISNI is now a claim; posted content's `status`
  (5 records: withdrawn or removed, when, why — eLife's retracted reviewed
  preprints) is kept, and a version label's notes are read, not kept;
  Retraction Watch's update typed `68818` (both sides of one Spandidos
  retraction, beside the publisher's own `retraction`) is set aside; and
  four more test deposits join the exclusion list, their March stamps
  read off their lines (homelab `ol-rht-dump-crossref-v2`, which found
  every DOI the survey named but those written with non-ASCII
  characters: the file escapes them, `\u2013`, and the search did not).
- 2026-09-26, still model 10: an author with nothing in it. The largest
  failure of the whole-file survey, 303,155 records, was an author with
  neither a family name nor a name. Every one among the dumped records
  but one (3,034 of 3,035, M) is `{"affiliation": [], "sequence":
  "additional"}` — no name, no ORCID, no affiliation — one to a record,
  beside the real authors, on dissertations from university repositories
  (Drexel, Wellington, La Plata, Iowa, UPC, North Texas, Queensland). It
  says nothing, and is set aside and counted, as an empty affiliation
  is. The one exception has a given name and nothing else ("ASDEX
  Upgrade Team", an organisation in a person's field): it still stops
  the run, under a signature of its own, so the next survey counts that
  shape across the file before anyone decides how to read it.
- 2026-09-26, still model 10: `null` links. The whole-file survey's
  second failure, 22,268 records, was a `null` where the reader expects
  text. Among the dumped records (M) it is two shapes, both in
  `resource`, which the normalizer does not keep: a work's landing link
  `{"primary": {"URL": null}}` (921: figshare's components for Optica and
  Copernicus, two AGU chapters) and a secondary link that is all nulls,
  `{"URL": null, "label": null}` (309: OUP's through Ingenta and BioOne,
  the Chinese Journal of Mechanical Engineering's). Both are read as
  absent; the next survey says whether the file holds a third.
- 2026-09-26, still model 10: no `member`. 15,535 records of the file
  name no member, and all but two of those dumped no publisher either
  (M). They are DOIs that moved: Taylor & Francis's, Elsevier's, De
  Gruyter's and smaller publishers' journals passed to new owners whose
  prefixes the records carry (`10.65087`, the Aquatic Ecosystem Health &
  Management Society; `10.65844`, Hasanuddin University — Crossref's
  `/prefixes`, 2026-09-26), deposited in 2025 and 2026, and Crossref's
  index does not attach the new member to the work, on the live API as
  in the file. `member` and `publisher` are now optional: absent where
  the record does not say them. The owner is still known, from `prefix`.
  The member is not looked up from the prefix: that would be another
  source's word, for a lookup table of its own if it is ever wanted.
- 2026-09-26, still model 10: a journal issue with no number (6,255
  records: `journal-issue` holding only the issue's dates, JoVE's,
  Tsinghua's, Permanyer's), and a record with no `resource` at all (195:
  Elsevier's, Springer's, JSTOR's, deposited 2016 to 2022, most with
  full-text links still). Neither is kept by the normalizer; both are
  read as absent. Of the 33,476 records the first dump holds, 4 now
  fail: two ISBNs, one language, one author with only a given name.
- 2026-09-26, still model 10: an ISBN is a claim (the user's call). 1,290
  records of the file carry one that is not an ISBN, and a record — a
  book, a chapter — stopped on it: publishers' check digits (Carl
  Hanser's, AIAA's), an ISBN-10's `X` on thirteen digits (Blackwell's),
  twelve digits (the Institute of Mathematical Statistics'), fourteen
  with a stray `9` in front. As an ISSN, a ROR and an ISNI are, it is
  kept as written beside why and never matched on, and the record
  normalizes. The sample's 186 of 2026-09-15 all passed, which is why it
  was strict.
- 2026-09-26, still model 10: a language may be ISO 639-2's three-letter
  terminologic code (303 records: `spa` on two Peruvian publishers',
  `eng`), read as the language it names; a bibliographic-only code
  (`fre`) has not been seen and still stops the run. An author's ORCID
  written `http://orcid.org/` (8) is the same iD, and one whose check
  character fails (1) is a claim, as the other ids with checks are.
  Every one of the 234 records the whole-file survey named as examples
  now normalizes.
- 2026-09-26, still model 10: DOIs beyond ASCII, the user's decisions.
  About 200 records of the file had a DOI the model refused; all 97 the
  survey named were found by their escaped spelling (homelab
  `ol-rht-dump-crossref-v3`) and checked at doi.org and Crossref's API.
  In `model::Doi`, for every source and every DOI, records' and
  references' alike:
  - every Unicode dash, and the minus sign, is `-`. The 61 en-dash DOIs
    are second registrations — Wiley's, Hogrefe's, AAIDD's — of articles
    whose hyphen twins are the live records, and doi.org redirects the
    one to the other (`10.1027/0838–1925.18.2.82`, 301); read as the
    twin's DOI, a second registration is a copy of the twin's record in
    the merge, one work;
  - a letter beyond ASCII is the DOI of record, lowercased as registries
    write it (33: Lavoisier's `réseaux`, Akadémiai Kiadó's `mentál`),
    since doi.org resolves them. Some publishers registered an ASCII twin
    too (`día` and `dia`); that the merge joins such twins waits on a
    count of them over the whole file (homelab
    `ol-rht-census-doi-spellings-v1`);
  - whitespace at an end is dropped (2: `10.3354/meps7782` and a
    non-breaking space — an early registration of an article whose DOI is
    `meps07782` — and `10.4021/gr2008.11.1240` with three); neither
    spelling resolves, and the user wants such readings counted;
  - a letter written as its UTF-8 bytes read as Latin-1 is repaired (1:
    Fraunhofer's `gã¼nthner`, whose `günthner` twin is live), a table the
    data will add to.
  Each reading is counted in the survey's report (`Sink::corrected`, new),
  by field and kind, so a reading that leaves a DOI no registry knows is
  a number, as an exclusion is. Crossref's own `aliases` (138 of the
  dumped records; a figshare item's `.v1`) were parsed and dropped, and
  are now kept, each a name the merge gives the work.

## 11. Dependencies

The scaffold was dependency-free by request. In use as of 2026-09-15, one
version each for the workspace: `serde` + `serde_json`, `quick-xml`,
`flate2`, `zstd`, `postcard` (chosen over `bincode`: a stable, documented
wire format, varints, borrowing deserialization). Not yet needed and not
added: `thiserror`, `anyhow`, `clap` (the binary's argument grammar is a
`match`), `rayon` (parallelism over files can be `std::thread::scope`),
`proptest`, `tracing`. No date crate: `PartialDate` is a packed integer
with its own calendar check. `smallvec` (2026-09-15) for the lists inside
records, sized from the observed distributions, so a record's usual lists
live inside it. Fetching (2026-09-15): `ureq` 3 over rustls
with transparent decompression off, so raw bytes land as served, plus
`sha2` and `md-5` for the digests the sources publish or imply; no async
runtime — one thread per transfer is the runner's own shape. arXiv's and
Semantic Scholar's paging will use the same client.

## 12. Sources consulted

- Crossref: [2026 public data file announcement](https://www.crossref.org/blog/2026-public-data-file-now-available/),
  [bulk downloads documentation](https://www.crossref.org/documentation/retrieve-metadata/bulk-downloads/),
  [March 2026 sample torrent](https://academictorrents.com/details/f692802461d1277d14551526ecb292a9637af254),
  [March 2026 full torrent](https://academictorrents.com/details/b5ee0e102689b3e67023dd024694c0f5f124646f).
- OpenAlex: [snapshot access](https://help.openalex.org/access/snapshot/),
  [snapshot data format](https://docs.openalex.org/download-all-data/snapshot-data-format),
  [API keys required from 2026-02-13](https://groups.google.com/g/openalex-users/c/rI1GIAySpVQ),
  works manifest at `https://openalex.s3.amazonaws.com/data/jsonl/works/manifest.json`.
- Semantic Scholar: [Datasets API](https://api.semanticscholar.org/api-docs/datasets),
  release listing at `https://api.semanticscholar.org/datasets/v1/release/latest`,
  [API key form](https://www.semanticscholar.org/product/api).
- PubMed: baseline listing at `https://ftp.ncbi.nlm.nih.gov/pubmed/baseline/` — 1,334 files, highest `pubmed26n1334.xml.gz` (13 MB).

## Appendix A — speculative normalized shape (not the plan)

Everything below is a hypothesis written before any record was parsed. It is
kept because it records what the downstream funnel will ask for, so the
discovery in §4a has something to check itself against. Nothing here is
committed; §4 is.

The per-source normalized shape. Shared across sources so the merge stage is
source-agnostic; not itself the merged schema. Fields chosen from what the
funnel of the candidate-management note consumes: hard filters on date / language / type /
retraction / access, cheap scoring on title / abstract / counts / topics.

CR = Crossref, PM = PubMed, AX = arXiv, OA = OpenAlex, S2 = Semantic Scholar.

| Field         | Type                                            | CR             | PM              | AX              | OA                       | S2                  | Notes                                         |
|---------------|-------------------------------------------------|----------------|-----------------|-----------------|--------------------------|---------------------|-----------------------------------------------|
| `source`      | `SourceId`                                      | ✓              | ✓               | ✓               | ✓                        | ✓                   |                                               |
| `native_id`   | `NativeId`: Doi \| Pmid \| ArxivId \| OpenAlexId \| S2CorpusId | DOI | PMID          | id              | `id`                     | `corpusid`          | required                                      |
| `ids`         | `ExternalIds`                                   | doi            | doi, pmc        | journal doi, datacite doi | `ids`          | `externalids`       | all strict; URL forms stripped                |
| `kind`        | `WorkKind` (closed)                             | `type`         | pub types       | preprint        | `type`                   | `publicationtypes`  | explicit per-source mapping table             |
| `title`       | `Option<NonEmptyText>`                          | `title[0]`     | `ArticleTitle`  | `title`         | `title`                  | `title`             |                                               |
| `abstract`    | `Option<NonEmptyText>`                          | JATS stripped  | sections joined | `abstract`      | inverted index rebuilt   | — (separate `Abstract` stream) |                                    |
| `authors`     | `Vec<Author>`                                   | ✓              | ✓               | ✓               | `authorships`            | `authors`           | `Person{given,family,orcid}` \| `Group{name}` \| `Unsplit{name}` |
| `venue`       | `Option<Venue{name,issn[],isbn[],publisher}>`   | ✓              | `Journal`       | `journal-ref`   | `primary_location.source`| `journal`, `venue`  | name and publisher interned                   |
| `published`   | `Option<PartialDate>`                           | `issued`       | `PubDate`       | `created`       | `publication_date`       | `publicationdate`   |                                               |
| `language`    | `Option<Lang>`                                  | ✓              | ✓               | —               | ✓                        | —                   | ISO 639                                       |
| `subjects`    | `Vec<Subject{scheme,code,label}>`               | —              | MeSH            | categories      | `topics`, `mesh`         | `s2fieldsofstudy`   | scheme-tagged; interned                       |
| `status`      | `WorkStatus`                                    | `update-to` typed | pub types    | withdrawn ver.  | `is_retracted`           | —                   | normal / retracted / corrected / withdrawn    |
| `access`      | `Option<Access>`                                | `license`,`link`| PMC presence   | open            | `open_access`            | `isopenaccess`      |                                               |
| `counts`      | `Counts{references,cited_by}`                   | ✓              | —               | —               | ✓                        | ✓                   | source-reported, not computed                 |
| `provenance`  | `Provenance{origin,fetched_at,source_version}`  | `indexed`      | `DateRevised`   | `updated`       | `updated_date`           | release id          | drives incremental updates later              |
| `references`  | `Reference` stream                              | ✓              | partial         | —               | ✓                        | `citations` dataset | separate edge records, never a `Vec` on `Work`|

`Reference = { from: NativeId, target: ReferenceTarget, asserted_by }`,
`ReferenceTarget = Doi | Pmid | OpenAlex | S2Corpus | Unresolved(UnresolvedRef)`.
`UnresolvedRef` keeps whatever structured fields the source gave (author,
year, journal, volume, page, unstructured string) for a later matching stage.

`Abstract = { native_id, text }` — a stream of its own, so a source whose
abstracts arrive separately (S2) needs no join.

`Author::Unsplit{name}` exists because S2 and OpenAlex's `raw_author_name`
give one string; splitting names is a modelling decision not made here.

### Candidate non-ID newtypes

| Type            | Rule                                                                                                  |
|-----------------|-------------------------------------------------------------------------------------------------------|
| `PmcId`         | `PMC` followed by digits                                                                              |
| `Orcid`         | 16 digits, last may be `X`, ISO 7064 checksum verified                                                |
| `Issn`          | 8 characters, checksum verified                                                                       |
| `PartialDate`   | `Year` \| `YearMonth` \| `YearMonthDay` \| `Range` (for `MedlineDate`); calendar-valid; sane year     |
| `Lang`          | ISO 639-1/2 code                                                                                      |
| `NonEmptyText`  | trimmed, non-empty, no control characters                                                             |
| `WorkKind`, `Subject.scheme`, `WorkStatus` | closed enums. No `Other`. An unknown value is a parse failure (§6)          |

Rule: nothing enters `model` as a bare `String` unless it is genuinely free
text (abstract body, unstructured reference, title, author name).
