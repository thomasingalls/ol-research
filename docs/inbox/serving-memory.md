# The serving artifacts, measured against the corpus

**Status:** analysis, 2026-09-24, before the first corpus epoch. The merge
now fits the box (merge-memory.md); what `epoch publish` builds from its
output has never run past about 4 M works. A subagent measured every stage
on three scratch stores (34,977 / 100,469 / 115,848 held works) and scaled
per item — per name, public id, held work, arc, `cited_as` row — never per
id, since the samples are 94 % stubs and the corpus 5 %. Every figure is
marked: **M** measured, **D** derived (a per-item cost measured on the
samples times a corpus count), **E** estimated. Times are the laptop's.

The corpus, from the merge surveys: 39,984,110 held works and 2,006,041
stubs, 91,073,972 names, 405,845,919 arcs, about 4.2 × 10⁸ `cited_as` rows
(E).

## Stage by stage

| stage | what it holds | sample (M) | corpus |
|---|---|---|---|
| names build | every name and public id as its own `String` in one sorted `Vec`, the FST built in memory, and the registry's whole fold by name though it uses only its ids (`serve/src/names.rs:157–231`, `main.rs:2852`) | 183 B a name, 219 B a public id | about 26 GB (D), 4–6 min |
| works build | every held work's title, abstract, authors and venue as owned strings in one `Vec`, and a map from every record's native name to its document (`merge/src/document.rs:176–237`) | about 3.7 KB a held work resident, 3.1 KB of it text | 80–120 GB (E) for the documents, about 11 GB more (E); `blocks.zst` 23–29 GB (D) |
| cited-as lookup | every row as a (name `String`, row) pair in one `Vec`, sorted, and a 12-byte-a-row buffer allocated whole (`serve/src/cited.rs:229–261`) | about 130 B a row, on few rows | 55–65 GB (D × E) |
| graph | 34 B an id and 5 B an arc; the forward arrays stream (`graph/src/build.rs:159–283`) | 52–221 MB | about 3.5 GB (E), files 5.1 GB |
| lexical index | the documents it is handed; Tantivy's writer 256 MiB (`search/src/index.rs:156`) | 750 B a document on disk | writer under 1 GB (E); index 24–30 GB (D); 25–30 min and up to an hour of merges (D/E) |
| `epoch publish` | its own work is manifests, but the documents stay alive from the works step through cited-as and the graph to the index (`main.rs:2994–3022`); `epoch check` assembles them again | 1.2 GB for the largest sample | **135–185 GB at the cited-as step (D/E)** |
| server | heap: the retired and split maps (empty after the first corpus merge), the journal, Tantivy's small state, the incremental fold; everything else mapped | 3–7 MB heap, 8–23 MB resident | heap tens of MB (E); mapped 60–70 GB (D/E) against a pod limit of 8 GiB that also covers the page cache |

## What breaks first

- Today's publish Job gives the container 8 GiB (`ol-homelab …/job-first-run-v7.yaml`): the names build, the first step, needs about 26 GB (D).
- On the whole box, `epoch publish` dies at the cited-as step: every document still held, and the cited-as pairs besides.
- The server does not break on memory — clean mapped pages are reclaimed — but at 8 GiB about an eighth of its 60–70 GB can stay cached. Seen in the code, not measured: cold reads through ZFS on SATA (which caches a mapped file twice), a works lookup that decompresses a whole 256-row block under one global mutex (`serve/src/works.rs:388–408`), and facet counts that read a column for every match of a broad query (`search/src/index.rs:420–425`).

## What to do before the corpus epoch

1. **Stop holding every document.** At once: drop the documents before the cited-as step and the graph, and stop `epoch check` assembling them again. Properly: assemble by id range with a spill, the merge's step-1 shape (`document.rs:176` asks for it), and give the works rows and the Tantivy writer one range at a time.
2. **Build the cited-as lookup from an external sort** — sorted runs on the scratch disk and an FST built as they merge, which `merge::dictionary::Sorter` already does for names: 55–65 GB to about 1 GB (E). The names build the same way.
3. **Trim the names build**: fold the registry for its ids only, as `registry::Ranked` does, not every name (about 6.5 GB); read `public.jsonl` as a stream instead of two hash maps (about 4 GB, D). Then size the publish Job from these figures.

## Done, 2026-09-24

1 and 2 are built, and 3 but for the works index's public ids (serving
plan §10, the same day): documents assembled by id range and the works
and lexical indexes fed a range at a time in one pass; the cited-as and
names lookups sorted on disk (`merge::runs`); the registry read for its
ids alone. Every artifact byte for byte what it was on the sample, and
the lexical index's answers the same. What the corpus's publish now holds
at once (E): a compact entry per source record for the join, about 3 GB;
one range's documents, 1–2 GB; the works index's public maps, about 4 GB;
a run of the sorts, 256 MiB; the graph, 3.5 GB — about 10 GB at the works
step, where it was 135–185 GB. To measure before the corpus's epoch.

## On the corpus, 2026-09-24

`ol-rht-publish-corpus-v1` on ol-rht 742eaae published epoch 5 from the
corpus's merge of record (batch 5, merged/v8.13) in 2,338 s, and
`epoch check` passed it in 1,737 s (M): names and works rebuilt byte for
byte, the graph's eight files matching their digests, all 41,990,151 rows'
92,996,995 promoted names and 41,990,151 public ids resolving to their
work, the lexical index holding all 39,984,110 held works. Resident
high-water (M): the names lookup 5.1 GB; the works and lexical indexes,
built together in about 15 minutes, 18.0 GB; `epoch check` 22.2 GB — so the
estimate of about 10 GB was low by half, and the 135–185 GB it replaced
was never run. On disk after ZFS's compression: works 19 GB, search
17 GB, graph 3.7 GB, cited-as 3.1 GB, names 1.5 GB. The server's pod,
its limit raised from 8 to 64 GiB for the page cache of these, switched to
epoch 5 the moment it was published and the site answered with 41,990,151
works; the pod serving epoch 4 was retired by the rolling handover.

Still open: what drives the 18 GB (the join's map of every source record
and the works index's public-id maps are the likely parts, E), and the
server's latency on cold reads at this size.

## The 18 GB, 2026-09-25

`epoch publish` and `epoch check` now print a line as each step ends, with
the heap then and the most it was during the step, read from the
allocator: heap alone, where the Job's `VmRSS` also counts the pages of
every file the process has mapped. On a sample store (the two baseline
and five update files, 100,469 held works, 1,587,371 ids), lined up with
the corpus Job's log:

- **The works index's public ids.** Its two maps of text took 187 MiB on
  the sample (M), about 119 B an id: about 5.0 GB on the corpus (D).
  The works index only asks an id for its public id, so it now reads the
  log into a slot per id (`merge::public::ById`, 8 bytes and a bit an
  id): 16 MiB on the sample (M), about 0.35 GB on the corpus (D).
- **Two ranges held at once.** Taking the next range of documents while
  the last was still held: on the corpus a range is about 500,000 held
  works (D, 524,288 ids a range, 95 % held) at 2.7 KB a document
  resident (M), 1.3 GB, held twice, and about 40 % more while a range is
  read back (M). A range now goes before the next is read.
- Together these are the ~9–10 GB the works pass held for its first
  seven minutes, in the publish and in the check's rebuild alike (M,
  `VmRSS` 9.0–10.8 GB and 9.1–9.6 GB); with both fixed, about 3 GB (D).
- **The publish's last 8 GB** came in the lexical index's last minutes
  (M, 10 GB at 19:48 to 17.5 at 19:52, down to 10.8 and up again), and
  the check's rebuild, which has no lexical index, never showed it: the
  lexical index merging its segments (E), heap and mapped pages together.
  The next publish's step lines will say how much of it is heap.
- **The check's 22.2 GB** came in its last six minutes, the cross-check
  (M), whose heap on the sample is 2.3 MiB (M): the pages of the works
  and lexical indexes it reads, mapped, which the kernel takes back when
  it needs them (D). Nothing to fix.
- The Job's `memory.peak` of 49,151 MiB is its limit (M): a container's
  count includes the page cache of what it writes — the spill and the
  artifacts — which the kernel reclaims at the limit. The run was never
  short of memory; it exited 0.

On the sample the works, names and cited-as artifacts are byte for byte
what they were, `epoch check` passes, and the works pass's peak went from
776 to 605 MiB in the publish and from 539 to 367 MiB in the check (M).

The merge of record does not wait on any of this: it writes the merged sets these stages read, and the site keeps serving the epoch it has until one it can read is published.
