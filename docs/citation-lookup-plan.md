# Citation lookup plan

**Status:** draft 1, 2026-09-27. CL1, CL2 and CL3 built the same day
(§5); CL4 and CL5 not started. Decisions marked *Decided* are the user's, from the
conversation of 2026-09-27; those marked *Recommended* are for the user
to take or change.
**Inputs:** the conversation of 2026-09-27; the
[reference-string landscape](inbox/citation-parsing-landscape.md) §3
(references with no title), §4 (the cascade) and §6.1 (Crossref's
matcher); Crossref's reference implementation, read 2026-09-27 —
`matching/cr_search_validation_matcher.py` in
[reference-matching-evaluation](https://github.com/CrossRef/reference-matching-evaluation)
(MIT, © Crossref); the [citation fields plan](citation-fields-plan.md)
(volume, issue and pages on the served row); [serving plan](serving-plan.md)
§11 item 36 (identifiers typed into the search box).

## 1. What this is for

A person pastes one citation — a line from a reference list — and gets
back the work it cites as an `ol:` id, or a plain "we could not find
it". A JSON route, and a tab in the web UI beside "Find by ID".

It is for one-offs. **Nothing here touches ingestion or the merge** (the
user, 2026-09-27). Matching the 756 M Crossref references that carry no
DOI (measured by the normalize run of 2026-09-26) is a scaling problem
with no answer yet. A lookup that takes a moment per citation is not an
answer to it. The code is shaped so that a batch *could* call it later
(D9), and nothing more.

The versions, in the user's order as amended 2026-09-27:

| | input | output |
|---|---|---|
| **v1** | one citation, as text | possible matches, each with why; or "could not", and why |
| v2 | several citations in one block of text | v1's answer per citation, each tied to its stretch of the input |
| v3 | a PDF with a text layer | v2, over the PDF's reference list |
| v4 | images: photos, screenshots, scanned PDFs | v2, over what the image says |

The user's first order put images before PDFs, so that when PDFs landed
they would "just work, for everything", scanned ones included. That can
wait while there are no users to impress (2026-09-27). Text PDFs need no
image reading at all, since the text is already there, and they are the
common case. This plan is v1; §9 sketches the rest.

**Not in this plan:** the unresolved stream and any batch; a parser
(v1 needs none, §3); the search box recognising identifiers (serving
plan §11 item 36), beyond sharing the piece that finds them (D2); the
search engine's quality, which is the search track's (§7).

## 2. Words, with one example

The running example is the most-cited paper there is, written two ways:

> **A.** Lowry OH, Rosebrough NJ, Farr AL, Randall RJ. Protein
> measurement with the Folin phenol reagent. J Biol Chem.
> 1951;193(1):265-75.
>
> **B.** Lowry OH, Rosebrough NJ, Farr AL, Randall RJ (1951) J Biol Chem
> 193:265

B has no title. Physics and chemistry styles write every citation that
way. Among Crossref's references with no DOI, having no title is the
usual case (landscape §3.1).

- **Citation.** The text pasted in: A or B.
- **Identifier.** A DOI, PMID or PMC id written inside the citation.
  Neither A nor B has one; many citations end with `doi:10.…`.
- **Candidate.** A work the search turns up for the citation's text. For
  A, perhaps twenty works that share words with it, Lowry's paper among
  them.
- **The check.** Comparing one candidate against the citation field by
  field. Does the citation contain the work's title, its first author's
  surname, its year, its volume, its first page? §3 has the details.
- **Agree.** A field agrees when the citation contains it. For Lowry's
  paper and citation A, all five do.
- **Possible match.** A candidate that passes the check. The answer is
  the list of them, best first:
  - one, when the citation is clear;
  - several, when it could be more than one work (a preprint and its
    published version, say);
  - none, when nothing passed.
- **Near miss.** A candidate that failed the check. It is shown too,
  marked, with what failed (D5).
- **Could not.** The answer with no possible matches, with a reason
  (D5).

## 3. How a lookup runs

Four steps.

**1. Identifiers.** Look for a DOI, PMID or PMC id anywhere in the text,
in any of the ways people write them: `https://doi.org/…`, `doi:`, a
bare `10.…`, mixed case, `PMID: 123`. Read each the way the model reads
DOIs (the soft hyphen and the rest, model 11), and resolve it through
the names table as "Find by ID" does. A DOI in running text has no clear
end: `…/nature12373.` ends a sentence. So the longest DOI-shaped run is
tried first, then shorter ones with a trailing `.`, `,`, `;` or `)`
taken off, until one resolves (D2). PubMed and OpenAlex links are read
too.

A citation that is one token, with no identifier the finder knows, is
read as Find by ID reads it (D12): as a name as written, and bare in
every namespace, publishers' included. A bare number inside a longer
citation is never tried: its volume of 193 would be PMID 193, another
paper, and a work a name leads to is always listed.

**2. Candidates.** Take the identifiers and any bracketed labels (`[12]`)
out of the citation's text. Search the rest against titles, authors and
journal names, but not abstracts, and keep the top twenty (D3). Crossref
also fetches twenty for a plain-text citation.

**3. The check.** Crossref's way of *reading* a plain-text citation's
numbers, ported, with our own rule on top (D4). "Found" is exact, as
Unicode compares text (D11). For each candidate, five fields are looked
for:
- the title, as whole words somewhere in the citation;
- a surname of the first author, in the first name of the citation's
  author list (the text up to its first comma, semicolon or bracket:
  `Lowry OH`, `D. Tkaczyk`);
- the year, among the citation's numbers;
- the volume, among them (or as whole words, where it is not a number);
- the first page, likewise.

The identifiers found in step 1, and a leading label that numbers the
list (`12.`, `[12]`), are taken out of the citation first: their digits
are not the work's.

**The rule: all of them, or the candidate fails.** "All" means every one
of the five that the work has. A candidate that fails is a near miss,
shown with what failed. The rule is loosened only by rules of our own,
each measured before it is taken (D4).

A field the work does not have counts for nothing either way. **Absence
is not agreement.** The merge adopted that rule on 2026-09-18, and
OpenAlex's matcher breaks it (landscape §18).

**Score.** It orders the list, and it is not the rule. It runs from 0
to 1: what agrees, over everything either side mentions.
- Every number the work has counts *for* it if the citation contains
  it and *against* it if not. That covers the volume, year, issue, each
  page, and any number in its title or journal's name.
- **Every number in the citation that the work cannot account for
  counts against it too**, and is shown in the why.
- The title and first author count for it or against it as they agree.

Crossref's readings come along with the port:
- `265-75` is read as 265 to 275;
- a volume that equals the year is looked for twice;
- the first five characters are skipped when numbers are gathered, so a
  leading "12. " is not counted.

**4. The answer.** The candidates that pass, best score first, then the
near misses. Each carries how it was found and what agreed (D5).

**Example A.** Title, author and year agree. Once volume, issue and
pages reach the served row (§7), volume and first page agree too, and
the 1 (the issue) and the 275 (the last page) are accounted for.
Lowry's paper passes. Before those fields land, it passes on title,
author and year, with 193, 1, 265 and 275 unaccounted for in its why.

**Example B.** B has no title, so Lowry's paper is a near miss: "the
title is not in the citation". It stays one until a rule of ours lets a
citation without a title pass on its other fields.

A citation with no title and a citation with a *different* title look
the same here, because nothing parses the citation to see which fields
it has. That is why the first loosening rule needs measuring rather
than guessing.

## 4. Decisions

**D1. The shape of the answer.** *Decided* (the user, 2026-09-27). The
answer is a list of possible matches rather than one or none, which is
"better than throwing up our hands and saying 'we could not find
anything' even if we could". Each match carries why. D4 says what a
candidate must do to be listed. If the lists turn out wrong more often
than right, the answer falls back to reasons only.

**D2. Identifiers and text, both, every time.** *Decided* (the user,
2026-09-27): "somebody shows up with a doi that doesn't actually belong
to the citation they paste in. we'd want to show what the doi belongs to
AND what their citation *actually* belongs to, and that they're
different."
- A work named by an identifier in the citation is listed, marked as
  found that way, and the check runs on it. The check is shown beside it
  and never removes it: without a parser, "the title is not in the
  citation" cannot be told from a citation that gives no title — a bare
  DOI would otherwise find nothing (found building CL3).
- The text steps run every time, not only when the identifier's work
  fails, because a lookup is one citation and a search is cheap. When
  the identifier and the text land on the same work, it is listed once,
  as found both ways.
- When they land on different works, the answer says so in as many
  words: the DOI names *this*; the citation's text matches *that*; they
  are different works.
- This happens in the data. On the sample, 492 of the 24,847 references
  that carried both a DOI and a PMID we could check named two different
  papers (merge plan §10, 2026-09-18).
- An identifier that names a stub (a work we know only as something
  cited) is still an answer, because the stub has an `ol:` id.
- An identifier that names nothing we have is reported as such.
- Two identifiers that name different works are both listed, marked.
- The finder is a function of its own, so the search box can use it for
  item 36.

**D3. Candidates from titles, authors and journal names, top twenty.**
*Recommended.* Abstracts share words with everything and add noise. A
stub has no text and is not in the search index, so it can only be
found by an identifier.

**D4. The bar: all of them, loosened by our own rules.** *Decided* (the
user, 2026-09-27). Crossref passes a work on three of the five fields.
That is tuned to Crossref's search, and the user called it "a bad
product choice": "i would start with 'all matches or none does' and
loosen it using our own rules."

What "all" means is *Decided* (the user, 2026-09-27: "yes. we loosen by
discovering sensible rules as we go, not by assuming. … there's some
obvious stuff to do we already expect to find, but we're not starting
there"):
- Every one of the five fields the work has must be found in the
  citation: title, first author, year, volume, first page.
- The year is exact. A year either side, which Crossref allows for
  online-first against print, is a loosening rule to be measured, not a
  starting point.
- Numbers in the citation that the work cannot account for do not fail
  it. Without a parser they cannot be told apart from an access date,
  an issue our row lacks, or a page count. They lower the score and
  show in the why.
- The issue and last page count in the score, not the rule. Many
  styles leave them out, as B does.

Crossref's floors (0.34 on the score, 0.4 on search relevance per
character of citation) and its two relevance weights in the score are
tuned to its Solr and its records, and are not carried over.

Each loosening rule is ours, stated in plain words, measured on the
answer key (§6) before it is taken, and logged here. The likely first
one is a citation without a title passing on author, year, volume and
first page, once those fields are on the row (example B).

**D5. What the answer says.** *Recommended.*

Per possible match and per near miss:
- the work's summary, as search results show it: title, authors,
  journal, date, retraction warnings;
- how it was found: identifier, text, or both;
- the score;
- each of the five fields as *agrees*, *not in the citation*, or *the
  work does not say*;
- the citation's numbers the work cannot account for.

Per identifier found: what it was as written, and what it resolved to.

Whether the text is identifiers and nothing more: `only_names`. The
identifiers' labels and links (`doi:`, `https://doi.org/`, `PMCID:`) count
as part of them. Such a text is not searched, since a bare token is a
name and not a word (2026-09-28, for D8).

When the identifier and the text land on different works, a statement
that they differ (D2).

For "could not", one reason:
- *nothing came up*: no candidates;
- *nothing agreed*: candidates, but all near misses;
- *nothing to go on*: no identifier, and too little text to search (an
  empty string, one word).

Near misses are in the answer and shown in the UI, marked as failed.
The user, 2026-09-27: "for now showing everything is good, we're in a
pre-v1 state anyway, the visuals would be helpful for me. easy enough to
remove later."

**D6. One route and one rpc.** *Recommended.* `GET /v1/citation?text=…`,
mirroring an rpc `LookupCitation` in `serving.proto`, since every JSON
route mirrors an rpc (`crates/ol-rht-serve/src/http.rs:1`). It is a GET,
like `/v1/resolve`, because one citation is short; v2's several will
want a POST body. The name stays clear of `/v1/chat*`, which the chat
branch's dev proxy takes.

**D7. Bounded like search, and logged.** *Recommended.*
- Text over 4 KiB is refused, not cut, as `service::bounded` does.
- A lookup runs in the search pool under the search deadline, since its
  search is a search: busy is a 429, too slow is a 504.
- It reads the text through the query reader (`crates/query-parser`),
  never through Tantivy's parser. Citations are full of parentheses,
  the input that made Tantivy's parser exponential (2026-09-25).
- Each lookup writes one log line, as the query log does: the outcome,
  bytes, milliseconds, what was listed, the top candidates' scores and
  failed fields, and the text, escaped.

**D8. One view: "Find a paper".** *Decided* (the user, 2026-09-28: "for
a visitor to the website, i think it makes more sense to consolidate the
'Find by ID' and 'Find by citation' into one view. from a website
visitor's perspective, these both do the same thing, even as from a
technical perspective they definitely do not").
- **One page and one call.** The page is `/find`: a text box that takes
  an identifier of any kind, a link, a title or a whole citation. It asks
  one thing of the server, `/v1/citation`, which reads identifiers of
  every kind (D12) and searches the words when there are any.
- **The text is kept in the URL**, so an answer is a link.
- **The list of possible matches** is shown as D5 has them, and "could
  not" with its reason.
- **Identifiers alone go straight to the paper**, as Find by ID did. When
  the text is identifiers and nothing more (the answer's `only_names`)
  and they name one paper, the page goes to it. Anything with words in it
  stays on the list.
- **The old pages' addresses are kept.** `/resolve` and `/citation`
  forward to `/find` with the same text. The API's `/v1/resolve` is
  unchanged, for a client that wants a name and nothing else.

The user looks at it and says whether it is right.

**D9. A crate of its own.** *Recommended.* `crates/citation-match`
holds finding identifiers, taking them out, and the check. They are
plain functions over text and a candidate's fields: no index, no store,
no server. The server supplies candidates and their fields; a batch
could, later.
- `search` gains a search over a chosen set of fields.
- `ol-rht-serve` gains the route, the rpc and the service function.
- `web` gains the page (D8).

**D10. Tests.** *Recommended.* Properties, since the input is anything
a person pastes (proptest, already in the workspace):
- the finder never fails on any text, and every identifier it returns is
  one the text contains;
- a DOI set in arbitrary text and followed by arbitrary punctuation is
  found;
- scores stay between 0 and 1;
- adding a number the work does not have to the citation never raises
  its score;
- taking a field away from a work never turns a pass into a fail, nor a
  fail into a pass on that field (absence is not agreement).

And cases:
- Crossref's own tests for this matcher
  (`matching/tests/test_cr_search_validation_matcher.py`, MIT), ported
  with attribution: the page readings, and its three citations — the
  CERMINE paper cited in IEEE's style, which passes; the same without its
  title, which Crossref matches and which is a near miss here until a
  rule of ours says otherwise; and a sentence that is no citation;
- examples A and B;
- the route's refusals.

**D11. What "found" means: exact, as Unicode compares text.** *Decided*
(the user, 2026-09-27: "sounds good"; ICU4X "sure"; edit distance when
the data asks for it, "ok").
- **Exact to start.** Two texts are the same when Unicode's canonical
  caseless match says so (The Unicode Standard §3.13, D145): decomposed,
  case-folded, decomposed again, and compared composed. Runs of
  whitespace are one space. The title must stand in the citation as
  whole words.
- **Nothing looser until the near misses ask.** Accents are not dropped,
  full-width forms are not folded, punctuation is not ignored, typos
  are not forgiven. Each is a loosening rule under D4, found and
  measured. Unicode's folding does fold a few things of its own — `ß` to
  `ss`, and the ligature `ﬁ` to `fi`, since its capital is `FI` — and
  those come with it.
- **ICU4X for the Unicode**: `icu_normalizer`, `icu_casemap` and
  `icu_properties` — the three component crates the `icu` meta-crate
  re-exports (the old `icu4x` crate is a deprecated pointer to it),
  taken alone so its calendars, collation and segmentation are not
  built. The Unicode Consortium's own Rust ICU, beside ICU4C and ICU4J.
- **If typos matter**, the textbook measure: the fewest one-character
  edits that turn the title into some stretch of the citation (Sellers,
  1980). It has one right answer, so any correct code gives the same
  number; our own code, checked in the tests against a slow brute-force
  version.
- **Why not a fuzzy-matching library's score.** Crossref's 0.7 was
  measured with fuzzywuzzy's `partial_ratio`. RapidFuzz, its maintained
  successor, computes `partial_ratio` exactly for a shorter string of up
  to 64 characters and by fuzzywuzzy's shortcut above that, which "only
  finds one of the best alignments and not necessarily the optimal one"
  (its docs). Many titles are longer. So the same name gives different
  numbers by library and by length, and a threshold on it depends on the
  library that happened to be chosen. Considered on the way: `moine`,
  which compares romanized input against Japanese and Chinese (another
  problem); `fuzzengine`, which has the right shape at 0.2.1 and some 37
  downloads a month; RapidFuzz's own Rust crate, which has the distances
  and `ratio` but no `partial_ratio`. The rule of thumb that came out of
  it: a library where the knowledge is data (Unicode's tables), our own
  code where it is a formula (an edit distance).

**D12. No namespace needed, for Find by ID or a citation.** *Decided*
(the user, 2026-09-28: "'find by id' should not require namespace — for
now we can run namespaceless requests through every namespace and return
a list of matches … i can't expect the general public to be able to come
up with 'reg1016' … i mean i don't even know what it maps to and i'm
writing this system").
- **A name as it stands answers as it always has.** `doi:10.1000/x`,
  `pmid:123` and `ol:…` go straight to the names table.
- **Anything else is read every way it can be**, and every reading that
  names something is listed with what it names: `found` in Find by ID's
  answer.
  - The citation finder's reading: a DOI as a link or in capitals,
    `PMID: 123`, a PubMed or OpenAlex link.
  - Or, for a single token, the token in every namespace:
    - DOI;
    - PMID;
    - PMC id, with or without its `PMC`;
    - OpenAlex id, in either case;
    - public id without its `ol:`;
    - each publisher namespace, so a PII finds its `reg1016`.
  - `12345` that is both a PMID and a PMC id lists both.
  - When every reading leads to one work, the page goes straight to it
    (D8).
- **Publisher namespaces** are found in the names table itself: one range
  query per namespace, once per epoch (`Names::publisher_namespaces`).
- **A public id written wrong** (`ol:` then something that is not one) is
  still refused as written. The `ol:` says what it was meant to be.
- **Cost on the dev store**, measured 2026-09-28: about 2 ms a Find by
  ID call, every namespace tried.

## 5. Milestones

**CL1 — Identifiers.** The crate with the finder (D2) and its
properties, plus the route, rpc and tab answering from identifiers only,
with "could not" otherwise. Useful on its own: paste a citation that
ends in a DOI and get the work.

*Built 2026-09-27:*
- **`crates/citation-match`**, `identifiers::find`. It finds DOIs, PMIDs
  and PMC ids in any text. Each DOI is read as `model::Doi` reads it, as
  every name it may be, longest first. There are four properties
  (anything is read and every name is one `merge::Name::parse` takes; a
  DOI, a labelled PMID and a PMC id set in arbitrary text are found) and
  nine cases.
- **`GET /v1/citation?text=`** and **`rpc LookupCitation`**, answering
  with:
  - `identifiers`: each as written, where it stands, the name it was
    read as, what it names (`one`, `shared`, `split`, `none`) and the
    works;
  - `matches`: each work with `named_by`, the names that led to it, and
    its summary;
  - `could_not`.

  Text over 4 KiB is refused.
- **The query log** gains a `lookup:` line.
- **The tab**, "Find by citation", beside "Find by ID". It lists the
  identifiers and the works, and says so when the citation's
  identifiers name different works.

What CL1 leaves for later: no text path (a citation with no identifier
gets "could not" and says why); no check; no pool, since only names are
looked up so far.

**CL2 — The check.** Crossref's readings ported and the rule of D4,
with Crossref's cases for the readings, examples A and B, and the
properties. No server change.

*Built 2026-09-27:*
- **`text`**: the compare form of D11 through ICU4X, words by Unicode's
  general categories, and whole-word containment.
- **`check`**: `Citation::read` takes out the identifiers and a leading
  label and gathers the numbers. `check(&Citation, &Work)` gives each of
  the five as *agrees*, *not in the citation* or *the work does not
  say*, the numbers left over, and the score. `Verdict::passes` is D4's
  rule.
- **Tests: 38 in the crate.** The new ones:
  - properties: anything is checked, with a score from 0 to 1; an extra
    number fails nothing and raises nothing; a field taken away agrees
    with nothing; a title in capitals is the same title; the compare
    form is its own; two encodings of one text compare equal;
  - cases: examples A and B, with and without volume and pages;
    Crossref's three citations and its page readings; the year exact;
    Unicode's folding and what it leaves alone; surnames and initials; a
    label and identifiers kept out of the numbers.
- The properties pass at 5,000 cases each.

Not yet wired into the server: the check runs on the identifier path in
CL3, with the text path.

**CL3 — Candidates.** The field-restricted search (D3), the service
function in the search pool (D7), the whole answer (D5) with both paths
of D2, and the lookup log.

*Built 2026-09-27:*
- **`search`**: `Fields::cited()` (title, authors, venue; no abstract)
  and `Opened::candidates`, a top-*n* search over them with no facets,
  within a deadline. `translate::query` takes its fields as an argument.
  `serve::Serving::candidates` and `Answers::candidates` carry it up.
- **`ol-rht-serve/src/lookup.rs`**:
  - both paths every time;
  - each work checked, its row read as title, first author and year
    (volume, issue and pages when the citation fields land);
  - ordered: passing first, then by score, then works the citation
    names, then by search rank;
  - split into `matches` and `near_misses`;
  - `differ` set when an identifier names one work and the words pass
    another;
  - `could_not` gains `nothing-agreed`.
- **`service::looked_up`** is async. It caps the text, reads the words
  through the query reader, takes a search thread through the helper it
  now shares with search (`search_thread`), and runs the lookup there
  under the search deadline: 429 when busy, 504 when late.
- **The protocol**: `CitationMatch` gains `by_text` and `verdict`, a new
  `Verdict` message carries the five fields and the rest, and the
  response gains `near_misses` and `differ`.
- **The log line** gains `candidates=`, `near=` and `nearest=score:failed`.
- **The tab** still shows only the matches. It says "found by its words"
  where it should, and warns when the identifier and the words differ.
  The verdicts and near misses are CL4.
- **Tests**: search 7 (one new); JSON 14 (two new: the words path and
  its outcomes, and a late lookup refused); gRPC 7; all pass, with
  clippy clean.

*Tried on the dev store* (110,469 held works; six citations of real works
written by hand, their volumes and pages made up):
- Vancouver, with and without the DOI: found, by DOI and words or by
  words alone; title, author and year agree; volume and pages left over.
- **IEEE: a near miss on its title.** PubMed's titles end in a full stop
  (`…for cardiologists.`) and IEEE closes the title with `,”`. This is
  the first loosening rule the data offers (§10, item 1), and it is not
  taken.
- The user's case, a citation carrying another paper's DOI: the paper
  its words describe is listed as passing, the DOI's paper is listed
  with title, author and year not in the citation, and `differ` is
  true.
- A citation without its title: a near miss on the title, as example B.
- A long title with an acronym in brackets: found.
- 70–110 ms a lookup, in a debug build. Every lookup brings 19–20 near
  misses, most agreeing on the year alone, so the tab will want to show
  the nearest few (CL4).

**CL4 — The page's list.** D8 in full, near misses included. The user
looks.

**CL5 — The first answer key run.** §6. It measures the strict rule and
the first loosening rules (D4).

CL5's key can be gathered while CL2–CL4 are built. No rule is loosened
before it runs.

## 6. How we will know it works

An answer key: citations, each with the work it cites, known without
asking this matcher. It has three kinds, each counted apart:

1. **Crossref's generated citations.** Crossref rendered the metadata of
   thousands of works in eleven citation styles, three of them without
   titles (ACS, AIP, and Elsevier without titles). It published the
   results with the DOI each came from (landscape §3.2). The answer is
   that DOI's work. These are independent of us, and the one source of
   citations without titles whose answers are known.
2. **Real citations from our own records.** These are references whose
   publisher deposited both the citation text and a DOI, where we hold
   the DOI's work.
   - The DOI is the answer, and it is taken out of the text before the
     lookup, so the lookup must find the work from the words alone.
   - The text is real, but from the easy side: references someone
     already matched are the well-formed ones (landscape §13.1).
3. **Citations of works we do not hold.** These come from kind 2 where
   the DOI names nothing we hold, again with the DOI taken out. The
   right answer is "could not". They are the only kind that catches a
   wrong work being listed, the silent error.

What is counted, per kind, for titled and untitled citations apart:
- the right work listed first;
- the right work listed, but not first;
- only wrong works listed;
- could not.

For every miss, the report also says **whose miss** it was:
- the right work was not among the twenty candidates (search's miss,
  §7);
- or it was, and it failed the check (ours).

The key starts small, a few hundred citations, and is read by eye. It
runs against a local store, not the server.

## 7. What this waits on and hands over

**Volume, issue and pages on the served row.** These come from the
[citation fields plan](citation-fields-plan.md), which the user is
working on in parallel (2026-09-27: "assuming that we'll have full
citation data in the search index in the coming days"). Until it lands:
- the check has three of its five fields: title, author and year;
- a citation's volume and pages go unchecked;
- the loosening rule for citations without titles cannot be tried
  (example B).

Everything here is built for all five fields and reads whatever is
there.

**The search engine.** Candidates come from the works index as it
stands: its provisional mapping, the default tokenizer, no stemming,
and a ranking not yet measured. The search track is waiting for the v12
merge to land in production before it takes a baseline. The user,
2026-09-27: "we shouldn't be alarmed if some of the search-dependent
bits don't quite work how we want yet. not our job to fix, yes our job
to keep track of." So every lookup where the cited work was not among
the candidates is kept, with its text, as a list the search track can
have. For real lookups the list lives in the log (D7); for the key, in
its report (§6).

## 8. Decision log

- 2026-09-27, the user:
  - one-offs only, as an API route and a tab: **no part of ingestion**
    until there is an answer to scaling;
  - v1 is one citation as text; v2 several, with candidates tied to the
    text; v3 PDFs; v4 images. Text PDFs come before images, amended from
    images first ("there are no users to impress like that yet");
  - a list of possible matches rather than "could not" whenever there
    are any, each with why; reasons only, if the lists prove bad;
  - identifiers first;
  - the bar is all of the fields or nothing, loosened only by rules of
    our own; Crossref's three of five is tuned to its search (D4);
  - the identifier's work and the text's match both shown, and said to
    differ when they do (D2);
  - near misses shown in the UI while pre-v1 (D5);
  - CL1, identifiers, first: "fine place to start to establish the
    pathways through the code";
  - "all" is every one of the five fields the work has, the year exact,
    extra numbers not failing; loosened by rules discovered as we go,
    not assumed, even the ones we already expect to find (D4);
  - "found" is exact, as Unicode's canonical caseless match compares
    text, through ICU4X (`icu`'s component crates); no fuzzy library;
    an edit distance only when the near misses ask for one (D11);
  - no parser in v1;
  - volume, issue and pages arrive in parallel, through the citation
    fields plan;
  - search quality is the search track's to fix and ours to keep track
    of.
- 2026-09-28, the user:
  - Find by ID and Find by citation as one view, "Find a paper" (D8);
  - no namespace needed, for Find by ID or a citation (D12);
  - "we can drop the period at the end of a title": a title is compared
    without the full stop that closes it (D4);
  - on two titles pasted alone that missed, the title-alone rule (D4):
    a citation that is a work's title and nothing more passes on the
    title. Both titles were the search's first candidate. The rule
    serves the evaluation plan's "type the exact title, get the paper"
    (its §10, item 7).
- 2026-09-28, found in the same misses: the first-author rule took any
  word of the printed name as its surname, so `Chinese Society of Health
  Management` agreed with an unrelated citation by its `of`. The family
  name is now the last word of the printed name, or the last before a
  comma (§12, item 4).

## 9. v2 to v4

Sketches, not plans.

- **v2, several citations.** The new problem is splitting a block of text
  into its citations: numbered lists, hanging indents, citations running
  on without a break.
  - GROBID's reference segmenter does this (Apache-2.0, a service on the
    box). A rule for the well-behaved cases could come first.
  - Each citation's answer carries the byte span it came from, as the
    query reader's words do.
  - The route takes a POST body.
- **v3, PDFs with a text layer.** GROBID takes a PDF to its reference
  list end to end, and the list goes to v2.
  - GROBID's own figures: under half of references come out entirely
    right from clean PDFs when the parse is scored strictly (landscape
    §5).
  - That score counts errors in the fields GROBID parses out. The check
    reads each reference's raw text, not those fields.
- **v4, images.** Photos, screenshots and scanned PDFs.
  - The text is read first, then v2 runs. The reading is OCR or a small
    vision-language model, the route the next-steps note of 2026-09-19
    sketched.
  - Scans bring OCR damage, which is where the landscape's degradation
    model (§14.2) would earn its keep.

## 10. Open items

1. **The loosening rules** (D4): which, in what order, each measured on
   the key.
   - Taken on the user's word, 2026-09-28:
     - a title's closing full stop, which PubMed's titles carry and
       styles that close a title with a comma or quote do not;
     - title alone (`Rule::TitleAlone`).
   - Seen, not taken: a citation without a title passing on its other
     fields, once volume and pages are on the row (example B).
2. ~~**What "all" means** (D4).~~ *Decided 2026-09-27*, as D4 reads.
3. **Crossref's data licence.** The repository is MIT; whether that
   covers the generated citations needs settling before any copy lands
   in the repo. Until then the key's files stay out of git.
4. ~~**Two new dependencies.**~~ *Decided 2026-09-27* (D11): ICU4X for
   Unicode, no fuzzy library, and no ASCII folding.
5. **Names**: the route, the rpc and the crate are built as
   `/v1/citation`, `LookupCitation` and `citation-match`; renaming them
   is the user's call.
6. **A DOI the model does not read back** (found 2026-09-27, building
   CL1). `model::Doi` lowercases `İ` to `i` and a combining dot
   (U+0307), then refuses that dot when it reads its own output again.
   So a DOI with a capital dotted I is stored in a form the names table
   calls not canonical. The finder leaves such a reading out rather than
   ask the table a question it refuses. This is a model question
   (a model version), not this plan's.

## 11. Held for later

- **A parser**: GROBID's citation model, or the small fine-tuned models
  of landscape §7. For v2's splitting and v3's PDFs, and as fields to
  feed the check.
- **Sending the hard cases to a larger model**: landscape §4, rung 6.
- **Journal abbreviations mined from our own resolved references**:
  landscape §14.3. They would let the journal name be a sixth field; the
  check does not look for it today.
- **Volume and first page as search terms**, so the numbers of a
  citation without a title help find candidates. This belongs to the
  search track or the citation fields plan.
- **biblio-glutton on the same key**, as the comparison it is meant to
  be (landscape §17).
- **The batch** over the unresolved stream: out of scope by the user's
  decision.

## 12. What we are compromising on

Kept as we go (the user, 2026-09-26). Each line gives the compromise,
what it costs, and what would lift it.

1. **Three of five fields until citation fields land.** A work is
   matched on title, author and year alone, and the citation's volume
   and pages go unchecked. Lifted by the citation fields plan (§7).
2. **Candidates from the provisional search.** A cited work the search
   does not surface cannot be found, whatever the check would say.
   These misses are counted apart (§6) and handed over (§7). Lifted by
   the search track.
3. **All or nothing to start** (D4). Right works fail on a typo'd title,
   an online-first year, or a style without titles, and show as near
   misses rather than matches. Lifted by rules of our own, measured on
   the key (CL5).
4. **The first author's family name is a guess.** Served authors are
   display strings, given names first, so the family name is taken as
   the last word, or the last word before a comma. Where it falls short:
   - a name printed family-first with no comma (`Lowry OH`);
   - a family name of several words, whose last word stands for it (`de
     Andrade Fernandes`, as `Fernandes`);
   - a group author, whose last word stands for all of it.

   Lifted by family names on the row (citation fields plan).
5. **The first author is looked for in the first name of the author
   list**: the citation up to its first comma, semicolon or bracket.
   Costs:
   - a style that puts the title or a number first loses that field;
   - authors joined without a comma (`D. Tkaczyk and P. Szostek`) put
     the second in the first name;
   - a sole author followed by a full stop (`Lowry OH. Protein…`) runs
     the first name into the title.

   Lifted by a parser (§11).
6. **Exact, as Unicode compares text** (D11). A citation that drops an
   accent, uses full-width letters, differs in punctuation or carries a
   typo does not agree, until a rule found in the near misses allows it.
   Text is compared in its own script, never transliterated, so no
   script is favoured.
7. **No arXiv ids.** The names table has no arXiv namespace, so an arXiv
   id in a citation is not an identifier here, and the text steps must
   find the work. Lifted by arXiv as a source.
8. **Stubs only by identifier.** A work we know only as cited has no
   text to search.
9. **Twenty candidates.** A work ranked lower is never checked.
10. **The key's real citations are the easy ones** (§6, kind 2). They
    overstate how well the lookup does on citations nobody has matched.
    Kind 1's untitled styles and kind 3 are the counterweight. A sample
    of unmatched citations judged by hand would lift it.
11. **Crossref's number readings, borrowed.** A range written short is
    read long (`265-75` as 265 and 275, when the tail is not above it),
    and a volume numbered as its year is found by the one number. A
    range such as `1999-2000` written `1999-00` is not read long (00 is
    below 99), and a date range of that shape would be. Lifted by
    measuring on the key.
12. **A leading label is only a number in brackets, or followed by `.` or
    `)`.** `12 Lowry` keeps its 12 as the citation's own number. That
    lowers the score, but it never fails the rule.
13. **The year checked is the row's promoted date**: the earliest date any
    source gives, which is often the online-first date. A citation of
    the print issue a year later fails on the year. Lifted by the
    citation's date coming with the citation fields (their D4), or by a
    year-either-side rule measured on the key.
14. **Every candidate that fails is a near miss**, up to twenty, most of
    them agreeing on nothing but the year. Kept whole in the answer
    while pre-v1 (D5); the page is to show the nearest (CL4).
15. **Title alone passes every work of that title.** A title shared by
    many works (`Editorial`, `Reply`) lists every one the search turns
    up, and only those in its top twenty. Lifted by the key showing how
    often, and a rule for it.
16. **No bare identifier inside a longer citation** (D12). A PMID or a
    PII written without its label in the middle of a citation is not
    tried, and the words must find the work. Lifted by a parser, which
    would know which number is which.
17. **Publisher namespaces are the epoch's** (D12). A publisher first
    seen in a batch since the epoch is not tried for a bare id until the
    next epoch.
