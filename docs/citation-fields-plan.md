# Citation fields plan

**Status:** draft 4, 2026-09-30. The survey (CF0) is done, and the user
settled draft 2's open decisions on its numbers (§3, §12). §4 is the
design CF1 and CF2 build. CF1, model 13's reading at normalization (the
title lists, the issue's own dates, pages and labels), is built and
surveyed over both whole files; it stays off main until model 13 is
whole. Draft 4 adds CF2's rules for the merge (the user, 2026-09-30).
Draft 1 (2026-09-24) was walked through on 2026-09-27; draft 2 is
superseded.
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
  123–129 (model version 13).
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
| journal, full name | `container-title`, the first of a list; the rest read, not kept | `Journal/Title`, in NLM's sentence case | the location's source name | yes, from the venue's source |
| journal, abbreviated | read, not kept (`short-container-title`, a list) | `ISOAbbreviation`, `MedlineTA` | no | no |
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

**D2. Take the more (the user, 2026-09-29).** Where everything two
records both give agrees and one gives more, the placement takes the
more: the levels are joined, and where the work sits comes from whichever
gives it. Over the 30,795,179 works whose row holds a PubMed and a
Crossref record (**M**, CF0 part 2), compared on volume, issue and where
the work sits:

| how the two compare | works | share |
|---|---|---|
| the same (227,957 only once notation is set aside) | 25,942,913 | 84.2% |
| they agree and Crossref gives more | 1,962,401 | 6.4% |
| they agree and PubMed gives more | 853,124 | 2.8% |
| they agree and each gives what the other lacks | 33,778 | 0.1% |
| a value both give differs (D3) | 1,541,995 | 5.0% |
| each gives something, nothing shared: joined unchecked | 9,684 | 0.03% |
| only one gives any, or neither | 451,284 | 1.5% |

The unchecked joins are not checked. The user: keep a record and a count,
"as we do for other things". So the promoted chain names, level by level,
the record each level came from, and the merge's report counts the joins,
with examples: every join, and those with no shared value apart.

**D3. Crossref wins a conflict (the user, 2026-09-29).** Where a value
both give differs, Crossref's stands and the difference is kept and
counted, as every field's is. The user: Crossref's data is generally the
cleaner for this system. OpenAlex, when merged, comes after PubMed. Most
conflicts are the two writing one thing two ways, not disagreeing:
PubMed's volume `20 Suppl` against Crossref's volume `20` and issue `S6`,
PubMed's issue `Pt 2` against Crossref's `2`, PubMed's pages
`E164; author reply E165` against Crossref's `E164`. Reading supplements,
parts and page lists into their own places (D8, D12) is to settle those,
and merge 17's tallies re-measure what is left. With the levels joined
(D2), an online-first deposit that gives no volume no longer needs a rule
of its own: PubMed's volume fills it.

**D4. Dates stay apart.** The user: print and online are "two independent
publication routes. one can exist without the other", and keeping them
apart at the data layer avoids "quite a lot of heartache".
- The issue's date is on the issue's level of the chain. The online date
  is on the record itself. Neither is folded into the other.
- "Print" is not the name. An electronic-only journal's issue has a date
  that is not a print date, and PubMed's `PubModel` says which media a
  journal uses. CF0 counts the dates under each model.
- Crossref's `issued` is Crossref's own earliest of print and online, not
  a third kind. 631,203 journal articles give only `issued` (**M**); it
  stands as the issue's date only where nothing else gives one, and the
  report counts how often (the user, 2026-09-29).
- **Comparing dates, and printing one.** The user, 2026-09-29: to compare
  two sources' dates where no mapping between them is known good, or
  where one lacks the date, compare each source's earliest. Where a
  mapping is known, like for like. For a print journal, Crossref's print
  date is PubMed's issue date: they agree as far as both give on
  13,156,414 of PubMed's 14,466,602 `Print` pairs. For PubMed's
  `Electronic` model, Crossref mostly gives no print date (2,127,401 of
  2,922,757 pairs, **M**). A citation prints neither the earliest nor an online date but
  the issue's, which goes with its volume. They part for an article
  online before its issue: Crossref's online date is a year before
  PubMed's issue date on 1,838,622 pairs.
- **An online date is not always a publication date.** Crossref's online
  date is two years or more after print on 9,112,606 journal articles
  (**M**): old volumes digitized and put online later. The display rule
  (§10) must never show such a date as when the work appeared, and
  `published`, the earliest, already passes over it.
- Crossref's `journal-issue` object gives the issue's own print and
  online dates, which model 13 keeps (the user, 2026-09-28) as the Issue
  level's dates. Over the whole file it differs from the article's online
  date on 9,409,481 records, and gives a print date where the article
  gives none on 1,669,210 (**M**, CF0 part 3). Where it differs it is the
  issue's month or year against the article's day: an article online on
  2026-01-24 in an issue of 2026-02, or of 2026-12 where the issue is the
  year.
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
- **Where the reading happens:** normalization, as model version 13, as
  dates and DOIs are read today. The daily path reads updates the same
  way.
- **The expansion rule is borne out** (**M**, CF0 part 2). Expanding
  PubMed's shortened end reproduces Crossref's whole range for the same
  work in 12,092,439 of 12,235,229 cases (98.8%). The 136,108 that differ
  disagree on the end page itself (`129-39` against `129-140`), not on
  the notation. 1,218 expand to before their start, and are typos
  (`209-3` where Crossref has `290-293`): kept raw, with the fallback
  below. PubMed gives its pages as `MedlinePgn` alone, never `StartPage`
  or `EndPage` (0 of 39,994,988 records); 49% of its ranges are
  shortened.
- **The grammar** (the user, 2026-09-29: "seems wise"), learned from the
  shapes CF0 counted:
  - prefixes and suffixes on page numbers: `S202-7`, `A205-6`,
    `274P-275P`, `E164`;
  - pages with a sub-page: `163-171.e2`, `424.e1-10`, `96.1-95`;
  - lists of ranges, some labelled: `526-7, 530`,
    `488-94; discussion 95-6`, `387; author reply 387-8`, `43-62; quiz
    62-3`, `519-24 contd`, `125-30, passim`;
  - roman numerals: `III-VI`, `xii-xiii`;
  - Crossref's article numbers written as a range of themselves:
    `21-00323-21-00323` (200,253 journal articles);
  - `???` between digits read as a dash, and counted: an en dash lost to
    an encoding, as in `1067???1074` (44,769 journal articles);
  - placeholders read as no value, and counted: `UNKNOWN` (1,156 PubMed
    page fields), `&NA;` (130,255 Crossref volumes). Draft 3 gave `UNKNOWN`
    14,200 and `Online first` 18,307 Crossref volumes: those were the
    counts of the shapes they were examples of (`a`, `a a`), not of the
    values. The surveys at 13 counted the values: `Online first` is on
    258 Crossref volumes and 499 issues, and is not a placeholder (§12,
    2026-09-29).

  Whatever it cannot read whole falls back the way dates do: keep what is
  certain, and the raw text beside it, counted. The user confirmed this
  on 2026-09-27 and expects the grammar to be refined later ("some churn
  on refining the parser at a later stage"), as the date grammar was
  over models 5 to 8.
- **An article number** is kept apart only where the source marks one
  (Crossref's `article-number`). An `e0123456` in a page field is a page
  and stays as written.

**D9. Journal articles, filled.** The chain holds books, series and
proceedings, but filling them waits: editors, edition and publisher place
are read and not kept by the Crossref reader, and the user has no
bandwidth for books now. `publisher` is dropped: citation styles do not
print one for a journal article, and it would come back as an entity
(§8). Model 13 reopens "no source reader changes" for five things only:
- the reading of pages;
- labels, if CF0 shows they need it;
- Crossref's `short-container-title`. The user, 2026-09-27: use it where
  there is nothing else. It becomes the journal abbreviation where PubMed
  gives none. The user tied it to the entity work: a journal's own id
  would later own its abbreviation (§8). It is on 107,434,316 of
  119,956,318 journal articles (**M**, CF0 part 3);
- the `journal-issue` object's own print and online dates (D4; the user,
  2026-09-28);
- Crossref's `container-title` and `short-container-title` kept whole, as
  written, where model 12 keeps the first of each (the user,
  2026-09-28). What a second entry means depends on the record's type
  (§4, titles), so the lists are kept as the source gives them and read
  in the view.

**D10. The free PDF link: left out.** Its source is OpenAlex, which is
not in the merge yet.

**D11. Entity ids come after this plan.** The user weighed putting ids
for authors, publishers and institutions first. Done second, they add to
what this plan builds rather than replacing it (§8).

**D12. Labels are text, with their markers read (the user,
2026-09-29).** A volume or an issue keeps the text it goes by, not a
number: 98.8% of PubMed's volumes and 95.4% of its issues are plain
digits, and 97.8% and 92.7% of Crossref's journal articles' (**M**), but
the rest carry what a citation needs. A supplement, a part or a special
issue written into a label is read out into a level of its own (D13):
`20 Suppl 2` is volume `20`, supplement `2`; `2 Pt 1` is issue `2`, part
`1`; `Supplement_1`, `3_suppl`, `S6`, `55 Spec No`. What else is written
stays as written: combined issues (`3-4`, `28/29`), `n° 229`, `2022.1`,
`[1977] VR`. The markers are learned from CF0's shapes, like the page
grammar (D8), with the same fallback.

**D13. The levels (the user, 2026-09-29).**
- **A journal's:** journal, series, volume, issue, supplement, part,
  special issue. Old journals number their volumes within a series:
  272,309 Crossref volumes read like `s2-XI` or `S5-X` (**M**).
- **A book's:** series, then book, then section, then chapter. The user:
  "book chapters are often linked to individually", and "sometimes books
  contain sections with chapters in those sections". A chapter's own
  record stays a work, reached by its DOI as now, and a chapter is also a
  level, for what a source deposits inside one: entries, components. A
  section is a level only where the source names one; most books have
  none.
- **More will turn up.** The user: books are messy, and what else the
  data holds is to be found and added as we go. A kind the data shows is
  added as a kind of its own, not forced into the nearest one; the chain
  is a list of levels, so that is one more entry, not a new shape.
- **Proceedings:** series, then proceedings, when they are filled (D9).

**D14. The journal is the chain's top level (the user, 2026-09-29).**
Journals do not nest in this data: 99.7% of Crossref's journal articles
(119,615,750 of 119,956,213) name at most a print and an electronic
ISSN, 217,933 name two of one kind, and 122,530 three or more (**M**);
none names an ISBN; a second title is the same journal's other name
(§4). Which of the few with more ISSNs are a parent series, and which a
journal's former ISSN, is not counted; the chain keeps every ISSN a
record gives, on its journal level. So the venue is promoted with the chain, by
D2 and D3, not in its own order (OpenAlex, then Crossref, then PubMed).
That changes the venue's source on 608,481 of the 30,795,179 two-source
works (2.0%, **M**), and it moves the venue's disagreement tally into
the chain's.

## 4. The shape: a chain of containers

Settled on CF0's numbers in draft 3. Each record describes its placement:
the containers it sits in, outermost first, and where it sits in the
innermost one.

- **A level** has a kind (D13), the label it goes by, as text (D12), the
  ids the record gives for it (ISSN, ISBN, DOI), and its own date. An
  issue's date belongs to the issue.
- **Where the work sits** is a list of page ranges (D8), an article
  number or, when books are filled, a chapter or an entry.
- **The journal is the top level** (D14): the venue is the chain's first
  level, not a field beside it.

Some placements:
- **An article:** journal (its ISSNs), then volume `45`, then issue `3`
  dated 2020-03, then pages 123–129.
- **A supplement article:** journal, then volume `20`, then supplement
  `6`, then pages S12–S15.
- **An old journal's article:** journal, then series `2`, then volume
  `XI`, then page 382.
- **A chapter:** book series (ISSN, number `1234`), then book (ISBN), then
  chapter 5, pages 45–67.
- **A chapter in a section:** book, then section `II`, then chapter 7,
  pages 201–230.
- **An entry inside a chapter:** book series, book, chapter (its DOI),
  then the entry's pages.

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
    // A journal's (D13).
    Journal, Series, Volume, Issue, Supplement, Part, SpecialIssue,
    // A book's, and proceedings', when filled (D9); a series above
    // either is `Series`.
    Book, Section, Chapter, Proceedings,
}

pub enum Within<'a> {
    /// Read at normalization (D8): one range or several, each labelled
    /// where the source labels it (`discussion`, `author reply`).
    Pages(Pages<'a>),
    /// Only where the source marks one.
    ArticleNumber(&'a str),
}
```

The promoted placement is the records' chains joined (D2), and each of
its levels, and where the work sits, names the record it came from. That
is the record the user asked for of every take-the-more: a join is
visible in the row, and one with no shared value is marked as such.

**Titles are text, not evidence.** The view keeps the venue as ids only
today, and its name as text read at assembly. The chain does the same: a
journal's or a book's title and a journal's abbreviation are text. That
keeps the view small, and row size matters on the corpus.

**One list, two meanings.** Crossref's `container-title` is a list, and
what its second entry is depends on the record's type (CF0 part 3 and
`ol-rht-dump-crossref-v4`, 2026-09-28):
- For a journal article, every entry names the *same* journal: the name
  in another language or script (`Математические заметки`,
  `Matematicheskie Zametki`), the same title punctuated another way
  (`TASK. Journal on…`, `TASK / Journal on…`, `TASK`), or an acronym
  (`YUJOR`). All 322 examples, from 65 journals, were of these kinds.
  None was a parent collection. They are the journal's names, which the
  entity work would give to one journal (§8).
- For a book chapter, the entries are two containers: the series, then
  the book (`Methods in Molecular Biology`, then `Phenotypic Screening`).
  Each is its own level of the chain.

`short-container-title` is a list too, often of one abbreviation in two
styles: `Med pregl` in NLM's, `MED PREGLED` in the citation indexes'
capitals. Which one a citation prints is the export's choice.

**Where each part is built:**
- Normalization (model 13) reads the values in the source's own shape:
  Crossref's `page` and PubMed's `Pagination` become `Pages` (D8), and a
  label's markers are read out (`45 Suppl 1`, D12).
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

**Promotion.** `Fields` gains the promoted placement, joined by D2's rule,
conflicts settled by D3, each level naming its record. `venue` goes: the
journal is the chain's first level (D14). It gives the report its
present, from and disagreement tallies, by level and by pair of sources,
and the joins D2 counts: every one, and those with no shared value
apart, with examples. The owned view's `fill_from` fills the placement
**whole** from an older copy only where the speaking copy has none.
Absence is not an assertion (merge plan §8), and the venue already fills
this way.

**Dates.** `View` gains `online: Option<PartialDate>`. The issue's date is
on the chain. `published` is computed as now, from each record's
earliest, and is what dates are compared by where no mapping between
sources is known good (D4).

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
    /// PubMed's `ISOAbbreviation`, else `MedlineTA`, else Crossref's
    /// `short-container-title` (kept from model 13).
    pub venue_short: Option<String>,
}
```

- **Display strings are derived.** `given family suffix` comes from
  `Author` wherever the document, the search index and the API want it,
  so nothing that shows authors today changes shape.
- **Each text field names the record it comes from.** Authors come from
  the promoted `authors` field's source (D5). The venue's name comes from
  the record the chain's journal level came from (D14). `venue_short`
  comes from the work's PubMed record, else its Crossref record.
- **The assembler becomes a table.** `Assembler`'s `Wanted` and `fill`
  become a table over those four fields rather than three `if`s.

**API.** `Work` gains the placement (its levels, where the work sits, and
which source it came from), the online date and `venue_short`. The
structured authors stay on the server: the formatter runs there, and no
client needs them yet. `Summary` is unchanged, which keeps search results
as light as they are now.

**Settled in draft 3** (the user, 2026-09-29): the journal is the top
level (D14), labels are text with their markers read (D12), and the
level kinds are D13's.

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

- **The current merge of record comes first.** It runs all the way
  through, published, before any of this lands (the user, 2026-09-27).
  It is at model 12 and merge 16: model 12 (ol-rht 6323949, 2026-09-27)
  keeps a PubMed relation named only by a DOI, which the whole-file
  survey at model 11 stopped for.
- **Model version 12 → 13.** Pages are read (D8), and labels too if CF0
  says so; Crossref's `short-container-title` (D9) and `journal-issue`
  dates (D4) are kept. Draft 2 called
  this model 12, which the relation DOIs took first.
  - Both sources are normalized again. The v11 normalize took about 35
    minutes for both side by side on the box (**R**, ingestion plan §10).
  - The sets need room for a second copy until the v12 sets are deleted,
    which is the user's call.
  - The Jobs are the user's.
  - The harness store (`target/scratch/harness-src`) is normalized again
    at 13.
- **Merge version 16 → 17:**
  - the placement and the online date in `View` and `Fields`;
  - authors follow D5;
  - the rows' layout changes.

  This means a full merge into `merged/v13.17/`. The registry is
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
    - Compatible is split by which source gives more.
    - Nothing to compare is split in two: only one source gives
      anything, which is harmless, or each gives values the other does
      not, which is the case D2's rule would join unchecked.
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
- **Part 3, from Crossref's raw file:** what the Crossref reader reads
  and does not keep, which the normalized sets therefore cannot show.
  It is counted by `crossref normalize --survey` through the reader's
  set-aside counts, so nothing a set holds changes and no model version
  is needed.
  - A second `container-title`, by work type. The normalized record keeps
    only the first, so a journal's parent collection (§11 item 1) would
    be lost there.
  - `short-container-title`, by work type (D9).
  - The `journal-issue` object's own dates against the article's (§10).
- **Where it runs.** Part 1 runs on the laptop over the harness store:
  PubMed's harness files and the Crossref 100-shard sample, which is
  enough to build and test it. Part 2 needs a merged set holding both
  sources, so its numbers come from the box after the current merge of
  record, in a Job the user runs. Part 3 is a survey of the raw file,
  run on the box like the model 10 and 11 surveys.

*Done 2026-09-29.* Part 3 ran as `ol-rht-survey-crossref-v4` and
`ol-rht-dump-crossref-v4` (2026-09-28). Parts 1 and 2 ran as
`ol-rht-survey-placement-v1` over the merge of record at model 12 and
merge 16: exit 0 in 44 minutes, 22 GB at the container's peak, its report
at `surveys/placement/ol-rht-survey-placement-v1/report.txt` on the bulk
claim. The user settled D2, D3, D4, D8 and D12 to D14 on its numbers,
which are recorded beside each.

**CF1 — Model 13: reading at normalization.**
- *Built 2026-09-29, off main:* Crossref's `container-title` and
  `short-container-title` kept whole (D9), and its `journal-issue` print
  and online dates (D4). The sample's counts equal survey-crossref-v4's,
  the works sets grow 0.1%, and both harnesses read 0 differences on a
  v13 copy of their store.
- Pages are read in each source, by D8's grammar, with its readings
  (`???`, the placeholders) counted. *Built 2026-09-29, off main*
  (`model::pages`): any Unicode dash between pages reads as a range dash
  too, counted, as a DOI's dashes are (the general fix for PMID
  30934041's `708–718`). On the laptop's PubMed files 86,756 page fields
  read and 46 were kept, and on the Crossref sample 4,531 read and 2
  kept. Nearly all the kept were `na` (40) and `IFC` (2, inside front
  cover), words whose reading the whole files' surveys at 13 are to
  settle. Both harnesses read 0 differences.
- Labels' markers are read (D12). *Built 2026-09-29, off main*
  (`model::label`): a volume or an issue is its text as written, or the
  levels written into it, outermost first; a marker in a shape not read
  keeps the field whole, counted by shape. The placeholders are one list
  for pages, volumes and issues. On the laptop's PubMed files 191 volumes
  and 1,131 issues read into levels and 1 was kept (`47 Suppl 1 UCTN`);
  on the Crossref sample, 8 issues (`S1`). The stats count every word in
  a plain label, so the whole files' surveys list the markers not read
  yet: the laptop's files already show `PP` (157 volumes), `sup1`,
  `Forthcoming`, `1(Special)` and `SI`, left as written until the
  surveys count them.
- *Surveyed 2026-09-29* on the box at model 13 (ol-rht 7214319): no
  record of the whole Crossref file or the PubMed baseline failed. Pages
  kept: 760,013 of Crossref's (0.56%), 45,899 of PubMed's (0.12%).
  Labels read into levels: 270,699 Crossref volumes and 2,467,606 issues,
  401,958 PubMed volumes and 621,148 issues. The grammar's second pass
  answers what they listed (§12, 2026-09-29): placeholders, words saying
  the work awaits its issue (`Label::Pending`), more markers, the level's
  own name, sections, named pages (`Pages::Named`), an item's number on
  its page, `+`, page counts, J-STAGE's issue before its pages, and a
  supplement before NLM's pages (`level_in_pages`).
- *Surveyed again 2026-09-29* at ol-rht 9184e74 (`ol-rht-survey-crossref-v6`,
  `ol-rht-survey-baseline-v10`): no record failed. Crossref's pages kept
  fell from 760,013 to 349,754, 265,199 named; PubMed's from 45,899 to
  19,389. The third pass reads what they listed (§12): blanks and more
  placeholders, pages in brackets, `+` after a range, a year after a
  label, `Supl` and its kin, `Edição Especial`, `/` between levels; and
  mends three misreadings of the second: `Número Especial` read as an
  issue `Número` (6,768 Crossref issues), `Hors-série n° 18` as volume 18
  (653), `n° 57, vol. 13` as two volumes (654).
- The stats report counts each reading, as DOI readings are counted.
- Fixtures cover each shape CF0 found, and the warts it showed: `&amp;`
  in a title, a garbled one, an ISSN given twice.
- The `lib.rs` version 13 paragraph and ingestion plan §10 are written.

*Done when:* each source's normalize survey over its whole file shows 0
failures, or only the fallback the user chose, and the laptop's harness
store is normalized at 13. The issue's own dates are parsed now, and a
malformed one would stop a record, so the Crossref survey at 13 runs
before anything is normalized of record. Then the user's normalize Jobs
run on the box.

**CF2 — Merge 17: the chain.**

*The rules* (the user, 2026-09-30, all seven as proposed):
1. Two records' placements are compared level by level (journal,
   volume, issue, supplement, part and the rest) and by first page. They
   agree where every level both give is the same but for notation (`06`
   and `6`, `E29` and `e29`); a level only one gives is added (D2). A
   level both give differently is a conflict, and then Crossref's whole
   placement wins (D3) and the other's values are kept as its
   disagreements: levels of two records that disagree are never mixed.
   Joins that share no value at all are counted apart.
2. A different last page alone is a disagreement, recorded, and does not
   block the join; the pages come from the record that wins them.
3. A label saying the work awaits its issue (`PP`, `ahead-of-print`) is
   no level, so another record's volume fills in; a label kept as
   written joins as its text; a supplement read before NLM's pages is a
   supplement level.
4. The journal level carries the ISSNs of every record that agrees, with
   PubMed's NLM id and OpenAlex's source id. ISSNs that do not overlap
   at all are a disagreement on the journal, as the venue's are today.
   `venue` as a field of its own goes; the journal's name stays text.
5. A placement has one date, the issue's, which a citation prints:
   PubMed's issue date; Crossref's `journal-issue` print date, else its
   online date, else the work's print date, else `issued` alone, counted
   as the fallback. The online date is the record's own (PubMed's
   electronic date, Crossref's `published-online`). `published`, the
   earliest, stays as it is.
6. Books wait (D9): the chain holds what each record gives, its
   journal's ids, its labels' levels, its pages or article number.
7. The rows change shape, so serve moves to 9 here rather than at CF3,
   which then goes on within 9.

*Built 2026-09-30, off main* (merge 17, serve 9): `merge::placement`, the
view's `placement` and `online` in the place of its venue, `Fields`'
promoted placement (each part naming its record, the others' values
beside it, the join's class) and online date, and authors in D5's order.
Each part is tallied as a field (`journal`, `volume`, `issue`, `pages`,
`issue date` and the rest) and the report counts the joins by class.
Both harnesses read 0 differences (1,833,848 and 1,836,439 ids), and
`compare` checks the placement, as it compares rows whole. Bytes on the
laptop's harness store (110,469 held works, 1,723,245 stubs): the merged
works sets grow from 167,676,304 to 177,119,750 bytes raw (+5.6%, about
85 a held work) and from 36,467,735 to 38,001,208 compressed (+4.2%);
the served works from 123,030,634 to 124,800,424 (+1.4%). That store
holds 8 works both sources hold, so how the corpus's placements join
waits for a merge survey on the box.
- Each source's view assembles the placement and gives the online date.
- Promotion joins the chains (D2), Crossref winning a conflict (D3); the
  journal is the chain's top level and `venue` goes (D14); each level
  names its record.
- Its tallies land by level, with the joins D2 counts, and D5.
- `merge --survey` prints the placement table: present by source, from
  which source, disagreements by pair, level and kind, and the joins,
  those with no shared value apart. The conflicts left once D8 and D12
  read the notation are the re-measure D3 asks for.

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
- A full merge at 13.17, then `epoch publish`, on the local store.
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
| The journal abbreviation is PubMed's, else the one in Crossref's record (D9) | The two are written differently: NLM's abbreviations follow its own rules, while a publisher's deposit follows whatever that publisher uses. One journal can appear abbreviated two ways, and a work neither source abbreviates has none | A journal entity that owns its abbreviation (§8), or NLM's catalogue |
| Stubs cite as identifiers only | Most rows of a reference list export with no fields | The corpus holding the works they name |
| `published` kept as the derived earliest date (D4) | Search's year facet, sorting and the page still use one date | Date search over the separate dates, and a display rule (§10) |
| `publisher` dropped (D9) | Nothing for journal articles; a book's citation needs it | Books filled, or publisher entities |
| The free PDF link left out (D10) | Search results cannot point at an open copy | OpenAlex merged |
| Two records' placements joined where they agree (D2) | A citation can carry a volume from one source and pages from the other, a combination neither printed; 9,684 of 30,795,179 joins share no value to check them by | Nothing needed so far: every level names its record, and the joins are counted, the unchecked apart |
| Crossref wins a conflict (D3) | Where PubMed was right, its value is shown only as the other source's, on up to 5.0% of two-source works before notation is read | A third source (OpenAlex) or the entity work settling who is right, measured after model 13 |
| `???` between digits read as a dash (D8) | A reading of text an encoding broke; 44,769 journal articles | Crossref's file written without the loss |
| Placeholders read as no value (D8): `UNKNOWN`, `&NA;`, `None`, `n/a`, `na`, `null`, `NP`, `no`, `nil`, and a blank field (`-----`, Hangul fillers) | A placeholder's intent (not applicable, not known) is kept only in the count | A field that says why a value is missing, if a use for it appears |
| Words saying the work awaits its issue kept as a statement, not a label: `ahead-of-print`, IEEE's `PP` (and its issue `99`), `Online first`, `Forthcoming` | Often stale (NLM's IEEE records of 2011 still say `PP`); in a page field they read as no pages, counted | The merge letting another source's volume stand where one record only waits; nothing to lift at normalization |
| A page count read as no pages (`26 pp.`, 7,948 Crossref fields) | The work's length is not kept | A field for it, if a use appears (a book's citation prints one) |
| A page or a range and those after it read as written, `+` its note (`1098+`, `237-239+`; 3,333 PubMed fields) | The last page is not known | Nothing: the source does not say it |
| A year written after a label dropped (`№1 2026`, `Vol 57, 2025`, `Issue 36 (2025)`) | A label whose year disagrees with the record's date loses the disagreement | Comparing the year with the date before dropping it, if a use appears |
| An item's number read before a whole last page: `36.1-37`, the first item on page 36 to page 37 (BMJ's journals; 105,100 Crossref fields, 908 PubMed) | NLM writes Current Protocols' sub-pages the same way, shortened (`1.1-11` is 1.1 to 1.11), which reads as pages 1 to 11; those with the end before the page (`13.1-11`) are kept | The whole files' examples, by source |
| The record's issue before its pages dropped only where it is the record's own (`2_263-2_285` in issue `2`, J-STAGE) | A field whose issue differs, or a record with none, is kept as written | Nothing needed: the check is the record's own issue |
| Named pages kept as named (`IFC`, `Cover`, `Toc`; 265,199 Crossref page fields, 122,983 of them `IFC`; PubMed's `Online`, American Family Physician's online-only items) | A named page has no number to sort or compare by | Nothing: it is where the work sits (the user) |
| Pages and labels the grammar cannot read whole (D8, D12) | Kept as written beside what is certain, not as parts; a citation prints them as written | The grammar refined as the counts show, as the date grammar was |
| An issue that is `S` and a number read as a supplement (D12): `S6`, `s73`, `S 01` | Where a publisher means something else by it (a section, a series), the level is misnamed; counted apart, not checked | The whole files' examples, and a publisher's own word where one disagrees |
| A volume `s` and a number, a hyphen and a roman numeral read as a series and a volume (D13): `s2-XI` | The same, for 272,309 Crossref volumes; a volume in digits (`S1-2`) is left as written, not having been counted | As above |
| A reference's volume and issue stay as written | The cited-as evidence compares a reference's `20 Suppl 2` as text, not as volume 20 | Reading them too, when the merge's cited-as evidence compares levels |
| A number sign names the field's own level: `n° 93` in a volume is volume 93 (411,000 Crossref volumes, Cairn's), though it counts issues (the user: "keep as volume") | The volume of such a journal is periodical-shaped; a chain compared with another source's volume and issue may disagree on which is which | The entity work's journals, which know how each journal numbers |
| A section is named, before its marker or after it, or lettered before the field's number (`Laryngol Sect`, `Sect B 1`); anything else with a section is kept | A statute's `Section 38-1322` and a section with another marker stay text | The whole files' examples |

## 10. Held for later

- **Date search and the date a page shows (D4).** Search that knows a
  work has an issue's date and an online date, and a rule for which date
  a page shows. The user expects both. The rule must not show an online
  date as when a work appeared: on 9,112,606 Crossref journal articles it
  is two years or more after print, a digitized backfile's.
- **Containers as works** (merge plan §11 item 25, answer C). The chain's
  shared levels become works, and each part is linked to its container.
- **Books filled.** Editors, edition and place (the Crossref reader, a
  model version), chapters and entries, and the book levels of the chain.
- **The journal abbreviation from NLM's catalogue,** which maps ISSNs to
  abbreviations for every journal NLM indexes. It is a table, not a
  source, and is not designed.
- **The export plan:** the CSL-JSON mapping from the chain, RIS, BibTeX
  and the formatter (hayagriva in-process was the recommendation of
  2026-09-24).
- **The page and label grammars' tail, for polish** (the user,
  2026-09-30: "so long as we are keeping track of this tail for polish at
  a later time"). After three passes the whole files' surveys at 13
  (`ol-rht-survey-crossref-v7`, `ol-rht-survey-baseline-v11`, reports on
  the claim under `surveys/normalize/`) failed no record; what they still
  keep as written, counted and printed as written, the commonest first:
  - *Crossref pages* (348,572 kept, 0.19%): JNCI's lettered items
    `871-b-871` (51,330); `69-0` on Bloomsbury chapters (41,211), meaning
    unknown; IEEE conference pages with their volume, `28-33 vol.1`,
    `1762-1768 Vol. 3` (47,209); Routledge's `I_5-I_28` (18,207); an
    article id in the page field, `archdischild-2025-329859` (14,735);
    JSME's underscored numbers, `_104-1_-_104-10_` and kin (about
    34,000); J-STAGE's `After2_18`, `455-456_2`, `52_2-52_2`, `A59_6`,
    `4431_12` where the issue is not the record's (about 36,000); IOP's
    `2541-2544-4` (6,266); SPIE's article number with its page count,
    `84472L-84472L-8` (5,495); `192-IBC` (3,910).
  - *Named pages that may be numbered:* Project MUSE's front matter
    `ebi-ebi`, `ebi-ebii` (48,217), `fmi-fmviii`, `a-a`, `A-B`, `First`.
  - *PubMed pages* (19,010 kept, 0.05%): ends before their start
    (`497-50`, 3,828, typos in every example CF0 checked);
    `2169-2179.e1-4` (1,375); `M49-6`, `I/285-307`, `S6-1-6`, `ORL-875-80`;
    `Varia, 834-41`; `suppl 2 p.`; `10 pages following 512`; `s:93`; and
    Current Protocols' shortened sub-pages, `15.1-12` kept and `1.1-11`
    misread as pages 1 to 11 (§9).
  - *Crossref volumes* (17,757 kept): `Original Series, Volume 9`
    (6,098); a month or an edition after the number, `N° 563 -
    Septembre`, `N° 7 – réédition` (2,719); `no.4 (1976:Jul)`; `n° 349 -
    350`; the issue in brackets or after `n.`, `vol. 15 (3)`, `v.38 n.2`
    (1,221); `Year: 17 - Number: 101`; `n° HS 4` (hors-série); a year
    before a volume read as a second volume, `2023 11` (190). Plain
    words not read: `os`, `ns` (old and new series, 38,646), `n o 62`.
  - *Crossref issues* (18,971 kept): `4, Part 1 of 2` (2,193); cumulative
    numbers in brackets, `Nr 4 (44)`, `№ 4 (170)`, `Vol. 2(34)` (about
    6,000); `Supplement 1 2S`; `N° 9, 1`; a date after the label,
    `Volume 36, 8 Jul 2025`; a month, `ISSUE 2 (JUNE)`.
  - *PubMed volumes and issues* (5,526 kept): named supplements, `43
    Suppl 2 UCTN`, `Suppl 4 European Workshop on…`, `36 Suppl Fur
    Praxis`; `34 Abstr Suppl 1`; `22(9)Suppl`, `9(4)Pt 2`; `Su` for a
    supplement (`56 Su 1 Pt 1`); `E-Suppl`, `Spec Suppl`, `[Special
    Issue]`; `Suppl. 1 to No. 6`, and `Suppl to 11` misread as a
    supplement labelled `to` (46; Crossref's `Suppl au 621`, 128). Plain
    words not read: `Chapter` (3,798 volumes), `Appendix`, `Vyp.` (1,625
    issues), NAR's `Database issue` and `Web Server issue`, `Half`.

## 11. Open items

1. **The conflicts left after model 13** (D3). Merge 17's tallies say how
   many of the 1,541,995 conflicts were notation that D8 and D12 now
   read, and which remain.
2. **The journal articles with more than two ISSNs,** 122,530, and the
   217,933 with two of one kind (D14): which are a parent series and
   which a journal's former ISSN. Kept on the journal level meanwhile;
   not counted.

Settled in draft 3 (2026-09-29): the journal as a level (D14), and
Crossref records that give only `issued` (D4).

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
- **2026-09-27, draft 2 reviewed.** The user committed it and answered
  its five open points:
  - `ol-rht survey placement` is fine as the name.
  - The journal as a level or as the venue: "idk", to be settled from
    the survey. A journal sometimes has a parent collection, which CF0
    part 3 looks for.
  - Crossref's `short-container-title` is kept in model 12 and used where
    PubMed gives no abbreviation. The user tied it to the entity work.
  - The date-style fallback for pages is confirmed, and later parser
    churn is expected.
  - Titles stay text.
- **2026-09-27, model 13, not 12.** Model 12 went to a PubMed relation
  named only by a DOI (ol-rht 6323949). The whole-file merge survey at
  model 11 stopped on the SVIN erratum for want of it, so it comes before
  the merge of record. The page reading and `short-container-title` are
  therefore model 13, and this plan's merge is into `merged/v13.17/`. The
  entries above say 12 as it was said then. CF0 runs on the store at
  model 12 and merge 16.
- **2026-09-28, the numbering checked, and CF0 part 3.** Model 12 did not
  settle the SVIN erratum after all: the baseline's copy of PMID 41585761
  has no `CommentsCorrections`, as NLM added them after the cut. Merge 16
  amended did (ol-rht f782d84, Crossref's list of deposits whose aliases
  are what they correct), with no version bump. So the merge of record is
  at model 12 and merge 16, after `ol-rht-merge-survey-v10`. Main and
  every worktree are at or below model 12, merge 16 and serve 8, so this
  plan's 13, 17 and 9 stand. The other session left them for this plan
  on purpose.
  - CF0 part 3 (`ol-rht-survey-crossref-v4`, the whole file, exit 0 in
    every share) found, among 119,956,318 journal articles: a second
    `container-title` on 173,277, `short-container-title` on 107,434,316,
    and the `journal-issue` dates as in D4. It also found 5,064,105 other
    records with a second title, a chapter's series and then its book.
  - The user: keep the `journal-issue` dates in model 13 (D4). The
    examples of the 173,277 come from `ol-rht-dump-crossref-v4`.
- **2026-09-28, the second titles, and the whole lists kept.**
  `ol-rht-dump-crossref-v4` printed 322 journal articles with a second
  `container-title`: 65 journals from 18 publishers, found in the first
  ~50 shards of each share, so a few publishers dominate. Every second or
  third title was the same journal's name in another language or script,
  punctuated another way, or as an acronym; none was a parent collection
  (§4, one list, two meanings). The user agreed that model 13 keeps
  `container-title` and `short-container-title` whole, as written (D9).
  This keeps a chapter's book behind its series, a journal's other names
  for the entity work, and both abbreviation styles. On the way it
  showed `&amp;` in titles, a garbled one (`g?ologiques`) and an ISSN
  given twice, which the text's cleaning and CF1's fixtures are to meet.
- **2026-09-29, draft 3: CF0 done, the open decisions settled.** The
  placement survey ran over the merge of record at model 12 and merge 16
  (30,795,179 two-source works; §7), and the user made these calls on
  its numbers:
  - **D2**, take the more. Don't check the 9,684 joins with no shared
    value; keep a record and a count "as we do for other things". Every
    level of the joined chain names its record.
  - **D3**, Crossref wins a conflict: "the data is generally cleaner for
    our current system to deal with".
  - **D8**, the grammar as listed, and `???` between digits read as a
    dash. Expanding NLM's shortened ends reproduced Crossref's range in
    98.8% of cases.
  - **D12**, labels are text, with supplement, part and special-issue
    markers read into their own levels.
  - **D13**, the levels. The user: "series, then book, then book chapter.
    book chapters are often linked to individually". A chapter is a
    work and a level both. Then: "sometimes books contain sections with
    chapters in those sections… anything else we'll need to discover and
    modify as we go". So a section sits between book and chapter where a
    source names one, and a new kind the data shows is added as its own.
  - **D14**, the journal is the chain's top level; the venue's source
    moves on 2.0% of two-source works.
  - **D4**, the issue's date for a citation, and `issued` alone as the
    fallback. The user, on comparing dates: "i'd take the earliest of the
    available dates from a given source to do a comparison if we don't
    have a known good mapping between the two sources, or if data is
    missing". That is `published`'s rule, and it stays for comparing and
    for when a work appeared. A citation prints the issue's date instead,
    and the two part only for an article online before its issue.
  - Also measured: Crossref's online date two years or more after print
    on 9,112,606 journal articles, digitized backfiles (§10).
- **2026-09-29, the grammar's second pass.** Model 13's first surveys of
  the whole files (`ol-rht-survey-crossref-v5`, `ol-rht-survey-baseline-v9`)
  failed no record, and counted what the grammar did not read yet. The
  user's calls:
  - More placeholders, `None`, `n/a`, `null`; but "ahead-of-print is of a
    different kind, and PP is worth investigating too". PP is IEEE's
    early-access volume, with issue `99`: every one of 157 on the laptop
    an IEEE journal's article ahead of print, and records of 2011 and
    2014 still so. Both are kept as words saying the work awaits its
    issue, with `Online first` and `Forthcoming`, the same kind.
  - More markers, and a section as a journal's level: agreed.
  - The level's own name read as a marker, and `n°` in a volume "keep as
    volume": "volumes can have periodic releases too, even if that makes
    them periodical-shaped".
  - Named pages: "named pages are a burden for search engines, but we
    can't very well just exclude them, so we have to be sure to capture
    this information". A supplement before the pages is a level of its
    own. `36.1-37` is an item on page 36 (the DOI names page 36 of issue
    7354), not issue 36. `1098+`: "can we just keep the start page?" A
    page count: "i'd be fine dropping it if it's not useful". J-STAGE's
    issue before its pages: "we can parse that out".
- **2026-09-29, the third pass.** The surveys of the second
  (`ol-rht-survey-crossref-v6`, `ol-rht-survey-baseline-v10`) failed no
  record. The user's calls on what they listed: blanks (`-----`, 4,803
  page fields of Hangul fillers), `NP`, `no` and `nil` as placeholders,
  "ok"; PubMed's page `online` "might be obvious if we look up a couple
  of records": eight of them are American Family Physician's online-only
  items, each with its volume and issue, so it stays a named page; pages
  in brackets (`[i]-[vi]`) are pages; a year after a label is dropped;
  `Supl`, `Supple`, `Supplementary`, `Edição Especial` and `/` are read;
  `237-239+` is a range and its `+`. Corrected in the same pass: three
  counts draft 3 took from a shape for its example (`UNKNOWN`, `None`,
  `Online first`).
