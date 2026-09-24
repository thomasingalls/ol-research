# The merge's memory, measured against the corpus

**Status:** analysis, 2026-09-24, and the order the user chose from it
("yes, record the note and start on step 0"). Written after the corpus
merge rehearsal (`ol-rht-merge-survey-v1` on the box) was killed for memory
at its 80 GiB limit. It supersedes the ranking in
[merge plan](../merge-plan.md) §11 item 16, whose arithmetic rested on the
laptop sample's 34.3 stubs per held work; the corpus has 0.05.

Every figure is marked: **M** measured, **D** derived from measurements
(a per-item cost measured on the sample, multiplied by a count measured on
the corpus), **E** estimated. The per-item costs come from probes run on a
scratch store of the 100,483 PubMed records on this machine, with the
repository's counting allocator; nothing in the repository was changed to
take them.

## What the rehearsal measured

The PubMed 2026 baseline, normalized at model version 8 (M): 39,994,988
records, 446,087,838 references, 36.5 GiB in 1,334 sets.

Pass 1 finished (M): 91,074,034 distinct names read in 743 s; 50,776
conflict components, 8 of them beyond the rules (journal supplements whose
single DOI 34 to 96 records carry); 39,983,721 held works and 2,006,039
stubs; 41,929,739 registry lines computed. "identity and registry done in
1557.5 s" is the last line the merge printed. Reference targets: 402,643,079
held, 3,046,696 stub, 40,182,312 none, 215,751 disagree.

Resident memory (M, read once a minute): 21 GB by the end of reading, flat
while resolving, a peak of 50.4 GB by the end of registry assignment, flat
at 52 GB into pass 2, then climbing about 6 GB a minute through the
references until the kill at 79.5 GB.

## Where it goes

**Pass 1 and registry assignment, about 48 GB (D)** against the 50.4 GB
measured:

| structure | per item (M) | corpus (D) |
|---|---|---|
| `Copies` (`copies.rs:130`) | 109 B a record | 4.4 GB |
| `Identity` after `add` | 171 B a name | 15.5 GB |
| `res.works` | 104 B a work | 4.2 GB |
| `held_order()` names (`identity.rs:1556`) | 159 B a work | 6.3 GB |
| their clone into `all` (`main.rs`, `merge_assign`) | — | ~5 GB |
| `Assignment.lines` | 140 B a line | 5.9 GB |
| the registry's fold, `HashMap<OwnedName, WorkId>` (`registry.rs:103`) | 70 B a name | 6.5 GB |

Every DOI's text is held twice in the arena (`arena.rs:24-25`), and the
assignment clones every name three more times.

**Pass 2, about 200–230 GB to finish (D).** On a base of about 31 GB, since
the registry's fold is still alive although the journal line needs only
its counts:

- views, 427 B each (M) × 40.0 M = 17 GB, of which about 7 GB is the
  `vec![None; n]` allocated before a view is read (`build.rs:974`);
- `cited_as`, one 40-byte row per name of every reference landing on a held
  work (`build.rs:1335-1351`), 1.23 rows a reference (M) × 402.6 M, plus the
  names' text: about 24 GB;
- unresolved references, 40.2 M × (208 B + 202 B of text) (M) = 16.5 GB;
- edges, 406 M × 12 B = 4.9 GB;
- then `finish`: 42.0 M `OwnedWork` rows × 1,920 B = 80.6 GB, and the stable
  sort beside it up to 40 GB more.

The kill came about 60% of the way through the references. The laptop's
~200 B a reference is closer to 100 B on the corpus's mix, and it is
`cited_as` and the unresolved rows, not the edges.

**At the target** — about 6 × 10⁸ works and 3 × 10⁹ arcs, once Crossref and
OpenAlex are in (graph plan) — today's costs come to about 2 TB (E). Both
passes have to change.

The two-pass structure is not the problem: works need their ids before a
reference can be aimed at one. The problems are that pass 2 holds every
output row in memory before writing any, and pass 1 holds every name,
record and work as a heap object.

## The approaches, ranked

**0. Stop the copies nothing needs.** Stream registry lines to the pending
sidecar instead of collecting them; pass the held works' names to the
registry by reference instead of cloning them into `all`; drop the
registry's fold once the ids are assigned; stop allocating `views` whole
before reading any. Output byte-identical. Pass 1's peak about 50 → 33 GB
(D). Needed whatever comes after.

**1. Pass 2 spilled by work id, then finished a bucket at a time.** Each
row pass 2 makes is appended, as it is made, to one of *B* files chosen by
the range its work's id falls in — edges, `cited_as`, unresolved rows and
relations by the citing work, stub citers by the stub, views by their
record's work. Then each bucket in turn is loaded, run through the existing
`held`/`stub`, deduplication and sorts, written straight to its shards
(`layout.rs` already walks shards in id order), counted for the report, and
freed. Pass 2's memory becomes one bucket: about 1–1.5 GB for PubMed with
*B* = 64 (D), about 1 GB at the target with *B* ≈ 512 (E). The spill is about
34 GB for PubMed (D), 350 GB at the target (E), written and read once,
sequentially — minutes on the node's NVMe, which must be mounted for it.
Stable sorts within a citing work see the same order, so the output should
be byte-identical, and the registry and the full/incremental equivalence
are untouched. About 1,000 lines in `build.rs`, `layout.rs`, `main.rs` and
`report.rs`. With 0: the first complete PubMed merge, at about 33 GB, now
bounded by pass 1.

**2. Pass 1 as a sorted name dictionary and dense arrays.** During the read
of copies, spill every asserted name to sorted runs and merge them into an
FST from name to its rank, so a name's integer order is its `Name` order.
Measured on the sample at 5.0 B a name (DOI 8.4, PMID 3.9, PMC 5.2,
publisher 6.6) against 171 today. Then a 4-byte parent per name for the
union-find, about 9 bytes per record, works of a trivial component implicit,
`held_order` an integer sort, the registry's fold a `Vec<WorkId>` indexed by
name, filled by streaming the log through the FST. PubMed's peak about
4–5 GB (D) — ten times under the 50.4 GB measured, sixteen under the kill —
and 30–40 GB at the target (E). What changes: a name's index follows name
order instead of read order, which shows only in `held_order`'s tie-break
(`identity.rs:1563`); that tie-break must be made to depend on content. The
incremental merge calls the same rule functions and the harness guards the
equivalence. About 2,000 lines and a 300-line external sort. A cheaper form
(2b), a hash-compacted arena at about 20 B a name plus its text, gives
PubMed 6.5 GB but the target 50–75 GB (E).

**3. External joins throughout**, only if 2 measures too high at the target:
references exploded into (name, reference) pairs and sort-merged against the
dictionary, registry assignment as joins and a sequential fold. About
10–17 GB resident at the target (E), for 150–300 GB more sequential I/O.

**Set aside:**
- M9 as the ingestion plan words it — pass 2 once per range of work ids,
  the input re-read each time. PubMed would need 30–60 reads, 6–15 hours;
  days at the target; and pass 1 is left as it is. A fallback if no scratch
  disk can be had. Anything memory-mapped must be kept off ZFS, whose ARC
  would cache it twice and serve misses as random SATA reads.
- An engine (DuckDB, DataFusion, Polars, RocksDB, LMDB). The union-find,
  the verdicts and the copy rules stay in Rust either way, ties that follow
  read order would need explicit ordinals everywhere, and a key-value
  store over 3 × 10⁹ names is memory again or random reads.
- Leaner rows alone: under 1, rows exist a bucket at a time. Splitting out
  stub rows (merge plan item 16's first option) saves 2.0 M × 1,680 B =
  3.4 GB on the corpus.
- Splitting pass 1 by source or by name hash: a work's PMID, DOI and PMC
  put nearly every component across the pieces, so putting them back is
  the whole job.

## The order, and what to measure

The user's choice: **0, then 1, then 2**, measuring between; 3 only if 2
measures too high at the target.

- After 0: `merge --survey` on the box again, for the per-phase heap the
  merge now prints.
- After 1: the old and new merged directories compared byte for byte on a
  sample store, the incremental harness, the spill's real size and pass 2's
  time.
- Before settling 2's form: the FST built over the corpus's real names, for
  bytes a name and lookups a second.
- For the target: names per namespace and `cited_as` rows per reference for
  Crossref and OpenAlex, from the sample's merged layer; the NVMe scratch's
  size and throughput.

## Step 0, done 2026-09-24

What changed, with no rule and no written byte moving:

- `registry::assign` takes the works' names as borrowed slices
  (`AsRef<[OwnedName]>`), so `merge_assign` lends the held works' and the
  stubs' names instead of cloning every list into `all`.
- `registry::assign_each` hands each line to the caller as it is made, and
  `registry::Stager` writes it straight into the sidecar — the same
  `.pending` file, its head written last over a padded first line, which
  `recover` reads with its padding trimmed — so the run's lines are never
  held. `Pending::stage` is now that writer over a list.
- The registry's fold is dropped once the ids are assigned; the journal
  line and the public ids take its two totals (`Totals`) instead.
- Not done: the subagent's fourth item, `views` allocated whole before a
  view is read. That allocation is the views' own inline storage, and it
  is full once they are read, so doing it later moves no peak.

Measured on the scratch store of 100,483 PubMed records (M): the identity
pass's peak 0.91 → 0.65 GiB, the heap entering pass 2 0.34 → 0.21 GiB; the
run's peak 5.38 → 5.25 GiB, still inside `finish`, which is step 1's to
remove. A first merge and a second (every id reused) wrote the same
registry lines (timestamps aside), the same journal lines, the same 175
shard files byte for byte and the same reports as the build before, and
`--survey` and `merge identity` still leave the registry and journal
untouched. The corpus figure waits for the next survey on the box.

## Step 1, built 2026-09-24

Pass 2's rows go to disk as they are made (`merge::spill`): each to the
file of the range its work's id falls in — a view by its record's work, an
edge, relation, `cited_as` row and unresolved reference by the citing work,
a stub's citer by the stub — a range being a whole number of shards
(`OL_RHT_SPILL_SHARDS`, 8 by default: 524,288 ids). `Builder::spill_to`
turns it on before a set is read; `Builder::finish_spilled` then reads the
ranges back in id order, joins each record's views in the order they were
read, and finishes each range by the same rules the in-memory `finish`
uses — the two now share one `finish_range` — writing the range's shards
(`layout::Shards`) before it reads the next, into a directory that takes
the merged sets' name only when every range is written. `ol-rht merge`
spills under the system's temporary directory, which on the box is the
pod's own scratch. The report's counts that were read off the rows
(`edges`, `relations`, unresolved rows by why) are counted as ranges finish.
The in-memory `finish` stays for the serving fixtures and the tests.

Measured on the scratch store (M), against the build before:

| | peak heap | resident |
|---|---|---|
| everything held | 5.38 GiB | 4.72 GB |
| spilled, 8 shards a range (3 ranges) | 2.32 GiB | 3.00 GB |
| spilled, 1 shard a range (24 ranges) | 1.30 GiB | 1.76 GB |

About 5.4 M rows, 0.24 GiB, spilled; the run takes the time it took. At
both widths the registry, the journal, all 175 shard files and the reports
were the same as the build before, byte for byte, on a first merge and on a
second, and `--survey` leaves `merged/` untouched. The full/incremental
harness was not run: its store is normalized at an older model, and the
full merge's output is unchanged. The corpus figure is the next survey's.

## Walls behind this one

- **Time.** Pass 1 decodes on one thread: 26 minutes for 36.5 GiB. The
  target needs parallel decoding.
- **The public-id fold** (`public.rs:85-89`, two hash maps) comes to about
  70 GB at the target (E). It needs a watermark and a compact set.
- **The data.** Pass 1 found 8 components beyond the rules, all journal
  supplements sharing one DOI. A real merge stops on the first. And some
  publisher ids carry markup NLM shipped in them (ingestion plan §8 item 9).
