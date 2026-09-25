# Citation fields plan

**Status:** draft 1, 2026-09-24. Nothing built. Every decision in §3 is a
recommendation for the user to take or change.
**Inputs:** the conversation of 2026-09-24 about exporting citations
through a CSL processor, which found that the merge drops most of what a
citation style prints; [merge plan](merge-plan.md) §5 (the merged record),
§8 (copies) and §11 items 6, 22 and 25; [serving plan](serving-plan.md) §3
and §5 (the works index, the incremental merge).

Figures are marked as the record-index note marks them: **R** read from a
report or a plan, **?** not measured, with the command that would measure
it in §8.

## 1. What this is for

A formatted citation — APA, Vancouver, Nature, whatever a user picks —
is made by a CSL processor from a handful of fields: the authors split
into family and given names, the title, the journal's full and
abbreviated names, the volume, the issue, the pages or article number,
the date that goes with that issue, and for books the publisher. The
processor is only as good as those fields. Today a served work has its
title, one display string per author, the journal's name, the earliest
date any source gives, and its identifiers. Nothing else reaches it.

The sources do give the rest. They are lost in two different places:

- **Short values that the merge never looks at.** Volume, issue, pages,
  article number and the issue's own date are in every source's
  normalized record and are not in a record's *view* — the evidence the
  merge keeps per record and promotes fields from (merge plan §5).
- **Strings that the join never takes.** The works index gets its author
  names and journal name by reading the promoted records' text at
  assembly (`Contributor::text`, `merge::document`). That text flattens
  every author to one display string and has no journal abbreviation or
  publisher.

This plan carries both through: the short values as evidence with a
promoted value and its disagreements, the strings as text. It changes
no source reader, so the corpus is not normalized again (§6).

**Not in this plan:** the CSL-JSON mapping, the export routes and the
formatter (hayagriva in-process was the recommendation of 2026-09-24),
which come after it and depend on it. Nor **stubs**, which have no
records and so no fields at all. Most rows in any reference list are
stubs; exported, they are identifiers and nothing else. Filling them is
the corpus's job, not this plan's.

## 2. What the sources give, and what reaches the row

Read from the normalized types on 2026-09-24 (`source-crossref`,
`source-pubmed`, `source-openalex`, each `normalize.rs`).

| field                        | Crossref                                   | PubMed                                                          | OpenAlex                                             | in the served row today           |
|------------------------------|--------------------------------------------|-----------------------------------------------------------------|------------------------------------------------------|-----------------------------------|
| authors, split               | `given`, `family`; organisations whole     | `LastName`, `ForeName`, `Initials`, `Suffix`; collectives whole | no                                                   | no: one display string each       |
| authors, as printed          | —                                          | —                                                               | `raw_author_name`                                    | no: OpenAlex's is its *profile* name |
| journal, full name           | `container-title`                          | `Journal/Title`, in NLM's sentence case                         | the location's source name                           | yes, from the venue's source      |
| journal, abbreviated         | not kept (`short-container-title`)         | `ISOAbbreviation`, `MedlineTA`                                  | no                                                   | no                                |
| volume, issue                | yes                                        | yes                                                             | `biblio`                                             | no                                |
| pages                        | `page`, a range as written                 | `StartPage`/`EndPage`, or `MedlinePgn` (`123-9`)                | `biblio.first_page`, `last_page`                     | no                                |
| article number               | `article-number`                           | in the pagination or an `ELocationID`, not marked as such       | in `first_page`, not marked as such                  | no                                |
| the issue's date             | `published-print`, `issued`                | `JournalIssue/PubDate`                                          | `publication_date` only, the earliest               | no: the earliest date, all sources |
| publisher                    | `publisher`                                | none for articles                                               | the source's host organisation                       | no                                |
| ISBN                         | yes                                        | —                                                               | —                                                    | no                                |
| editors, edition, place      | not kept                                   | —                                                               | —                                                    | no                                |

Two rows need saying plainly:

- **The date a work shows is not the date a citation prints.** Each
  source's view takes its earliest date (Crossref the earliest of
  `issued`, online and print; PubMed the earlier of `ArticleDate` and
  the issue's `PubDate`), and the merge promotes the earliest of those.
  That is right for "when did this appear" and for search. A citation
  prints the year of the issue once there is one, next to that issue's
  volume. An article online in December 2019 and printed in volume 45
  in 2020 would cite as `2019;45(3)`, a year its volume is not from. On
  the sample the promoted year disagreed between PubMed and Crossref on
  4,078 of 39,934 works, 4,040 of them by one year, which the merge
  plan reads as online-first against the issue (**R**, merge plan §5).
- **OpenAlex's author names are its author profiles' names.** The text
  reads `authorships[].author.display_name`, the name OpenAlex's
  resolved author goes by, which need not be the spelling on this paper.
  `raw_author_name` is the spelling on the paper. The normalized record
  keeps both.

## 3. Decisions

Each is a choice this plan cannot make for the user. The recommendation
is first.

**D1. Where volume, issue, pages, article number and the issue's date
live.** *Recommended: in each record's view, promoted like the other
fields, with the other sources' values kept where they differ.* This
group is called **biblio** below, OpenAlex's word for it. The other
choice is to carry it as text, read from one record at assembly as the
author names are. That is cheaper: text is a serve version and a
rebuild of the works index, where a view field is a merge version and a
full merge (§6). But:
- the values are short and comparable, and sources disagree on them in
  ways worth seeing (online-first against print, a reissued page
  range), which is what §5 of the merge plan keeps disagreements for;
- the matching stage will need a journal, volume and first page to match
  citations that carry no identifier (merge plan §6's unresolved
  stream; the publisher-id work of 2026-09-18 found that a bare id needs
  journal, year and volume to corroborate it), and that evidence belongs
  in the merged row;
- a row that carries its evidence can have its fields re-decided by the
  incremental merge without the normalized layer (merge plan §5), and
  text cannot.

A full merge is cheapest now, while the box's store is built from two
PubMed baseline files and not the corpus (as of 2026-09-24).

**D2. Biblio is promoted as one group, from one record.** *Recommended.*
Promoting the volume from Crossref and the pages from PubMed could print
a citation no source ever printed, and when two sources disagree about an
issue they usually disagree about all of it. The group's date is part of
the group.

**D3. Which record's biblio.** *Recommended as a first cut, to be revised
by the survey (CF1):* among the records that give a volume or an article
number, Crossref, then PubMed, then OpenAlex; if none gives either, the
same order over all of them.
- Crossref's record is the publisher's own deposit, updated when an issue
  is assigned, with page ranges written in full.
- PubMed's is NLM's record of the issue, but older ones abbreviate page
  ranges (`123-9`).
- OpenAlex's is largely taken from the other two.

Preferring a record with a volume is what keeps an online-first deposit
from beating the issue it was later printed in. A disagreement is: the
volumes differ, the issues differ, the first pages differ, or the article
numbers differ, compared as written after trimming space. The survey will
show which of those are only notation.

**D4. The citation's date is biblio's date, not the promoted
`published`.** *Recommended.*
- Crossref's is `published-print`, else `issued`.
- PubMed's is `JournalIssue/PubDate`. A `MedlineDate` that was kept
  rather than read goes out as what is certain of it (model version 7),
  with its raw text beside it.
- OpenAlex has only its one date.

`published` stays as it is, for display and search.

**D5. Authors: Crossref, then PubMed, then OpenAlex.** *Recommended.*
Today's order puts OpenAlex before PubMed (`build.rs`, `AUTHORS`), so a
work with no Crossref record takes OpenAlex's names, which are not split,
over PubMed's, which are. This changes which author count is promoted,
and so the count's disagreement tally, and the names a PMID-only work
shows. The author count's disagreement rate was 2.6 % on the sample
(**R**, merge plan §5). How many works it moves is **?** (§8).

**D6. OpenAlex's names as printed.** *Recommended:* the text takes
`raw_author_name`, not the profile's `display_name`, for display and
citation alike, so the work shows one list of names, the paper's.

**D7. We never split a name ourselves.** *Recommended.* A name no source
split stays whole, and a CSL processor prints it as given (CSL's
`literal`): OpenAlex's names, Crossref's organisations, PubMed's
collectives. Splitting `Jean-Paul van der Berg` by rule is a guess, and
the lesson of 2026-09-24 was correctness over making the data fit the
shape. What a split name keeps is family, given (PubMed's `ForeName`,
else its `Initials`, as the display name does now) and suffix. Particles
(`van der`) are not split by any source and stay in the family name.

**D8. Pages as written.** *Recommended.*
- Crossref's `page` is kept as written, and so is OpenAlex's first–last.
- PubMed's `StartPage`–`EndPage` is used where given, else its
  `MedlinePgn`.
- `123-9` is not expanded to `123-129` in the merge. That is a reading,
  and CSL styles reformat ranges themselves (`page-range-format`).

The first page, which the disagreement rule and the matching stage use,
is what comes before the hyphen. An article number is kept apart only
where a source says it is one (Crossref's `article-number`). Where
PubMed or OpenAlex give one in the page field, it stays in the pages.

**D9. Scope: journal articles, and nothing that needs a source reader
changed.** *Recommended.* Crossref's abbreviated title, editors, edition
and publisher place are not kept by the Crossref reader. Keeping them is
a model version, which means normalizing the whole corpus again, so they
wait for the next model version that happens for another reason. Until
then the abbreviated journal name comes from PubMed alone, and a book
chapter cites without its editors.

**D10, for the user without a recommendation: the free PDF link.** The
README's "Nice to have" wants OpenAlex's open-access URL, PDF URL and
licence, which the merge also drops. They would be text, like the
journal abbreviation, and could ride in the same serve version (CF2).
The user said on 2026-09-24 that PDFs can wait for OpenAlex; this only
notes that the same rebuild could carry them.

## 4. The shape

**Evidence (merge crate).** `View` gains
`biblio: Option<Biblio<'a>>`, borrowed like the rest of the view:

```rust
pub struct Biblio<'a> {
    pub volume: Option<&'a str>,
    pub issue: Option<&'a str>,
    /// As the source writes the range (D8).
    pub pages: Option<&'a str>,
    /// Only where the source marks one.
    pub article_number: Option<&'a str>,
    /// The issue's date (D4), at its precision.
    pub issued: Option<Bound>,
    /// A PubMed `MedlineDate` kept rather than read, as written.
    pub issued_raw: Option<&'a str>,
}
```

`None` when the record gives none of the four locating values. `Fields`
gains `biblio: Option<Field<Biblio>>`, promoted by `promote` with D3's
order and rule. That gives the report its present, from and disagreement
tallies, and the survey its table, for free. The owned view's
`fill_from` fills biblio **whole** from an older copy only when the
speaking copy has none (absence is not an assertion, merge plan §8;
whole, by D2, as the venue already fills).

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
    /// Crossref's `publisher`, else OpenAlex's host organisation.
    pub publisher: Option<String>,
}
```

The display string (`given family suffix`) is derived from `Author` where
the document, the search index and the API want it, so nothing that shows
authors today changes shape. Each text field names the record it comes
from:
- authors from the promoted `authors` field's source (D5);
- venue from the venue's, as now;
- `venue_short` from the work's PubMed record;
- `publisher` from its Crossref record, else its OpenAlex one.

`Assembler`'s `Wanted` and `fill` become a table over those five rather
than three `if`s.

**API.** `Work` gains a `biblio` message (volume, issue, pages, article
number, the issue's date, and which source it came from) and
`venue_short`. The structured authors stay on the server: the formatter
runs there, and no client needs them yet. `Summary` is unchanged, which
keeps search results as light as they are now.

## 5. The incremental merge

**Biblio needs nothing new.** It is in each record's view, which is in
the row, and the batch re-runs `promote` on the component as it does for
every other field. The rows the batch writes carry it, and so does the
works delta.

**The text needs one generalisation.** `Engine::text_for`
(`serve/src/incremental.rs`) matches on three fields by name and fetches
each from the batch, from the row that held it before, or from the
normalized layer through `Records`. It becomes the same table as the
assembler's. `venue_short` and `publisher` come from records other than
the venue's, so a batch that touches a work may have to read one more
record from the normalized layer, which `Records` already does. Where no
reader is given, the batch stops for a full merge as it does today (merge
plan §11 item 22). `Incoming::absorb` fills the two new fields the way it
fills the others.

## 6. Versions, rebuilds, order

- **Model version: unchanged** (8). No source reader changes (D9), so the
  normalized layer stands and nothing is normalized again.
- **Merge version 11 → 12:**
  - `View` and `Fields` gain biblio;
  - authors follow D5;
  - the rows' layout changes.

  This means a full merge into `merged/v8.12/`. The registry is untouched
  and a full merge should append nothing to it; checking that is part of
  CF3. The full merge and `epoch publish` are the user's, locally and on
  the box (its `first-run-vN` Job).
- **Serve version 6 → 7:** the row's text changes. This means
  `names build`, `works build` and `epoch publish`, which the user's
  publish runs anyway after a merge.
- **The server** waits, not ready, for an epoch it can read, and hands
  over rolling (2026-09-24), so a version bump no longer takes the site
  down.
- **The API change is additive.** Deploy the server first. An older web
  client ignores the new fields.

## 7. Milestones

**CF1 — Views and the survey.**
- Each source's `view` fills biblio (D3, D4, D8).
- Promotion and its tallies land, and merge version 12.
- The survey (`merge --survey`, which writes nothing) prints biblio's
  field table: present by source, which source it came from, and
  disagreements by pair and by kind.
- It also prints three counts:
  - records with a volume and no pages, and the reverse;
  - works where biblio's year differs from the promoted `published`
    year;
  - the bytes a row grows by.

*Done when:* the survey runs over a store at model version 8 and its
numbers are reported to the user, who confirms or changes D3's order and
disagreement rule.

**CF2 — Text.**
- The text changes: `Author`, `venue_short`, `publisher`, D5, D6, D7.
- The assembler and `text_for` become tables.
- Serve version 7.
- The document and the search index take the derived display strings.

*Done when:*
- the `serve` synthetic fixtures and `rows_tests` cover a split name, a
  whole name, and each new field from a record that is not the venue's;
- both hold-out harnesses (`pubmed26n1334` and an update file, per the
  2026-09-19 note) read 0 differences;
- `compare` checks the new fields as it checks every key.

**CF3 — The rebuild (the user's).**
- A full merge at 12, then `epoch publish`, on the local store.
- A read of a few works by hand: a work with two sources, an
  online-first article, a PubMed-only work, and a work whose authors are
  whole names.
- The registry appends nothing.
- Then the box.

**CF4 — Serving and the paper view.**
- The proto's `Work` gains `biblio` and `venue_short`.
- `just ts-types`.
- The JSON surface's tests.
- The paper view shows the issue under the venue (`45(3):123–129`,
  `Article e0123456`) and nothing when there is no biblio.

After CF4, the export plan: the CSL-JSON mapping from a row, the
CSL-JSON, RIS and BibTeX downloads, and the formatter.

## 8. What to measure

All **?**. The survey is the safe command: it opens no writer.

```
cargo run --release -p ol-rht -- merge --store <store> --survey
```

The local store is normalized at model version 2 and today's binary
cannot read it. The survey needs either the box's store (a Job there, the
user's to run) or the local sample normalized again at model 8 into a new
directory, also the user's call.

- Held works with biblio from each source, and with none.
- Biblio disagreements by pair, and how many are notation only (`123-9`
  against `123-129`, `Pt 2` suffixes, leading zeros).
- Works whose biblio year differs from the promoted `published` year:
  how many citations D4 changes.
- Works whose authors move from OpenAlex to PubMed under D5.
- Held works whose authors are all whole names after D5: the ones no
  style can format by family name.
- Works whose journal name comes from PubMed, and so is in NLM's sentence
  case (`The New England journal of medicine`), which most styles print
  as given.
- Bytes per held row, merged set and works index, before and after.

## 9. Open items

1. **Hayagriva and abbreviated page ranges.** Whether it expands `123-9`
   under a style that asks for full ranges. citeproc-js does. If
   hayagriva does not, the export plan expands them before handing them
   over, not the merge (D8).
2. **The journal abbreviation beyond PubMed.** Crossref's
   `short-container-title` needs a model version (D9). NLM's catalogue
   maps ISSNs to abbreviations for every journal it indexes, which would
   cover works PubMed does not hold. A table, not a source; not
   designed.
3. **Containers** (merge plan §11 item 25). A journal issue modelled as a
   work would be the natural owner of volume and issue, with its parts
   borrowing them. Biblio per record does not stand in its way.
4. **Author identity** (merge plan §11 item 6). Split names are not
   identities. ORCIDs are in Crossref's and PubMed's records and stay
   there for the persons work.
