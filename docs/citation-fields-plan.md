# Citation fields plan

**Status:** draft 2, 2026-09-27. Nothing built. Draft 1 (2026-09-24) was
walked through with the user on 2026-09-27. Their calls are in §3 and
the decision log (§12). The shape in §4 is a sketch that the survey
(CF0) revises into draft 3 before anything else is built.
**Inputs:**
- The conversation of 2026-09-24 about exporting citations through a
  CSL processor. It found that the merge drops most of what a citation
  style prints.
- The walkthrough of 2026-09-27.
- [Merge plan](merge-plan.md) §5 (the merged record), §8 (copies) and
  §11 items 6, 22 and 25 (containers and parts).
- [Serving plan](serving-plan.md) §3 and §5 (the works index, the
  incremental merge).
- [Ingestion plan](ingestion-plan.md) §10 (model versions).
- The citation lookup plan, draft 1, in worktree `citation-matching`,
  uncommitted as of 2026-09-27.

Figures are marked as the record-index note marks them: **R** read from a
report or a plan, **?** not measured. The survey that measures the **?**
is CF0 (§7).

## 1. What this is for

The user named the needs on 2026-09-27:
- search improvement;
- the citation lookup;
- exporting search results;
- running citation data through a formatter;
- downstream, checking that a language model's citations name real
  works.

All of them rest on the same machinery. A work needs its authors split
into family and given names. It needs the containers it was published in
(journal, volume, issue; series, book), where it sits in the innermost one
(pages, article number), and its dates. That is what a CSL processor makes
a formatted citation from, what the lookup checks a pasted citation
against, and what a hallucination check compares.

Today a served work has its title, one display string per author, the
journal's name, the earliest date any source gives, and its identifiers.
Nothing else reaches it.

The sources give the rest. They lose it in two places:

- **Values the merge never looks at.** Volume, issue, pages, article
  number and the dates of the issue and of online publication are in
  every source's normalized record. None of them is in a record's
  *view*, the evidence the merge keeps per record and promotes fields
  from (merge plan §5). The view keeps one date per record, the
  earliest.
- **Strings the join never takes.** The works index gets its author names
  and journal name by reading the promoted records' text at assembly
  (`Contributor::text`, `merge::document`). That text turns each author
  into one display string and has no journal abbreviation.

**What changed from draft 1:**
- The shape is a chain of containers that works for journals, books,
  series and proceedings alike, not a journal-shaped group (§4).
- Page ranges are read at normalization, and `123-9` is stored as
  123–129 (model version 12).
- The issue's date and the online date stay apart.
- `publisher` is dropped.
- A survey comes before any of it.

**Not in this plan:**
- The CSL-JSON mapping, the export routes and the formatter. They come
  after this plan and depend on it.
- **Stubs**, which have no records and so no fields at all. Most rows in
  any reference list are stubs. Exported, they are identifiers and
  nothing else. Filling them is the corpus's job.
- **Books filled.** The chain holds a book's levels, but filling them
  waits (D9).
- **Entity ids** for authors, publishers, institutions and journals. They
  come after this plan (§8).

## 2. What the sources give, and what reaches the row

Read from the normalized types at model version 11 on 2026-09-27
(`source-crossref`, `source-pubmed`, `source-openalex`, each
`normalize.rs`). The merge of record reads PubMed and Crossref. OpenAlex is
not in it yet, so its column describes what will arrive.

| field | Crossref | PubMed | OpenAlex | in the served row today |
|---|---|---|---|---|
| authors, split | `given`, `family`; organisations whole; since model 10, a given name alone | `LastName`, `ForeName`, `Initials`, `Suffix`; collectives whole | no | no: one display string each |
| authors, as printed | — | — | `raw_author_name` | no |
| journal, full name | `container-title` | `Journal/Title`, in NLM's sentence case | the location's source name | yes, from the venue's source |
| journal, abbreviated | read, not kept (`short-container-title`) | `ISOAbbreviation`, `MedlineTA` | no | no |
| journal, ids | ISSN | ISSN, `ISSNLinking`, `NlmUniqueID` | source id, ISSN-L | yes, as the view's venue |
| volume, issue | as written | as written | `biblio` | no |
| pages | `page`, a range as written | `Pagination`: `StartPage`/`EndPage`, `MedlinePgn` (`123-9`), or both | `biblio.first_page`, `last_page` | no |
| article number | `article-number` | in the pagination or an `ELocationID`, not marked as such | in `first_page`, not marked as such | no |
| part of a set | `part-number` | — | — | no |
| the issue's date | `published-print`; the `journal-issue` object's own dates are read, not kept | `JournalIssue/PubDate`, with `PubModel` saying which media the journal uses | `publication_date` only | no: folded into the earliest |
| the online date | `published-online` | `ArticleDate` | — | no: folded into the earliest |
| ISBN | yes | — | — | no |
| editors, edition, place | read, not kept | — | — | no |

Three rows need saying plainly:

- **The date a work shows is not the date a citation prints.** Each
  source's view takes its earliest date, and the merge promotes the
  earliest of those. A citation prints the year of the issue next to that
  issue's volume. An article online in December 2019 and printed in
  volume 45 in 2020 would cite as `2019;45(3)`, a year its volume is not
  from. On the sample the promoted year disagreed between PubMed and
  Crossref on 4,078 of 39,934 works, 4,040 of them by one year, which the
  merge plan reads as online-first against the issue (**R**, merge plan
  §5).
- **OpenAlex's author names are its author profiles' names.** The text
  would read `authorships[].author.display_name`, the name OpenAlex's
  resolved author goes by. That need not be the spelling on this paper;
  `raw_author_name` is. The normalized record keeps both.
- **The sources are nested underneath, and the JSON is flat.** Crossref's
  deposit schema nests journal, volume, issue and article; book series or
  set, book and chapter; proceedings series, proceedings and paper. It
  deposits pages as a first and a last page. The REST JSON we read
  flattens all of it into `container-title`, `volume`, `issue` and `page`.

## 3. Decisions

The user's calls of 2026-09-27. Where a call waits on the survey, the
survey is named.

**D1. Where, and in what shape.** In each record's view, as parsed values,
in a shape that works for any container: a chain of containers, outermost
first, and the work's place in the innermost one (§4). Keeping the values
as text read at assembly "defeats the whole purpose". The user's reason
for the chain: a structure built up around journals, with everything
else special-cased on it, fits nothing else, and a layer or two of
container nesting gets in the way. By the time that shows, sunk cost
argues against changing the format. The user: raw text beside the parsed
values may help debugging on sample data, "but not for the actual
ingestion process". What a source wrote stays in its raw file. The
normalized record keeps the reading, and keeps the raw text only where
the reading cannot be made whole, as dates do (D8).

**D2. Promotion across records: waits on CF0.** The user's inclination
is: where everything two records both give agrees and one gives more,
take the more. Draft 1's alternative took the whole placement from one
record, so that no citation is printed that no source printed. CF0 sorts
every pair of records into four cases:
- **same;**
- **compatible:** everything both give agrees and one gives more, so the
  user's rule applies;
- **conflicting:** a value both give differs, so D3 decides and the
  disagreement is counted;
- **nothing to compare:** they share no value, so the rule would join
  them unchecked.

The counts decide the rule, and whether "compatible" needs at least one
shared value.

**D3. Which record is preferred: tentatively agreed; CF0 confirms.**
Among the records that give a volume or an article number: Crossref,
then PubMed, then OpenAlex. If none gives either, the same order over all
of them.
- **Crossref** is the publisher's own deposit, usually updated when an
  issue is assigned.
- **PubMed** is NLM's record of the issue.
- **OpenAlex** is largely taken from the other two.

Preferring a record with a volume keeps an online-first deposit from
beating the issue it was later printed in.

**D4. Dates stay apart.** The user: print and online are "two independent
publication routes. one can exist without the other", and keeping them
apart at the data layer avoids "quite a lot of heartache".
- The issue's date is on the issue's level of the chain. The online date
  is on the record itself. Neither is folded into the other.
- "Print" is not the name. An electronic-only journal's issue has a date
  that is not a print date, and PubMed's `PubModel` says which media a
  journal uses. CF0 counts the dates under each model.
- Crossref's `issued` is Crossref's own earliest of print and online, not
  a third kind. CF0 counts the records that give only `issued`, whose kind
  is unknown.
- `published`, the earliest date, stays as it is, for now as a value
  derived from the others. It keeps driving the year facet, sorting and
  the API. Date search over several dates, and the rule for which date a
  page shows, are held for later (§10). The user expects both.

**D5. Authors: Crossref, then PubMed, then OpenAlex.** Today's order puts
OpenAlex before PubMed (`build.rs`, `AUTHORS`). Until OpenAlex is merged
this changes nothing. When it is, a work with no Crossref record takes
PubMed's split names over OpenAlex's unsplit ones. It is a compromise
(§9): the user expects the persons work to rework it.

**D6. OpenAlex's names as printed.** The text takes `raw_author_name`, not
the profile's `display_name`, for display and citation alike.

**D7. We never split a name ourselves.** A name no source split stays
whole, and a CSL processor prints it as given (CSL's `literal`). That
covers Crossref's organisations and its given-name-only authors
(`Siddhartha`, `Nguyen Minh Duc`, `F.`, `ASDEX Upgrade Team`; 81 records
of the March 2026 file, **R**), PubMed's collectives, and OpenAlex's
names. A split name keeps family, given (PubMed's `ForeName`, else its
`Initials`, as the display name does now) and suffix. Particles
(`van der`) stay in the family name.

**D8. Pages are read at normalization and stored whole.** The user:
`123-9` "has to become 123-129 when stored", and what a later formatter
does with a range is an access pattern, not what gets stored.
- **Where the reading happens:** normalization, as model version 12, as
  dates and DOIs are read today. The daily path reads updates the same
  way.
- **The expansion rule** is checked against evidence before it is built,
  as the date grammar was: a notation is read from what other records
  show, not from a handful of examples. The evidence is independent.
  On works both sources hold, PubMed writes NLM's shortened end and
  Crossref, from the publisher, often writes the whole range. CF0 counts
  how often expanding one reproduces the other.
- **Shapes that are not a plain range** (`iii-xiv`, `S12-5`,
  `123-9; 131`) get a grammar learned from CF0's counts. Whatever that
  grammar cannot read whole falls back the way dates do by default:
  keep what is certain, and the raw text beside it, counted. The
  fallback is the user's to confirm on CF0's numbers.
- **An article number** is kept apart only where the source marks one
  (Crossref's `article-number`). An `e0123456` in a page field is a page
  and stays as written.

**D9. Journal articles, filled.** The chain holds books, series and
proceedings, but filling them waits: editors, edition and publisher place
are read and not kept by the Crossref reader, and the user has no
bandwidth for books now. `publisher` is dropped: citation styles do not
print one for a journal article, and it would come back as an entity
(§8). Model 12 reopens "no source reader changes" for the reading of
pages, and for labels if CF0 shows they need it, nothing else.

**D10. The free PDF link: left out.** Its source is OpenAlex, which is
not in the merge yet.

**D11. Entity ids come after this plan.** The user weighed putting ids
for authors, publishers and institutions first. Done second, they add to
what this plan builds rather than replacing it (§8).

## 4. The shape: a chain of containers

A sketch for CF0 to revise. Each record describes its placement: the
containers it sits in, outermost first, and where it sits in the innermost
one.

- **A level** has a kind (journal, volume, issue, supplement, part,
  series, set, book, proceedings), the number or label it goes by, the
  ids the record gives for it (ISSN, ISBN, DOI), and its own date. An
  issue's date belongs to the issue.
- **Where the work sits** is a page range, an article number or, when
  books are filled, a chapter or an entry.

Some placements:
- **An article:** journal (its ISSNs), then volume `45`, then issue `3`
  dated 2020-03, then pages 123–129.
- **A supplement article:** journal, then volume `45`, then supplement
  `2`, then pages S12–S15.
- **A chapter:** book series (ISSN, number `1234`), then book (ISBN), then
  chapter 5, pages 45–67.

```rust
/// Where a record says the work was published: the containers it sits
/// in, outermost first, and where it sits in the innermost.
pub struct Placement<'a> {
    pub chain: SmallVec<[Level<'a>; 3]>,
    pub within: Option<Within<'a>>,
}

pub struct Level<'a> {
    pub kind: LevelKind,
    /// What the container goes by: `45`, `3`, a supplement's `2`.
    pub label: Option<Label<'a>>,
    /// The container's ids as the record gives them: ISSN, ISBN, DOI.
    pub ids: SmallVec<[ContainerId<'a>; 2]>,
    /// The container's own date: an issue's.
    pub date: Option<PartialDate>,
}

pub enum LevelKind {
    Journal, Volume, Issue, Supplement, Part,
    Series, Set, Book, Proceedings,
}

pub enum Within<'a> {
    /// Read at normalization (D8).
    Pages(Pages<'a>),
    /// Only where the source marks one.
    ArticleNumber(&'a str),
}
```

**Titles are text, not evidence.** The view keeps the venue as ids only
today, and its name as text read at assembly. The chain does the same: a
journal's or a book's title and a journal's abbreviation are text. That
keeps the view small, and row size matters on the corpus.

**Where each part is built:**
- Normalization (model 12) reads the values in the source's own shape:
  Crossref's `page` and PubMed's `Pagination` become `Pages`, and labels
  are read if CF0 shows they need it (`45 Suppl 1`).
- Each source's view (merge version 17) assembles the chain from those
  fields and the record's form. For example, a journal article's volume
  and issue become its Volume and Issue levels.
- Normalized records stay source-shaped, as every field is. The chain is
  the merge's common language, as the view is.

**How it grows into containers as works** (merge plan §11 item 25, answer
C). Two parts whose chains agree down to the issue sit in the same issue.
C turns that shared level into a work and links each part to it with
`is-part-of`. No reshaping is needed. Item 25 notes the parts' volume,
issue and pages were "not yet in the views"; the chain puts them there.

**The export maps the chain onto flat fields.** CSL has a container, a
collection and a few numbered parts, and BibTeX is similar. Deep nesting
is lost there, at the edge, not in what is stored.

**Promotion.** `Fields` gains `placement: Option<Field<Placement>>`,
promoted by D3's order and D2's rule. It gives the report its present,
from and disagreement tallies, by level and by pair of sources. The
owned view's `fill_from` fills the placement **whole** from an older copy
only where the speaking copy has none. Absence is not an assertion
(merge plan §8), and the venue already fills this way.

**Dates.** `View` gains `online: Option<PartialDate>`. The issue's date is
on the chain. `published` is computed as now, from each record's
earliest.

**Text (merge crate, served row).** `merge::view::Text` changes:

```rust
pub enum Author {
    Person { family: String, given: Option<String>, suffix: Option<String> },
    /// A name no source split (D7).
    Whole(String),
}
pub struct Text {
    pub abstract_text: Option<String>,
    pub authors: Vec<Author>,          // was Vec<String>
    pub venue: Option<String>,
    /// PubMed's `ISOAbbreviation`, else `MedlineTA`.
    pub venue_short: Option<String>,
}
```

- **Display strings are derived.** `given family suffix` comes from
  `Author` wherever the document, the search index and the API want it,
  so nothing that shows authors today changes shape.
- **Each text field names the record it comes from.** Authors come from
  the promoted `authors` field's source (D5). The venue comes from the
  venue's, as now. `venue_short` comes from the work's PubMed record.
- **The assembler becomes a table.** `Assembler`'s `Wanted` and `fill`
  become a table over those four fields rather than three `if`s.

**API.** `Work` gains the placement (its levels, where the work sits, and
which source it came from), the online date and `venue_short`. The
structured authors stay on the server: the formatter runs there, and no
client needs them yet. `Summary` is unchanged, which keeps search results
as light as they are now.

**Open, for draft 3:**
- **The journal as a level or as the venue.**
  - *(a)* The chain could start below the venue, which stays promoted
    as today (OpenAlex, then Crossref, then PubMed).
  - *(b)* The venue could be the chain's outermost level, promoted with
    it. That is the generic answer, and it makes the ISSNs part of the
    agreement D2 needs. But it changes the venue's promotion order to
    D3's, and the venue's disagreement tally with it.
  - CF0 counts how often the venue's promoted source differs from the
    record D3 would choose.
- **Labels as numbers or as text.** CF0 shows leading zeros, roman
  numerals, years as volumes and ranges of issues.
- **The level kinds.** The list above is from Crossref's forms
  (`kind.rs`, `Form::is_container`) and its deposit schema. CF0 shows
  which appear.

## 5. The incremental merge

**The placement needs nothing new.** It is in each record's view, which
is in the row, and the batch re-runs `promote` on the component as it
does for every other field. The rows the batch writes carry it, and so
does the works delta.

**The text needs one generalisation.** `Engine::text_for`
(`serve/src/incremental.rs`) matches three fields by name. It fetches
each from the batch, from the row that held it before, or from the
normalized layer through `Records`. It becomes the same table as the
assembler's.
- `venue_short` comes from a record other than the venue's, so a batch
  may have to read one more record, which `Records` already does.
- Where no reader is given, the batch stops for a full merge, as it does
  today (merge plan §11 item 22).
- `Incoming::absorb` fills the new field the way it fills the others.

## 6. Versions, rebuilds, order

- **The current merge of record comes first.** Model 11 and merge 16 run
  all the way through, published, before any of this lands (the user,
  2026-09-27).
- **Model version 11 → 12.** Pages are read (D8), and labels too if CF0
  says so.
  - Both sources are normalized again. The v11 normalize took about 35
    minutes for both side by side on the box (**R**, ingestion plan §10).
  - The sets need room for a second copy until the v11 sets are deleted,
    which is the user's call.
  - The Jobs are the user's.
  - The harness store (`target/scratch/harness-src`) is normalized again
    at 12.
- **Merge version 16 → 17:**
  - the placement and the online date in `View` and `Fields`;
  - authors follow D5;
  - the rows' layout changes.

  This means a full merge into `merged/v12.17/`. The registry is
  untouched, and a full merge should append nothing to it. The full
  merge and `epoch publish` are the user's, locally and on the box.
- **Serve version 8 → 9:** the row's text changes. This means
  `names build`, `works build` and `epoch publish`, which the user's
  publish runs anyway after a merge.
- **The server** waits, not ready, for an epoch it can read, and hands
  over rolling (2026-09-24), so a version bump does not take the site
  down.
- **The API change is additive.** Deploy the server first. An older web
  client ignores the new fields.

## 7. Milestones

**CF0 — The survey.** A read-only command,
`ol-rht survey placement [--store <root>] [--limit N]`. It reads the
store's normalized sets at their model version and, where the store has
one, its merged set. It opens no writer.

- **Part 1, per source and per record form:**
  - which container fields a record gives;
  - each value's shape, as a signature (digits as `9`, letters as `a`,
    runs collapsed: `123-9` is `9-9`, `S12-S15` is `a9-a9`, `iii-xiv` is
    `a-a`), counted, with examples by native id, the way the date survey
    reported `MedlineDate`;
  - page ranges whose end is shorter than their start;
  - which dates a record gives (Crossref's print, online, or `issued`
    alone; PubMed's issue date read or kept, and its online date), by
    PubMed's `PubModel`;
  - how often a record's issue year and online year differ.
- **Part 2, over works whose merged row holds a record of each source.**
  Records are paired through the merged set, as the merge paired them,
  not by DOI.
  - For volume, issue, first page, last page and article number, and
    for the pair as a whole: same, compatible, conflicting, or nothing to
    compare (D2), with examples.
  - Conflicts that differ only in notation (space, case, leading zeros,
    `Pt` or `Suppl` spellings) are counted apart.
  - Pairs where one record gives a volume and the other does not, by
    source (D3).
  - Each PubMed range with a shortened end, against Crossref's range for
    the same work: expanding reproduces it, differs, or Crossref's is
    shortened too (D8).
  - The two sources' issue dates and online dates: the same, or apart by
    how much (D4).
  - How often the venue's promoted source is not the record D3 would
    choose (§4, open).
- **Where it runs.** Part 1 runs on the laptop over the harness store:
  PubMed's harness files and the Crossref 100-shard sample, which is
  enough to build and test it. Part 2 needs a merged set holding both
  sources, so its numbers come from the box after the current merge of
  record, in a Job the user runs.

*Done when:* the numbers are reported to the user, who settles D2's rule,
D3's order, the page grammar and its fallback, labels as numbers or
text, the level kinds, and the journal as a level or the venue. §4 is
then revised to match as draft 3.

**CF1 — Model 12: reading at normalization.**
- Pages are read in each source (D8), and labels if CF0 says so.
- The stats report counts each reading, as DOI readings are counted.
- Fixtures cover each shape CF0 found.
- The `lib.rs` version 12 paragraph and ingestion plan §10 are written.

*Done when:* each source's normalize survey over its whole file shows 0
failures, or only the fallback the user chose, and the laptop's harness
store is normalized at 12. Then the user's normalize Jobs run on the box.

**CF2 — Merge 17: the chain.**
- Each source's view assembles the placement and gives the online date.
- Promotion and its tallies land by level, and D5.
- `merge --survey` prints the placement table: present by source, from
  which source, disagreements by pair, level and kind.

*Done when:*
- both hold-out harnesses (`pubmed26n1334` and an update file, per the
  2026-09-19 note) read 0 differences;
- `compare` checks the placement as it checks every key;
- the bytes per row before and after are measured on the laptop.

**CF3 — Serve 9: the text.**
- `Author`, `venue_short`, D6 and D7.
- The assembler and `text_for` become tables.
- The document and the search index take the derived display strings.

*Done when:* the `serve` synthetic fixtures and `rows_tests` cover a
split name, a whole name, a given-name-only name, and `venue_short` from
a record that is not the venue's; both harnesses read 0 differences.

**CF4 — The rebuild (the user's).**
- A full merge at 12.17, then `epoch publish`, on the local store.
- A read of a few works by hand:
  - a work with two sources;
  - an online-first article;
  - a PubMed-only work;
  - a supplement;
  - a work whose authors are whole names.
- The registry appends nothing.
- Then the box.

**CF5 — Serving and the paper view.**
- The proto's `Work` gains the placement, the online date and
  `venue_short`.
- `just ts-types`.
- The JSON surface's tests.
- The paper view shows the placement under the venue (`45(3):123–129`,
  `Article e0123456`, `45(Suppl 2):S12–S15`) and nothing when there is
  none.
- Checked by typecheck and build. Then the user looks.

After CF5, the export plan: the CSL-JSON mapping from a row and its
chain, the CSL-JSON, RIS and BibTeX downloads, and the formatter.

## 8. Entity ids follow

The user expects `ol:`-style ids for authors, institutions, funders,
publishers and venues. They come after this plan (D11). Here is where
they meet it:

- **Authors.** An id per author position, beside the printed name. The
  printed name stays, because a citation prints the byline and not the
  person's known name (D6). The persons work will have to line up author
  3 in Crossref with author 3 in PubMed. If it decides the shown list
  should be the lined-up list, D5 and the author half of CF3 are
  revisited, and nothing else is.
- **Journals and series.** The chain's levels carry the ids a record
  gives (ISSN, ISBN). A journal entity would own the abbreviation, cover
  works PubMed does not hold, and make `venue_short` its fallback.
- **Publishers** are dropped here (D9) and come as entities.
- **Institutions and funders** are never printed in a citation.

Nothing here closes a door the entity work needs:
- names are never split by us (D7);
- the paper's spelling is kept (D6);
- each text field names its record;
- everything else stays in the normalized layer, as written, with who
  asserted it.

## 9. What we are compromising on

| compromise | what it costs | what would lift it |
|---|---|---|
| Authors come from one record's list, whole (D5) | Another source's names for the same positions go unused, and a list one source got wrong is shown whole | The persons work lining up positions across sources (§8) |
| Names as printed, never a person's known name (D6) | A page shows the byline's spelling, which varies across a person's papers | Entity ids for persons |
| Names no source split stay whole (D7) | Styles that print a family name and initials print these whole. That is right for `ASDEX Upgrade Team`, and wrong for `Nguyen Minh Duc` or `F.` | A source that splits the name, or a person entity with a split name |
| Journal articles filled only (D9) | A book chapter cites without editors, edition or place, and its chain holds only what the source already gives | A model version that keeps Crossref's editor, `edition-number` and `publisher-location`, and filling the book levels |
| The journal abbreviation comes from PubMed alone | A work PubMed does not hold has none, and styles that abbreviate print the full name | Crossref's `short-container-title` (a model version; §11 item 2) or NLM's catalogue |
| Stubs cite as identifiers only | Most rows of a reference list export with no fields | The corpus holding the works they name |
| `published` kept as the derived earliest date (D4) | Search's year facet, sorting and the page still use one date | Date search over the separate dates, and a display rule (§10) |
| `publisher` dropped (D9) | Nothing for journal articles; a book's citation needs it | Books filled, or publisher entities |
| The free PDF link left out (D10) | Search results cannot point at an open copy | OpenAlex merged |

D2's rule and D8's fallback are added here once CF0 settles them.

## 10. Held for later

- **Date search and the date a page shows (D4).** Search that knows a
  work has an issue's date and an online date, and a rule for which date
  a page shows. The user expects both.
- **Containers as works** (merge plan §11 item 25, answer C). The chain's
  shared levels become works, and each part is linked to its container.
- **Books filled.** Editors, edition and place (the Crossref reader, a
  model version), chapters and entries, and the book levels of the chain.
- **Crossref's issue-level dates.** The `journal-issue` object's own
  print and online dates are read and not kept. They are worth keeping
  only if CF0 shows the article's dates miss them.
- **The journal abbreviation from NLM's catalogue,** which maps ISSNs to
  abbreviations for every journal NLM indexes. It is a table, not a
  source, and is not designed.
- **The export plan:** the CSL-JSON mapping from the chain, RIS, BibTeX
  and the formatter (hayagriva in-process was the recommendation of
  2026-09-24).

## 11. Open items

1. **The journal as a level or as the venue** (§4). Decided in draft 3,
   on CF0's count.
2. **Crossref's `short-container-title` in model 12.** Model 12 happens
   anyway, so the journal abbreviation for works PubMed does not hold
   could ride on it. Its presence and its style (ISO 4 or the
   publisher's own) are **?**: the normalized sets do not keep it, so
   counting it needs the raw file. The user's call. It is a journal
   field, not a book one.
3. **Crossref records that give only `issued`,** whose date is of no
   stated kind (D4). CF0 counts them, and draft 3 says where their date
   goes.

## 12. Decision log

- **2026-09-24, draft 1.** Volume, issue, pages, article number and the
  issue's date were a flat group ("biblio") in each record's view, taken
  whole from one record. Split authors, the journal's abbreviation and
  the publisher were text. No source reader changes.
- **2026-09-27, pulled up in priority.** The user named the needs (§1),
  which "the base machinery i think is covered here" serves.
  - The current merge of record goes all the way through first, and this
    work pays its own full merge after it.
  - Journal articles only: "i don't have the mental bandwidth to pull
    books into this".
- **2026-09-27, entity ids second (D11).** The user's correctness
  instinct was to put ids for authors, publishers and institutions first.
  The concern was whether doing them second would cause rework. It adds
  to this plan rather than replacing it (§8). `publisher` is dropped, the
  one field an entity would replace.
- **2026-09-27, D1–D10 walked through.**
  - **Settled:** D5, D6 and D7 as recommended ("this will get reworked
    in the future in any case"). D9, and D10 left out.
  - **D1:** the view, as parsed values, in a shape generic over
    containers. That is the user's instinct, against a journal-centric
    format that sunk cost would later defend. The chain of containers
    was proposed and accepted: "sounds pretty good, let's move forward".
  - **D2 and D3:** wait on data. The user's inclination is to take the
    more where everything else agrees. The order was tentatively
    agreed.
  - **D4:** the issue's date and the online date stay apart.
    `published` is kept, derived, for now.
  - **D8:** pages are read and stored whole, at normalization (model 12),
    the user's choice over reading them in the view.
  - **Order:** the survey (CF0) comes first.
