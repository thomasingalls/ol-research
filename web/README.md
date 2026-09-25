# ol-rht web

A small query interface over the citation graph. Four screens, one person, no
product ambitions:

- **Search** — text box, list of results. A result is the same row as any
  list's, less the identifiers: title, authors, venue and year, and a badge
  for a retraction or an expression of concern, so a reader is warned before
  they open it. The ids and names are on the work's page.
- **Work detail** — a work's metadata, its ids, and where to read it.
  Under the year and venue, the ids are one list behind a ghost button that
  shows the first of them — a DOI if there is one — and a click on any line
  copies it: every **name**, an alias or a shared name saying so, and the
  public id last. A name is copied without its namespace (`10.1234/x`,
  `12345678`), which is what a reference manager or PubMed's search box
  takes; the public id is copied whole, `ol:` and all. The list stays open
  after a copy, so the check lands beside what was clicked.
  Under the abstract, where to read it is a split button. Its face is the
  first place the work has, in the order publisher (a DOI), PubMed Central
  (a PMC id), PubMed (a PMID), OpenAlex, and its menu holds the rest; each
  opens in a new tab, sending no referrer. The places come from the promoted
  names only. A publisher's own id has nowhere to go, and an alias or a
  shared name may lead to another work, so none of those is a place.
  Beside them, four lists, one tab each, and every tab the rest of a
  sentence about this paper: **Cites**, *Cites the same sources as*,
  **Cited by**, *Cited by the same works as*. The line under the tabs says
  the open one's sentence whole ("This paper is cited by these."). The tabs
  sit a pair to a row where the header has 28rem; narrower — a phone, or the
  side column at 1280–1535px — they stack one to a row, the pairs set apart
  by a wider gap, so no label is cut. *Cites* and *Cited by* are paged by
  cursor with a "Load more", and their tabs carry their counts, which come
  with the work. A tab the work's own counts show to be empty — it cites
  nothing, or nothing cites it, and so there is nothing to compare by
  either — is drawn muted, and still opens on a "No works" that says why.
  Every row is clickable. A retraction or an expression of concern is
  badged.
- **Related works** are the two tabs in italics: two questions, one call each.
  *Cites the same sources as* — papers that cite some of the same sources as
  this one. *Cited by the same works as* — papers cited by some of the works
  that cite this one. Each row says the overlap in words ("7 of the 9 works
  citing it also cite this one"); the cosine beside it only says why the row
  is where it is.
- **Resolve** — paste a name (`doi:…`, `pmid:12345678`, `pmc:PMC…`,
  `openalex:W…`, `reg1038:…` — a publisher's own id) or a public id. One work jumps straight there; a **shared**
  name offers the choice between the works it was accepted into; an id retired
  by a **split** says so.
- **Chat** — a shell and nothing behind it yet. What you send is shown, the
  page says that no agent is connected, and nothing answers. The
  conversation is held by the screen, not the URL, and is gone when you
  leave it.

Vocabulary, kept the same as the pipeline: a **work** is one entity; its
**names** are the identifiers that point at it; a **stub** is a work we know
only because something cited it — it has names but no record behind it.

A stub is not missing data. It is 98% of what a reference list points at, and
its identifiers are the whole of what the corpus holds for it — so a stub row
leads with its identifier where a title would go, at the same weight, and is
badged `stub`. It is never a blank row and never a "not found": the identifier
is the answer. A list that is mostly stubs, as *Cited by the same works as*
usually is, is therefore a list of identifiers, which is what it should look
like.

## Layout

On a wide screen (Tailwind's `xl`, 1280px and up), opening a work does not
leave the search. The search panel moves to the left edge and the page becomes
three columns: the results, the work, and its lists. The results scroll with
the page as they did before, and the work and its lists stay in view beside
them, each scrolling on its own. Clicking another result, or a row in one of
the lists, changes the middle and right columns and leaves the results where
they are. Searching again from the left changes the results and leaves the
work open.

Below that width the work is a page of its own, as before: the work, then its
lists, and "← Search" to go back.

Pointing at a row (or focusing or touching it) fetches its work, so a click
usually opens on the work itself rather than on a loading placeholder. Both
go through `workQuery` in `src/queries.ts`, so TanStack Query hands the page
what the row fetched, and never asks for a work it already holds or is
already fetching.

Opening a work from the centred search animates, and so does going back to
it: the search panel slides to the left edge, and once it is mostly out of the
way the work and its lists come in from the right. On Back they leave first,
and then the search slides back to the middle. This is the browser's View
Transitions API, started by the router (`defaultViewTransition` in
`src/router.ts`, which names the two transitions) and drawn by the rules at
the end of `src/index.css`. Nothing else animates — not work to work, not a
tab, not a search. There is no animation below `xl`, for anyone who has asked
for reduced motion, or in a browser without view-transition types; the layout
just changes.

## URLs

Routing is [TanStack Router](https://tanstack.com/router), code-based: routes
declared beside the components they render, composed in `src/router.ts`.
Search and work share a layout route, `browse`, which owns the search panel
and `q` — that is what keeps the panel mounted when a work opens beside it.
No file-based codegen — four screens do not earn a generated route tree.

| route            | URL                                          | notes                                       |
| ---------------- | -------------------------------------------- | ------------------------------------------- |
| `/`              | `/?q=protein+folding`                        | `q` is a validated search param, not state  |
| `/work/$workId`  | `/work/ol:3FWKF485MRW1QF?q=protein+folding`  | `workId` is a public id; `q` is the search beside it, if any |
| `/work/$workId`  | `/work/ol:3FWKF485MRW1QF?list=cited-alongside` | which list is open: `references` (the default, which normalises away), `cites-same`, `citers`, `cited-alongside` |
| `/resolve`       | `/resolve?text=pmid:10410805`                | one outcome redirects to the work           |
| `/chat`          | `/chat`                                      | a shell; the conversation is not in the URL |

A link from one work to another keeps `q` and `list`, so the results stay
beside it and the same list opens on it.

Every screen is therefore a link you can paste to someone else, and the browser's
Back button returns you to the search you ran rather than an empty box. A public
id never contains a slash, which is why it can be a path segment where a name
like `doi:10.1000/xyz` could not be — names are resolved on `/resolve` instead.

Routing is all the router does. Fetching stays with TanStack Query inside the
components, with no route loaders, so there is exactly one path to the API.

These are real paths, not hashes, so whatever serves the built `dist/` must fall
back to `index.html` for unknown paths. `pnpm dev` and `pnpm preview` both do.

## Nice to have

- **Counts on the related tabs.** *Cites* and *Cited by* show how many works
  they hold because the work record carries both numbers. *Cites the same
  sources as* and *Cited by the same works as* do not, because nothing holds
  theirs: it is `total` from `/v1/related`, which is the walk itself (up to
  250,000 arcs), so asking for the count costs as much as asking for the list.
  The fix belongs on the server — the two counts in the search index, or an
  O(1) lookup by id, the way `references` and `citers` are on the work — and
  then these tabs show them as the other two do. A stacked tab has room for
  its count; side by side at the narrowest the pairs allow (a 28rem header),
  "Cited by the same works as (30,531)" would not fit, so the pairs would need
  a wider header before they sit side by side, or the count would go on the
  line under the tabs.
- **Free PDFs, and counts on search results.** A PMC link is the one free
  full text the page can offer today; the work's `open_access` says a free
  copy exists but not where. OpenAlex records do say where — Unpaywall's best
  open URL, and each copy's PDF and licence — and the OpenAlex reader keeps
  them, but the merge carries on only the yes or no. Carried through to the
  work record, they are a "free PDF" link on the work's page. The reference
  and citer counts are left off search results until they are shown to be
  cheap on the full collection.

## Install

Requires `pnpm` and Node (both already on the machine; this project does not
install them).

```sh
cd web
pnpm install
```

## Run

```sh
pnpm dev        # dev server on http://localhost:5173
pnpm build      # typecheck + production build into dist/
pnpm typecheck  # types only
pnpm preview    # serve the built output
```

**Do not change the dev server port.** The API allows exactly two origins,
`http://localhost:5173` and `http://127.0.0.1:5173` — Vite's default. Use
whichever of the two the dev server prints; to a browser they are different
origins, and the wrong one is a CORS failure, not a 404. If the port has to
move, the server takes a repeatable `--allow-origin`.

## Pointing it at the API

One environment variable:

| var            | meaning                         | default                          |
| -------------- | ------------------------------- | -------------------------------- |
| `VITE_API_URL` | base URL of the ol-rht HTTP API | `pnpm dev`: `http://127.0.0.1:50052`; a built bundle: the page's own origin |

That is the JSON API's own port, not the gRPC one. A built bundle defaults to
its own origin because it is served beside the API: the public route sends
`/v1` to `ol-rht-serve` and everything else to these files, so the image
carries no hostname.

Copy `.env.example` to `.env.local` and edit it to change the port. The URL the
app is actually talking to is printed in the page footer, so it is never a
guess.

If the API is not running, the UI says so plainly ("API unreachable") rather
than showing an empty result. There is no mock data and no fallback anywhere.

## The API contract lives in one file

`src/api.ts` is the one place a screen calls the API from, and the one place
its types come from. The types themselves are no longer written there: since
2026-09-19 **both halves of every call** are generated from the Rust that
serves them, into `src/generated/serving.ts`, and `api.ts` re-exports them. The
server writes that file (`crates/ol-rht-serve/src/ts.rs`); `just ts-types`
regenerates it and `just check` fails while it is stale. Do not edit it by hand
— a change there is a change to `crates/ol-rht-serve/src/http.rs` or to
`crates/ol-rht-serve/proto/olrht/v1/serving.proto`.

Response bodies are generated from the Rust and not from the `.proto` because
the bytes are serde's rendering of the prost types and not proto3 JSON: field
names are **snake_case**, not `publicId`, and a `uint64` is a JSON number and
not a string. A generator reading the protocol would be wrong on both.

Requests are generated from the Rust for a different reason: they are not
protocol messages at all. A route declares the parameters it takes as a struct
and reads the query string into it, so the struct is at once what the server
accepts, what it refuses, and what the generated `Routes` map says may be sent.
Two of their names differ from the protocol's on purpose — `page_size` for its
`page`, `facet_values` for its `facets` — and the struct is where the
corrected spelling lives.

`Routes` maps each path to its request and response, and the one `get` in
`api.ts` is indexed by it:

```ts
function get<P extends keyof Routes>(
  path: P,
  params: Routes[P]["params"],
): Promise<Routes[P]["response"]>
```

So a parameter this API does not read, a required one left out, a count sent as
a string, a `by` that is not one of the two, or a path that is no route are each
a **type error on the line the call is written on** — the same guarantee a route
path already had against the router, now on the other end of the wire. On the
morning of 2026-09-19 this client sent `page` where the route reads `page_size`
and could only find out at runtime; that is what this is for.

The refusal body is the one shape still written by hand, because the HTTP
surface builds it and the protocol never mentions it.

```
GET /v1/epoch                                         -> { answered, merged, nodes, works, arcs }
GET /v1/resolve    ?text=                             -> { answered, outcome }
GET /v1/work       ?work=                             -> { answered, work }
GET /v1/references ?work=&page_size=&cursor=&hydrate= -> { answered, arcs, cursor, total }
GET /v1/citers     ?work=&page_size=&cursor=&hydrate= -> { answered, arcs, cursor, total }
GET /v1/search     ?query=&limit=&facet_values=&hydrate= -> { answered, total, hits, facets }
GET /v1/related    ?work=&by=&limit=&hydrate=         -> { answered, related, seeds_used, total, seeds_total }
```

Read from `Routes` in `src/generated/serving.ts`, which is the authority; the
table above is the same nine lines in prose.

`by` is `cites-same` or `cited-alongside` — the same two strings the URL's
`list` carries for those two tabs, so a tab is a link.

`/v1/expand` and `/v1/path` also exist and are in `Routes`; no screen here uses
them, so there is no method for them on `api`. In particular, related works are **one** call to `/v1/related`
and never two calls to `/v1/expand` stitched together here: expand seeds from
whatever the caller passes it, so a client that seeds from the page of citers
it happens to hold gets an answer no other client would get. The seeding is the
server's to do.

Every row of every list — search hits, references, citers, related works — is
an id, the fields its own context adds, and one **`summary`**:

```
{ public_id, stub, title, names[], authors[], venue, published,
  retracted_by[], corrected_by[], concern_by[] }
```

plus `score` and the index's own `title` on a hit, `by` on an arc, and
`touches`, `degree`, `score` on a related row. One shape, so `WorkRow` renders
a row from any list without knowing which. The three `_by` lists are the
sources that say so, as on the work; a row badges the first and the last, as
the work's page does.

A server from before search took `hydrate` refuses it as an unknown parameter,
so the server goes out before a client that sends it. An older client against
a newer server is fine: it sends no `hydrate`, and its hits come back as they
did, with a `summary: null` it never reads.

`summary` is **`null` unless the request asks to `hydrate`**, and null for an
id nothing behind it has a row — `null` on the wire and not a missing key, so
the generated type is `Summary | null` and never `summary?`. It is read through
`summaryOf()`, which falls back to the public id. The id alone is still a row
and still a link.

Things the contract will punish you for getting wrong:

- **Everything is a query parameter.** A work is never a path segment, because
  a name like `doi:10.1000/xyz` contains a slash.
- **An unknown query parameter is a 400**, as is a doubled one, a non-numeric
  count, or a `hydrate` that is not literally `true` / `false` — `1`, `yes`,
  `TRUE` and an empty value are all refused. Send only the parameters a route
  reads, which since 2026-09-19 is what the types already say.
- **Caps are refused, not clamped**: `page_size` <= 100 (50 by default),
  `limit` <= 100 (10), `facet_values` <= 20 (5). Related works ask for 25, and
  100 on "Show more".
- **`seeds_used` against `seeds_total`.** A work with 882 citers may be seeded
  with fewer of them: `seeds_total` is what it had, `seeds_used` is what was
  walked, and the list says both — "measured from 200 of the 882 works
  citing it" — rather than implying it looked at all of them. The pair comes
  from the
  answer itself, not from the work's own counts, so the fraction is the one
  the server measured.
- **`page` is the page size, not the page number.** One request is one page;
  the response's `cursor` asks for the next, and is `""` on the last.
- **Read `code`, not the HTTP status.** An error body is
  `{ code, message }` and never carries `answered`, so it cannot be mistaken
  for an answer. The UI shows `message` verbatim, because it says why.
- **A cursor from an older epoch is refused** with `failed_precondition`. That
  is not retryable: the list restarts from the beginning, which is what the
  references/citers lists do on their own.
- **`abstract` is `""` both when absent and when withheld** by its source's
  terms. The UI therefore never claims an abstract does not exist.

Every 200 carries `answered` — epoch, batch, `at`, sources and attribution.
The footer shows the attribution lines and `at`: the sources require both
attribution and a statement of how current the data is.

## The pnpm release-age setting, and why

`pnpm-workspace.yaml` sets:

```yaml
minimumReleaseAge: 20160
```

That is **14 days, expressed in minutes** — pnpm will not resolve a package
version that was published more recently than that. Freshly published npm
versions are the usual vector for a compromised package: an attacker who takes
over a maintainer account publishes a malicious patch release, and installs
that run `latest` in the next few hours pick it up. Waiting two weeks means
someone else finds it first.

The cost is that you are always slightly behind. That is the intended trade.

It bites in practice: at the time of install, `vite` 8.3.0 was 8 days old, so
pnpm resolved `vite` 8.2.2 (30 days old) instead.

Related settings, if they are ever needed:

- `minimumReleaseAgeExclude` — per-package exemptions.
- `minimumReleaseAgeStrict` — prompt for young picks instead of silently
  choosing an older mature version.

The same policy applies to the `shadcn` CLI, which is pinned to a version at
least two weeks old rather than run as `@latest`.

## shadcn components

Components are installed with the shadcn CLI, never hand-written. To add one:

```sh
pnpm dlx shadcn@4.21.0 add <component>
```

Keep the version pinned to something at least two weeks old. Currently
installed: `button`, `input`, `card`, `badge`, `skeleton`, `alert`,
`input-group`, `empty`, `textarea`, which `input-group` imports,
`dropdown-menu` and `button-group`, the Read button's, and `separator`,
which `button-group` imports.

The list tabs beside a work are not a `tabs` component. Which tab is open lives in
the URL, so each tab is a `Link` — a real one, that can be middle-clicked,
copied and pasted to someone else — styled as a segmented control. A component
holding that in its own state would have to be kept in step with the URL, and
would not be a link.

`components.json` records the choices made at init: the Radix UI primitives,
the `radix-nova` style, neutral base color, and the `@/` import alias.

Dark mode follows the system setting, in CSS alone. Init puts the dark
palette under a `.dark` class that something would have to set; here it is
under `@media (prefers-color-scheme: dark)` instead, and Tailwind's `dark:`
means that same query (the `@custom-variant` line in `src/index.css`). A
component added later needs nothing: its `dark:` classes go through the same
variant. A toggle, if one is ever wanted, means putting the class back.
