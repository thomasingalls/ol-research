# Ingestion pipeline — plan

**Status:** draft 4 — source facts verified against current documentation on
2026-09-14 (§12). Nothing here is implemented.
**Inputs:** [architecture summary](inbox/citation-graph-architecture-summary.md);
decisions from three rounds of discussion on 2026-09-14, logged in §10.

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

## 2. Sources

Five. Summary first; per-source facts follow. The bullets mix documented
facts with *expected* warts from prior experience of these sources; every
wart is a claim the POC run confirms or refutes, not a commitment.

| Source           | Native ID  | Nodes | Edges                                 | Full-scale channel                          | POC channel                              |
|------------------|------------|-------|---------------------------------------|---------------------------------------------|------------------------------------------|
| Crossref         | DOI        | yes   | yes — `reference[]`                   | annual file: one 208 GB tar of `.jsonl.gz`, torrent | the official 31 MB random-sample torrent |
| PubMed           | PMID       | yes   | partial — `ReferenceList`, mostly PMC | baseline + daily update files, HTTPS        | one late-numbered baseline file          |
| arXiv            | arXiv ID   | yes   | **no**                                | OAI-PMH harvest                             | OAI-PMH, ~10 pages                       |
| OpenAlex         | `W…`       | yes   | yes — `referenced_works[]`            | quarterly S3 snapshot over HTTPS, 666 GB    | one manifest-chosen part: 10,873 records, 20 MB |
| Semantic Scholar | corpus ID  | yes   | yes — `citations` dataset             | Datasets API, 30 shards per dataset, API key | **paused** — needs a new API key        |

### Crossref

- Nearly 1.8 × 10^8 records in the March 2026 file. Distributed as one
  208 GB `tar` of gzipped JSON-lines files, one record per line — the
  pre-2024 `{"items":[…]}` layout is gone. Via Academic Torrents
  (doi 10.13003/nggf-vt1j) or AWS requester-pays (~$18). Records are the
  REST API's item objects *(confirm on the sample)*.
- Crossref also publishes a 31 MB **random sample** as its own torrent: 100
  `.jsonl.gz` files. That is the POC input — no file selection from the big
  tar needed, and a random draw is the best schema-diversity sample available.
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
- Terms: metadata is free to reuse and not subject to copyright; abstracts
  carry some limitations.

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
- Observed in `pubmed26n1334.xml.gz`: `<PMID Version="1">` on every record;
  `ArticleId` uses both `IdType="pmcid"` (7,309) and `IdType="pmc"` (1,380)
  for the same identifier — the first wart logged for the PubMed struct.
- The DTD is large. Strict modelling of it is the biggest single modelling
  surface in this plan; the POC file will show how much of it is exercised.
- Terms: NLM provides the data freely; some abstracts may be under copyright
  *(confirm the current NLM data terms before product use)*.

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
scope here; it changes that arithmetic.

## 3. Architecture

### 3.1 Stages

```text
acquire    bytes to data/raw/, byte-identical where the channel allows, manifest entry per file
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

Trait shapes — illustrative, not code. The lifetimes are the point (§5):

```text
Source      name, native-ID type, enumerate origins, read records into a reusable buffer
Parser      &'a [u8]  →  Native<'a>                       borrows; strict
Normalizer  Native<'a>, &mut impl Emit  →  ()             consumes; pushes normalized records by reference
Sink        implements Emit; serializes immediately; owns output buffers and symbol tables
```

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
  ol-rht/             CLI:  fetch <source> | normalize <source> [--limit N] [--survey] | dump --json
manifests/<source>.json   committed: every raw file's URL, fetched_at, bytes, sha256
data/                     gitignored
  raw/<source>/…          source of truth
  normalized/<model-version>/<source>/…   rebuildable
```

Each source crate carries `fixtures/`: single raw records that once failed to
normalize, each with a test asserting they now do (§6).

`hello` is removed when the first real crate lands.

### 3.4 Concurrency

Bulk inputs are already sharded into many files. Parallelism is a work queue
over files (rayon), one file per task, sequential within a file, no shared
mutable state between workers. PubMed needs a streaming parser: files are
tens of MB gzipped with tens of thousands of records each. API mode is async
and rate-limited per source.

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
run surfaces many warts instead of one. Default is stop-at-first.

## 7. POC acquisition

Target ~10k records per source, disk-conservative, downloads over APIs.
93 GiB free on this machine; the POC needs well under 1 GB. Software is
installed and run by you, not by me; the plan gives commands.

| Source           | What to fetch                                                                    | ≈ size     | Notes                                                              |
|------------------|----------------------------------------------------------------------------------|------------|--------------------------------------------------------------------|
| Crossref         | the official random-sample torrent: 100 `.jsonl.gz` — **stalled, no seeders**; fallbacks below | 31 MB | `aria2c`; byte-identical; zero API load |
| PubMed           | `pubmed26n1334.xml.gz`, the highest-numbered baseline file (most recent records)  | 13 MB      | direct HTTPS; `--limit` for 10k                                    |
| arXiv            | OAI-PMH `ListRecords`, ~10 pages, one date window or `set`                        | 10–20 MB   | each page saved verbatim                                           |
| OpenAlex         | `updated_date=2025-07-25/part_0000.gz`, 10,873 records                            | 20 MB      | direct HTTPS; byte-identical                                       |
| Semantic Scholar | **paused** — first ~10k lines each of one `papers`, `abstracts`, `citations` shard, streamed, once a key exists | ~10 MB | not byte-identical files — manifest records release, shard, line count. The three prefixes will not share corpus IDs; fine for schema coverage |

Sampling goal for the POC is *schema diversity*, not a connected subgraph.
Connectedness is the graph experiment's concern and can be assembled later
from OpenAlex `referenced_works` by seed-and-expand.

Full-scale raw for all five is over a terabyte (Crossref 208 GB, OpenAlex
works 666 GB, S2 papers + abstracts + citations ~350 GB, PubMed ~40 GB). That
is beyond this machine and is a planning item for M8, not a surprise for
later.

### POC fetch commands

Run by you. Everything lands under `data/raw/<source>/`; `ol-rht fetch` (M2)
takes over from here and writes the manifests — these first files get their
manifest entries backfilled from disk.

```bash
brew install aria2
```

Crossref sample (a torrent; aria2 downloads the `.torrent` and follows it):

```bash
aria2c --file-allocation=none --seed-time=0 -d data/raw/crossref \
  https://academictorrents.com/download/f692802461d1277d14551526ecb292a9637af254.torrent
```

Drop `--seed-time=0` to seed back for a while; it is 31 MB.

If it stalls (it did: no web seed, two trackers), in order of preference:

1. Retry from the saved `.torrent` with extra public trackers; aria2 resumes
   from its control file. Leave it running in the background — a sample
   torrent may have one intermittent seeder.

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
3. Semantic Scholar API key — paused; M5 resumes when one exists.
4. `--survey` opt-in is assumed (§6). Veto if it smells like a lenient mode
   in disguise.

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
- **M3** — `source-openalex` (widest schema, second edge source, cross-IDs).
- **M4** — `source-pubmed` (streaming XML, the largest modelling surface).
- **M5** — `source-s2` (three streams). Paused until a key exists; the slot
  stays, and M6 can move ahead of it.
- **M6** — `source-arxiv` (OAI-PMH).
- **M7** — full POC run across all five; the fix loop until it is green;
  review the stats report and the budget.
- **M8** — full-scale plan: storage, incremental origins (`DeleteCitation`,
  update files, OAI `from`), model-version invalidation.
- Then: the merge plan.

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

## 11. Dependencies

The scaffold is dependency-free by request; this is what the pipeline needs,
listed for approval rather than assumed: `serde` + `serde_json`, `quick-xml`,
`flate2`, `zstd`, `postcard` (or `bincode`), `thiserror`, `anyhow`, `clap`,
`rayon`, `proptest` (dev), `tracing`. No date crate: `PartialDate` is a
packed integer with its own calendar check. For fetching: `reqwest` +
`tokio` (needed from M2, since arXiv and Semantic Scholar have no
plain-download channel for the POC).

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
architecture doc's funnel consumes: hard filters on date / language / type /
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
