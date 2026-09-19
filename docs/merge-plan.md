# Merge plan

**Status:** draft 1, 2026-09-15 — written against the overlapping sample the
ingestion plan's M5 produced (its §7) and a census of that sample taken the
same day (§7 here). MM2, the identity pass and the registry, landed the
same day (§9); §7 carries its first run.
**Inputs:** [ingestion plan](ingestion-plan.md) §1 (what ingestion owes this
plan); the architecture summary of 2026-09-14 (dense ids, two graph
orientations, the funnel), folded 2026-09-15 into the
[graph plan](graph-plan.md) and the
[candidate-management note](inbox/candidate-management.md); the discussion
of 2026-09-15, logged in §10.

## 1. What the merged layer is

The third layer of the ingestion plan's §1: one record per work across
sources, built from the normalized layer and nothing else. It is where
cross-record state lives — identity, precedence, the graph's nodes and
edges — which is exactly why ingestion has none.

What it is for, in order: the citation graph's nodes and edges, keyed by an
id of ours; the search sinks' input (title, abstract, authors, venue, date,
kind, language, subjects); the funnel's hard filters (date, language, kind,
retraction, access); and the product's link-out through the names the world
uses.

What it is not:

- **Not a golden record.** A merged work keeps every source record it is
  made of, every name any source gave it, and every disagreement between
  them; it promotes one value per field for display and use and says why
  (§5). The sources' disagreements are the product's most interesting data
  (ingestion plan §1), so the design's job is to keep them visible, not to
  resolve them out of existence.
- **Not a re-normalization.** It reads normalized records through their
  crates' types and never touches raw.
- **Not the matching stage.** A reference with no resolvable name — an
  unstructured citation string, or structured fields without an id — is
  kept, keyed to the citing work, for a later stage that matches it to a
  work (§6). Nothing here matches on text except to corroborate an identity
  conflict inside a component of two or three records (§3.3).

This pipeline is an aggregator over aggregators. OpenAlex's `ids` (and
Semantic Scholar's `externalids`, when it lands) have done identity
resolution once already; their assertions are an input to §3 and the
yardstick the report measures against. On the sample the yardstick reads:
OpenAlex's PubMed links are wrong 3 times in 44,720, and its DOI is a work's
second DOI or second spelling 6 times (§7). The merge exists for those
cases and for the enrichment around them, not to redo the aggregators'
work.

## 2. Vocabulary

Chosen 2026-09-15 so that "internal" and "external" never appear.

| term          | meaning                                                                                                                                                                                                                                                             |
|---------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **work**      | one thing the world cites; one merged record; one graph node                                                                                                                                                                                                        |
| **`WorkId`**  | our id for a work: minted by us, opaque, derived from nothing, a dense `u32`. It never leaves the system                                                                                                                                                            |
| **name**      | an identifier the world uses for a work — a DOI, a PMID, a PMC id, an OpenAlex id; later an S2 corpus id, an arXiv id. A namespace (`ingest::Key`) and a value. A work has many names; each carries who asserted it, in which field, and its verdict                 |
| **record**    | one source's normalized record of a work, by source and native id; a work is made of one or more                                                                                                                                                                    |
| **stub**      | a work with names and no records: something we hold cites it, and we hold nothing else                                                                                                                                                                              |
| **assertion** | a record saying "this name is mine": its native id, or a foreign id it carries — a claim in the ingestion plan's sense, strict when valid, kept raw when not                                                                                                         |
| **component** | the records and names connected by shared names, before any judgement                                                                                                                                                                                               |
| **verdict**   | what the rules decide about one assertion: accepted or rejected, with a reason                                                                                                                                                                                      |
| **promoted**  | of a work's accepted names, the one per namespace used for display and link-out; the rest are *aliases*. Of a field's candidate values, the one the precedence rule chose                                                                                           |
| **shared**    | a name accepted into more than one work: the world uses one identifier for two things (one DOI on five letters to the editor, §7). Never promoted; an edge through it is ambiguous                                                                                   |
| **lineage**   | reserved: the grouping of a work's versions over time — preprint, published, corrected reprint — with its own id when it comes (§11). A citation always points at a work, never a lineage, because sources cite versions                                             |
| **part**      | a relation, not a grouping: supplements, datasets and figures are works of their own, related to a version                                                                                                                                                          |

The product reaches the world through names mapped from a `WorkId` — a DOI
to link out, a publisher's identifier to buy. If a public identifier of our
own is ever wanted it is another mapped one, minted separately; the
`WorkId` still never leaves.

## 3. Identity

### 3.1 Assertions

Every normalized record asserts names. This table is the whole list for
the three sources that exist; a new source adds rows through its
`Contributor` (§8) and nothing else changes.

| source   | field                                    | namespace | kind   | on the sample (2026-09-15)                                     |
|----------|------------------------------------------|-----------|--------|----------------------------------------------------------------|
| PubMed   | `pmid`                                   | PMID      | native | every record                                                   |
| PubMed   | `ids.doi` (`ArticleId IdType="doi"`)     | DOI       | claim  | 50,036 valid, 1 malformed, 384 absent                          |
| PubMed   | `ids.elocation_doi` (`ELocationID`)      | DOI       | claim  | 48,007 valid, 23 malformed; a DOI `ids.doi` lacks exactly once |
| PubMed   | `ids.pmc`                                | PMC       | valid  | 16,147                                                         |
| OpenAlex | `id`                                     | OpenAlex  | native | every record                                                   |
| OpenAlex | `doi`                                    | DOI       | claim  | 71,238 valid, 49 malformed, 4,863 absent                       |
| OpenAlex | `pmid` (`ids.pmid`)                      | PMID      | valid  | 45,358                                                         |
| Crossref | `doi`                                    | DOI       | native | every record                                                   |
| Crossref | `alternative_ids` (`alternative-id`)     | publisher | claim  | the non-DOI ones, scoped by the record's registrant (2026-09-19)|
| PubMed   | `ids.pii` (`ArticleId IdType="pii"`)     | publisher | claim  | as above; 135,998 records across the two sources carry one     |
| PubMed   | `ids.elocation_pii` (`ELocationID`)      | publisher | claim  | as above; need not equal `ids.pii`                             |

Not assertions, decided on the census:

- **OpenAlex location ids.** 2,468 works have a location whose DOI is not
  the work's, and 392 one whose PMID is not the work's: copies of other
  versions — a preprint under the published work — and so lineage evidence
  (§6, §11), not names of this work.
- **Crossref `alternative-id` where it is a DOI.** 13,659 are DOI-shaped,
  28 differ from the work's own DOI; publisher-supplied and unranked. A
  DOI in that slot is refused by the publisher-id rule below, whether it
  restates the record's own DOI or names another, so it is still counted
  and not used (§11, item 9). The rest of the slot is a publisher id and
  is a name since 2026-09-19.
- **Crossref `is-same-as` (23) and `is-identical-to` (1).** Crossref's own
  identity assertions. Kept as typed relations (§6); promotable to
  assertions when a case shows they are right more often than not.
- **A malformed claim** is never a name. It stays on the record, counted,
  as the ingestion plan's §6 decided.

**Publisher ids** (the namespace, decided and built 2026-09-19; `merge::publisher`).
A publisher id is what a publisher calls an article besides its DOI:
Elsevier's PII, PubMed's two `pii` slots, Crossref's non-DOI
`alternative-id`. They are kept on every record and every reference
(model version 2, 2026-09-18) and are names since merge version 8. The
rules, each with the measurement behind it:

- **The namespace is the registrant that issued the id**, spelled
  `reg1016` and never `10.1016`: the merged layer must not grow a second
  thing shaped like a DOI. `Name::parse` refuses a registrant-shaped
  namespace outright rather than calling it unknown.
- **No key without a registrant.** A publisher id on a record with no DOI
  is scoped by nothing and is no name: 344 records of 102,735 (0.335 %),
  all PubMed's, and only 22 of their 414 values are even Elsevier-shaped.
  A record with two DOIs of different registrants tie-breaks by the lower.
- **The value is stored exactly as written** — no case folding, no
  punctuation stripped, because punctuation is part of an identifier.
  The one exception is Elsevier's printed PII, which the world writes
  punctuated (`S0006-3495(25)00123-4`) and bare (`S0006349525001234`) and
  which we write bare. Both spellings occur for one article on 8,350
  DOIs, and the split is by source, not by publisher: PubMed writes the
  punctuated form (51,120 rows), Crossref the bare one (8,801), 8,345
  that way round and 5 the other, none writing both for one article.
  Every one agrees once rewritten; no normalised id is owned by more than
  one article, no raw id is, and no rewritten value collides with an
  unrelated one.
- **The rewrite is gated on the publisher, not on the shape** (the user,
  2026-09-19, against a measurement that argued for shape): anything that
  went through Elsevier's machinery matches Elsevier's rules, but a value
  that merely looks like a PII is not thereby Elsevier's, and another
  publisher's identifier must not be mangled. `publisher::ELSEVIER` is
  the checked-in list of registrants, and the normalize surveys count the
  PII-shaped values whose registrant is off it, by registrant: that count
  is how the list grows, on evidence rather than on a guess.
- **A value that is just the DOI again is not a publisher id**, tested
  *after* the rewrite against the DOI and its suffix with the printed
  PII's punctuation dropped and the case folded. Order matters: 786 rows
  restate their own DOI only once rewritten, and the only two apparent
  cross-source disagreements in the whole sample are this rule failing on
  a punctuation variant — on `10.1016/s0002-8703(99)70115-8` Crossref's
  alternative id is the suffix unpunctuated and PubMed's PII is the same
  suffix punctuated.
- **A bare article number is not a name.** Of the 20,647 publisher ids
  that more than one record carries, 18,194 are one work seen from two
  sources — the id doing its job — and 2,453 are different works; 2,137
  of those 2,453 are bare digits with or without an `e` (`4275`, `e112`,
  `320`). A journal's article numbers restart, so such a value names a
  work only once a journal qualifies it: evidence for the matching stage
  (§6), never a name. Nor is a value that is not printable ASCII without
  whitespace, or longer than 64 characters — one PII slot holds a whole
  citation, `Läkartidningen. 2026;123:25102-`.
- **A publisher id one source claims for more than one of its own
  records is no article's identifier** (added 2026-09-19 after the first
  merge run). A source holds one record per article — one PubMed record
  per PMID, one Crossref record per DOI — so a second record of that
  source claiming the same value is the source itself saying the value
  names more than one of its articles. The values are real: `LiveAll1` on
  259 Crossref records, `JAMP-14-1`, `IJMS-51-1` and `2025-vol6` on ten
  PubMed records each — a collection tag, a volume and issue, a year and
  volume. Joined, they fuse every article of an issue into one work and
  the components grow past the rules: a component of 762 records and
  1,018 names on the first run. The census therefore runs in the identity
  pass before any component exists, because a union cannot be undone
  (`Identity::settle`); the claim is rejected with its reason (§3.3) and
  never joins. On the store of 2026-09-19, nine values cost 307 of the
  48,293 publisher assertions. The test is *one source's* records and not
  all of them: one work seen from two sources carrying the same publisher
  id is the id doing its job — 18,194 of the 20,647 the census found on
  more than one record — and rejecting those would empty the namespace of
  the use it is for. The price is that a source's duplicate records of one
  article (§3.3) lose their shared publisher id, which costs a name and
  never a merge.
- **References are not in it.** Of 3,532,199 PubMed reference records
  exactly one carries a publisher id, so `targets` is unchanged.
- **OpenAlex has no publisher-id slot at all** — its id fields are `doi`,
  `pmid` and `mag` — so nothing was added there.

Of 135,998 records carrying a publisher id, 102,735 hold one that is not
their own DOI, 35,833 hold an Elsevier-shaped one, and the values are
82,423 distinct strings. What the identity pass makes of them on the
store of 2026-09-19: 48,293 assertions — 36,612 from PubMed's `ids.pii`,
2,247 from its `ids.elocation_pii`, 9,434 from Crossref's
`alternative_ids` — leaving 39,484 promoted publisher names, 412 aliases,
2 shared and 307 rejected.

### 3.2 Components

Union over shared names: two records are in one component when an
assertion of one names a name of the other, transitively, before any
verdict. On the sample the 177,166 records and their names fall into
82,723 components, of which 82,638 hold at most one DOI, one PMID, one
PMC id, one OpenAlex id and one record per source. Those are works and
need no judgement. The 85 that do not are the conflict components — 0.1
% — and every one is of a shape §3.3 names; the census counted 83 because
it looked for wrong PMIDs only, and the pass found wrong DOIs too. Those
counts are from before publisher ids were names (§3.1, 2026-09-19): a
work with two of them is now a conflict component too. No new *shape*
comes with it — a publisher id is never a record's native id — so the
rules of §3.3 cover every one. Measured on the whole store the same day,
242,624 records and 425,617 names fall into 148,178 components, 510 of
them conflicts, and the pass resolves all 510.

### 3.3 Verdicts and the conflict vocabulary

A conflict component is resolved by a closed set of rules, applied in
order, each corroborated by a second signal. Every assertion in it ends
with a verdict. A component no rule covers stops the run with the
component printed, until the shape is modelled with a fixture and a
rule — the discipline of an unknown field in ingestion (its §6).
`--survey` tallies the unmodelled shapes without writing.

The second signal is title agreement, deliberately weak because it only
ever decides among two or three records that already share a name: markup
stripped, PubMed's bracketed translation unwrapped, case folded, everything
but letters and digits removed. Two titles agree when they are equal; or
one is a prefix of the other (a subtitle one source dropped); or one equals
the other record's vernacular title; or, on a fingerprint of twenty
characters or more, they are within one edit in ten — a plain `S` for a
fraktur one, `Toward` for `Towards`, a typo. The last clause landed after
the first run (2026-09-15) rejected two true links for exactly those. On
the sample the strict form of the rule calls 47,070 of 49,620
PubMed–Crossref pairs equal and the rest differ by a dropped subtitle,
curly quotes, hyphenation, or whitespace inside markup (§7), which the
other clauses absorb.

The shapes, from the census, each with its rule:

| shape                                                                                                            | on the sample                                                                                                                              | rule                                                                                                                                                                                            |
|------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **enrichment** — a record adds a name no other record of the component has                                       | 53 of the 61 OpenAlex-versus-PubMed DOI differences: PubMed has no DOI and OpenAlex supplies one                                            | not a conflict; accepted                                                                                                                                                                        |
| **duplicate records** — several records of one source in one component, titles agreeing                          | 52 OpenAlex pairs (a `W44…` and a `W71…` for one DOI), two of them with a second PMID; 6 PubMed groups sharing one DOI, two of them five letters to the editor each | one work with several records of that source, flagged; the record with the lowest native id is promoted for that source's fields                                                                |
| **two DOIs, one work** — two DOIs bridged by a PMID, titles agreeing                                             | 3: two journals that changed publisher (`10.1097` to `10.1016`, `10.1258` to `10.1177`), a re-registration with a page range                 | both accepted; the DOI a Crossref record holds is promoted, else the PubMed record's (§3.4)                                                                                                     |
| **two spellings of one DOI**                                                                                     | 2, APA's legacy `10.1037//`                                                                                                                | both accepted, both matchable; promoted per §3.4; not a change to the `Doi` type                                                                                                                |
| **two PMIDs, one DOI** — OpenAlex's PMID is not the PMID of the PubMed record with that DOI, titles agreeing     | 10                                                                                                                                         | both accepted; the held PubMed record's PMID promoted, the other an alias — PubMed holds two records for the article, an early and a final one, and OpenAlex linked the other                    |
| **wrong link** — a foreign id names a record whose title disagrees                                               | 8: six OpenAlex records whose W number is a PMID and whose DOI is that PMID's article's while the title is another paper's (`W39406291`, `W31695178`, `W20395417`, …); one OpenAlex PMID on the wrong record; one translation (§11) | the assertion is rejected with the reason; the component splits into the works the remaining assertions support                                                                                 |
| **shared name** — after splitting, one name is accepted into two works                                           | 4: a bogus DOI, `10.12739/10.12739`, on ten OpenAlex works; a DOI on two different articles, twice; a Chinese journal's DOI on two works with different PMIDs | both works keep the name, neither promotes it; an edge through it is ambiguous (§6)                                                                                                             |

One verdict is reached before any component exists. A publisher id one
source claims for more than one of its own records identifies no article
(§3.1), and a name like that has to be refused *before* the union, not
after, because a union cannot be undone and the component it makes is
past the rules by the time they see it. `Identity::settle` runs that
census once every record is in and rejects the claim on every record that
made it. It is a rejection like a wrong link — the assertion stays on the
work with its verdict, nothing is deleted (§3.4) — so a rejected
assertion now carries which rule rejected it, `wrong-link` or
`not-an-article-id`, and a holder only where a record holds the name.

Order: duplicates and spellings first (they merge nothing across sources),
then the title test on every bridge, rejecting what fails it, then §3.4's
promotion on what remains. As built (§8): records sharing a name are one
work when their titles agree; a name asserted by records of two works goes
to the work whose record holds it natively, and is rejected on the other;
a name no record holds natively is shared. The eight wrong links are the
only rejections on the sample — 8 assertions of 220,897; everything else
is an accepted name with a standing.

### 3.4 Precedence for names

Registry over aggregator, decided 2026-09-15: a source's assertion about
its own namespace outranks another source's assertion about it, and the
registry of a namespace outranks everyone.

| question                                            | order                                                     | why                                                                                            |
|-----------------------------------------------------|-----------------------------------------------------------|------------------------------------------------------------------------------------------------|
| which PMID is promoted                              | the held PubMed record's, then OpenAlex's                 | NLM mints PMIDs                                                                                |
| which DOI is promoted                               | Crossref's record's, then PubMed's `ids.doi`, then OpenAlex's | Crossref mirrors the DOI registry; PubMed's is deposited by the publisher; OpenAlex's is derived |
| which spelling of a DOI                             | the one Crossref holds, then PubMed's                     | as above; on the two APA cases Crossref was not looked up, so PubMed's stands                  |
| which record of one source, among duplicates        | the lowest native id                                      | deterministic, and the older record has been cited longer                                      |
| which publisher id is promoted                      | the lower registrant, then the lower value                 | nobody ranks them and no source mints them; deterministic is all the rule can be (2026-09-19)  |

A rejected or outranked assertion stays on the work with its verdict.
Nothing is deleted.

## 4. The registry

A rebuild can recompute everything in the merged layer from the normalized
sets except one fact: which `WorkId` a work was given last time. That fact
is the registry, and it is the only part of the merged layer that is of
record. The merged sets (§8) are a cache, rebuilt from normalized plus
registry, the way normalized is rebuilt from raw. This refines the
ingestion plan's §3.5, which said the merge output would be of record.

Append-only, as receipts are: a log at `<store>/merged/registry.jsonl`, one
JSON line per event, four kinds.

| line    | fields                 | meaning                                                                                  |
|---------|------------------------|------------------------------------------------------------------------------------------|
| `mint`  | `id`, `names[]`, `at`  | a new `WorkId`, with the names that earned it                                            |
| `name`  | `id`, `name`, `at`     | a name attached to an existing id; the latest line for a name wins, which is how a name moves |
| `drop`  | `name`, `at`           | a name no work claims any more                                                           |
| `merge` | `id`, `into`, `at`     | an id retired into an older one; edges and lookups follow it                             |

Read once at the start of a run and folded into a table in memory — ids
and names only — so lookups never touch the log. An id is never reused. On
a rebuild each cluster looks up its accepted names:

- no name known: mint;
- every known name maps to one id: reuse it, attaching the names that are
  new;
- known names map to several ids: a merge — the oldest id wins and the
  others are retired into it;
- one id claimed by several clusters: a split — **the part that holds
  the records keeps the id**, and the parts that are names only are
  minted, a name leaving a work not being that work ending. Where no
  part holds records, or where several do, no part continues the work:
  the id is retired with no successor and every part is minted. Ruled
  2026-09-19 on the fourteen splits the corpus holds (§11, item 3).

Which is why the assignment is told how many of the clusters it is given
hold records: they come first, and the stubs after, which the minting
order already asked for.

Minting order is deterministic — held works first, sorted by their
promoted names, then stubs, sorted — so two tests hold: a rebuild on
unchanged data and rules mints nothing, and a registry built from scratch
on the same inputs is byte-identical. Churn — ids minted, merged and
retired per rebuild — is a number in every report.

Stubs are minted too, decided 2026-09-15: a name seen only as a reference
target gets a work with no records. A reference names one work, so a
reference carrying several unheld names — a PMID, a DOI and a PMC id — is
one stub with all of them, and an unheld name beside a held one is that
work's, asserted by the citing record: counted (514 on the sample), not
yet used (§11). On the sample that is 3,942,065 stubs from 4,469,772 names
beside 82,744 held works (§7); at full scale, with every source's corpus
held, the stubs left are the interesting long tail — foreign DOIs,
mistyped ones, works no source holds. A stub that later gains a record
keeps its id.

The first run (2026-09-15) minted 4,024,811 ids in 22 s into 379 MB of
log; the second appended nothing; the run after the similarity clause of
§3.3 landed appended two `merge` lines and nothing else.

Dense ids are for the graph's CSR; they promise stability, not locality,
and the graph relabels into a position space of its own at every
compaction (graph plan §4).

Why a log and not a table: it is the store's existing discipline (raw
immutable, receipts only grow); a partial write is a truncated last line
and not a corrupted table; "why does this work have this id" is answered
from the log alone; write-once objects sync to a bucket. At scale the log
wants a binary form and the fold a sorted file read through a memory map;
both are the ingestion plan's M9 problem and change nothing here.

## 5. The merged record

One per work, in the works set (§8). Four parts.

**Records.** Each source record the work is made of: source, native id,
whether it is the promoted record for its source, and — since version 2
of the merged layout, 2026-09-16 — its evidence: every assertion it made
(§3.1, the field it came from and whether native) and its view, which is
its values for the promoted fields below with its titles and its
revision stamp (Crossref `indexed`, PubMed `DateRevised`, OpenAlex
`updated_date`) — and, since version 3, 2026-09-17, its version and the
set its speaking copy came from (§8). The verdicts — the names' standings, the promoted
fields — are a function of the records' evidence, and carrying the
evidence is what lets an incremental merge (serving plan §5) re-run the
rules on one work with the rows alone. Everything else a record holds —
its abstract, its author names, its references — stays in the
normalized layer, reachable through this list: the merged record is a
summary and an index, not a superset. A row's lists are in canonical
order (records by source and native id, names by name), so a row does
not depend on the order its records arrived in.

**Names.** Every accepted name with its namespace, value, standing
(promoted, alias, shared) and who asserted it (source and field); every
rejected assertion with its reason. A stub's names have no asserter: each
carries its citers instead, the works whose arcs landed on the stub
through that name with the source of each citing record (2026-09-16), and
its sources are its citers' (2026-09-17), so that an incremental merge can
take a citation back when the record that made it goes. Also written as the
names set, one row per name, for lookups and the graph build.

**Promoted fields, and the disagreements.** The fields the funnel and the
search sinks consume, each chosen by a precedence rule with a comparison
rule beside it. The other records' values are stored only where they
differ under that rule — `None` when consistent, the pattern the OpenAlex
normalizer already uses for its census-derived fields — so a consistent
work is small and a disagreement is visible. The first-cut table is a
hypothesis the report revises (§10):

| field       | promoted from, in order                                | a disagreement is                       | measured 2026-09-15, over works with two or more candidates (§7)                                          |
|-------------|--------------------------------------------------------|-----------------------------------------|------------------------------------------------------------------------------------------------------------|
| `title`     | PubMed, Crossref, OpenAlex                             | the §3.3 comparison failing             | 0.7 % of 50,114: Crossref's `<subtitle>` markup and `RETRACTED:` prefixes, PubMed's own abbreviations      |
| `abstract`  | PubMed, OpenAlex, Crossref                             | presence only                           | 40.1 % of 50,116 have one in some sources and not others                                                   |
| `authors`   | Crossref, OpenAlex, PubMed                             | the count differing                     | 2.6 %; Crossref–PubMed 1.4 %, Crossref–OpenAlex 1.6 %                                                     |
| `venue`     | OpenAlex source id, Crossref ISSNs, PubMed NLM id      | disjoint ISSN sets                      | 0.3 %; 78 of the 138 are Crossref's electronic ISSN against PubMed's print one for one journal (§11, 6)    |
| `published` | the earliest date any source gives, at its precision   | years differing                         | 10.2 %; PubMed–Crossref 4,078 of 39,934, 4,040 of them one year apart: online-first against the issue      |
| `language`  | PubMed, Crossref, OpenAlex                             | codes differing                         | 0.5 %; `fra`, `kor`, `rus` at PubMed against a publisher's `eng`                                            |
| `form`      | Crossref, then PubMed mapped, then OpenAlex mapped     | a genre that cannot live in the form    | 0.7 %; Crossref's book-chapter against PubMed's implied journal-article 140 times                          |
| `genre`     | PubMed, then OpenAlex, then Crossref mapped            | as above                                | 22.7 %; PubMed–OpenAlex 7,998 of 44,791, `review` against `article` 5,726 of them; the order was revised   |
| `status`    | retracted if any source says so; corrected, concern likewise | a source not knowing               | 46 of 67 retractions known to OpenAlex alone; 141 corrections to PubMed alone, 280 to Crossref alone       |
| `access`    | OpenAlex, a Creative Commons license at Crossref, a PMC copy at PubMed | flags differing         | 0.1 %; the 52 pairs of duplicate OpenAlex records differ 16 times                                           |
| counts      | kept per source, never promoted                        | —                                       | the graph computes its own                                                                                 |

The first cut put OpenAlex before PubMed for `genre`; the first report
reversed it. Where the two differed, PubMed's curated publication type
was the specific one — review, letter, editorial, news, guideline
against OpenAlex's `article` — 7,070 times, and OpenAlex's the specific
one 288 times, mostly on records PubMed holds at `Publisher` status with
the publisher's types alone.

Kind is two fields because the sources' vocabularies are two different
things. Crossref's `type` is the registry's *form* — journal-article,
book-chapter, posted-content, proceedings-article, dataset and 25
more — and says nothing about what the item is. OpenAlex's type and
PubMed's publication types are the *genre* — article, review, erratum,
editorial, letter, retraction, preprint, data-paper — and the census
cross-tab shows one form holding a dozen genres: journal-article is
40,955 articles, 1,928 reviews, 484 errata, 318 editorials, 162 letters,
54 retractions and 39 preprints. Both are closed enums with explicit
mapping tables from the three sources — in each source crate's `view`
module, keyed by Crossref's and OpenAlex's types and by PubMed's
publication-type descriptor UIs, a record's genre being the most
specific of its types by a fixed priority; an unmapped value stops the
run, or `--survey` counts it. The genre vocabulary is OpenAlex's plus
the four PubMed adds (comment, expression of concern, guideline, news).
The disagreement worth storing is a genre that cannot live in its form:
20 works on the sample after the revision above, 12 of them PubMed
articles whose DOI Crossref registered as a journal issue.

Status is derived, not copied: retracted from PubMed's `Retracted
Publication` type or `RetractionIn` relation, Crossref's `updated-by:
retraction`, OpenAlex's `is_retracted`; corrected from `ErratumIn` and
`correction` / `corrigendum` / `erratum`; concern likewise. A retraction is
a fact once any registry or aggregator asserts it, so the union is
promoted and the sources that do not know are the disagreement.

**Flags.** Stub; duplicate records; a shared name.

## 6. Edges, relations, unresolved

Edges are `(from: WorkId, to: WorkId, sources)`: the same edge asserted by
three sources is one edge with three bits. `A → B` is "A cites B", the
graph plan's convention (its §1). Each source's reference stream resolves
its target through the names table:

| source   | a reference's target                                                            | resolved 2026-09-15 (MM3): held, stub, self-citation, no name, disagree, empty                                       |
|----------|---------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------|
| OpenAlex | an OpenAlex id, always                                                          | 2,262,555: 39,237 held, 2,223,129 stubs, 189 self                                                                   |
| PubMed   | a PMID, a DOI, a PMC id, in any combination, as claims; or none                 | 1,231,899: 43,137 held, 834,857 stubs, 211 self, 353,356 no name, 338 whose PMID and DOI resolve to different works |
| Crossref | a DOI claim; or structured fields; or an unstructured string; or nothing        | 2,744,441: 64,781 held, 2,372,645 stubs, 81 self, 292,080 fields or text only, 14,854 empty                         |

A target that resolves is an edge. A target whose name no held work has is
a stub (§4), and an edge to it. A reference whose names resolve to
different works is settled by **its own citation text where that text
repeats one of its DOIs** (2026-09-18, §10), and otherwise follows the
registry id — PMID, then PMC id, then OpenAlex id, before a DOI — and
keeps every name in the unresolved stream as a `disagree` row either
way. The 338 on the sample are a name beside a
held one (§11, item 14) that is also a stub of its own, and the examples
are Cochrane reviews: the PMID names one version, the bare DOI
(`10.1002/14651858.cd010508`) names the review across versions — lineage
evidence (§11, item 1). A target through a shared name alone is ambiguous
and goes to the unresolved stream with that reason (none on the sample:
every reference through a shared DOI carried a PMID beside it). A target
with no name goes to the unresolved stream with whatever the source
gave — Crossref's key, year, author, titles, volume and page or its
unstructured string; PubMed's citation text — keyed by the citing
`WorkId`, for the matching stage: 645,436 rows on the sample. A reference
with neither a name nor any of that is counted and dropped (14,854, all
Crossref's). Self-citations, an edge inside one work, are allowed and
counted: 399 on the sample, to be looked at.

Relations that are not identity become a typed stream `(from, to, kind,
source)`, kinds closed (`merge::relation::Kind`): PubMed's
`CommentsCorrections` (23 kinds), Crossref's `relation` types (the deposit
schema's, plus the Crossmark words some publishers deposit there) and
Crossmark updates both ways as `update-to:<type>` / `updated-by:<type>`,
and the OpenAlex location DOIs and PMIDs of §3.1 as `has-version`. Status
derives from them (§5); lineage will (§11). A relation's other end
resolves through the names table like an edge's, minting a stub if need
be: 15,303 relations on the sample — `has-version` 3,834, `has-review`
3,450, `comment-in` 2,573, `has-preprint` 1,687, `is-supplemented-by` 743,
`erratum-for` 511 — of which 284 point at held works and 15,206 at stubs,
14,267 of the stubs minted for them; 355 targets were outside our
namespaces (a URI, an accession number, a PubMed relation without a
PMID) and are counted, not kept.

Agreement, restricted to citing works held by both sources and targets
held: Crossref and OpenAlex share 36,837 edges, Crossref alone asserts
14,676, OpenAlex alone 1,730; PubMed and Crossref share 39,462, PubMed
alone 3,259, Crossref alone 24,530; PubMed and OpenAlex share 23,269,
PubMed alone 9,712, OpenAlex alone 15,403. PubMed's lists are partial by
construction; OpenAlex's, derived from Crossref's, miss 28 % of them on
this sample — 14,676 of the 51,513 Crossref asserts where both hold the
citing work and the target is held; "a fifth" until 2026-09-15 was a
miscount — which the report keeps measuring, and the graph will at scale
(graph plan §3). In all, 5,021,301 distinct
edges on the sample: 2,222,128 by OpenAlex alone, 1,909,179 by Crossref
alone, 483,820 by PubMed and Crossref, 368,316 by PubMed alone, 22,248 by
all three.

## 7. What the sample says

The census of 2026-09-15 over JSON dumps of the three normalized stores
(`census2.py`, in the session's scratch space), then the identity pass's
own report the same day (`ol-rht merge identity`, §8), which agrees with
the census wherever both count. Records: 50,404 PubMed, 76,142 OpenAlex,
50,620 Crossref, distinct by native id; 42 of them were fetched twice and
counted once.

Components, by which sources hold a record of the work:

| pattern            | works  |
|--------------------|-------:|
| PubMed + OpenAlex + Crossref | 44,182 |
| OpenAlex only      | 31,327 |
| PubMed + Crossref  |  5,375 |
| Crossref only      |  1,000 |
| PubMed + OpenAlex  |    496 |
| PubMed only        |    276 |
| with two OpenAlex records (and PubMed, Crossref) | 57 |
| with two or five PubMed records | 6 |
| OpenAlex only, several records | 4 |
| **all**            | **82,723** |

Conflict components: 85, resolved into 106 works. By shape, from the
pass's report: 50 with duplicate OpenAlex records and 2 more with a second
PMID beside; 6 with duplicate PubMed records and a second PMID; 10 with
two PMIDs; 3 with two DOIs; 2 with two spellings of one DOI; 8 wrong
links; 4 shared names. Verdicts: 220,889 names accepted, 8 assertions
rejected, none joined on an untestable title. Standings: 77,603 DOIs
promoted, 5 aliases, 17 shared; 50,968 PMIDs promoted, 24 aliases; 16,130
PMC ids promoted; 76,090 OpenAlex ids promoted, 52 aliases.

Edges: in §6. Stubs: 3,942,065 works from 4,469,772 names — 1,849,092
DOIs, 613,313 PMIDs, 227,287 PMC ids, 1,780,080 OpenAlex ids — 3,561,850
of them with one name, 233,430 with two, 146,284 with three, 501 with
more; many the same work under two namespaces, which only holding them
would show. Resolved edges between held works, as the census counted
them before MM3's reference rules (the graph's held build counts 68,815,
graph plan §11 item 10): 68,813 distinct —
asserted by all three sources 22,261, by Crossref and OpenAlex 14,576, by
Crossref and PubMed 17,233, by OpenAlex and PubMed 1,021, by Crossref alone
10,019, PubMed alone 2,403, OpenAlex alone 1,300.

Status, over the 44,245 works all three hold: 61 retracted by some source —
5 by all three, 10 by OpenAlex and Crossref, 46 by OpenAlex alone, 1 by
Crossref alone; corrected by both PubMed and Crossref 198, by Crossref alone
267, by PubMed alone 99.

Field agreement is in §5's table, measured by the merge's own report
(`ol-rht merge`, 2026-09-15) after the census's estimates: the title
rule of §3.3 leaves 0.7 % of pairs in disagreement where the census's
strict comparison had 5 %, and the census's 217 language disagreements
are 216 under the merge. Two samples worth keeping: PubMed's language
disagrees with Crossref's on 216 works and it is `fra` against `eng`
every time in the sample of six; PubMed's title is a bracketed English
translation where Crossref carries the vernacular on 158 works, and
PubMed's `VernacularTitle` matches Crossref's title on every one. The
kinds cross-tab after the genre revision of §5: journal-article holds
60,374 articles, 8,693 reviews, 835 letters, 784 editorials, 518 errata,
113 news items, 103 guidelines, 88 comments, 57 retractions and 9
expressions of concern; the other forms are 5,693 dissertations, 3,565
preprints, 606 books, 504 chapters, 58 conference papers, 57 datasets,
48 reports, 21 components.

The merged layer as written: 4,039,076 works (82,744 held, 3,956,332
stubs), 5,021,301 edges, 4,704,928 names, 15,303 relations, 645,774
unresolved rows, in 62 shards, 120.8 MiB; 48 s and 5.0 GB resident on
the laptop, of which the identity pass and the registry are 25 s (§8).

## 8. Machinery

Follows the ingestion crate's shape; the rules of the ingestion plan's §5
apply where they can (stream, no DOM, binary zstd output, measure), with
one designed exception: the names table is global state, sized by names
and not by records.

- **`crates/merge`**, source-agnostic: `WorkId`, `Name`, the registry, the
  names table, components and verdicts, the merged record, edges, the two
  passes, the report. It depends on `ingest` for the set reader and the
  store, on `model` for the id types, and on no source crate.
- **`Contributor`**, one small trait each source crate implements for its
  own records, the way it implements `Source` — the merge is written once
  against these answers:
  - `names(&Work, out)`: the record's assertions, each a name with the
    field it came from and whether it is native or a claim;
  - `target(&Reference)`: the names a reference points at, or the fields
    and text it gives instead;
  - `view(&Work)`: the record's values for the promoted fields of §5, in
    one source-agnostic shape, borrowed from the record;
  - `relations(&Work, out)`: the typed relations of §6;
  - `text(&Work, table)`: the record's abstract, author names and venue
    name as plain text, for the serving layer's documents (serving plan
    §3; 2026-09-16). `merge::document` joins those texts to the merged
    record, one document per held work, for the lexical index and the
    works index.
- **Two passes** over every normalized set of every source, both streaming
  through `ingest::Reader` with each set's symbol table installed, so
  interned values come back as text:
  0. **copies** — before either pass, every deletions set and every
     copy's revision stamp and version are read (`merge::copies`,
     2026-09-17). A record delivered more than once — a baseline file and
     the update files after it, a snapshot and a later lookup, two
     versions of a versioned citation — has one copy that speaks for all
     of it: the latest revision stamp, then the higher version, then the
     later set in path order, then the later place in its set. A
     deletions set withdraws the copies in the sets before it in path
     order (PubMed's `DeleteCitation`, 2026-09-16). The newest deletion
     counts: of the deletions sets naming a record, the latest in path
     order withdraws every copy before it, whatever the copy's stamp,
     since a deletion carries no stamp of its own, and a copy in a later
     set speaks again. That may lose information, an accepted loss
     (§10, 2026-09-17).

     What the speaking copy gives, and what a superseded copy still
     gives, is **absence is not an assertion** (2026-09-18, §10): a copy
     that has no value for something says nothing about it, and only a
     deletion takes anything back.
     - Set-valued — the record's **names** and its **references** — is the
       union over every copy a deletion has not withdrawn. References
       because PubMed replaces a publisher's reference list with PMC's
       when an article lands there — PMIDs only, and fewer entries — and
       the identifiers the publisher gave are evidence already in hand;
       names because a copy that stops giving an id (a version's own DOI
       at F1000Research, twice in the sample) has not said the id is
       wrong, and the references that cite it should keep landing on this
       work.
     - Single-valued — the **promoted fields** of §5 and the **text** of
       §3 — is the newest copy's value, or where the newest has none, the
       value of the newest copy that has one. The stamp, the version and
       the set stay the speaking copy's: they name the delivery, not the
       work. The venue fills as one id and not field by field, since its
       three ids name one journal. A value that is merely *shorter* is not
       an absence and does not fill: a truncated author list or a title
       the source revised still speaks, because it is there.
     - **Relations** are the speaking copy's alone, the one exception,
       because the serving layer does not read a work's relations back and
       so an incremental merge cannot take their union. Measured cost on
       the sample: 15 of 23,482 re-delivered records have an erratum link
       in an older copy that the newest dropped; the `corrected` flag those
       links set survives anyway, being single-valued and sticky.
     - A **withdrawn** copy gives nothing at all, references included. Two copies in one set are one delivery for
     references, since a references set names the record and not the
     copy: the one versioned citation of the sample's update files;
  1. **identity** — assertions to components to verdicts, then the registry:
     look up, mint, attach, merge; the output is the names table, name to
     `WorkId`, and the registry's new lines;
  2. **records and edges** — every work's view joins its `WorkId`'s
     candidates, every reference resolves through the names table; the
     merged sets are written. Each record's view names its set and
     version, so that a merged row says which copy spoke — what the
     incremental merge ranks a batch's copy against, and what the search
     documents take their text from (2026-09-17).
  The names table is in memory: a few million names on the sample, well
  under a gigabyte. Pass 2 accumulates each record's view, every edge,
  every relation and every unresolved row in memory as well and writes
  the shards at the end: 5.0 GB resident on the sample, the unresolved
  rows' citation text and the four million works' rows most of it. At
  full scale that does not fit, and the shape's answer is to run pass 2
  once per range of `WorkId`s, keeping only that range's candidates and
  rows — the input read *k* times, memory bounded by *N/k* — which dense
  ids make trivial. Pass 1's table becomes a sorted file read through a
  memory map. Neither changes the code's shape; both are the ingestion
  plan's M9.
- **Layout**, under the store: `merged/registry.jsonl` (of record, §4),
  `merged/journal.jsonl` and `merged/public.jsonl` (of record, serving
  plan §4 and §6), `merged/v<model>.<merge>/` (a cache, replaced whole by
  every complete run) with `delta/` beside its shards (what incremental
  merges append, serving plan §5), and `merged/merge.lock` while any
  merge runs, so two never append to the registry or the deltas at
  once; the merge having a version of its own that any change to its
  rules or shapes bumps. Sets are sharded by `WorkId` range, 65,536 works
  a shard (`shard_00000` …), in the normalized layout's framing — a
  header, length-prefixed postcard values, zstd, a symbol table per
  set — through `ingest::binary::Writer`, the framing factored out of the
  normalized sink with the record kinds this layer has appended to
  `ingest::Kind`: `works` (held and stubs), `edges`, `names`,
  `relations`, `unresolved`, `cited_as` (2026-09-18: one row per name a
  reference gave beside the work it landed on, with the citing work, the
  source, the work it landed on and the namespace that landed it — item
  14's evidence, kept at last), `symbols`. A work's shard is its id's; an
  edge's, a relation's or an unresolved row's is its citing work's. A
  set's header names the merge as its source and the model version; the
  directory names the merge version, which `merge dump` checks. Rows
  borrow their text from the set they are read from, names as the text
  form of §2 (`doi:…`, `pmid:…`).
- **Fail fast.** A conflict shape outside §3.3 stops the run; an unmapped
  kind stops the run; `--survey` tallies both without writing. Invariants
  checked at the end of every run: one promoted name per namespace per
  work; every name maps to one live work unless shared; every held record
  is in exactly one work; no `WorkId` reused; every edge's ends exist.
- **Commands.** `ol-rht merge identity [--store <root>] [--limit N]
  [--survey]` runs pass 1, assigns ids through the registry and prints its
  report — landed 2026-09-15: 22 s over the sample, 2.4 s of it reading
  every record's names, the rest the registry's four million lines;
  `--limit` and `--survey` write nothing. `ol-rht merge [--store <root>]
  [--survey] [--limit N]` runs both passes, appends the registry's lines,
  writes the sets and prints both reports — landed the same day: 48 s
  over the sample, byte-identical between two runs, nothing appended to
  the registry by the second; `--survey` counts a value outside the
  merge's tables instead of stopping on it, and it and `--limit` write
  nothing. `ol-rht merge dump <set.zst>... [--limit N]` prints any merged
  set as JSON lines. `ol-rht merge --batch <path>... [--public-key
  <file>]` is the incremental merge, the serving plan's §5.
- **The report.** Works by source pattern; conflict components by shape and
  verdict; registry churn; per promoted field, works with a value, works
  compared, works disputed, who it was promoted from, and per pair of
  sources how many candidates differed; the form × genre cross-tab and
  the conflicts; status by the sources asserting it; per source,
  references resolved to held works, to stubs, self-citations, no name,
  shared, disagreeing, empty; edges by the sources asserting them and the
  agreement between each pair; relations by kind and by what their
  targets were; the unresolved by reason; what was written; throughput
  and allocations, as every report has.

What ingestion's symbol tables are worth is decided here (ingestion plan
§10): the merge resolves each set's symbols to text and interns into its
own tables per output set, so ingestion's per-file tables earn their disk
bytes and nothing else. The first merge run says whether the field set
stays.

## 9. Milestones

- **MM1** — this plan agreed; the ingestion plan's §1, §3.3, §3.5, §9 and
  §10 point at it. *2026-09-15.*
- **MM2** — identity: `Contributor` for the three sources; assertions,
  components, the seven shapes of §3.3 as fixtures with their rules; the
  registry and its fold; the names table; `merge identity` and its report
  on the sample; the determinism and zero-churn tests. *Landed 2026-09-15:
  `crates/merge` (§8) with the three contributors in their source crates;
  the seven shapes and the hairball as tests on the sample's own cases;
  the registry with its five line kinds (a `split` joined the plan's
  four, for an id whose names part between works) and the two tests;
  `ingest::Key` gained `Pmc`. The first run over the store is in §7 and
  §4. What running it changed: a fourth way for titles to agree (§3.3), a
  reference is one stub (§4), and two wrong-link classes the census had
  not seen (§3.3).*
- **MM3** — records and edges: views, the promoted fields and their
  comparison rules, the `form` and `genre` tables, status, relations,
  stubs, the unresolved stream; the merged sets and `dump`; `merge` whole;
  the field-agreement report that revises §5's table. *Landed 2026-09-15:
  `Contributor` gained `view`, `relations`, `citation` and `native`;
  `crates/merge` gained `view`, `kind`, `relation`, `record`, `build` and
  `layout`; `ingest::binary::Writer` and the merged kinds; `ol-rht merge`
  and `merge dump`. The first run is in §5, §6 and §7. What running it
  changed: the genre precedence (§5), relation targets minting stubs (§6:
  14,267 on the sample), the reference-disagreement rule (§6), and the
  338 disagreements and the ISSN pairs as evidence for items 14, 1 and 6
  of §11.*
- **MM4** — the graph handoff, which is the [graph plan](graph-plan.md)'s
  G2 (2026-09-15): forward and transpose CSR from the edges set in
  `WorkId` order, a label beside every arc, held works and stubs
  selectable; the experiment follows as G3. What the merged layer owes
  it: the edges set as the graph's one input, sorted by citing work with
  `by` on every edge, which it is; `Sources` widened to `u16` before the
  ninth source, a merged-layout version bump (§11, item 19); the edges
  delta log the incremental merges will write, in `WorkId` space, with
  `add`, `remove` and `retire` lines (§11, item 20; graph plan §5); and
  the agreement figures of §6 kept at scale. The sample's subgraph settles
  tooling and correctness, not bits per edge — that needs the corpus.
  *Landed 2026-09-16 as the graph plan's G2: `crates/graph` and `ol-rht
  graph build | query | check`; the graph's agreement table, conditioned
  the way §6's is, reproduces it figure for figure.*
- **MM5** — Semantic Scholar and arXiv contributors when their sources land
  (ingestion plan M6, M7); the search sinks over the works set.
- Then: lineage (§11); the matching stage over the unresolved stream;
  OpenAlex's retirements as registry lines — **but see the note below,
  `merged_ids` no longer exists**; the scale shape of §8 on
  the box. *Incremental merges landed 2026-09-16 as the [serving
  plan](serving-plan.md)'s SP5: the delta logs (item 20), the journal,
  the public id (item 21), PubMed's deletions in both the full and the
  incremental merge, and the merged layout's version 2, whose rows carry
  each record's evidence (§5).*

## 10. Decision log

- 2026-09-15, our own id — a `WorkId` minted by us, opaque, derived from
  nothing, dense: a DOI-derived id fails on a work with several DOIs and a
  work with none, and a collection's mistake would rename our work instead
  of becoming a rejected assertion. Sequential, not random: one minting
  authority, and the graph is a CSR over it. It never leaves the system;
  the product reaches the world through names. Vocabulary chosen so that
  "internal" and "external" never appear: work, `WorkId`, name, record,
  stub, assertion, verdict, promoted, alias, shared; lineage and part
  reserved.
- 2026-09-15, the registry — the one fact a rebuild cannot recompute is
  which id a work had, so that is the one thing of record: an append-only
  log with four line kinds, folded at run start; the merged sets are a
  cache. Reuse, mint, merge (oldest wins), split (retire and mint both).
  Deterministic minting order, hence two tests: zero churn on unchanged
  input, byte-identical from scratch. Stubs minted in the first cut.
- 2026-09-15, identity precedence — registry over aggregator: PubMed's
  record over OpenAlex's PMID link, Crossref's record over PubMed's DOI
  over OpenAlex's. Evidence: of 61 DOI-versus-PMID differences, 53 are
  enrichment, 6 are a second DOI or spelling of one work, 3 are OpenAlex
  links to the wrong record, and on 44,720 links that is the aggregator
  being right 9,999 times in 10,000.
- 2026-09-15, DOI spellings — every DOI of one work is a name of it, both
  spellings of a double-slash DOI included; one promoted; no change to the
  `Doi` type.
- 2026-09-15, the merged record — records, names, promoted fields with
  disagreements only where they differ, flags; everything else by
  reference to the normalized layer. The precedence table is a hypothesis
  the report revises.
- 2026-09-15, two kinds — `form` from the DOI registry's vocabulary and
  `genre` from the aggregators' and PubMed's, because the census cross-tab
  showed journal-article holding a dozen genres; the disagreement worth
  keeping is a genre that cannot live in its form.
- 2026-09-15, shared names — the census found one DOI on five letters to
  the editor and one on two different articles; a name accepted into two
  works is shared, never promoted, and an edge through it is ambiguous
  rather than guessed.
- 2026-09-15, fail fast here too — a closed conflict vocabulary, unknown
  shape stops, `--survey`, invariants at the end of every run.
- 2026-09-15, machinery — a source-agnostic crate and a `Contributor` per
  source crate; two streaming passes with the names table as the one
  global; sharded sets in the normalized layout's framing; the scale shape
  (ranges of ids, a mapped table) named and deferred to M9.
- 2026-09-15, MM2 landed — the identity pass reads every record's names
  in 2.4 s and holds titles for conflict records only, re-reading the
  sets that hold one; a reference is one stub with all its names, not one
  per name, which took the stubs from 4.47 million to 3.94; the title test
  gained its similarity clause after two true links were rejected on a
  fraktur letter and a plural; the registry gained a `split` line; the
  first run minted 4,024,811 ids, the second minted nothing, and the run
  after the rule change merged exactly the two works the rule joined.
  Two classes of wrong link the census missed: six OpenAlex records whose
  W number is a PMID and whose DOI belongs to that PMID's article while
  the metadata is another paper's; and a translation, a PubMed record in
  two languages whose DOI's Crossref title is the vernacular one, which
  the rules split for want of a vernacular title on the PubMed side.
- 2026-09-15, MM3 landed — the second pass keeps one owned view per
  record and promotes per work at the end; the merged sets are written
  through the normalized layer's own framing, its `Writer` factored out
  and the merged kinds appended, rather than a second framing. What the
  first report changed: genre is PubMed's before OpenAlex's (§5, the
  7,070-to-288 count); a relation's target mints a stub like a reference's
  (14,267 more stubs, 0.4 %); a reference whose names disagree follows
  the registry id and keeps every name as a `disagree` row rather than
  guessing (§6). Kept as hypotheses the numbers did not overturn: the
  access rule (a Creative Commons license at Crossref, a PMC copy at
  PubMed, OpenAlex's status), which disagrees 27 times in 31,957; the
  venue rule, whose 138 disagreements are mostly one journal's two ISSNs.
  Two runs of the same store write byte-identical shards, and the second
  appends nothing to the registry — the zero-churn test now covers the
  whole merge. A `--limit` run then found that assigning held works and
  stubs to the registry in two calls could give one id twice — a name a
  vanished record held that a reference still cites is a held work's in
  the registry and a stub's in the run — so the assignment is one call
  over held works then stubs, and that case is a split, as §4 says.
- 2026-09-15, the graph notes — the architecture summary's numbers,
  provenance, updates and ordering were reconsidered in the inbox notes
  and taken, with two amendments, into a plan of their own
  ([graph plan](graph-plan.md), draft 1); the summary is retired and its
  funnel is a note of its own. What changes here: `Sources` stays a byte
  until the ninth source and widens to `u16` then, a merged-layout
  version bump the graph's label does not follow; the edges delta log is
  a merged-layer artifact the incremental merges write; the graph is a
  cache of the edges set with the same byte-identical test the merged
  sets have; MM4 is the graph plan's G2. And §6's "a fifth" was a
  miscount: OpenAlex misses 28 % of Crossref's edges on the sample where
  both hold the citing work.
- 2026-09-17, copies — decided with the user: the newest copy of a
  record, by the revision-stamp rule its fields already follow, speaks
  for its names and its references too, since the latest version is the
  likeliest to be right. Until then the first copy's names counted and a
  name only a later copy carried was minted as a work of its own, which a
  probe showed splitting the work's id once a batch had attached the name
  (serving plan §11, item 22). Built the same day as §8's step 0, with a
  withdrawn copy's references no longer making stubs and a stub name's
  sources taken from its citers; the merge's version is 3. On the sample
  with PubMed's first five update files, 26,305 PubMed copies are
  superseded and 5 withdrawn.
- 2026-09-18, a record's citations are the union over its copies —
  decided with the user after the churn of §11 item 23 was traced. The
  cause was not a rule about stubs but the copy rule meeting PubMed's
  data: when an article lands in PMC, PubMed swaps the publisher's
  reference list for PMC's, which carries a PMID on every entry, no DOIs,
  and fewer entries. Of the 1,972 records in `pubmed26n1337` that the
  store already held, 976 have fewer references in the newer copy and 769
  fewer DOI or PMC ids; over those records the newer copies hold 88,236
  references against 103,982, 5,779 DOIs against 39,036 and 2,168 PMC ids
  against 14,233, while PMIDs rise from 60,614 to 73,084. Of the 769 that
  lost identifiers, 546 gained a PMC id of their own in that same copy and
  210 already had one. Cross-checked on one article (PMID 41547727): PMC's
  full text holds all 24 references with 25 DOIs, Crossref holds all 24
  with DOIs, OpenAlex counts 24, and only PubMed's later copy holds 12.
  NLM's own instruction is to replace ("Revised or deleted citations
  should replace existing citations in your local database"), and the
  annual baseline is a snapshot, so nothing at PubMed gives those
  identifiers back. So the newest copy still speaks for names, fields,
  relations and text, and references are cumulative: both merges read
  every standing copy's list, and the incremental merge adds arcs and
  removes none for a re-delivered record (it still takes back a withdrawn
  record's). The cost accepted: a citation a later copy genuinely removes
  stays until something withdraws the record. Merge version 4.
- 2026-09-18, the citation text settles a disagreement — decided with the
  user, narrow scope. When a reference's names resolve to different works
  and the reference's own citation text repeats one of its DOIs, that DOI
  wins over the order of §6; where no DOI of its own is repeated, the
  order decides as before, and the `disagree` row is written either way.
  Measured over the store's 3,521,743 PubMed references: 774,466 carry a
  DOI and 512,911 of those have a citation text repeating it; 618,825
  carry both a DOI and a PMID, of which 24,847 name a PubMed record we
  hold and can check; **492 of those 24,847 disagree** — the PMID's own
  record carries a different DOI — and of the 492, **449 have a text
  repeating the reference's DOI and not one repeats the PMID's**. The
  asymmetry has a cause: the DOI and the citation string are the
  publisher's deposit travelling together, while the PMID beside them is
  PubMed's own later match, so a disagreement is usually PubMed's match
  being wrong. Two caveats kept in mind: the 2% is measured only where we
  hold the cited record, and our slice is recent PubMed, where matching
  is freshest and most error-prone; and Crossref's unstructured
  references are not measured yet. The 43 that corroborate nothing stay
  on the order (the user's call). One case the rule leaves open: two DOIs
  of one reference, both repeated in its text, naming different works —
  nothing corroborates over the other, so the order decides, and an
  incremental merge that attaches one of them may move the arc where a
  full merge would not. None seen; noted rather than ruled on. Found by tracing a divergence between
  the two merges on the `pubmed26n1335` hold-out, where a reference
  carrying `10.1126/science.aay9097`, its matching citation text, and a
  PMID and PMC id belonging to another paper landed on the other paper.
  Merge version 6.
- 2026-09-19, a stub's name says who wrote it out — decided with the
  user, who asked whether the batch could be made exactly right from the
  data already on disk, without an index and without re-reading records.
  It can, and the missing fact was small: each citer of a stub's name is
  now a record with two marks — whether that reference's own citation
  text repeats the name, and whether the reference named a held work too.
  One record's several references to the stub are one citer, their marks
  joined: the text mark by *or*, since one reference writing the name out
  corroborates it, and the held-beside mark by *and*, since one reference
  naming nothing held makes the name its own. Merge version 7.

  The reason a stub's own row is enough is worth keeping. §6 picks a
  *name* — a DOI the text repeats, else the lowest namespace — and then
  follows it to a work. Dividing a stub changes no name's namespace and
  no name's text mark; it changes only which work a name names. So the
  name §6 picked before the division is the name it picks after, and
  that name was this row's, since the reference landed here. A name of
  some other work lost before and cannot win now. What escapes the
  argument is a name *disappearing* — the 2026-09-18 entry's open case
  and one more — and those are refused, not guessed (serving plan §11,
  item 28).
- 2026-09-19, the split rule (§11, item 3) — decided with the user on
  the fourteen splits the corpus holds, not on which rule is plainest.
  **The part that holds the records keeps the id; the part that is names
  only becomes a stub of its own; retire-and-mint is the fallback, for
  where no part holds records or several do.** All fourteen have exactly
  one record-holding part and one bare stub, and the recurring shape is
  a DOI wrongly associated with a PMID: `#509692` was minted for
  {`doi:…ijbiomac.2025.146545`, `pmid:41478481`}, the record now says
  `…150018`, and the old DOI leaves alone, cited by one reference. Under
  the old rule that live work lost its id and its public id for a name
  that was never its own. Retiring an id is for a work that ended.

  Three things fell out of it, and they are why the change is small.
  The registry needs no new line kind: a keeper reuses its id, or merges
  it into an older id it also claims, both of which the log already
  says, and only the fallback writes `split`, which already carried its
  parts — so the merged layer's format and every line written before
  today are untouched. The full merge and the batch now *agree* where
  they did not: a held work and a stub contesting an id is the shape the
  batch has always settled by keeping the id and minting the stub for
  the leftover names, and the full merge was splitting it; the rule the
  full merge now follows is the batch's own, which is what makes the
  assignment ask how many of the clusters it is given hold records. And
  the fallback is no longer a stop: the batch performs it, the parts'
  citers settled again by §6 through the row's own evidence (serving
  plan §11, items 24 and 28).

- 2026-09-18, absence is not an assertion — decided with the user, who
  asked for it as "data that is *there* supersedes data that is missing on
  a per-record basis", and for a sanity check first. Measured over the
  sample store's 242,624 records, of which 23,482 were delivered more than
  once (49,812 copies; 23,457 of them PubMed's), comparing each record's
  newest copy against its older ones:
  - **Fields the newest copy has none of, an older copy having one: 5.**
    All five are the derived `corrected` flag, from an erratum link the
    newer copy dropped. Not one title, abstract, date, venue, language,
    form, genre or author count is ever lost between copies. What does
    change between copies is 257 titles, 154 dates, 281 abstracts, 37
    genres, 2 forms and 1 language — revisions, where the newest copy
    wins, and the rule does not touch them.
  - **Names the newest copy drops: 2 records.** Both are F1000Research
    versioned articles whose PubMed record moved from the version-1 DOI to
    the version-2 DOI (`…/f1000research.52613.1` to `.2`, and
    `…/173028.1` to `.2`). PubMed keeps one PMID with `Version` 2, so the
    two DOIs name two versions of one record. Keeping both means citations
    to either version land on the same work, and the name does not fall
    out of the registry into a stub.
  - **Relations the newest copy drops: 15 records**, every one an
    `ErratumIn` / `ErratumFor` link. These are the exception (§8): the
    serving layer does not read relations back, so the incremental merge
    cannot take their union, and the flag the link sets survives anyway.

  So the rule is nearly a no-op on today's data, which is the point of
  the check: it cannot be doing anything surprising, while the loud case
  it generalizes — the reference lists PMC replaces — cost 24,081 dropped
  registry names before it was found. The surprises it *could* hold, and
  where the line is drawn: a value that is merely shorter still wins,
  because it is there, so a truncated author list is a revision and not an
  absence; the status flags are sticky, since a source says "retracted"
  and never "not retracted", which keeps a mistaken flag until a record is
  withdrawn; and the venue fills as one id, never field by field, so a
  later copy that moved the article cannot leave a journal that does not
  exist. Two copies that both have a field the newest lacks and disagree
  would be settled by the order the sets are read rather than by stamp;
  no such case exists in the sample, where the only filled field is a
  boolean. Merge version 5, and one stop is gone with it: a record that
  drops a name no longer parts its work between two, so that batch no
  longer asks for a full merge.
- 2026-09-17, the newest deletion — accepted with the user: when several
  deletions sets name one record, the latest in path order is the one
  that counts, in both merges. It may lose information when datasets are
  merged as batches of deliveries. A deletion names a record and not a
  revision, so the order of the sets is all the rule has to go on. A copy
  newer by its stamp but delivered before the deletion is withdrawn with
  the rest, and an incremental merge of a set older than a deletions set
  the store already holds cannot bring the record back. A withdrawn
  record's names, fields and references leave the merged layer, and so
  every answer, though raw and normalized keep them. Accepted for now,
  beside the user's condition that paths once true stay findable (serving
  plan §11, item 24).

- 2026-09-19, publisher ids get a namespace of their own — decided with
  the user and built the same day (`merge::publisher`, `Name::Publisher`,
  `ingest::Key::Publisher`, merge version 8). §3.1 states the rules; this
  is the evidence behind each of them.

  *Keyed by the registrant, spelled `reg1016`.* A publisher id means
  nothing unscoped — `320` is an article number in some journal — and
  the only scope every record carries is the registrant of its own DOI.
  It is written `reg1016:S0006349525001234` and never `10.1016:…`,
  because the user's first priority was that the merged layer must not
  grow a second thing shaped like a DOI: that would be very confusing.
  `Name::parse` refuses a registrant-shaped namespace outright rather
  than calling it unknown, so the confusion cannot arrive from outside
  either. The consequence taken deliberately: a record with no DOI
  asserts no publisher name at all — 344 records, 0.335 %, all PubMed's,
  and only 22 of their 414 values are even Elsevier-shaped.

  *Written exactly as the publisher wrote it, with one exception.* No
  case folding and no punctuation stripped, because punctuation is part
  of an identifier. The exception is Elsevier's printed PII, written
  punctuated (`S0006-3495(25)00123-4`) and bare (`S0006349525001234`) for
  one article on 8,350 DOIs — a split by source rather than by publisher:
  PubMed writes the punctuated form (51,120 rows), Crossref the bare one
  (8,801), 8,345 that way round, 5 the other, none writing both for one
  article. Every one agrees once rewritten, no id normalised or raw is
  owned by more than one article, and no rewritten value collides with an
  unrelated one. The rewrite happens where an id becomes an assertion and
  not in the normalized layer, so the layer still says what the source
  said and extending the list below costs a merge and not a re-normalize.

  *Gated on the publisher, not on the shape.* The measurement argued for
  shape — the rewrite is safe on the whole sample whoever the registrant
  is — and the user overruled it: anything that goes through Elsevier's
  machinery will match Elsevier's rules, but a value that merely looks
  like a PII is not thereby Elsevier's, and another publisher's
  identifier must not be mangled to make ours tidier. The first survey
  under the rule is the argument for it. Over PubMed's 126,740 records
  and 15,101 Crossref works, 2,000 PII-shaped values sit under
  registrants that are not on the list, spread over 45 of them: 503 and
  50 under Cambridge's 10.1017, 425 under 10.3168, 192 under 10.31083,
  81 and 22 under 10.1093, 57 under 10.1074. Had the shape alone decided,
  every one of those would have been rewritten. `publisher::ELSEVIER`
  holds 10.1016 and 10.1053 and nothing else; the counter, printed by
  registrant, is how it grows, and each addition is a full merge because
  it changes the canonical spelling of names already written.

  *The DOI restated is not a publisher id, tested after the rewrite.*
  786 rows restate their own DOI only once rewritten, and the only two
  apparent cross-source disagreements in the whole sample are this rule
  failing on a punctuation variant: on `10.1016/s0002-8703(99)70115-8`
  Crossref's alternative id is the suffix unpunctuated and PubMed's PII
  is the same suffix punctuated, so testing before the rewrite keeps one
  and drops the other. The test folds case and drops the printed PII's
  punctuation from both sides, which is the only place case is folded.
  It is the single largest refusal: 45,802 of PubMed's 135,073 rows and
  5,520 of Crossref's 11,945.

  *A bare article number is not a name.* Of the 20,647 publisher ids more
  than one record carries, 18,194 are one work seen from two sources and
  2,453 are different works; 2,137 of those are bare digits with or
  without an `e`. A journal's article numbers restart, so such a value
  would fuse unrelated works: 28,724 of PubMed's rows and 2,782 of
  Crossref's are refused on it. Nor is a value that is not printable
  ASCII without whitespace (41 rows) or longer than 64 characters (4) —
  one PII slot holds a whole citation.

  What it leaves: 60,127 names from PubMed's rows and 3,636 from
  Crossref's. What it costs the user: merge version 8, so a full merge
  before the next epoch, and with it a §3.2 count that will move — a work
  with two publisher ids is now a conflict component. None of them is a
  new *shape*: a publisher id is never a record's native id, so every one
  resolves by the rules §3.3 already has, and the run cannot stop on one.
  References were left out: of 3,532,199 PubMed reference records exactly
  one carries a publisher id. OpenAlex was left out: it has no
  publisher-id slot at all.

  Later the same day the first merge run said the rules above were not
  enough. `merge identity` over the whole store stopped on a hairball of
  762 records and 1,018 names: some publisher-id slots hold an issue's
  identifier rather than an article's — `LiveAll1` on 259 Crossref
  records, `JAMP-14-1`, `IJMS-51-1`, `2025-vol6` on ten PubMed records
  each — so every article of an issue shared one name and fused into one
  work, and a handful of such values reached hundreds of records by
  chaining through the conflict components that already existed. The test
  suite was green throughout, which is the lesson: a namespace change is
  verified by a merge run, not by `cargo test`.

  The fix is a rule and not a longer list of shapes, because `JAMP-14-1`
  is no shape anyone could have predicted. A source holds one record per
  article, so a source claiming one publisher id for two of its own
  records is that source saying the value names more than one of its
  articles: the claim is rejected on every record that made it, with its
  reason, and never joins. It has to be caught before the components are
  built — the §3.3 machinery would have resolved these cases correctly,
  but only if it ever saw them, and by the time it does the component is
  a hairball, which is what the hairball guard is for. A union cannot be
  undone, so publisher ids are now held back from it until every record
  is in and a census can run (`Identity::settle`). Nine values, 307 of
  48,293 publisher assertions; the pass then resolves all 510 conflict
  components. The test counts one *source's* records and not all of them,
  because one work seen from two sources sharing a publisher id is the id
  doing its job — 18,194 of the 20,647 the census found on more than one
  record — and the literal rule would have emptied the namespace of its
  use. Bentham's 10.2174 ids are untouched by it: they are all distinct,
  and only the one marketing tag they put in the slot is refused.

  Two things the rule does not do. It is not a shape rule, so the bare
  article numbers §3.1 refuses stay refused in front of it — that kept
  31,506 publisher-id rows off the normalize surveys of 2026-09-19
  (28,724 PubMed, 2,782 Crossref) and so out of the name arena, where the
  merge's memory goes, and keeps their verdict from depending on what
  else is in the run. And it
  is itself a fact about a run, which is a divergence between a full merge
  and an incremental one (§11, item 24).

## 11. Open items

1. **Lineage.** The grouping id for a work's versions, with the OpenAlex
   location DOIs, Crossref's `has-preprint` / `has-version` /
   `is-translation-of` and PubMed's `RepublishedFrom` as its evidence.
   Whether supplements need a second grouping or only the part relation
   (§2 says the relation). Not designed.
2. **Shared names in the graph.** An edge through a shared name is
   unresolved for now; a PubMed reference carrying the PMID beside the
   shared DOI resolves through the PMID and is not affected. Whether the
   five-letters case should be five works or one is a product question.
3. ~~**The split rule.**~~ Ruled 2026-09-19, on the fourteen splits the
   corpus holds rather than on the plainness of retire-and-mint. **The
   part that holds the records keeps the id; the part that is names only
   is minted as a stub; retire-and-mint is the fallback where no part
   holds records or several do.** Every one of the fourteen has exactly
   one record-holding part and one bare stub — the recurring shape is a
   DOI wrongly associated with a PMID, the record now saying a different
   DOI and the old DOI leaving alone (`#509692`, whose PubMed record
   pmid:41478481 moved from `…ijbiomac.2025.146545` to `…150018`) — so
   under the old rule fourteen live works lost their ids and their public
   ids for a name that was never theirs. Retiring an id is for a work
   that ended, and none of these did.

   The fallback stands because the same evidence does not name a keeper:
   two stubs a reference no longer cites together (item 23) have no
   records between them, and two record-holding parts have no way to
   rank each other. It is what `pubmed26n1336` shows, and what the batch
   now performs rather than stopping on (serving plan §11, item 25).

   The registry needed no new line kind for either half. A keeper simply
   reuses the id, or merges it into an older one it also claims, which
   `merge` already says; only the fallback writes `split`, which already
   carried its parts. So the log's format is unchanged and replay of
   every line written before today is unchanged with it.

   Still open here: whether a split should ever be *avoided* — the
   stub-grouping question of item 23 is the one that makes most of them.
4. **Stubs at scale.** Expected to be DataCite DOIs, mistyped DOIs and
   works no source holds; the count per namespace is in every report.
5. ~~**`form` and `genre` tables.**~~ Done 2026-09-15, in each source
   crate's `view` module: Crossref's 30 types are the forms and imply a
   genre; OpenAlex's 25 types are the genres and imply a form; PubMed's
   61 publication types the sample showed map by descriptor UI to a genre
   with a priority or to nothing (a study design, a funding note), and a
   type outside the table stops the run — the rest of NLM's list gets
   added as the corpus shows it. Open within it: OpenAlex's `article` on
   5,726 works PubMed calls reviews is why the precedence flipped, and
   PubMed's `Publisher`-status records carry only the publisher's types.
6. **Author and venue identity.** Entity resolution problems of their own;
   the first cut promotes one source's list and one source's venue id and
   counts disagreements. The venue rule's false positives are known: 78 of
   its 138 disagreements are Crossref's electronic ISSN against PubMed's
   print one for one journal (`2380-6583` / `2380-6591`); the ISSN-L table
   would settle them.
7. ~~**Abstracts for the search sinks.**~~ Done 2026-09-16 (serving plan
   SP2): `merge::view::strip_markup`, `join_sections` and
   `inverted_index`, and a `text` method on `Contributor` giving a
   record's abstract, author names and venue name as plain text. Which
   abstract is indexed when several sources have one, and whether
   PubMed's labels go in with the text, is indexing policy (serving plan
   §11, item 11).
8. **`access`.** Measured 2026-09-15: 27 disagreements in 31,957
   comparisons, 16 of them between the old and new record of one work at
   OpenAlex; but Crossref and PubMed can only say "open", never "closed",
   so the rule is really OpenAlex's status with two corroborations.
9. **Crossref's `alternative-id` and `is-same-as`** as name sources, on
   evidence. Half of it closed 2026-09-19: an `alternative-id` that is
   not a DOI is a publisher id and a name (§3.1), 3,636 of 11,945 rows on
   the lookup batch. What is still open is the DOI-shaped half — 13,659
   of the census's, 28 of them not the work's own DOI — which the
   publisher-id rule refuses rather than promoting to a DOI assertion,
   and `is-same-as` (23) and `is-identical-to` (1), still typed relations.
10. **The registry at scale.** A binary log and a memory-mapped fold; the
    ingestion plan's M9.
11. ~~**A key for `keys`.**~~ Done 2026-09-15: `ingest::Key` has `Pmc`; no
    source looks up by it, which `Lister::batch` expresses.
12. **Translations.** A PubMed record listing two languages whose DOI's
    Crossref title is in the other language (PMID 25772389, the Spanish
    and English editions of one article) is one work to PubMed and two
    DOIs; the rules reject PubMed's DOI for want of a vernacular title.
    A language-aware clause, or the lineage of item 1, decides it.
13. **The title test's blind spot.** Two records whose titles are the
    same short word — `Erratum`, `Editorial`, `Correction` — corroborate
    any wrong name they share. Not observed on the sample; the fix is a
    minimum length for the equality clause, or a second signal (year,
    venue) once the views of MM3 exist.
14. **Reference-level assertions.** A reference carrying a held name and
    an unheld one asserts that the unheld name is that work's: 849 of
    188,027 references landing on held works, all PubMed's, on the sample
    with the update files (2026-09-18). Whether the assertion is *true* is
    still open — the merge found 338 of them are stubs of their own (§6),
    and the examples are Cochrane reviews whose PMID is one version and
    whose DOI is the review, so believing them would be wrong there;
    item 1's lineage is the answer, not a merge. What changed on
    2026-09-18 is that they are now **kept**, in the `cited_as` set (§8):
    the name, the citing work, the source, the work the reference landed
    on, the namespace that landed it, and five flags: whether the pair had
    another reference landing there, whether the name named nothing at the
    time, whether the citation text repeats a DOI of the work it landed on
    and whether it repeats this name (§6), and whether a relation rather
    than a reference named it. That is what an incremental merge
    needs to resolve such a reference again when a batch gives one of its
    names to another work — the divergence of serving plan §11, item 26,
    which the union of a record's copies made more visible (4 rows in the
    baseline harness, 19 when `pubmed26n1337` is held out). Using them for
    identity remains an open question; using them to re-resolve is not.

    Later the same day the set was widened from those 849 rows to **every
    name a reference gave** beside the work it landed on, held names
    included (serving plan §11, item 28): who cited each name, which is
    what a batch needs when a name leaves a work — a deleted record's
    PMID that references still cite — and when a work loses its last
    record and must become the stub its citers name. Stub names keep
    their citers in the stub's own row, as before.

    On 2026-09-19 the stub's own row was given the same fact the
    cited-as rows carry: each citer of a stub name is now a *record*
    with a mark — whether that reference's own citation text repeats the
    name (merge version 7). One record's several references to the stub
    are one citer, marked where any of them wrote the name out. That is
    the whole of what §6 asks the text, so a reader holding a stub's row
    can settle its references again by §6 itself rather than by an
    imitation of it, which is what serving plan §11, item 28 now does
    when a record arrives for part of a stub.
15. **The registry's size.** 94 bytes a line as JSON, 379 MB for the
    sample's four million ids; the binary form of item 10 before the
    corpus.
16. **Pass 2's memory.** 5.0 GB resident on the sample (§8), most of it
    the unresolved rows' citation text and the four million works' rows;
    the range shape of §8 bounds it, and the unresolved stream could be
    written as it is read once shards are open per range. Before the
    corpus, with item 10.
17. **Journal-issue DOIs on articles.** 12 PubMed articles carry a DOI
    Crossref registered as a `journal-issue`: the publisher deposited the
    issue's DOI on every article. Flagged as a kind conflict; whether the
    DOI should be an alias or rejected is a rule to write on evidence.
18. **Duplicate OpenAlex records disagree with themselves.** The 52 works
    with two OpenAlex records differ on access 16 times and on genre 3: the
    older record is stale. The promoted record of a source is the one
    whose native id is the promoted name (the lower W number, §3.4), which
    is the older one; the revision stamp the merged record keeps is the
    evidence for a rule that prefers the newer. Two *copies* of one record
    are settled since 2026-09-17 (§8: the newest copy speaks for all of
    it); two *records* are not. An incremental batch that
    holds one of a work's duplicates adds that record's arcs and removes
    none, because the twin's list is only in the normalized layer; a
    full merge tidies it (serving plan §5).
19. **`Sources` width.** A byte, one bit per `SourceId`, three used;
    eight is the ceiling, and the ingestion plan's planned sources and
    long tail name at least nine more. Widen to `u16` before the ninth
    source: it is written into every merged set, so that is a
    merged-layout version bump and a rebuild; the graph's label byte
    indexes a table of asserter sets and does not follow it (graph plan
    §3). Nothing to do until the ninth source is scheduled.
20. **The edges delta log.** The incremental merges of §9 write every
    change to an edge as a line in `WorkId` space — `add(from, to, by)`,
    `remove(from, to, by)`, `retire(id, into)` echoing the registry's
    `merge` and `split` — which the graph folds over its base (graph plan
    §5). Its framing and place, and whether the other merged sets get a
    delta of the same shape, are the serving plan's; designed with the
    incremental merges, after MM5. Designed 2026-09-16 (serving plan §3,
    §4, §8): one delta file per merged kind under the merged directory,
    JSON lines stamped with the batch, and `merged/journal.jsonl` with
    one line per merge run as the clock every artifact is positioned by.
    Built the same day (`merge::delta`, serving plan SP5).
22. **A record index into the normalized layer.** Three cases of the
    incremental merge stop for want of one (serving plan §5): a text
    only a record outside the batch has, a duplicate's reference list,
    and the names the citing references gave a work that lost its last
    record. Each is "read one normalized record by native id", which the
    sets, being streams, cannot do; a per-source index from native id to
    set and block is the answer, with the registry's binary form (item
    10).
23. **A stub's grouping, once the registry holds it.** A stub's names
    are the names references cited together, so a copy delivered again
    with a different list can leave a stub whose names no reference cites
    together any more, and the next full merge splits it: two new ids,
    and the stub's public id retired. After one PubMed update file held
    out of the sample and merged as a batch, the full merge split 7,234
    stubs (serving plan §11, item 27). Whether a stub keeps the names it
    was given until none is cited — stability over precision, for works
    nobody holds — is a rule to decide before daily update files.
24. **A refusal that depends on the run.** Whether a publisher id
    identifies an article is decided by how many of one source's records
    claim it (§3.1, §3.3), which is a fact about the whole run. An
    incremental merge sees only the batch and the works it touches, so a
    value a full merge refuses can be accepted in a batch — never the
    reverse, since a merged row does not carry the assertions that were
    refused. The next full merge tidies it, as it does the duplicate
    records of §11 item 18. Whether the refused values should instead be
    carried forward as a checked-in list, so a batch decides as the full
    merge did, is a question for the first daily update files.
21. ~~**The public id.**~~ Built 2026-09-16 (serving plan SP3):
    `merge::public` mints one for every id lacking it when `merge` is
    given `--public-key <file>` — a keyed BLAKE3 hash of the id, `ol:`
    and fourteen Crockford characters — into `merged/public.jsonl`; and
    `merge::journal` writes the batch line every complete run appends.
    The log fills on the first run with the key.

## 12. Sources consulted

The ingestion plan's §12; the census script and the dumps it read, in the
session's scratch space, superseded by the merge's own report at MM2.
