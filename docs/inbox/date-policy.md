# Dates a source got wrong

**Status:** proposal, 2026-09-23 — written for the five date signatures in
the `--survey` run over the whole PubMed 2026 baseline. Speculative: no
crate was changed, nothing here is scheduled, and §6 is a recommendation
with the user's decision still to come; §8 is what it does not settle. It
sits inside the [ingestion plan](../ingestion-plan.md) §6 and does **not**
propose weakening its contract: what it proposes is four named, counted
tolerances, one place where the pipeline should be made *stricter* than it
is, and an argument for why a source asserting something that cannot be
true is not the same kind of failure as a shape we have not modelled, and
cannot be fixed the same way.

**Update, 2026-09-24:** reviewed against the survey of the whole
baseline on model version 5 and 28 of its example records fetched from
`efetch`. What landed as model version 6, and what the evidence changed,
is in the [ingestion plan](../ingestion-plan.md) §10 under 2026-09-24.
Two corrections to what follows: signature 4 is not a false day — every
example the survey kept is a year or an issue number where a day goes, a
shape; and the code references are to the tree before model version 5,
which moved them.

Every figure is marked by how it was got:

- **M** — measured here by a command this note names, against files on
  this machine.
- **N** — read out of NLM's own documentation, fetched today and quoted.
- **C** — read out of the code, with file and line.
- **?** — not determined. What would determine it is given instead.

**The survey report is not on this machine, and neither is the corpus it
ran over.** `data/raw/pubmed/` holds baseline files 1333 and 1334, update
files 1335 to 1339, and the 2026-09-15 lookup sample; the whole-baseline
survey ran on the cluster (`crates/ingest/src/run.rs:169-188`, **C**). So
this note carries **no per-signature counts**, which is the largest gap in
it, and §8 says what would close it. What it carries instead is: what each
signature is, exactly, at the level of the code path and the string that
reaches it; what the model can already express; and two things found while
looking that change the shape of the question.

## 1. What the five signatures actually are

A signature is `field: kind` (`crates/ingest/src/source.rs:83`, **C**), so
all five name a field and an error, and a survey keeps the count plus the
first three example messages per signature — no more
(`Survey::EXAMPLES = 3`, `run.rs`, **C**). That limit matters later.

| # | signature | raised at (**C**) | the raw it reports |
|---|---|---|---|
| 1 | `History/PubMedPubDate: day does not exist in that month` | `normalize.rs:679-692` → `Date::new`, `date.rs:398` | `"{Year}-{Month}-{Day}"` as written |
| 2 | `History/PubMedPubDate: time of day out of range` | `normalize.rs:886-915` | `"{Hour}:{Minute}"` as written |
| 3 | `JournalIssue/PubDate: not a recognized MedlineDate form` | `date.rs:814` via `normalize.rs:697` or `:727` | the whole `MedlineDate` string |
| 4 | `JournalIssue/PubDate: MedlineDate part: day is not 1..31` | `date.rs:747-754` → `Day::parse` | the whole `MedlineDate` string |
| 5 | `JournalIssue/PubDate: MedlineDate part: day does not exist in that month` | `date.rs:756-761` → `Date::new` | the whole `MedlineDate` string |

They are not five instances of one thing. Sorted by what is actually
wrong:

**A value that cannot be true.** Signature 1 is a history date such as
`2019-02-30`: three parts, each individually well-formed, naming a day
that has never existed. Signature 5 is the same fact inside a range —
`2001 Feb 29-Mar 2` (**M**, these strings run against the real parser).
Signature 4
is a day outside `1..31` at all: `1975 Jan 32`, `1975 Jan 0`, or a token
of three or more digits where a day belongs, `1975 Jan 1-100` (**M**). No
amount of modelling makes any of these a date. They are the source being
wrong, and the only decisions available are which parts of the assertion
to keep and whether to say so.

**A publisher writing a null into a string field.** Signature 2's
reported example is `null:null`. `Hour` and `Minute` are `#PCDATA` in the
DTD (**N**, `el-Hour.html`: "Contains the numeric value for the hour in a
date"), so `null` is structurally legal and semantically empty: it is not
an out-of-range time, it is a serializer emitting the word for absence
into a field that has no way to be absent — `PubMedPubDate` is
`(Year, Month, Day, (Hour, (Minute, Second?)?)?)`, so the *pair* can be
omitted but a written `Hour` cannot be blank. The code reaches
`DateError::Time` for it only because the numeric check fails first
(`normalize.rs:890-901`, **C**), and the same signature also covers a real
`25:00` or `12:60`. **Two different facts are being reported as one
sentence, and the survey's three examples are the only way to tell them
apart.** Splitting that signature is worth doing whatever else is decided.

**A shape we have not modelled.** Signature 3, in part. `MedlineDate` is
free text and the grammar in `date.rs:782-920` is a catalogue of forms
observed in real data, extended each time a new one appears — the
`Seasons` variant for `1976-1977 Winter`, the case-insensitive
`Month::from_name` for `1975 MAR-APR`, the ` & ` separator for
`2020 Feb & Mar`. Each extension is the fix the plan's §6 prescribes, and each one
recovers a real date.

**But signature 3 is also, invisibly, the first group.** Because the
grammar only recognizes forms that *need* free text, a bare
`1975 Feb 30` — year, month, impossible day, no range — is not a
recognized form and reports as `not a recognized MedlineDate form`
(**M**, §3). A reader of the survey cannot tell which of its hits are
shapes to model and which are days that never existed. That conflation is
the second reporting defect, and it is the reason this note exists as one
proposal rather than five.

## 2. What the data on this machine says

**M**, `gzcat` of all seven raw files into `/tmp`, then a Python scan of
`<PubDate>` and `<PubMedPubDate>` blocks and a Rust harness built against
`crates/model` (read-only; it depends on the crate by path and lives
outside the repo).

The seven local files hold **126,740 records**. Their `PubDate` elements:

| shape | records | share |
|---|---|---|
| `Year, Month, Day` | 78,116 | 61.6 % |
| `Year, Month` | 34,098 | 26.9 % |
| `Year` | 12,053 | 9.5 % |
| `MedlineDate` | 2,328 | 1.84 % |
| `Year, Season` | 145 | 0.11 % |

- **Zero** of the 78,116 full dates is impossible.
- The 2,328 `MedlineDate` values are **348 distinct strings** once XML
  entities are decoded, and the real parser accepts **every one of
  them**: 2,274 spans, 25 year-season, 23 year, 5 year-quarter, 1
  year-month.
- **632,881 `PubMedPubDate` blocks**: zero impossible days, zero
  non-numeric or out-of-range `Hour`/`Minute`, and zero `Second`
  elements — so `Unmodelled("a Second in a history date")` never fires
  here either.

So **none of the five signatures fires anywhere on this machine.** By the
rule of three that bounds them together below about 1 in 42,000 records
*in material like this* — and the caveat swallows the number, because
files 1333-1339 are the newest end of the baseline, records loaded in
2026, and the free-text dates live in the 1970s and 1980s.

For comparison, and because the model is shared: **436,205 Crossref
`date-parts` values** in `data/raw/crossref/lookup/` (355,167 of them full
year-month-day) contain **zero** impossible days, months outside 1..12, or
years outside 1000..2999 (**M**). Whatever this class of defect is, it is
not universal across sources; at Crossref the dates are already integers
and are evidently validated.

To reach the era where `MedlineDate` is common, 1,000 records were fetched
from NLM in five `efetch` calls of 200 ids, sampled from an `esearch` for
`1975:1990[dp]` (**M**). The search's id list caps at 9,999 and the cap
landed on December 1990, so the sample is one month rather than the era —
a real limitation. It yielded 36 `MedlineDate` values in 12 distinct
forms. All twelve parse. **Two of them parse to the wrong answer**, which
is §4.

## 3. What `PartialDate` can already express

`crates/model/src/date.rs:110-127` (**C**). Seven variants: `Year`,
`YearMonth`, `Day(Date)`, `YearSeason`, `YearQuarter`, `Seasons` (a
seasonal volume over one year or two), and `Span` (two `Bound`s, each a
year with optional month and optional day).

**Coarsening needs no new model at all.** Every degradation this note
contemplates already has a variant: a day that does not exist in its month
degrades to `YearMonth`; a `MedlineDate` whose remainder is not understood
degrades to `Year`; a `Span` whose end bound carries a false day degrades
to an end bound with `day: None`, which `Bound` already allows. The merge
consumes all of this through one function, `view::bound`
(`crates/merge/src/view.rs:414`, **C**), which flattens any `PartialDate`
to a single `Bound`, and `View.published` is already `Option<Bound>`
(`view.rs:97`) — so *absence* is expressible too, and is what a Crossref
record with `[[null]]` already produces.

What the model cannot express is the thing this proposal actually needs:
**a value the source asserted that we did not keep.** There is no
`PartialDate` for "1975, and the source said more than that but it was
false or unreadable". That state exists in the project already, for
identifiers: `Claim<'a, T, E> = Valid(T) | Malformed { raw, error }`
(`crates/model/src/claim.rs`, **C**), introduced for exactly this reason —
PubMed puts the literal word `DOI` in a DOI slot — and the plan's §6 defends it in
plain words: *"That is not an `Other` on a closed vocabulary … it is the
data itself being the bug, kept visible."* The date fields do not use it.
`Dates.published` is a bare `PartialDate` (`normalize.rs:237`, **C**) and
`HistoryEntry` is a bare `Date` plus `Option<TimeOfDay>` (`:556-563`).

Two further things the model does not express, both relevant: a `Span`
has no invariant that its end is not before its start, and a parsed
`PartialDate` keeps nothing of how the source wrote it — `2020 Feb & Mar`,
which the source means as one issue *for both months*, and
`2020 Feb - Mar`, which is a range, become the identical `Span` (**M**).

## 4. Two things found while looking

### 4.1 The pipeline already accepts a date that cannot be true

In the December 1990 sample, `Nurs Stand` volume 5 issue 12 and
`Nurs Times` volume 86 issue 50 both carry `<MedlineDate>1990 Dec
12-8</MedlineDate>` (PMIDs 2123665, 2123672, 2255640, **M**). The parser
accepts it and produces `Span { start: 1990-12-12, end: 1990-12-8 }` — an
issue that ends four days before it begins. `Nurs Times` volume 86
*issue 49* is `1990 Dec 5-11` (PMIDs 2255626/27/33): consecutive weeks of
a weekly, so issue 50 is the week of **December 12-18**, and `12-8` is
`12-18` with the end day's leading digit elided, the same way NLM elides
page ranges (`MedlinePgn` on that very record is `18-20`). Two unrelated
journals write it the same way in the same week. It is a convention, not a
typo — and we read it as the wrong date, silently.

The same sample has `Nursing (Lond)` at `1990 Dec 21-Jan 10` (PMIDs
2320314, 2320315): an issue running to 10 January **1991**, which we parse
as a span ending 1990-01-10. Also silent, also wrong.

This is the strongest single argument in this note. **The project's
strictness today is about shape, not about truth.** A false date that
happens to fit a recognized shape is accepted without comment; a false
date that does not fit stops the entire run. Whatever policy is adopted
should make that distinction deliberate rather than accidental.

### 4.2 NLM does not document a grammar for `MedlineDate`

NLM's DTD documentation defines the element negatively (**N**,
`https://dtd.nlm.nih.gov/ncbi/pubmed/doc/out/190101/el-MedlineDate.html`):

> Contains the entire date string for a `<PubDate>` that does not fit the
> available date patterns: YYYY, YYYY + MM, YYYY + MM + DD, YYYY + SEASON.

Content model `( #PCDATA )`, two examples (`1998 Dec-1999 Jan`,
`2015 Nov-Dec`), and **no enumeration of permitted forms anywhere**. The
former field-description page (`bsd/mms/medlineelements.html`) now returns
404; `bsd/licensee/elements_article_source.html` documents `PubModel` and
`ArticleDate` and says nothing about `MedlineDate`'s syntax.

`MedlineDate` is therefore not an under-documented vocabulary we can
finish cataloguing. It is defined as *the set of strings that did not fit*
— an open escape hatch, populated over fifty years by whatever a journal's
issue label said. The plan's rule that "a value matching no documented
form is a failure" (plan §6) has no documented form to appeal to here: the
catalogue in `date.rs` is our own empirical list, and the honest
expectation is that **signature 3 will keep firing on every corpus larger
than the last one.** That is an argument about the *shape of the work*,
not about tolerance: extending the catalogue is right and cheap, and it
will never reach zero.

## 5. The options, and what each costs

**Reject the record** — today's behaviour. Costs, at ~37M records: a
PubMed-only work is lost outright, and 44 % of held works in the merged
sample are PubMed-only (65,742 of 148,210, read out of
[metadata-quality](metadata-quality.md) §9). It loses a title, an
abstract, authors, MeSH terms and a reference list to a wrong digit in a
date. For signature 1 it loses all of that for a processing timestamp that
**nothing outside `source-pubmed` reads** (**C**: `HistoryEntry` appears
only in `normalize.rs`, the stats tally in `main.rs:4194` and `dump`).

**Reject the field, keep the record.** For `PubDate`, `View.published`
becomes `None`; the merge's `published()` then takes another source's date
if there is one (`build.rs:1631`, **C**) and nothing at all if there is
not, and `Document.year` becomes `None` (`document.rs:120`), which removes
the work's `/year/<y>` facet from the index entirely
(`search/src/schema.rs:99`). A reader loses the ability to find the work
by date at all. It also sits badly with merge v5: absence is not an
assertion, so a dropped date does not merely say "unknown", it removes the
record from the vote.

**Keep a coarser value.** `1975 Feb 30` becomes `YearMonth(1975, Feb)`;
`1975 Jan 32` becomes `Year(1975)`. The reader keeps the year facet and a
sortable date; the merge keeps a vote. The cost is precision the source
never truthfully had, plus one real side effect: `view::earlier` ranks a
more precise bound *before* a coarser one in the same year
(`view.rs:449`), so a coarsened PubMed date can lose promotion to another
source's dated bound. That is the right outcome — the other source's date
is the one that is actually known — but it is a behaviour change worth
naming.

**Keep the raw text beside the parsed value.** The `Claim` precedent.
Costs bytes on the malformed path only, forces downstream code to match
on it, and bumps the model version, which invalidates the normalized layer
and requires a re-run (plan §6; CPU-bound and parallel, cheap relative to
fetching). It is the only option under which **nothing becomes unknowable
later** — the assertion the source made survives in the record, not only
in a survey report that is thrown away.

**Something else: make the reporting tell the truth first.** Two of the
five signatures currently describe themselves wrongly (a `null` is not an
out-of-range time; an impossible day is not an unrecognized form), and a
survey keeps three examples per signature and no tally of distinct raw
values. For dates the raw value *is* the finding. A survey that reported,
say, the twenty most frequent distinct raw values per value-level
signature would let one run extend the catalogue completely, which is the
same argument that put parse failures into surveys on 2026-09-23.

## 6. Recommendation, signature by signature

A recommendation, not a decision. The default stays what it is: anything
not named below still stops the run.

**1 — `History/PubMedPubDate: day does not exist in that month`. Keep the
record; hold the entry's date as a claim.** `HistoryEntry.date` becomes
`Claim<'n, Date, DateError>`. Do not coarsen it: a processing timestamp
rounded to a month is not worth having, and the `PubStatus` — that the
publisher says it *received* the article, at a time it wrote down wrongly
— is the half that has any value. Nothing downstream reads this field
today, so the cost of the wrapper is a `match` in the stats report and
the dump. Of the five, this is the one where rejecting the record is
least defensible: the record is thrown away for a field no consumer has.

**2 — `History/PubMedPubDate: time of day out of range`. Split the
signature, then treat both halves as claims.** `null:null` and `25:00` are
different findings and the report should say so: a distinct error for a
non-numeric placeholder, so the survey groups them apart. Then
`HistoryEntry.time` becomes `Option<Claim<'n, TimeOfDay, DateError>>` and
both halves keep the record with the raw text in it. The tempting
alternative — read `null` as "no time given" and move on — is exactly the
silent loss the fail-fast principle exists to prevent; it would be
invisible in the record and survive only as a tally. The claim costs
nothing extra and is honest per record.

**3 — `not a recognized MedlineDate form`. Extend the catalogue as the
first answer, and put a named floor under it.** Extending is right: each
form recovered is a real date, and §4.2 says the catalogue will never be
complete. The floor: when the string begins with a four-digit year in
`1000..=2999` and the remainder is not understood, keep
`PartialDate::Year` **and the raw string**, recorded as a coarsened claim
and counted in the stats report with samples; when it does not begin with
a year, fail the run as today, because there is nothing recoverable and
the case is rare enough to deserve a human. Every one of the 348 distinct
local strings and all 12 in the NLM sample begins with a four-digit year,
as do both of NLM's documented examples — which is evidence, not a rule
(**?**, §8).

This is the consequential decision in this note, and it should be argued
on its own terms: it means a `PubDate` no longer has to be fully
understood for the record to exist. What justifies it is that the
alternative is not "understand it later" but "lose the record now", and
that the year — the part a reader filters and facets by, and the part the
merge compares — is carried by the first four characters and is not in
doubt.

**4 — `MedlineDate part: day is not 1..31`. Coarsen the bound that carries
the bad day, keep the raw, count it.** Here the shape *is* understood, so
we know exactly which part is false: `1975 Jan 32` keeps 1975-01 and drops
the day; `1975 Jan 1-100` keeps the start bound whole and drops the end
bound's day. The year and month are separate assertions and are almost
certainly true. Rejecting the record for a digit we have already located
is the least proportionate response available.

**5 — `MedlineDate part: day does not exist in that month`. The same rule
as 4.** `2001 Feb 29-Mar 2` keeps `2001 Feb` as its start bound. And the
same rule should apply to the structured `PubDate` path
(`normalize.rs:737-739`), which raises the identical error from `Date::new`
and does not fire today (**M**: 0 of 78,116 locally) — two paths to the
same fact should not answer differently.

**Cross-cutting, and not one of the five: give `Span` an invariant.** A
span whose end precedes its start is not a date (§4.1), and we mint them
today. Checking it turns silent wrong answers into either a modelled
repair — NLM's elided end day, `1990 Dec 12-8` as December 12-18, and a
month-decreasing range as the following year, `1990 Dec 21-Jan 10` as
1991-01-10 — or, where neither repair applies, a named failure. Both
repairs are modelling in the plan's §6 sense: they are forms, discovered in real
data, that we had read wrongly. This is the only item here that makes the
pipeline *more* strict, and it should probably land before the tolerances
do, because it is the one that is currently producing false data.

## 7. What it costs

**The reader** loses precision they never truly had, and only on records
where the source contradicted itself: a work shows *1975* where the source
wrote a day that never existed. Because the raw text is kept, a record
view can still show "as published: 1975 Jan 32" — the reader can be told,
which is the difference between coarsening and hiding.

**The merge** gains a vote it did not have and loses a tie-break: a
coarsened bound sorts after any dated bound of the same year in
`earlier()`, so where another source knows the day, that source is
promoted. Where PubMed is the only source — 44 % of held works in the
merged sample — the alternative was no date at all, or no work at all.

**The index** keeps its `/year/<y>` facet, which a dropped field would
remove outright. This is the concrete difference between "coarsen" and
"drop the field" and it is why this note recommends coarsening.

**Later** nothing becomes unknowable, provided the raw is kept. That is
the whole reason for preferring a claim over a quiet coarsening: the
survey's report is transient and keeps three examples, while the record is
the artifact. Every tolerance above is narrow (a named error at a named
field), counted (a line in the stats report beside `doi valid/malformed/
none`), and recorded in the data (the raw string travels with the record).
The costs that are real: a model version bump and a re-normalize of the
whole layer; three more `match` arms for every consumer of a date; and the
permanent risk that a coarsened date is *silently fine* to downstream code
that forgets to ask whether it was coarsened — which is an argument for
the claim wrapper, whose whole point is that it cannot be read without
being matched on.

## 8. What I could not determine

- **How often each signature fires, and on what strings.** The survey
  report and the full baseline are both off this machine. What would
  settle it: the report from the cluster run, or `ol-rht pubmed normalize
  --survey` re-run with the distinct-raw-value tally of §5 — the second is
  worth more, because three examples per signature is not enough to write
  a grammar from, and the raw values are the entire evidence for this note's
  §6 items 3, 4 and 5.
- **Whether every `MedlineDate` begins with a four-digit year.** 348
  distinct strings here, 12 in the NLM sample and both of NLM's examples
  do; NLM documents no such rule, and a 50-year free-text field is exactly
  where a counter-example hides. Recommendation 3 fails the run when it is
  violated, so a counter-example is loud rather than silent — but the
  frequency is unknown.
- **Whether the elided end day is documented convention.** The empirical
  case (§4.1) is strong — two journals, consecutive weeks, matching page
  elision — but I did not find it stated in NLM's documentation, and
  `Citing Medicine` was not consulted. Before implementing the repair,
  read it there.
- **Who writes `null:null`, and whether it is one publisher's deposit
  window.** A contiguous block in one baseline file would argue for a
  counted exclusion rather than a per-record claim. Not answerable without
  the corpus.
- **Whether history dates will ever be read.** Today nothing reads them,
  which is most of the argument for recommendation 1's cheapness. If the
  freshness and first-seen work (serving §11 item 2) ever uses PubMed's
  `entrez` history date as an arrival time, this field stops being
  provenance and becomes load-bearing, and recommendation 1 should be
  revisited rather than inherited.
- **Whether the `Span` separator loss matters.** `2020 Feb & Mar` — one
  issue for two months — and `2020 Feb - Mar` become the same value today.
  Nothing downstream distinguishes them, so nothing is currently wrong;
  it is noted because it is the same class of quiet flattening this note
  is otherwise arguing against.
