# Metadata quality in the normalized layer

**Status:** note, 2026-09-19 — one measurement pass over the whole
normalized layer and the merged sets at `data/merged/v2.8`, made to test a
guess about bad free text and answering it in the negative. Speculative
notes, not a plan; nothing here re-ranks or amends the [ingestion](../ingestion-plan.md),
[merge](../merge-plan.md), [graph](../graph-plan.md) or
[serving](../serving-plan.md) plans. **No parsing or merge behaviour was
changed and none is proposed as settled** — §10 carries a recommendation
and the user's decision, which was to hold the current prior rather than
act. Every figure below is measured over the sample unless it is labelled
otherwise; the one hand-classified figure says so and gives its sample
size. Scripts stayed in a scratchpad, and nothing derived from the data is
committed.

## 1. The hypothesis, refuted

The investigation started from a guess: that publisher pages scraped
wholesale were contaminating abstracts, and that site chrome — `Search for
more papers by this author`, `Download Citation`, `Permissions`, cookie
walls — was becoming a high-frequency token shared across a publisher's
whole output, a ranking contaminant sitting under the `abstract` field's
40.1 % disputed rate.

The measurement refutes it. Across 242,629 distinct normalized records:

- **`Search for more papers by this author` occurs in exactly one record
  in the corpus** — `openalex:W2005501262`, the PRISMA 2009 statement that
  prompted the question. Zero at PubMed, zero at Crossref.
- `Download Citation` 3 records, `Track Citations` 3, `Permissions` 17
  across all three sources, `View Full Text` 1, `Shopping cart` 1,
  `Click here` 1.
- The largest chrome family is the ACS prefix `High Resolution Image
  Download MS PowerPoint Slide` — 204 OpenAlex records, 0.335 % of that
  source's abstracts.
- **At index level, chrome flags 21 of 148,210 held works: 0.014 %.**

The discovery method was empirical rather than a fixed list: an 8-gram
document-frequency sweep per source (a saturating hash-bucket counter to
find candidates recurring in 40 or more distinct abstracts, then exact
counts for the survivors), plus the most common opening and closing
eight-word shingles, plus a battery of named markers. The sweep was free
to name any recurring phrase. It named almost no chrome.

What it named instead is boilerplate that *replaces* an abstract rather
than decorating one, and each family is small:

| block | records | share of that source's abstracts |
|---|---|---|
| APA `PsycInfo Database Record … all rights reserved` | 944 pubmed, 96 openalex | 0.93 %, 0.16 % |
| McGill eScholarship digitization notice | 292 openalex | 0.48 % |
| `(Uploaded by Plazi …) No abstract provided.` | 175 openalex | 0.29 % |
| Springer `The online version contains supplementary material available at 10.1007/…` | 158 pubmed | 0.16 % |
| Université de Montréal `Thèse/Mémoire numérisé par la Direction des bibliothèques` | 95 openalex (four opening shingles) | 0.16 % |

The APA and Springer blocks are tails appended to a genuine abstract, not
replacements. Only the repository notices are the whole field, and they
reach 600 indexed documents, 0.405 % of held works. Everything else at the
top of the frequency table is legitimate scientific phrasing — `the aim of
this study was to evaluate`, PROSPERO registration URLs, the PRISMA title
quoted inside systematic-review abstracts.

A refuted guess recorded honestly is worth more than a tidy narrative, and
the sweep that refuted it found the thing that matters.

## 2. OpenAlex records whose abstract belongs to a different paper

While sampling the long tail, the pattern below appeared repeatedly. It
was verified by reading `data/raw/openalex/works/updated_date=2025-07-25/part_0000.gz`
directly with plain Python, independently of this pipeline's parsers, so
the defect is in OpenAlex's published snapshot and not in our ingest:

| OpenAlex id | title | venue and year | DOI | abstract |
|---|---|---|---|---|
| `W26692027` | *Ipes Ibérica, se queda con los "petfoods" de Núter* | Revista alimarket, 2010 | `10.1371/journal.pone.0145529` | gene duplication and gene families |
| `W16170003` | *End of Life Care and Terminal Care for the Elderly … in Japan* | Medical Entomology and Zoology, 2013 | `10.1136/vr.157.12.347` | a three-year-old cat with lymphadenopathy |
| `W17026306` | *STUDY OF GAMMA EMISSION FROM PLUTONIUM.* | none, 2012 | `10.1103/physrevlett.97.083901` | surface gap solitons at a dielectric interface |

In each case the DOI and the abstract agree with each other and the title
is the outlier: three different papers' fields in one record. `title` and
`display_name` carry the same wrong value, so it is not a display
artefact.

**The detector.** For each record, the fraction of the title's content
words (four characters or more, stopwords removed, matched to the abstract
by token or by a two-character-tolerant stem) that appear in the abstract.
Restricted to records where title and abstract are both confidently
English by function-word density, the title has at least four content
words and the abstract at least fifty tokens — the English gate removes
the confound of a foreign-language title with a genuine translated
abstract, which is common and legitimate.

**The calibration.** PubMed is the control: its title-abstract pairing
comes from one XML record and is not assembled from anything. Whatever
rate PubMed shows at a given threshold is the detector's floor.

| source | comparable records | coverage < 0.15 |
|---|---|---|
| pubmed (control) | 89,481 | 45 — **0.050 %** |
| crossref | 23,846 | 12 — **0.050 %** |
| openalex, all | 42,024 | 2,919 — **6.95 %** |
| openalex, bulk snapshot cohort | 10,424 | 2,824 — **27.09 %** |
| openalex, API-lookup cohort | 31,600 | 95 — **0.301 %** |

Crossref lands on PubMed's floor to three decimal places, which is the
strongest evidence that the detector measures a real defect rather than
its own arbitrariness. The rate is not a property of OpenAlex as such: the
records fetched one at a time through the API are as clean as Crossref,
and the whole excess sits in the bulk snapshot, where the legacy MAG
identities were assembled.

**Manual corroboration.** Sixty records were drawn at random from the
snapshot cohort with both a title and an abstract, and read. Seventeen —
28 % — had an abstract belonging to a different paper, agreeing with the
automatic 27.09 %. Six more were repository boilerplate, a placeholder, or
a bibliographic citation sitting in the abstract field; one was OCR
garbage whose topic did at least match its title.

## 3. More than one way for a record to be wrong

The working prior has been that identifiers tend to be real and free-text
body content is where failed scrapes and placeholder text live. This
finding complicates it, and the complication is the point rather than a
footnote.

The identifier on `W26692027` **is** real. `10.1371/journal.pone.0145529`
resolves, is well-formed, and names an existing paper. It simply names a
different paper than the title does. Trusting identifiers first does not
rescue this record, because the record is internally incoherent: no single
field is corrupt, and no field is a scrape artefact. The damage is in the
assembly — which is worse than a bad abstract, because the merge joins on
names, so a conflated record drags its wrong title onto the real work that
shares its DOI. Four such collisions are visible in the sample, where the
DOI OpenAlex attached is also held at Crossref and the two titles share
under 30 % of their tokens; the rest are invisible only because we do not
hold the other half.

The user's reading, and the one this note records: there is evidently more
than one way for a record to be wrong, and a single pathology should not
be assumed. Scraped free text is one. Field-level noise — markup,
placeholders, copyright tails — is another, and is small (§6, §7).
Incoherent assembly, with every field individually plausible, is a third,
and on this evidence it is the largest by a wide margin.

**Cross-reference.** A separate survey the same day found that OpenAlex's
own reference matcher treats absence as agreement — its author check
returns true when either side has no author metadata — which is the
inverse of this project's merge v5 rule that **absence is not an
assertion** (merge plan §8, §10, 2026-09-18). That finding is written up
in [the citation-parsing landscape](citation-parsing-landscape.md); it is
not restated here. Read beside this note, the two point the same way:
OpenAlex's *record assembly* is the weak point, not its identifiers, and
in both cases the failure is a join made on insufficient evidence.

## 4. How the corpus was measured

Every `.works.zst` under `data/normalized/v2/` was read through
`ol-rht <source> dump`: 268,959 records, **242,629 distinct** after
deduplicating by native id, since a PMID, DOI or OpenAlex id can appear in
several sets — pubmed 115,867, crossref 50,620, openalex 76,142. The
merged layer at `data/merged/v2.8` holds 5,227,338 works, of which
**148,210 are held** and 5,079,128 are stubs, and 6,544,183 edges.

To measure what the lexical index actually eats rather than what the
normalized layer holds, `merge::view::{strip_markup, join_sections,
inverted_index}` and each source's `text_of` were ported, and
`merge::document::Assembler`'s promotion replayed: abstract by the merge's
precedence PubMed → OpenAlex → Crossref, authors and venue from
`fields.authors.from` and `fields.venue.from`, the text taken from that
source's promoted record with the speaking copy resolved by set name. Of
148,210 held works, 128,725 have an abstract source assigned and text
resolved for 128,532 of them, 99.85 %.

Stubs are outside all of this: they have names and no records, so no free
text at all.

## 5. Length, and why a threshold is the wrong instrument

Per distinct record, abstract as the index would build it:

| | pubmed | crossref | openalex |
|---|---|---|---|
| with an abstract | 101,836 (87.9 %) | 27,830 (55.0 %) | 60,954 (80.1 %) |
| median words | 237 | 235 | 215 |
| p99 | 494 | 457 | 670 |
| p99.9 | 753 | 580 | **1,769** |
| longest | 1,320 w / 10,259 c | 1,198 w / 11,125 c | **4,684 w / 25,769 c** |

Across the 128,532 indexed abstracts: median 228, p90 339, p99 579, p99.9
1,314, longest 4,684. Over 600 words, 1,117 documents (0.87 %) hold 3.37 %
of abstract tokens; over 800 words, 388 documents (0.30 %) hold 1.72 %.

A threshold exists — PubMed and Crossref effectively stop at about 800 and
600 words, and 310 of the 388 documents past 800 are OpenAlex — but it
would be the wrong instrument. **Of those 388 long abstracts, three carry
a chrome or repository marker.** The rest are genuine full-text bodies:
whole reviews, consensus guidelines, taxonomic revisions. Cutting at 800
words would discard 1.72 % of the abstract text to remove three bad
documents. Length is a symptom of OpenAlex carrying full text under the
`abstract` name, which is a different question from correctness, and one
for the indexing policy rather than for parsing.

## 6. The other failure modes

Per distinct record, each detector stated where it is not obvious. Blank
cells are zero or under a record.

| | pubmed | crossref | openalex |
|---|---|---|---|
| no abstract at all | 12.11 % | 45.02 % | 19.95 % |
| markup residue in the abstract (§7) | 0.13 % | **2.09 %** | **2.97 %** |
| title of two tokens or fewer | 0.27 % | 0.52 % | **1.86 %** |
| title entirely upper case, four tokens or more | 0.11 % | 0.17 % | 1.65 % |
| placeholder or one-character author name | 0.002 % | 0.03 % | 1.79 % |
| venue missing | — | 0.15 % | 1.09 % |
| venue a placeholder (`unknown`, `n/a`, `journal`) | — | 0.01 % | 0.75 % |
| venue is a URL | — | — | 0.25 % |
| title equals the venue name | — | 0.006 % | 0.42 % |
| abstract of five tokens or fewer | 0.09 % | 0.17 % | 0.87 % |
| `no abstract available` and kin, 25 tokens or fewer | 0.013 % | 0.02 % | 0.23 % |
| junk title (`Correction`, `Erratum`, `In This Issue`, `Index`, `Untitled`, …) | 0.09 % | 0.15 % | 0.21 % |
| abstract is the title repeated | — | 0.006 % | 0.08 % |
| abstract is a copyright notice and nothing else | 2 records | — | 2 records |
| no title at all | 129 records | 37 records | 18 records |

Titles that are not titles exist and are negligible here. The most
duplicated normalized titles are `correction` 32, `erratum` 11, `reply` 7,
`editorial` 6 at PubMed and `discussion` 16, `correction` 10, the empty
string 9 at Crossref. There is no mass of `Title page` or `Front matter`,
which reflects a sample drawn from indexed recent literature rather than a
broad Crossref crawl; a fuller corpus should be expected to differ, and
this is one of several reasons the user chose to wait (§10).

Abstracts that are a copyright notice and nothing else are four records in
the whole corpus. Abstracts that repeat the title are 61 at OpenAlex and
three at Crossref. Text in the wrong field entirely is real but is the
conflation of §2, not a field swap: the Persée-style bibliographic
citation sitting in an abstract field, and HAL's `International audience`,
both appeared in the hand-read sample, at rates too low for the sweep's
threshold to name them.

## 7. Two defects in this pipeline's own text path

Both were traced from the measurement and are recorded here as findings.
**Neither was changed.**

**OpenAlex abstracts never pass through `strip_markup`.**
`crates/source-openalex/src/view.rs::text_of` reconstructs the abstract
from the inverted index and returns it directly, where the Crossref path
calls `strip_markup` and the PubMed path reaches it through
`join_sections`. Measured consequence: **1,567 OpenAlex abstracts, 2.57 %,
carry literal tags into the index** — `<sub>` 3,384 occurrences, `<em>`
2,379, `<sup>` 2,176, `<i>` 1,608 — against 67 at Crossref (0.24 %) and 55
at PubMed (0.05 %).

**`strip_markup` decodes entities in the same left-to-right pass that
strips tags,** so an entity decoded near the end of the pass is never
re-examined and double-escaped markup surfaces as visible text. Measured
consequence: **997 Crossref abstracts, 3.58 %, carry a surviving entity**,
dominated by `&lt;` at 1,922 occurrences — nearly all of them a mangled
`p < 0.001` — with `&gt;` 410 and `&amp;` 99. OpenAlex adds `&#13;` 783
times, a carriage return that survives because of the first defect.

Together these mint fake index terms. Over the indexed abstracts,
**26,651 tokens in 2,417 documents come from surviving markup — 0.0895 %
of abstract tokens**: `sub` 6,791, `em` 4,758, `sup` 4,346, `i` 3,219,
`lt` 1,411, `gt` 1,172, `b` 956, `amp` 915, `13` 783. Small in the
aggregate, and exactly the shape the original guess was looking for: a
meaningless term at high frequency, concentrated in one source.

## 8. What share of the index this is

Over the 148,210 held works, 34,871,866 indexed tokens:

| field | tokens | share |
|---|---|---|
| abstract | 29,788,204 | **85.42 %** |
| authors | 2,330,520 | 6.68 % |
| title | 2,167,197 | 6.21 % |
| venue | 585,945 | 1.68 % |

The abstract is six sevenths of the lexical index, so its hygiene is
nearly the whole question.

- Documents carrying **any** flag from §1, §2, §6 or §7: **8,092, 5.46 %
  of held works**, holding 1,931,004 tokens — **5.54 % of the index**.
- Documents whose **abstract** is flagged: 6,490, 4.38 %; those abstracts
  hold 1,646,892 tokens, 5.53 % of abstract tokens and **4.72 % of all
  indexed tokens**.
- Of that, the conflated abstracts of §2 are 2,909 documents, chrome is
  21, and markup residue is 0.0895 % of abstract tokens.

The merge's abstract precedence already does most of the defending. Of the
128,725 held works with an abstract source:

| abstract from | documents | flagged |
|---|---|---|
| pubmed | 102,007 | 390 — **0.38 %** |
| openalex | 26,424 | 6,087 — **23.04 %** |
| crossref | 294 | 13 — 4.42 % |

Where PubMed holds an abstract it wins, and OpenAlex's damaged text never
reaches the index. The exposure is the 26,424 works where it does not.

## 9. Concentration, and the cost of demotion

| works held by | held | flagged | rate |
|---|---|---|---|
| openalex only | 31,348 | 7,020 | **22.39 %** |
| crossref only | 1,004 | 84 | 8.37 % |
| pubmed only | 65,742 | 404 | 0.62 % |
| crossref + openalex + pubmed | 44,232 | 543 | 1.23 % |
| crossref + pubmed | 5,374 | 38 | 0.71 % |
| openalex + pubmed | 500 | 3 | 0.60 % |

By source count: one source 7.65 %, two sources 0.70 %, three sources
1.23 %. **7,508 of the 8,092 flagged works are held by one source**, so
everything they carry is unique by construction — demoting them loses it
outright, and there is nothing to fall back on.

Their position in the graph, which is what a demotion would actually cost
beyond the index:

- **5,982 of the 8,092, 73.9 %, neither cite nor are cited** in this
  corpus. They are inert.
- 391 are cited, 1,409 in-edges. A stub still receives citations, so these
  survive demotion.
- 1,965 cite something, **73,096 out-edges — 1.12 % of the corpus's
  6,544,183 edges**, and these do not survive: references come from the
  record.
- Only 1,784 of the 7,566 flagged OpenAlex records carry any references at
  all, totalling 41,622 of that source's 2,262,555, 1.84 %.

So **"treat the record as a stub"** would demote 8,092 held works, 5.46 %,
removing 5.54 % of indexed tokens; strip 7,508 works of their title,
venue, year, authors, form, genre and access with no other source to
supply them; and drop 73,096 citation edges. Against that, three quarters
of the demoted set is already graph-inert, so most of the cost is the
metadata rather than the graph.

## 10. A recommendation, and where it was left

**Recommendation, not a decision.** Do not demote records. The defect is
confined to one field of one cohort of one source, and demotion is a
whole-record punishment for it: it costs 7,508 unique works and 73,096
edges to clean 4.7 % of tokens. Since abstracts are 85.42 % of the index,
**dropping the abstract and keeping the record** buys nearly all of the
search benefit for none of the cost — no work leaves the index, no title
or venue is lost, no edge is dropped. The §2 coverage check is a usable
gate for it: at a threshold of 0.15, PubMed and Crossref both sit at
0.050 %, so the rule would cost those sources essentially nothing while
removing about 2,900 OpenAlex abstracts. The two defects of §7 are a
separate and much smaller question.

**The user's decision, 2026-09-19: hold the current prior a while
longer.** Not because the evidence is doubted, but because we do not yet
have a full corpus from any source, and the more recent data is cleaner in
general — the split between the 27.09 % snapshot cohort and the 0.301 %
API-lookup cohort is itself a version of that observation. A rule written
now would be fitted to a sample whose composition is going to change.

So this closes as an open question with the evidence laid out, not as a
settled rule. What would settle it: whether the snapshot cohort's rate
holds once a full OpenAlex snapshot is in hand rather than the slice the
sample took; whether a full Crossref stays on PubMed's 0.050 % floor once
it is no longer drawn from indexed recent literature; and whether the
conflated records are worth *repairing* — the DOI and abstract agree, so
the correct title is recoverable from Crossref for any conflated record
whose DOI we hold — rather than dropped.
