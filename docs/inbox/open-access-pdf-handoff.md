# Reading open-access PDFs in place, and filling missing references

**Status:** idea note, 2026-09-26, recorded at the user's request so the
idea is not lost. §1–§9 are the first idea: reading PDFs. §10 is a
second idea from the same day that uses the same trick to fill in
references publishers never deposited. Speculative, not a plan: nothing
here is built, scheduled or decided, and nothing in it amends the plans
of record. The
compliance reasoning in §5 is the user's working theory, not legal
advice; §6 lists what must be checked against each publisher's real
terms before anything ships. Facts are marked **P** (primary: this
repository's code, or the project's own page, read 2026-09-26), **S**
(secondary, or from memory of the product) or **U** (not verified).
Related: [the industry around the corpus](industry-landscape.md) §4.3
and §4.6 (Elicit's extension, "the customer's own entitlements"),
[ambitions beyond citations](ambitions-beyond-citations.md) §6 (full
text is never served), [the capability matrix](competition-list.md)
(Zotero, Unpaywall, LibKey), [citation parsing](citation-parsing-landscape.md)
§15 (a PDF arriving with no metadata).

## 1. The idea

Many articles are open access, but "open" covers different permissions.
Some licences let anyone redistribute the article, so the site could
show it directly. Others let anyone read and download the PDF from the
publisher, but do not let a third-party site embed or serve it.

For the second kind: **a browser extension downloads the PDF from the
publisher to the reader's own disk, in the reader's own browser, and
hands it to the site's PDF viewer.** To the reader, it feels as if the
article is on the site. The site never holds, serves, proxies or frames
the publisher's file. The reader did what the terms allow, which is
download a copy, and the viewer opened a file on the reader's machine.

The viewer this implies is [EmbedPDF](https://github.com/embedpdf/)
(§7).

Words used below:

- **reader**: the person reading. The software is always called the
  **viewer**.
- **copy**: one place a work's full text can be had: a publisher's PDF,
  an author's manuscript in a repository, a PMC copy. OpenAlex calls
  these *locations*.
- **tier**: what a copy's terms let the site do with it (§2).
- **the extension**: the proposed browser extension.
- **handoff**: the extension passing the downloaded bytes to the viewer
  in the site's page.

**Running example.** Work *W* has three copies:

1. The publisher's PDF. It is free to read and has no licence, which
   OpenAlex calls *bronze*.
2. The authors' accepted manuscript in a university repository, under
   CC BY.
3. A PMC copy.

## 2. Three tiers

| tier | what the copy's terms allow | examples | what the site does |
|---|---|---|---|
| 1. redistribute | anyone may copy and share it | CC BY, CC BY-SA, CC0, public domain; CC BY-ND for unchanged display | the viewer opens it directly; the site may fetch and even keep it |
| 2. download only | anyone may read and download it; a third party may not serve or embed it | bronze (free to read, no licence); publisher-specific OA licences; arXiv's default licence; repository copies marked all rights reserved; CC BY-NC if the site counts as commercial | **the extension**: the reader's browser downloads it, the viewer opens the local file |
| 3. nothing to download | no free copy, or an HTML page only | closed access | link out to the landing page (see §8 for the reader's own subscription) |

For *W*, the repository manuscript is tier 1 and the publisher's PDF is
tier 2. The site could open the manuscript directly, or reach the
published version through the extension. Which one to prefer is a
product choice, not a licensing one.

Two notes on the table:

- **CC BY-ND** forbids adaptations, not verbatim sharing, so displaying
  it unchanged with attribution is tier 1 (S).
- **CC BY-NC** depends on whether the site is commercial, which is the
  user's call. It is not decided here.

arXiv's terms say e-prints may not be stored and served without the
holder's permission or a permissive per-submission licence, and ask for
a link back to arXiv (P, arXiv's terms of use, checked 2026-09-15). An
arXiv paper under the default licence is therefore tier 2; one under CC
BY is tier 1.

## 3. Where the tier comes from: data already ingested

The normalized layer already carries most of what deciding a tier needs
(P, this repository):

- **OpenAlex locations**
  (`crates/source-openalex/src/normalize.rs`, `Location`). Each copy
  has:
  - `is_oa`, `landing_page_url` and `pdf_url`;
  - `license`, one of cc-by, cc-by-sa, cc-by-nd, cc-by-nc, cc-by-nc-sa,
    cc-by-nc-nd, cc0, public-domain, other-oa, publisher-specific-oa or
    unspecified-oa (`vocab::License`);
  - `version`: published, accepted or submitted.

  Each work also has `open_access.status`: diamond, gold, green,
  hybrid, bronze or closed.
- **Crossref licences**
  (`crates/source-crossref/src/normalize.rs`, `License`). Each has a
  URL, a `content-version` (vor, am, tdm, unspecified or stm-asf) and,
  since model 10, a start (`LicenseStart`). A start after publication
  means **a copy can change tier on a date**: for example, a CC BY
  licence that begins a year after publication.
- **Crossref links**, each with a `content-type`, a `content-version`
  and an `intended-application` (text-mining, similarity-checking,
  syndication or unspecified). The field says what a link is for. A
  text-mining link is provided for mining, often under a click-through
  TDM licence, so fetching it for reading is a different use. Do not
  treat such links as reading links without checking (U).
- **PubMed**: a PMC id means a PMC copy exists. PMC's own terms differ
  between the open-access subset, which has a licence per article, and
  the rest, and they restrict automated retrieval outside the subset (U).

**The gap.** The merged view keeps one bit, `access: Option<bool>`
(`crates/merge/src/view.rs`). A tier needs per-copy facts carried into
the merged record: URL, licence, licence start, version and where the
copy lives. When OpenAlex's licence for a copy and Crossref's licence
disagree, deciding whose word wins is a merge question for later.

## 4. How the handoff could work

A sketch, not a design:

1. The reader clicks *Read* on *W*'s page. The site knows *W*'s tier-2
   copy: the candidate PDF URLs and the landing page.
2. The page asks the extension for that copy. It goes through a small
   script the extension adds to the site's pages: the page and the
   script exchange `window.postMessage`, and the script talks to the
   extension.
3. The extension fetches the PDF from the publisher inside the reader's
   browser, with the reader's cookies and IP address, as if the reader
   had clicked the publisher's download link. It could do this from a
   tab on the publisher's landing page, which passes any bot check the
   reader already passed, or from its background worker with permission
   for that host. Which of these works for which publisher is U.
4. The extension checks that it got a PDF, which starts with `%PDF-`.
   An HTML page instead, such as a challenge page or a login wall, is a
   failure. The extension opens the landing page in a tab so the reader
   sees what the publisher wants, and retries nothing on its own.
5. The extension saves the copy to the reader's disk (the `downloads`
   API). The reader keeps it, exactly as after clicking *Download*.
6. The extension hands the bytes to the page as an `ArrayBuffer`, and
   the viewer renders them. **The bytes never reach the site's
   servers.**
7. To reopen the paper, the site asks the extension for the file again,
   or keeps a copy in the browser's own storage on the site's origin.
   Whether that second option counts as "the site storing it" is §6,
   item 5.

**Without the extension**, the same viewer works if the reader downloads
the PDF themselves and drops it on the page. That is the zero-install
version of the same handoff, and the natural first piece to build. The
extension only removes the clicks. A dropped file must be matched to its
work (a DOI in the file's XMP metadata or on its first page;
[citation parsing](citation-parsing-landscape.md) §15). A file the
extension fetched needs no matching, because the site asked for a known
work.

## 5. Why this might stay inside the terms (the working theory)

- **The reader downloads the copy**, from the publisher, under the
  publisher's terms for personal use. The extension acts for the reader,
  on the reader's click, one article at a time.
- **The site never receives the file.** It serves a PDF viewer, and the
  viewer opens a file on the reader's disk, as a desktop PDF app or a
  reference manager's built-in viewer would.
- **The publisher still sees the download.** The request reaches the
  publisher's site, so their download counts stay whole. Serving the
  site's own copy of the file would take that traffic away. This is
  reasoning, not a checked fact, but it matters to the sellers' fears
  that [the industry note](industry-landscape.md) tracks.
- **The pattern is old.** Reference managers' browser connectors, such
  as Zotero's, save the PDF from the page the reader is on into the
  reader's library and open it in their own viewer (S). Elicit's
  extension uses the reader's logins to download papers their
  institution licenses (P, per [the industry note](industry-landscape.md)
  §4.3). Unpaywall's extension and LibKey Nomad route readers to copies
  (S).

## 6. What could break it: check before building

1. **Automated-download clauses.** Many publishers' terms forbid robots,
   spiders or "systematic downloading" (U, per publisher). One fetch per
   click by the reader's own browser is arguably not that. Prefetching,
   background fetches or "download every reference" look exactly like
   it. The rule would be: only on a click, one at a time, never
   prefetch. This matches the project's rule of being gentle with
   providers.
2. **Presentation clauses.** Some terms may forbid showing their content
   inside a third party's interface, whatever route the file took (U).
   In any case, the viewer should show where the copy came from: the
   publisher, the licence, the version, the day it was downloaded and a
   link to the landing page. The more the site makes it *feel* as if the
   article is available directly, the more this matters.
3. **Commercial use.** When the reader downloaded a CC BY-NC copy, is
   the site's displaying it commercial use by the site? The site does
   not copy or distribute the file (U).
4. **Anything derived from the text.** Full-text search, highlights,
   entity recognition and summaries (the ambitions of
   [ambitions beyond citations](ambitions-beyond-citations.md) §6) must
   stay on the reader's machine unless the licence allows more. §6 of
   that note assumed text obtained under a mining licence. A reader's
   personal download is not the site's to mine. The reader's own notes
   and highlights stored on the site's servers are probably fine, but
   notes that quote the text are U.
5. **A copy in the browser's storage.** Storage on the site's origin
   (OPFS or IndexedDB) sits on the reader's device but is controlled by
   the site's code. The copy in the Downloads folder is the clean case,
   and the browser-storage copy is U.
6. **Extension store review.** Chrome Web Store and Firefox add-on
   review look at host permissions, a single stated purpose and privacy
   disclosures. Asking for permission per publisher host on first use,
   rather than for every site up front, is the conservative shape (U).
7. **Bot defences.** Publishers behind a challenge service may challenge
   an extension's background fetch. A fetch from a tab passes whatever
   check the reader passed (U).
8. **Browsers.** Chrome's `externally_connectable` lets a page message
   an extension directly, and Firefox's support for it is U. The
   postMessage bridge in §4 works in every browser (S). Safari needs its
   Safari Web Extension packaging and an Apple developer account (S).

## 7. The viewer: EmbedPDF

Read from the project's repository and docs site, 2026-09-26, two
fetches (P unless marked):

- **What it is.** A framework-agnostic PDF viewer. Its engine is a fork
  of PDFium (Google's PDF library) compiled to WebAssembly, and it runs
  in the browser. It has React, Vue, Svelte, Preact and plain
  JavaScript bindings. Features: annotation (highlights, sticky notes,
  free text, ink), redaction, search, text selection, zoom and rotation.
- **Licence: the two sources disagree.** The repository README says
  everything is Apache 2.0 except a "CloudPDF server" component under
  FCL-1.0-ALv2. The docs site says MIT. Both are permissive, and a viewer
  that runs in the browser does not need the server component. Settle
  which is current before depending on it.
- **Versions.** The README says v3 is under active development and not
  yet recommended for production; v2 is.
- **Packages** named on the docs site: `@embedpdf/snippet` (everything,
  with a UI), `@embedpdf/engines` (`PdfiumEngine`) and `@embedpdf/pdfium`
  (low level).
- **Opening from memory** is the one call the handoff needs, and the
  pages fetched did not show it (U). PDFium itself opens documents from
  memory, so expect it to exist.
- **Fit.** `web/` is React 19 with Vite and TanStack Router (P). Tier 1
  needs the same viewer, opening a copy the site fetched itself.

## 8. Later, maybe

- **The reader's own subscriptions.** The same extension, pointed at a
  paywalled article the reader's institution licenses, would fetch it
  through their login. That is Elicit's model and the second row of
  [the industry note](industry-landscape.md) §4.6, "the customer's own
  entitlements". Subscription terms commonly allow an authorised user a
  personal copy (U). It is a bigger step and closer to the sellers'
  fears, so it is only noted here.
- **"Open every reference."** Tempting, and exactly what §6, item 1
  warns against. If it is ever built, each fetch is still one click.
- **A local library.** The files the extension saved could become the
  reader's library, like Zotero's, with import from Zotero.

## 9. If it became a plan

In order, each piece useful without the next:

1. **Per-copy facts in the merged record**: URL, licence, licence start,
   version and host, from OpenAlex and Crossref, so every work knows its
   tiers. This includes deciding whose licence wins when the sources
   disagree.
2. **The viewer in `web/`**, opening a dropped file and a tier-1 copy.
3. **The extension**: Chrome first, one publisher at a time with
   permissions asked per host, the postMessage bridge and the checks in
   §4.
4. **Publisher terms, read and recorded per host**, beside the
   per-source terms. This is the eventual "data terms first class in
   code" ambition, applied to full text.

## 10. The same trick for missing references

Added later on 2026-09-26. The user raised this one in frustration and
wanted it recorded all the same.

**The idea.** When a publisher's page for a paper lists references that
the site does not have, the extension notices, reads the list from the
page and fills in the gap. The trick is the same as §1: the download
lands on the reader's desktop. The reader's browser fetched the page,
the site never visited the publisher, and the saved page is the reader's
copy.

Words used in this section:

- **citing work**: the paper whose reference list this is.
- **cited work**: one entry in that list.
- **deposited reference**: a reference the publisher sent to Crossref.
- **contribution**: what the extension would send back to the site.

**The gap it aims at.** Every reference deposited with Crossref has been
open metadata since 2022-06-03 (P, per [the industry note](industry-landscape.md)
§5). What is missing is the references that were never deposited. The
other bulk source fills little of that gap. On the sample, counting
only citing works both sources hold and cited works that are held:

- 36,837 edges are shared by Crossref and OpenAlex.
- 14,676 are Crossref's alone.
- 1,730 are OpenAlex's alone.

(P, [merge plan](../merge-plan.md) §6.)

**A sketch:**

1. **Detect.** On a publisher's article page the reader opened, whether
   by browsing or through the reading handoff of §4, the extension reads
   the reference list. Many pages mark references up for machines:
   `citation_reference` meta tags in the Highwire style, or DOI links
   inside the list (S; which publishers do which is U). The site tells
   the extension what it already holds for the citing work, so a gap
   shows as a difference. The PDF from §4 carries the list too, but
   parsing it in the browser is harder
   ([citation parsing](citation-parsing-landscape.md)).
2. **Download to the desktop.** The extension saves the page, or its
   reference list, to the reader's disk, as *Save page as* would.
3. **Extract on the reader's machine.** Each entry becomes identifiers
   (a DOI or PMID) where the page gives them. Entries without
   identifiers raise an open choice:
   - send the reference string for the site to match, which catches
     more but sends the publisher's formatted text; or
   - send identifiers only, which is cleaner but misses those entries.
4. **Contribute, with consent.** The extension sends the citing work,
   the cited identifiers, the page's URL and the time. In the merge this
   is a new asserter, "contributed by readers". The graph's label byte
   already indexes asserter sets, so a contributed edge can be told
   apart from a deposited one everywhere it appears.

**Why it might be defensible (the working theory):**

- "A cites B" is a fact. In the US facts are not copyrightable (Feist;
  from memory, per [the industry note](industry-landscape.md) §5).
- The user doubts that publishers' terms legitimately cover the
  reference lists at the end of a paper at all (the industry note §2).
- The site never touches the publisher. The reader's browser loads
  pages the reader opened, and only identifiers leave the machine.

**Why it is harder than the PDF case.** These are the questions to
settle before building anything:

1. **Something comes back.** In §4 nothing leaves the reader's machine.
   Here the whole point is that facts reach the site and are served to
   everyone. Downloading to the desktop makes the *fetch* the reader's.
   The extraction and the sending are still the site's software working
   for the site. The trick covers the fetch, not what is sent.
2. **Website terms bind the reader.** A publisher's terms forbidding
   automated extraction are a contract with the reader, where they are
   enforceable at all (U). An extension that extracts on the reader's
   behalf could be read as the site inducing a breach (U, legal).
3. **The EU database right.** [The industry note](industry-landscape.md)
   §5 already names it as the complication. The Database Directive
   (96/9/EC, article 7(5)) forbids the "repeated and systematic
   extraction" of insubstantial parts of a protected database (S, from
   memory, not legal advice). Many readers' extensions each taking one
   page's list add up to exactly that pattern. Whether a publisher's
   reference lists are a protected database is U, and so is which
   jurisdiction applies (where the site runs, or where readers are).
4. **Privacy.** An extension that reads publisher pages is watching
   where the reader browses. It must act only on a click ("send these
   references"), never passively, and send nothing about the browsing.
   Extension stores have user-data policies to meet (U).
5. **Poisoning.** Anyone can forge a contribution, for example made-up
   references to inflate a paper's citation count. The server cannot
   check a contribution against the page without visiting the page,
   which is the very thing it avoids. Possible defences:
   - accept only cited identifiers that resolve to known works;
   - refuse a contribution that does not parse, rather than repair it;
   - keep contributed edges apart from deposited ones, marked as such;
   - count an edge as confirmed only when independent readers agree, or
     when another source (OpenAlex, PMC, a parsed PDF) asserts it too.
6. **Gentleness.** Only pages the reader opened, one at a time. The
   extension never follows the list to visit the cited papers' pages.

**Routes to try first**, none of which involves a publisher's website:

- **Measure the gap by publisher** on the corpus: which Crossref members
  deposit no references at all.
- **PMC and Europe PMC reference lists** for the open-access subset
  ([ingestion plan](../ingestion-plan.md), the long tail;
  [citation parsing](citation-parsing-landscape.md) §6.5).
- **OpenCitations** (citation parsing §6.4).
- **Semantic Scholar's citation datasets**, ODC-BY with attribution.
- **Reference lists parsed from open-access PDFs the site may hold**,
  tier 1 of §2. That is the "PDF arrives with no metadata" case of
  citation parsing §15, applied to copies whose licence allows it.
