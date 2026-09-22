# A record index into the normalized layer

**Status:** design note, 2026-09-19 — merge plan §11 item 22 and serving
plan §11 item 18, written out after both of the day's merge bugs turned
out to be the same bug. Speculative: nothing here is scheduled, no crate
was changed, and **the staging of §7 is a proposal, not a milestone**;
§8 carries a recommendation and §9 the questions it does not settle.
Companion to the merge plan's §6, §8 and §11 item 14, and to the serving
plan's §5 and §11 items 26 and 28.

Two divergences in §1.2 were found by reading and are **not** among the
three bugs this note was written about: `sole` on a relation row, and the
public id's collision test. Neither is measured; both are §9's.

Every figure is marked by how it was got:

- **R** — read out of a real run's report or a manifest on disk, verbatim.
- **A** — arithmetic on a type's `size_of` or on an **R** figure, with the
  multiplication shown.
- **M** — measured here, by a command this note names, against
  `/Users/thomasingalls/Code/ol-rht/data`.
- **?** — not measured. The exact command that would measure it is given
  instead. Nothing in this note is a guess dressed as a number.

The one section to read if only one: **§1.2, the inventory.** It is the
argument; everything after it is engineering.

---

## 1. The problem, stated as a class

### 1.1 What a derived mark is

The merged layer is produced two ways, and they must agree exactly.

A **full merge** builds it from the normalized layer from scratch:
`merge::identity` is the first pass, `merge::build` the second
(`crates/merge/src/lib.rs:18-22`). An **incremental merge**
(`crates/serve/src/incremental.rs`, 3,238 lines) applies one batch's
delta to a published epoch. The plans hold both to a stated equality:
after *n* batches, a full merge must append nothing to the registry and
its merged sets must equal the base with the deltas folded (serving plan
§5).

Along the way both compute **derived marks**: facts about a reference
that are in no single record but fall out of looking at the whole
picture. They are all, without exception, functions of two things — the
reference (its names and its citation text) and the **names table at the
moment of asking**. That second argument is the whole difficulty. A mark
is not a property of a record; it is a property of a record *against a
table*, and the table moves under it.

Two things are already shared, and they are the proof that sharing works.

`merge::build::resolve_reference(names, citation, resolve)` at
`crates/merge/src/build.rs:699-756` is **the one rule** for landing a
reference. The full path calls it at `build.rs:1274`; the batch at
`incremental.rs:2036`, `:2108` and `:2307`. `build::repeats` (`:762`),
`build::rank` (`:817`), `build::corroborated` (`:771`) and
`build::join_citers` (`:796`) are shared with it.

`merge::build::held` at `build.rs:1666-1878` is the larger case, and it
deserves stating because it settles the argument. **Every** promoted
field of a held work — title, authors, venue, published, language, form,
genre precedence, access, `abstract_in`, `status`, the reference and
cited-by counts, `kind_conflict`, `Flags`, the name standings, the
rejected assertions — is computed by that one function, called by the
full path at `build.rs:1389` and by the batch at `incremental.rs:1313`.
Not one of those has ever drifted. They are, by any measure, harder rules
than `held_beside`.

**The marks around those calls are not shared.** They are written out by
hand at every site, and the sites have multiplied. The difference between
`held` and `held_beside` is not difficulty; it is that both paths hold
the same inputs for `held` — a work's records, which the merged row
carries whole — and do not for `held_beside`, which needs a reference
that the batch may not be carrying. That is the whole of it.


### 1.2 The inventory

Every derived fact the two paths compute independently, what it means,
where each computes it, and whether they can drift. The marks first, then
the rules around them.

Line numbers are the working tree as read on 2026-09-19, while
`crates/merge/src/build.rs`, `record.rs` and `crates/serve/` were being
edited in other sessions — among them the patch to `citers_of` named
below. Read them as pointers to a function, not as addresses; the
function names and the doc comments quoted here are what will still be
findable.

#### `in_text` — the citation text repeats *this* name

The reference's own citation string contains this name, compared
case-insensitively as ASCII. Only a DOI is ever written out beside the
identifier it names, so it is false for every other namespace by
construction. It exists because §6 asks the text *before* the registry's
order, so a reader settling a reference again needs to know what the text
said.

| path | site |
|---|---|
| full | `build.rs:1286-1289` (`backs`), consumed at `:1297` and `:1334` |
| batch | `incremental.rs:481-487` (`repeats`), consumed at `:522`, `:2132`, `:2323` |
| batch | `incremental.rs:775`, `:1123`, `:1238-1243`, `:2399` — **copied from a stored row** |

**Drift: no**, on the predicate. Both bottom out in the shared
`build::repeats` (`build.rs:762-768`), and the DOI-only gate is written
identically twice. The copying sites are only as right as the row.

#### `held_beside` — the reference also named a work we hold

Some name of this reference resolves to a **held** (non-stub) work, so
the reference contributes no stub name of its own: its other names are
assertions about that work, not a work (merge plan §11 item 14). §6 can
still land it on a stub over a held work the order would prefer, which is
why the citer is kept at all (`crates/merge/src/record.rs:105-113`).

| path | site |
|---|---|
| full | `build.rs:1292-1294` → `stub_cited` at `:1161` |
| batch | `incremental.rs:467-477`, called at `:2136` and `:2324` |
| batch | **`incremental.rs:777` — hard-coded `false`** |
| batch | **`incremental.rs:1246` — hard-coded `true`** |
| batch | **`incremental.rs:2402` — hard-coded `true`** |

**Drift: yes.** `incremental.rs:777` (`citers_of`) is the open bug — item
26's blind spot, which another agent is patching today. Its comment
reasons the value away:

> The reference landed on this work, which is held; what else it named is
> item 26's question, not this one.

The mark is defined over **all** the names the reference gave
(`build.rs:1292-1294`), but `citers_of` sees only the divided row, and
`gave` / `took_a_part` (`incremental.rs:1119-1129`) is built solely from
`row.names`. §4 walks the failing shape.

The two `true`s are defensible in their own blocks: at `:2402` the
reference has just had its arc moved off a held work; at `:1246` it gave
a name a split part took. But they are constants where the full path runs
a computation, so they are C2's class with the sign flipped — and the
comment at `:1246` asserts a claim about the *next* division ("and will
not of the next division either") that nothing checks.

The blast radius is larger than the one line. `incremental.rs:1152` — the
split path's test for whether a leftover name is a name of its own —
reads `!c.held_beside` on citers that, for a row that was **held**, came
through `citers_of` at `:1106-1112` and are therefore always `false`. The
test passes unconditionally on that branch.

#### `by_text` — the citation text backs the **landing**

Do not read this one off the identifier. `by_text` is not "this name is
in the text"; that is `in_text`. It says **the reference's text repeats
some DOI that names the work the reference landed on** — a property of
the landing, replicated onto every row of that reference. So a row with
`by_text: true` says the arc's current home is corroborated by the
publisher's own deposit, which is why a rival name arriving later must
beat text and not merely rank (`record.rs:415-418`).

| path | site |
|---|---|
| full | `build.rs:1318-1320` — `any(backs(n) && resolve(n) == Work(to, stub))` |
| batch | `incremental.rs:507-510` — `any(backs(n) && matches!(target(n), Work(w,_) if w == to))` |

**Drift: cosmetic.** The predicates are textually different — the full
path pins the stub flag to the landing's (known `false` in that branch),
the batch wildcards it. Equivalent, because stubness is a function of the
work id. Two expressions of one rule, maintained apart.

#### `beside` — the name named no work when the reference was read

Only such a row can carry an arc away when a later batch gives that name
to a work (serving plan §11 item 26).

| path | site |
|---|---|
| full | `build.rs:1332` — `self.resolve(name) == Target::Unknown` |
| batch | `incremental.rs:517` — `target(name) == Target::Unknown` |

**Drift: only through the resolver.** Same formula; different resolvers —
`Builder::resolve` over the run's arena against `target_of` over the
batch table, `Live`, then the epoch's fst (`incremental.rs:2679-2693`). A
`Resolved::Split` answers `Target::Unknown` in the batch
(`incremental.rs:2692`); the full merge has no split concept and,
re-resolving from scratch, may answer `Work(..)`.

#### `landed` — the namespace that won the landing

| path | site |
|---|---|
| full | `build.rs:1310-1314` — `min_by_key(rank)` over names resolving to the landed work |
| batch | `incremental.rs:497-501` — the same, in `beside_rows` |

**Drift: no.** Both call the shared `build::rank`. The loop is duplicated;
the rule is not.

#### `sole` — was this the pair's only reference landing there

| path | site | how |
|---|---|---|
| full | `build.rs:1422-1431`, in `finish` | not twice in the whole run's raw edge list |
| batch | `incremental.rs:2470` | `landings.get(&(from, by, to)) == Some(&1)` |

**Drift: yes, confirmed, on relation rows.** These are structurally
different computations, not two copies of one. `landings` is incremented
only on non-stub **reference** edges (`incremental.rs:2044`, `:2146`,
`:2334`). A relation-only row — pushed with `sole: true` at
`incremental.rs:2080` and `:2189`, and at `build.rs:1194` in the full path
— has no `landings` entry, so the batch's lookup returns `None` and
writes `sole: false`; the full path's `twice` set does not contain the
key either, so `build.rs:1430` leaves `sole: true`. Same row, opposite
byte.

A second, narrower difference: the batch recomputes `sole` only for pairs
in `put_cited_as`, so a row whose pair the batch did not restate keeps a
`sole` computed against an older corpus. The full merge recomputes all of
them.

Inert today — the `!row.beside` guard at `incremental.rs:2372`
short-circuits before `sole` matters, and relation rows have `beside:
false` — so this is a stored-byte divergence rather than a live one. It
is invisible to the harness (§1.3).

#### A stub's row shape — `standing`, `by`, `native`

One `Promoted` name per namespace, first in name order, the rest `Alias`;
a stub name's `Sources` are exactly its citers' sources; `native` always
false.

| path | site |
|---|---|
| full | `build.rs:1881-1917` (`build::stub`), standings at `:1883-1893`, `by` at `:1896-1898` |
| batch | `incremental.rs:1693-1725` (`stub_row`), standings at `:1699-1705`, `by` at `:1707` |

**Drift: none today, and the whole of §3.4's promotion rule written
twice.** They agree because both iterate names in sorted order — the full
path because `Resolution::stub_works` sorts
(`crates/merge/src/identity.rs:1583-1585`), the batch because it holds a
`BTreeMap<OwnedName, Vec<Citer>>` — over the same `OwnedName: Ord`. It
will drift the moment either is edited.

#### "Does this reference make a stub at all?"

| path | site |
|---|---|
| full | `identity.rs:849-874` (`cite_in`) — a stub is interned only when `held.is_empty()`, and all unheld names union into one stub |
| batch | `incremental.rs:1850-1870` (`classify`) + `:2672-2675` (`names_a_stub`) |

**Drift: none; the rule is written twice.** Both treat `Target::Shared` as
held. The grouping union-finds are duplicated too:
`identity.rs:1313-1327` and `:1572-1587` against `incremental.rs:2656-2663`
(`root`), `:1669-1687` (`Stubs::groups`) and the leftover loop at
`:1171-1195`.

#### §6's "text first, then rank", applied to a stored row

The composition — text beats order, order decides the rest — lives once
in `resolve_reference`. The batch re-encodes it twice more, over rows
rather than references:

| path | site |
|---|---|
| full | `build.rs:699-756`, with `corroborated` at `:771-793` and `rank` at `:817-827` |
| batch | `incremental.rs:2366-2372` — item 26's arc move, ranking a stored `in_text` / `by_text` pair |
| batch | `incremental.rs:1531-1576` (`split_arc`) — the division, over the row's own names |

**Drift: structurally possible.** `resolve_reference` runs `corroborated`
over *all* the reference's names and refuses a text corroborating two
different works (`build.rs:784-787`). Item 26's encoding compares exactly
two things — the arriving name against the stored `landed` namespace —
from stored booleans, and never re-runs `corroborated`. `split_arc`
restricts the contest to the divided row's names, which
`incremental.rs:1518-1528` argues is sound and which serving plan §11 item
28 proves; it refuses the shapes the proof does not cover rather than
guessing. `rank` cannot drift; the composition around it is written three
times.

#### A held row's text

What the row carries and the lexical index reads.

| path | site |
|---|---|
| full | `crates/merge/src/document.rs:225-353` — streams the sets, text from the speaking copy, an older copy filling only what the speaking copy left empty (`fill`, `:314-353`) |
| batch | `incremental.rs:1332-1406` (`text_for`) — per field, the source's promoted record else its first, then that record's text whole |

**Drift: plausible, and this one is already the record index's business.**
Both agree on *which source* supplies each field — shared
`merge::document::abstract_source` (`document.rs:135-140`) and the
`fields.authors.from` / `fields.venue.from` stamps. They differ on which
*copy* of that source's record supplies the bytes and on the older-copy
fallback: the full path merges across copies field by field, the batch
picks one record and takes its text. `text_for` is exactly the function
that reads the normalized layer today (§2.2), and it is one of the three
cases item 22 was written for.

#### Lookup-level facts derived two ways

Not marks on a reference, but the same shape of problem: one fact, two
derivations.

| fact | full | batch | note |
|---|---|---|---|
| a name is **shared** | `crates/serve/src/names.rs:189-208` — inferred from a key naming more than one work | `crates/serve/src/fold.rs:80-96` — read off `Standing::Shared` | different derivations; agree only because `names_table` (`build.rs:852-869`) enforces shared-xor-held |
| a work is a **stub** | `names.rs:169-182`, a bitset from `row.stub` | `fold.rs:184-188` → `core.rs:594-601`, where `Retired`/`Dropped` answer `false` | two stores, one fact |
| **public id** | `crates/merge/src/public.rs:194-223` — collides only against the public log | `incremental.rs:2699-2728` — also rejects any key the serving fst knows, including an ordinary name | **the acceptance rules genuinely differ**; the hash cannot, both call `public::derive` |
| **edge label** | `build.rs:1433-1446`, rebuild | `incremental.rs:1734-1749` + `fold.rs:351-364`, add/remove over a base | two algebras; compared by the harness |
| a **relation** keeps a name alive | `identity.rs:833-836` | `incremental.rs:800-805` → `:1156-1160` | different inputs, same intent |

#### The whole reference pass, written five times

Beneath the individual marks sits the larger duplication. The full path's
`Builder::reference` (`build.rs:1265-1348`) is 84 lines. The batch has the
same work spread over four blocks:

- `incremental.rs:2022-2090` — `kept_of`, for records a work keeps that the
  batch does not carry
- `incremental.rs:2094-2205` — the batch's own records
- `incremental.rs:2295-2345` — `cited_only`, a copy superseded by a newer
  one whose references still count
- `incremental.rs:1100-1265` — the split path, which resolves references
  again out of stored evidence rather than out of records

`kept_of` is worth singling out. At `incremental.rs:2043-2057` its
`Referenced::Edge` arm handles `!stub` and does nothing at all for a stub
landing — no `Citer` is pushed. That may be right, a kept record's citers
being already in the stub's row and untouched. But `stale`
(`incremental.rs:2243`, `:2442`) removes a pair's citations from every
stub *before* the landings restate them, and a kept twin of a deleted
record shares its `(work, source)` pair. **?** — unverified, and neither
sweep raised it independently. The probe: a work with two records of one
source, one deleted in the batch, the survivor citing a stub; merge both
ways and diff the stub's citers.

#### What cannot drift, and why it matters

For completeness, and because it is the argument of §6: everything below
is **one function called by both paths**, and none of it has drifted.

`merge::build::held` (`build.rs:1666-1878`) — every promoted field,
genre precedence included, plus flags, name standings and rejected
assertions. `merge::build::resolve_reference` (`:699`). `join_citers`
(`:796`), called from `build.rs:1400` and `incremental.rs:793`, `:1252`,
`:1706`, `:1818`. `rank` (`:817`). `repeats` (`:762`).
`merge::publisher`'s acceptance and canonical spelling, which runs below
both paths inside `Name` equality. `merge::copies::Delivery::older_than`
(`copies.rs:63`), the speaking-copy rule. `merge::document::Document::of`
(`document.rs:112-130`), reached by the batch through `Row::document()`
(`crates/serve/src/row.rs:149-156`). `merge::title::agree` and
`Form::admits`, reached only through `held`.

Two more things that are *not* dual-computation sites, so that the
inventory is not read as longer than it is. The reference-outcome and
`Tallies` counts are produced inside shared code and only aggregated
differently (`build.rs:1345` against `incremental.rs:2110-2115`), and
`Applied` (`incremental.rs:250-315`) is a per-batch ledger with no
full-merge counterpart. The artifact manifests are computed only on a
full rebuild; the batch appends deltas and does not maintain them.

### 1.3 What the inventory says

Six marks on a reference, three more rules written twice around them, and
five lookup-level facts derived two ways. Against **one** full-path site
each, the batch has **fourteen** sites that write a mark: five copy a
stored value, three write a constant, and six compute. Three of those
computations — `sole`, the public-id collision test, a held row's text —
are structurally different from the full path's, not copies of it.

Four things follow, and the last is the one that decides the question.

**First, the constants are not the disease.** Each was written by someone
reasoning correctly about one block. The disease is that the reasoning is
*local* and the mark is *global*: `held_beside` is a statement about the
whole names table, and every site that writes it has to re-derive, by
hand, what the table says about a reference it may not be holding.

**Second, a stored mark is a cache of a fact about a table that has since
moved.** `Citer.held_beside` on a stub's row was true when written. If a
later batch gives one of that reference's other names to a held work, the
reference now names a held work and should stop making a stub name — and
nothing revisits the stored citer. The batch updates only the citers it
re-resolves. The marks go stale even where every site computes them
correctly.

**Third, the shape of the fix is already in the tree.** `merge::build::
held` decides genre precedence, title, access, status, flags and the
rejected assertions in one function both paths call, and it has never
drifted. It can be one function because both paths hold the same input —
a work's records, which the merged row carries whole. The marks drift
because they are computed *around* the shared call rather than inside it,
and they are computed around it because the batch has a row where it
needs a reference.

**Fourth — and this is load-bearing — the diff harness is not a safety
net for all of it.** `serve::compare::differences`
(`crates/serve/src/compare.rs:31`) walks rows by id, names by key, every
work's lists, `public_of`, `is_stub`, name resolution and six fixed
lexical queries. A row's `names` are `row::Named`, which carries
`cited_by`, so **every `Citer` field is compared byte for byte**
(`compare.rs:52-92`) — which is why both of today's bugs were caught. But
`compare.rs` never touches the `cited_as` set (**M**: `grep -n cited_as
crates/serve/src/compare.rs` returns nothing). So the confirmed `sole`
drift above is invisible to it, and so is the one the serving plan
already records, at §11 item 28's close: when a reference's arc moves and
lands on a *held* work, no cited-as row is restated, and the old row still
names the work the reference landed on before —

> `serve::compare` does not walk the cited-as set, so no hold-out would
> show it.

That is a fourth bug of the same class, standing open and invisible to
the test that caught the other three. Its stated cause is exactly the
record index's absence: "the batch does not hold a record it is not
carrying."

---

## 2. What a record index actually is

### 2.1 What it keys on and what it holds

**Key:** `(SourceId, native id)` — the pair that already identifies a
record everywhere else in the system. `OwnedRecord`
(`crates/merge/src/build.rs:142-153`) carries `source` and `native`, and
its `view` carries the `set` the copy came from
(`merge::copies::prefix_name`, `crates/merge/src/copies.rs:322-329`).

**Value:** where the record's bytes are — the set, and a position inside
it precise enough to start decoding there.

**What it is for:** one operation, `read one normalized record by native
id`, which merge plan item 22 names and which the sets, being streams,
cannot do. Everything in §1 follows from having it: a path that can fetch
a reference can *compute* its marks instead of copying or guessing them.

The reverse direction matters too and is nearly free: `native id → work`
at the last full merge, which is what `self.previous` reconstructs today.

### 2.2 The obstacle nobody has written down yet

Item 22 says "a per-source index from native id to set and block". **There
are no blocks.** A normalized set is one file per kind, and the whole file
is a single zstd frame at level 3 (`crates/ingest/src/binary.rs:49`,
encoder at `:256`), inside which records are varint-length-prefixed
postcard values read strictly front to back (`binary.rs:305-314`,
`:562-589`). `Reader::open` (`binary.rs:608-609`) inflates the entire file
into a `Vec<u8>` and *also* recursively inflates the sibling symbols file
(`:631-643`), because every interned string in a record is a `u32` index
into it.

Measured (**M**, `zstd -l` and `zstd -dc … | wc -c` on
`data/normalized/v2/pubmed/baseline/pubmed26n1333.*`):

| file | frames | compressed | inflated | time |
|---|---|---|---|---|
| `.works.zst` | 1 | 27.2 MiB | 100,146,937 B | 0.09 s |
| `.references.zst` | 1 | 26.8 MiB | 72,778,344 B | 0.07 s |
| `.symbols.zst` | 1 | 395 KiB | 1,083,281 B | ~0.00 s |

One frame each — so the format itself forbids a seek. This is what
`Records::record` costs today. The trait is at
`incremental.rs:154-168` (the serving plan calls it
`serve::incremental::Texts` at §5; that name is stale, it is `Records`),
and the real implementation is `record_in::<S>` at
`crates/ol-rht/src/main.rs:1580-1650`: it opens the one set the row named,
inflates it whole, and scans every record comparing native ids
(`main.rs:1598-1602`), then opens and scans the references file the same
way (`main.rs:1645+`).

**A** — one record read costs, on that set:
100,146,937 + 1,083,281 + 72,778,344 + 1,083,281 = **175,091,843 bytes
inflated to keep one record**, about 0.16 s of zstd alone. Compare
serving plan §11 item 31's 25.6 MiB to keep 349 KB, which is 75×; this is
of a different order.

So an index alone buys nothing. The set must become seekable, and there
are two ways, one of which is far cheaper than it looks.

**(a) Re-frame the set as concatenated zstd frames.** Write a frame per
*N* records instead of one frame per file. **M** — verified here with the
`zstd` CLI, in the scratchpad: two files compressed separately, their
bytes concatenated, `zstd -dc` on the result printed both payloads in
order. So the *format* guarantees that concatenated frames read as one
stream, and a re-framed set would be byte-for-byte equivalent to a reader
that inflates the whole file. `Reader::open` uses the `Read` form of
`zstd::stream::decode_all` (`binary.rs:608-609`), which is the streaming
decoder and should follow the format — **?**, and it is a one-line test,
not an assumption: concatenate two set files and open the result through
`Reader::open`. If it holds, **every existing reader keeps working
unchanged** and re-framing is invisible below the index.

The index then holds a byte offset into the compressed file (a frame
boundary) and an ordinal within the frame, and a lookup inflates one
frame rather than one file. The symbols file stays whole and small (26 MB
compressed across the entire sample — **M**, `find data/normalized/v2
-name '*.symbols.zst' -exec du -ck {} +`).

This still needs a re-normalization, because the frames must be written.
The raw layer is intact, so it is a replay, not a loss. **?** — its cost:
time `ol-rht pubmed normalize` over the raw layer with `--out` pointed at
a scratch directory.

**(b) Do not touch the normalized layer: mirror the records the merge
already read.** The full merge reads every record of every set four times
in pass 1 alone (`merge_copies`, `merge_add`, `merge_titles`,
`merge_targets` — `crates/ol-rht/src/main.rs:1094-1105`, `:1121-1126`).
It can write, as it goes, a **record store** under the merged directory:
the records, re-framed and indexed, in the merged layer's own framing.

The reason this is sufficient, and it is a real argument rather than a
convenience: a batch only ever needs a record it is **not** carrying —
one outside the batch. Every such record was present at the last full
merge by definition, so a merge-built mirror is complete for the batch's
purposes and stale only in ways the batch already holds the newer copy
of.

**With one gap, which must be closed or the option fails.** A record
delivered by a *previous* batch since the last compaction is in the
normalized layer but not in the mirror. The batch after it may want that
record's reference list — `citers_of`'s case exactly. The fix is the
shape the layer already uses five times over: the mirror gets a delta of
its own, each batch appending the records it carried, folded like the
other five and compacted at the next full merge (serving plan §3, §4).
That is one more append-only log and one more line in the journal, not a
new idea; but it is work option (a) does not need, because a batch's sets
are normalized before it runs and are therefore already seekable.

Option (b) needs no `model` bump, no re-normalization, and no change to
`ingest::binary`. It costs a second copy of what it mirrors (§3) and the
delta above.

### 2.3 Where it lives

Under the merged directory, beside the sets, because it is the merge's
own product and dies with the merged layer:

```
merged/v<model>.<merge>/records/<source>.{fst|keys|slots}
merged/v<model>.<merge>/records/manifest.json
```

`merged/` is `Store::merged` → `merged/v{model}.{merge}`
(`crates/ingest/src/store.rs:103-105`); today `merged/v2.8/`
(`model::VERSION = 2`, `merge::VERSION = 8`). Adding it means one accessor
in `store.rs`, which is the only place in the tree that speaks paths, and
— if it is to be part of an epoch — one line in the artifacts map at
`crates/ol-rht/src/main.rs:2710-2716`.

Three conventions it must copy, all already enforced elsewhere:

1. **Name the directory for the versions it depends on and refuse on
   mismatch in both directions** — write refuses a wrongly-named directory
   (`crates/merge/src/layout.rs:84-90`), read refuses a directory not
   named for this binary (`layout.rs:208-233`).
2. **Write into `<dir>.part`, manifest last, then rename over the
   target** (`crates/serve/src/names.rs:138-141`,
   `crates/serve/src/works.rs:314-318`), so an artifact is never half
   published and a rebuild is byte-identical — which `ol-rht epoch check`
   depends on.
3. **Digest every file** — `FileInfo { bytes, sha256 }`
   (`crates/serve/src/digest.rs:14-21`), and `Mapped::open` re-checks the
   byte length on every open (`crates/serve/src/mapped.rs:20-38`).

Whether it belongs in an **epoch** at all is an open question (§9). It is
not read to answer a query; it is read by the merge. The argument for
putting it in the epoch is that a batch reads it, and a batch must not
read an artifact from a merged layer the epoch does not name.

### 2.4 The manifest, and the trap

The canonical shape, from the three that exist
(`crates/serve/src/works.rs:41-74`, `names.rs:47-80`, `cited.rs:129-148`):
the version triple that names the directory, the journal batch, `built_at`,
any shape constant the binary must match, one count per thing the build
tallied, and `files: BTreeMap<String, FileInfo>` — a `BTreeMap` so the
JSON is sorted and the rebuild is byte-identical.

Counts the record index should carry: `records` per source, `sets`,
`frames`, `frame` (the records-per-frame constant, checked like
`works.rs`'s `block`), and `raw_bytes`.

**The trap, made today on the names artifact.** Every manifest in the tree
is `#[serde(deny_unknown_fields)]`. A new *required* field therefore
breaks an older base the moment a newer binary reads it — and worse, the
failure is uninformative, because **the manifest is parsed before its
version is checked**, so a parse error cannot say which version the base
is. The `split` field is the worked example, and the comment on it
(`crates/serve/src/names.rs:70-77`) is the rule:

> Defaulted, and `SPLIT` is not required below, so that a base written
> before the file existed still reads and simply answers with no parts —
> what the layer answered anyway until 2026-09-19. A manifest is read
> before its version is checked, and a parse error there says nothing
> useful about which version it is.

The fix was three coordinated places, and a new artifact must copy all
three: `#[serde(default)]` on the field (`names.rs:76`), the file left
**out** of `check`'s required list (`names.rs:116`), and the open guarded
by `manifest.files.contains_key(...)` (`names.rs:399`). It is the only
`#[serde(default)]` in the repository. Note what it does **not** fix: an
*old* binary reading a *new* manifest still fails on `deny_unknown_fields`,
and nothing but a version bump addresses that direction.

---

## 3. Size and cost, measured

The pass-1 report the design is sized against, verbatim (**R**):

```
stubs: 5079128 works from 5870063 names cited and held by no work
  (doi 2071850, pmid 1713606, pmc 304527, openalex 1780080);
  by names per stub: 1: 4513264, 2: 342804, 3: 221821, 4+: 1239
5238452 ids minted in all
30.5 s, 87640842 allocations
```

Against 148,199 held works; the works artifact on disk at batch 6 reads
`held: 148210` (**R**, `data/works/v2.8.5/manifest.json`), the eleven
being what the batches added.

### What is on disk now (all **R**)

| | |
|---|---|
| `names/v2.8.5` | `nodes` 5,238,452 · `keys` 11,534,123 · `names.fst` 151,577,546 B |
| `works/v2.8.5` | `works` 5,227,338 · `held` 148,210 · `stubs` 5,079,128 · `block` 256 · `blocks` 20,463 · `raw_bytes` 780,831,942 · `blocks.zst` 253,112,176 B |
| `cited/v2.8.5` | `keys` 58,641 · `rows` 229,439 · `cited.rows` 2,753,268 B |
| registry | 1,271,346 JSON lines at ~94 bytes |

**A** — the registry: 1,271,346 × 94 = **119,506,524 bytes**, 114 MiB.
Merge plan §11 item 15 gives 379 MB for the sample's four million ids.

**A** — the cited-as row is 12 bytes (`crates/serve/src/cited.rs:36`), and
229,439 × 12 = 2,753,268, which is `cited.rows` exactly.

### The normalized layer (all **M**)

`find data/normalized/v2 -name '*.zst' | sed 's/\.[a-z]*\.zst$//' | sort -u | wc -l`,
per source:

| source | sets | |
|---|---|---|
| pubmed | 85 | |
| crossref | 1,012 | |
| openalex | 915 | |
| **total** | **2,012** | four kinds each: works, references, deletions, symbols |

Compressed bytes by kind, whole layer: works 302,824 KB, references
401,316 KB, symbols 26,392 KB, deletions 8,048 KB — **738,580 KB**, 722
MiB. Records in it: **242,629 distinct** (**R**, `metadata-quality.md` §1,
measured over the whole normalized layer on 2026-09-19).

### Sizing the index

An entry, fixed part: set ordinal `u32` (4 B — 2,012 sets today, `u16`
would do and will not at corpus scale) + frame offset `u64` (8 B) +
ordinal within frame `u16` (2 B) = 14 B, **A** padded to 16.

**A** — 242,629 × 16 = **3,882,064 bytes**, 3.7 MiB for every record in
the sample's normalized layer.

The keys. PubMed and OpenAlex natives are integers and want a sorted
array; Crossref's are DOIs and want an FST, the structure `names` already
uses. **A** — from the names manifest, 151,577,546 ÷ 11,534,123 = **13.1
bytes a key** measured on this corpus's own names; 242,629 × 13.1 =
**3,178,440 bytes**, 3.0 MiB if every key went into an FST.

**A** — the whole record index on the sample: 3,882,064 + 3,178,440 =
**7,060,504 bytes, 6.7 MiB.** Next to a 722 MiB normalized layer and a
241 MiB works base, it is not a cost.

**A** — the per-record figure to carry forward is 16 + 13.1 = **29.1
bytes a record**. **?** — the corpus total is that times the corpus's
record count, which is not measurable: no source is fully ingested. The
number to fill in is the one G3 waits for.

The **record store** of §2.2(b) is a different order. It mirrors what it
holds, so on the sample it is bounded above by the 722 MiB the normalized
layer occupies, and below by whatever a trimmed record costs — names,
references with their citation text, and the view's text, without markup.
**?** — measure by writing a mirror in a scratch directory during one
`merge identity --survey` run and taking its size; §7's first slice does
exactly this.

### How it reads, without repeating the works index's mistake

Serving plan §11 item 31's measurement — hydrating 100 held rows
decompresses **25.6 MiB to keep 349 KB** — is a block-size problem, not a
lookup-count problem. Mechanically: a slot's position is fixed by
`WorkId` (`works.rs:450`, `ix / BLOCK`), so the index cannot group rows by
what a reader wants; ids are minted held-first, so held rows cluster; a
block of 256 stubs is a few kilobytes while a block of 256 held works is
~687 KB (serving plan §11 item 13); zstd has no random access inside a
frame, and the slot offsets are written *inside* the compressed payload
(`works.rs:157-168`); and the cache is one block deep (`works.rs:346`).
**A** — 25.6 MiB ÷ 687 KB ≈ **39 fat blocks touched** for a hundred rows,
349 KB kept.

Three properties keep the record index off that rake:

1. **Frames sized for the read, not for the id space.** The read is "one
   record"; the frame is the unit of inflation. A frame of 64 records at
   PubMed's measured density — 100,146,937 B inflated over one baseline
   set — is **?** until the set's record count is known
   (`ol-rht pubmed dump data/normalized/v2/pubmed/baseline/pubmed26n1333.works.zst
   | wc -l` counts it). The point is that the frame size is a knob the
   index owns, and `binary.rs:49` already says of the current one that
   "the level and the frame size are knobs not yet turned".
2. **The offsets live outside the compressed bytes**, in the index, so
   finding the frame costs no inflation at all. That is precisely the
   inversion the works index got wrong.
3. **The keys are memory-mapped and the frames are not cached.** 6.7 MiB
   of index maps; the frames are read once each and dropped. On a 128 GB
   box shared with the search engine, the index is resident and the
   working set is one frame.

---

## 4. How each of today's three bugs would have gone

### Bug 1 — `write_stub_rows` skipped duplicate citers

`incremental.rs:1757`. A record's several references to one stub are one
citer, and the marks join: `in_text` by `|=`, `held_beside` by `&=`
(`build::join_citers`, `build.rs:796-812`). The batch kept the first citer
and dropped the rest, losing a later `true` on `in_text` and a later
`false` on `held_beside`. Fixed by extend-then-join
(`incremental.rs:1815-1819`).

**Would the record index have prevented it? No — and this is the most
useful thing in this section.** This bug is not about reading a record;
it is about *combining* the marks of references the batch already held.
The fix was to call the shared `join_citers`, and the shared function was
already there to call. The record index buys nothing here.

What would have prevented it is the smaller half of §7: one function that
takes a record's whole reference list and returns its citers already
joined, so that "join them" is not a step a caller can forget.
`join_citers` has **five call sites** today — `build.rs:1400` and
`incremental.rs:793`, `:1252`, `:1706`, `:1818` — each a separate chance
to omit it, and one of them did. The fix that landed added a sixth caller
rather than removing a chance.

### Bug 2 — a reference can name a held work and still land on a stub

Nothing recorded it until `held_beside` was added to `Citer`. §6 asks the
citation text first and the registry's order second, and either can land a
reference on a stub although the reference also named a held work — so
"landed on a stub" stopped implying "named no held work" the moment
version 6 landed (`crates/merge/src/lib.rs:80-82`).

**Would the index have prevented it? Partly, and not by itself.** The
missing thing was a *field*, and a field is missing whether or not you can
read records. What the index changes is that the field need not have been
added: under §7's slice 3, a path wanting `held_beside` for a citer
fetches that citer's record and re-runs the rule, and the answer is
correct against the table *as it stands now* rather than as it stood when
the row was written. That is strictly better than the field, because the
field goes stale (§1.3) and the recomputation cannot.

So: the index would not have stopped the bug being introduced. It would
have made the class of it — "a mark nothing recorded" — unreachable,
because nothing would need recording.

### Bug 3 — `citers_of` hard-codes `held_beside: false` (open)

`incremental.rs:764-795`. The batch wants the citers of a name while it
belonged to a held work — the case where a work loses its last record and
becomes the stub its citers name (serving plan §11 item 28). It
reconstructs them from `cited_as` rows, which carry `in_text` and
`by_text` but not `held_beside`, so it writes `false`.

**Would the index have prevented it? Yes, cleanly, and this is the one
that makes the case.** Walk the real shape through:

A Crossref record **C** cites a reference giving three names: `doi:10.
1/x`, `pmid:222` and `pmc:PMC9`. At the last full merge, `doi:10.1/x` is
held by work **#40**; the PMID and PMC id are held by nobody.
`resolve_reference` (`build.rs:699`) finds the three names resolve to one
work, returns `Edge { to: #40, stub: false, settled: Agreed }`, and
`build.rs:1322-1341` writes three `cited_as` rows: all with `to: #40`,
`landed: Doi`, `by_text` computed once at `:1318`, and `in_text` per name
— true only on the DOI if C's citation string spells it out. The PMID and
PMC rows are marked `beside: true`; the DOI row is not. No `Citer` is
written anywhere, because the landing was held.

Now a batch withdraws **#40**'s last record. The work must keep its id and
become the stub its citers name (item 28). The batch asks `citers_of(doi:
10.1/x, #40)`, which finds C's row, checks the arc still exists, and
builds `Citer { from: C, by: Crossref, in_text: <from the row>,
held_beside: false }`.

Is `false` right? Only if no *other* name of C's reference is held now.
`citers_of` cannot tell: it has one row per name, it cannot group the
three rows back into the one reference they came from (two references of
one record to one work are indistinguishable in that set), and it has no
access to C's reference list. If a batch has since given `pmid:222` to a
held work, the truth is `true`, C makes no stub name, and the full merge
will not count it — while the batch does. The next compaction diverges,
and because the divergence is in a stub's citers, `serve::compare` *would*
catch it — after the fact, in a hold-out, on a corpus that happens to
contain the shape.

Under the record index: `citers_of` holds `(Crossref, C)` — that pair is
in the `cited_as` row — fetches C's record from the index, walks its
reference list, finds the one reference whose names include `doi:10.1/x`,
and calls the same `resolve_reference` the full merge calls, against the
live table. `held_beside` comes out of that call. There is no second
computation to keep in sync, and the answer is right for the table as it
is, not as it was.

The same fetch fixes `incremental.rs:1152`'s unconditional test, and it
fixes the fourth, open, invisible bug of §1.3 — the cited-as row that goes
stale when an arc moves — because restating the row needs exactly the same
record.

The division has the same shape with a sharper edge, and it is the one
that reaches a comparison. A reference gives `pmid:X`, which is this row's
name and becomes a leftover, and `doi:Y`, which some *other* held work has
and which this division never touches. The full merge re-resolves the
reference after the split: `pmid:X` → the new stub, `doi:Y` → still held,
so `held_beside` is **true** on the stub's citer. The batch's
`took_a_part` never sees `doi:Y` — it is not a name of this row — and
`c.held_beside` is the hard-coded `false`, so the citer is minted with
`false`. Same citer, opposite mark, on a freshly minted stub whose
`cited_by` `compare.rs:52-60` does compare.

---

## 5. What it costs

**Build time.** Pass 1 already sweeps every set four times
(`main.rs:1094-1105`, `:1121-1126`) and the whole first pass is 30.5 s
(**R**). Writing the index rides the first sweep, which already decodes
every record and already knows its set. **?** — the delta: run
`ol-rht merge identity --store <root> --survey` before and after and
compare the reported seconds. **That `--survey` is not optional.** Without
it — and without `--limit` — `merge identity` calls `merge_commit`
(`main.rs:1207-1232`) and **appends to `merged/registry.jsonl`, the store
of record**. `--survey` makes it print "none written".

**Disk.** 6.7 MiB for the index on the sample (**A**, §3). The record
store of §2.2(b) is bounded above by 722 MiB (**M**) and its real size is
**?**. Against a 128 GB box shared with the search engine, the index is
free and the store is a decision.

**Memory in pass 2.** Already 5.0 GB resident on the sample (merge plan
§11 item 16), most of it the unresolved rows' citation text and the four
million works' rows. The index adds nothing to pass 2 *if it is written in
pass 1 and streamed* — the keys accumulate, and 242,629 keys is nothing
against five million rows. **Flag, do not assume:** another agent is
planning pass 2's memory today, and the shape they are considering — "the
unresolved stream could be written as it is read once shards are open per
range" — is the same shape this index wants. If pass 2 becomes ranged,
the index must be written per range and merged, or written in pass 1
where it belongs. Coordinate before either lands.

**Incremental-apply time.** A batch is 2.7 s on the sample's 4,989
records (serving plan §11 item 17), three fifths of it row reads. Each
record fetch through the index costs one frame inflation instead of
175,091,843 bytes (**A**, §2.2). How many fetches a batch makes is **?**
— it is zero today for every case that currently stops, and one per
`citers_of` citer under §7's slice 3, which is the number to watch. If
`citers_of` fetches per citer, a stub with many citers pays per citer;
the fix, if it bites, is to fetch per *record* and cache, which `Rows`
(`incremental.rs:735-753`) already does for rows.

**Migration.** This changes the merged layer, so it is a new `<merge>`
version — merge plan §8 says "any change to its rules or shapes bumps",
and version 8's own entry is explicit that even extending a constant
table counts (`crates/merge/src/lib.rs:87-99`). So `merge::VERSION` goes
to 9, `merged/v2.9/` is a new directory, and the artifacts become
`names|works|cited/v2.9.5`, `graph/v2.9.1`.

What happens to the published epochs: **nothing, and that is the design.**
A merged directory is a cache replaced whole by every run
(`crates/merge/src/layout.rs:1-8`); reads refuse a directory not named for
the running binary (`layout.rs:208-233`). `merged/v2.7` and `merged/v2.8`
are both on disk today and epoch 7 names one of them. The old epoch keeps
answering from artifacts that still exist, until the user publishes a new
one built on `v2.9`. The cost is a full merge and an epoch publish, both
the user's, and the disk for a third merged directory until serving plan
§11 item 15 decides when the old ones go.

**The one thing that is not free**, if §2.2(a) is taken rather than (b): a
re-framed normalized layer is a `model` version bump and a
re-normalization of the raw layer. That is `normalized/v3/`, every
downstream version with it, and the whole ingestion replayed. **?** —
time `ol-rht pubmed normalize` over one baseline file with `--out` to a
scratch directory and multiply by the set count. Option (b) avoids all of
it, which is why (b) is what §7 stages.

---

## 6. The alternatives, and why not

**Leave the marks derived; extract one shared function both paths call.**
The obvious move, and it is more than half right — §7 stages it as slice
2, and `merge::build::held` is the standing proof that it works for
everything harder than this. It fails *alone* for one reason, which §4's
walkthrough makes concrete: three of the fourteen batch sites **cannot
call it**, because they do not hold the reference. `citers_of` has a
`cited_as` row and no record. The split path has a stub's stored citers
and no record. Item 26's arc-move has a row. A shared function makes the
sites that *can* compute agree; it leaves the sites that cannot exactly
where they are, which is writing constants — and those are the sites the
open bug is in.

**Store the marks in the merged artifacts instead.** This is the cheap
patch, and it is *very* cheap. The cited-as row is 12 bytes
(`crates/serve/src/cited.rs:36`) with four of byte 11's eight bits used
(`cited.rs:84-96`), so `held_beside` there costs **zero additional
bytes** — and **A**, `Citer` is `WorkId(NonZeroU32)` 4 + `SourceId`
(`#[repr(u8)]`) 1 + `bool` 1 + `bool` 1 = 7 at alignment 4, which is 8
either way, so the field it already gained cost nothing in memory too.
It would close bug 3 this afternoon. Two reasons not to stop there. First, it is what the layer
already does, and the layer already has four marks and has grown one per
bug — `by_text` and `in_text` on 2026-09-18, `Citer.in_text` on
2026-09-19 (merge version 7), `Citer.held_beside` the same day. Each
addition is a merged-shape change and a full rebuild. Second and
decisively, **a stored mark is a fact about a table that has moved**
(§1.3): `held_beside` stored today is wrong tomorrow if a name it
depended on becomes held, and nothing revisits it. Storing the mark
converts a *sync* bug into a *staleness* bug, which the diff harness is
even worse at catching because a compaction rewrites the stored value and
the difference vanishes.

That said — take the free bits anyway, today, for bug 3. It is not
exclusive with any of this.

**A checking mode that runs both paths and diffs.** It exists. It is
`serve::compare::differences` (`crates/serve/src/compare.rs`), driven by
`crates/ol-rht/tests/incremental.rs` (one PubMed baseline file held out)
and `crates/ol-rht/tests/update_files.rs` (every update file held out,
one batch each), both under `OL_RHT_STORE`. It is how both of today's bugs
were found. So the honest accounting is not "cheap detection versus
expensive prevention" — the cheap detection is already bought and
running, and the question is what it misses. Two things: it does not walk
the `cited_as` set (§1.3), and it only finds shapes the sample happens to
contain. The first is not hypothetical — the `sole` disagreement on
relation rows is a confirmed divergence sitting in the merged layer right
now, and no hold-out has ever reported it. The right move here is not to
build a checking mode but to **extend the one that exists** to walk
`cited_as` — cheap, due whatever else happens, and it would make both the
`sole` drift and the stale-row bug visible. It still catches drift after
the fact rather than preventing it.

**Do nothing; patch instance by instance.** The honest case for it: three
bugs found and two fixed in a day, by a harness that works, on a system
nobody is serving from yet. The case against is the count in §1.3 —
fourteen batch sites against one, growing by roughly one site per rule —
and the fact that the fourth bug is already open and the harness cannot
see it.

---

## 7. The staging

Four slices, each of which stands alone and measures something.

**Slice 1 — the index, and nothing else uses it.** Take §2.2(b), because
it is reversible: write `merged/v2.9/records/` during pass 1 — the
trimmed records, framed, with keys per source and frame offsets, and a
manifest carrying `records`, `sets`, `frames`, `frame` and `raw_bytes`,
every file digested, `#[serde(default)]` on anything that might not exist
in a later base. Re-point `record_in::<S>` (`main.rs:1580-1650`) at it.
Change no mark, no rule, no shape but the new directory; no delta log yet,
so a batch still falls back to the scan for a record a previous batch
delivered.

Three numbers come out of it, and they settle the next two decisions:

- **`Records::record` latency before and after** — today 175,091,843
  bytes inflated (**A**) against one frame. This is the number that says
  whether any of this is worth doing. If it does not fall by two orders
  of magnitude, stop here and take slice 2 alone.
- **The mirror's size** against the 722 MiB (**M**) the sample's
  normalized layer occupies — which decides (a) against (b) at corpus
  scale.
- **The pass-1 delta** against 30.5 s (**R**), and the index's own size
  against the 6.7 MiB of §3.

**Slice 2 — one `Landing`, called everywhere.** A function taking a
record's whole reference list and a resolver, returning, per reference,
the edge and every mark, with the citers already joined — `held`'s shape,
applied to references. Replace `build.rs:1265-1348` and the four batch
blocks with calls to it. No index needed; no behaviour change intended.
Two more duplications are in reach the same day and should go with it:
`build::stub` (`build.rs:1881-1917`) against `stub_row`
(`incremental.rs:1693-1725`), which is §3.4's promotion rule written
twice, and `cite_in` (`identity.rs:849-874`) against `names_a_stub`
(`incremental.rs:2672-2675`). What it measures: that the two paths still
agree, by the existing hold-out harnesses — a real test, since the blocks
are not textually identical today (§1.2, `by_text`).

**Slice 3 — the three constants become fetches.** `citers_of`,
`incremental.rs:1152` and `incremental.rs:2402` fetch the record through
the index and call slice 2's function. Delete `held_beside` from `Citer`
if nothing reads it any more; keep it if the fetch is too slow and mark it
a cache with a checked recompute. What it measures: batch time against
2.7 s, and fetches per batch.

**Slice 4 — restate the cited-as rows an arc move leaves stale**
(serving plan §11 item 28's close), which needs exactly the same fetch as
slice 3, and settle `sole` on a relation row (§1.2) while the rows are
being rewritten anyway.

**Slice 0, orthogonal and due regardless of all of the above:** extend
`serve::compare` to walk the `cited_as` set, and spend the free bit on
`held_beside` in the cited-as row. Without the first, slices 2 to 4 are
being tested by a harness that cannot see the set they change most. Both
are an afternoon; neither blocks anything; the second is a merged-shape
change and so a full merge.

---

## 8. A recommendation, not a decision

**Build it. Prove it with §2.2(b), because (b) needs no `model` bump and
no re-normalization and is therefore reversible; expect (a) to win at
corpus scale, and decide between them with slice 1's number in hand, not
now. But do the two cheap things of slice 0 first, this week, because
neither waits on any of it.**

Why not commit to one today. (b) — a merge-built mirror under the merged
directory — is a `<merge>` bump and a day's work, and it proves or kills
the whole idea by slice 1. But it costs a second copy of what it mirrors,
and at corpus scale a second copy of the normalized layer on a 128 GB box
shared with the search engine is not obviously affordable; it also needs
a delta log of its own (§2.2). (a) — re-framed normalized sets — has
neither cost and is the thing the normalized layer will want anyway, but
it is a `model` bump, a full re-normalization of the raw layer, and every
downstream version with it. The measurement that decides it is a trimmed
mirror's size against the 722 MiB the sample's normalized layer occupies,
and slice 1 produces it as a side effect.

The cheap things: spend the free bit on `held_beside` in the cited-as row
(`cited.rs:84-96` has four of eight bits used, so it costs nothing), and
extend `serve::compare` to walk the `cited_as` set. Together they close
bug 3 and make the confirmed `sole` drift and the stale-row bug visible
for the first time. Both are an afternoon; both are a merged-shape change
or a test change, neither is an architecture.

The case for the index itself, stated as plainly as it can be. The
merged layer already proves that the shared-function answer works: every
promoted field of a held work, genre precedence and all, is one function
both paths call, and it has never drifted. The marks drift because they
sit *around* that call rather than inside it, and they sit around it
because the batch has a row where it needs a reference. Fourteen batch
sites against one, five copying a stored value, three writing a constant.
The record index is what lets those three become calls.

The case against, which is real. The harness works — it found both of
today's bugs the day they landed — and it is already bought. Nobody is
serving from this. A `<merge>` bump costs a full merge and an epoch,
which are the user's and which take hours. And §4 is honest that the
index would have prevented **one** of the three bugs outright: bug 1 was a
forgotten call to a function that already existed, and bug 2 was a missing
field. What the index changes is the *class*: it makes "a mark nothing
recorded" unreachable, because nothing needs recording.

The thing that tips it is §1.3's fourth point. The harness is not blind
by accident — it does not walk the `cited_as` set, and the serving plan
already records one bug living in that blind spot and now a second. A
harness that catches drift in what it walks, plus a layer that keeps
growing marks it must then keep in sync, is a pattern with a known ending.

What would change the recommendation: if slice 1 shows `Records::record`
latency does not fall by two orders of magnitude, the index is not
carrying its weight and the shared-function slice alone is the answer.

## 9. Open questions — the user's to settle

1. **Mirror or re-frame** — §2.2(b), a merge-built record store under the
   merged directory, against §2.2(a), re-framed normalized sets. (b) is a
   `<merge>` bump, a second copy of the records, and a sixth delta log,
   because a record a previous batch delivered is in no earlier mirror.
   (a) is a `model` bump and a full re-normalization of the raw layer,
   with every downstream version following, but no duplication and no
   delta. The argument for (b) is that it is reversible and proves the
   idea in a day. The argument for (a) is that the normalized layer will
   want seeks for its own sake and doing this twice is worse than once —
   and that at corpus scale, on a 128 GB box shared with the search
   engine, (b)'s second copy may simply not fit.
2. **Does the record index belong in an epoch?** It is read by the merge,
   not by a query. If a batch reads it, it must not read one from a
   merged layer the epoch does not name.
3. **Is the free bit taken now?** `held_beside` in the cited-as row costs
   nothing and closes bug 3 today. It is not exclusive with any of this,
   but it is a merged-shape change and therefore a full merge.
4. **Four divergences found by reading, none of them item 22's**, and each
   wants a decision of its own:
   - `sole` on a relation row (§1.2) — **confirmed by reading, not yet
     measured**. The batch writes `false`, the full merge `true`. Inert
     today. Fix it, or state why a relation row's `sole` is allowed to
     mean nothing.
   - The **public id's collision test** differs: `merge::public::mint`
     (`public.rs:194-223`) collides only against the public log, while
     the batch (`incremental.rs:2699-2728`) also rejects any key the
     serving fst knows, an ordinary name included. Vanishingly unlikely
     to fire; genuinely two rules. `compare.rs:174-178` would catch it.
   - A **held row's text** (§1.2): which *copy* supplies the bytes, and
     whether an older copy fills an empty field. The full path merges
     field by field (`document.rs:314-353`); the batch takes one record's
     text whole (`incremental.rs:1332-1406`). Compared by
     `compare.rs:85-90`, so a hold-out would show it — but nothing has,
     which may mean the shape is absent from the sample rather than
     absent from the rules.
   - `kept_of`'s silence on a stub landing (`incremental.rs:2043-2057`)
     when a twin of a deleted record is kept — **unverified**, raised by
     reading only. The probe is in §1.2.
5. **Pass 2's memory** — another agent is planning it today, and the
   ranged shape it may take is the same shape this index wants. Whoever
   goes second inherits the constraint.
6. **The corpus number.** 29.1 bytes a record (**A**) times a record count
   nobody has. Everything about size here is a sample figure.
