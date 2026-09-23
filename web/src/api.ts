/**
 * The whole contract with the Rust API lives in this file: how a call is
 * made, and what comes back.
 *
 * What comes back is not written here any more. Every response body is
 * generated from the Rust types that serialize it — see
 * ./generated/serving.ts, and crates/ol-rht-serve/src/ts.rs, which writes
 * it. These shapes were held by hand until 2026-09-19, and a hand-written
 * contract drifts without saying so: a field added to the protocol is a
 * field this client silently never reads, and one renamed is an
 * `undefined` the UI renders as a blank line. Generated, a protocol that
 * moved is a diff, and `just check` fails until the two agree again.
 *
 * Requests are generated too, since 2026-09-19. A route's parameters are
 * the struct it reads a query string into, so `Routes` below maps each
 * path to what may be sent to it and what comes back, and `get` is
 * indexed by that: `page` where the route reads `page_size` is now a type
 * error where the call is written, which is where the mistake was. The
 * refusal body is the one thing still by hand — the HTTP surface builds
 * it and the protocol never mentions it.
 *
 * Vocabulary, the same as the pipeline's:
 *   work   an entity — one thing that was published, carrying a public id
 *   name   an identifier that points at a work, as `namespace:value`
 *          (doi:…, pmid:…, pmc:PMC…, openalex:W…, and `reg1038:…` and the
 *          rest of the `reg<digits>:…` ids a publisher gives its own works)
 *   stub   a work we know only because something cited it: it has names but
 *          no record behind it, so there is no title or venue to show
 */
import type {
  Name,
  PubDate,
  RelatedBy,
  Routes,
  Summary,
} from "@/generated/serving"

// The protocol's shapes, under the protocol's names. Re-exported from
// here so that the client is still the one place a screen imports from,
// and a screen never has to know which of these was generated.
export type {
  Answered,
  Arc,
  Disputed,
  EpochInfo,
  EpochParams,
  ExpandParams,
  ExpandResponse,
  Facet,
  FacetValue,
  GetWorkResponse,
  Hit,
  ListParams,
  Name,
  Neighbour,
  NoPath,
  Path,
  PathOutcome,
  PathParams,
  PathResponse,
  Paths,
  PathStopped,
  PubDate,
  RelatedBy,
  RelatedParams,
  RelatedResponse,
  ResolveOutcome,
  ResolveParams,
  ResolveResponse,
  Routes,
  SearchParams,
  SearchResponse,
  Shared,
  SourceRecord,
  Split,
  Summary,
  Walk,
  Work,
  WorkPage,
  WorkParams,
} from "@/generated/serving"

/**
 * Where the JSON facade is. In a built bundle the app and the API share an
 * origin — the public route sends `/v1` to the server and everything else
 * to these files — so the page's own origin is the default and the bundle
 * carries no hostname. `pnpm dev` is the exception: Vite serves the app and
 * the server is a separate process, which is what the dev origins in
 * `ol-rht-serve` are for. `VITE_API_URL` overrides either.
 */
export const API_BASE: string =
  import.meta.env.VITE_API_URL ??
  (import.meta.env.DEV ? "http://127.0.0.1:50052" : window.location.origin)

/** A public id, e.g. "ol:…". Opaque to the UI — never parse it. */
export type PublicId = string

/**
 * What every row of every list arrives in: an id, the context's own fields,
 * and the summary — which is null when the request did not hydrate, or when
 * nothing behind the id has a row. Read it with `summaryOf`, never directly.
 *
 * Hand-written, and wider than the protocol on purpose. `Arc` and
 * `Neighbour` are the generated shapes; this is what `summaryOf` will
 * accept, which includes a server from before the summary was one message,
 * when the two fields below sat on the row itself. The protocol has since
 * reserved both field numbers, so nothing generated mentions them — but a
 * screen reading a server that has not been rebuilt should show the row it
 * has rather than empty out, and that is a client's problem to hold.
 */
interface RowEnvelope {
  public_id: PublicId
  summary?: Summary | null
  title?: string
  names?: Name[]
}

/**
 * What a row renders from: a `Summary` when the server sent one, and as
 * much of one as we have when it did not.
 *
 * Not a `Summary`, which the protocol says is whole. The fields missing
 * here are the ones an unhydrated row does not carry, and they are absent
 * rather than filled in with an empty string — `stub` most of all: a row
 * nobody asked to hydrate does not say whether there is a record behind it,
 * and this will not say so on its behalf.
 */
export type Rendered = Partial<Summary> & { public_id: PublicId }

/**
 * The summary a row renders. Falls back to the id alone, which is still a
 * row and still a link — never an empty line.
 */
export function summaryOf(row: RowEnvelope): Rendered {
  if (row.summary) return row.summary
  return { public_id: row.public_id, title: row.title ?? "", names: row.names ?? [] }
}

/**
 * The server's refusal body: `{ code, message }` alongside an HTTP status.
 * Hand-written and staying that way — it is built by the HTTP surface
 * itself (`http::Refused`), not by the protocol, so there is no message to
 * generate it from.
 */
interface Refused {
  code: string
  message: string
}

/**
 * Thrown for every failed call. Read `code`, not `status`: the server's codes
 * are `invalid_argument`, `failed_precondition` (a shared name, a cursor from
 * an older epoch, or no epoch published), `not_found`, `internal`.
 * `status` is 0 when the server was never reached at all.
 */
export class ApiError extends Error {
  readonly status: number
  readonly code: string | null

  constructor(message: string, status: number, code: string | null = null) {
    super(message)
    this.name = "ApiError"
    this.status = status
    this.code = code
  }

  /** True when we never got a response at all — server down, wrong port, CORS. */
  get unreachable(): boolean {
    return this.status === 0
  }

  /**
   * The epoch moved under a list, so a cursor from the old one is no longer
   * good. The caller restarts the list rather than retrying the cursor.
   */
  get staleCursor(): boolean {
    return this.code === "failed_precondition"
  }
}

/**
 * Arcs per page — the server's `page_size`, which is a size and never a
 * page number: a list is walked by cursor, not by number.
 * The caps are refused, never clamped: page_size <= 100, limit <= 100.
 */
export const PAGE_SIZE = 100

/** Related works asked for at first, and after "Show more". The cap is 1000. */
export const RELATED = 25
export const RELATED_MORE = 100

/** A path this API serves. `Routes` is generated; see the note at the top. */
export type Route = keyof Routes

/**
 * Every call to the API goes through here. The path decides both halves of
 * it — `Routes[P]["params"]` is the struct the route reads its query string
 * into, `Routes[P]["response"]` the message it answers with — so a
 * parameter the route does not take, or a required one left out, is a type
 * error on the line the call is written on. That is the same guarantee a
 * route path already has against the router, now on the other end of the
 * wire.
 *
 * The server refuses any parameter a route does not read, so this sends what
 * it is handed and nothing else: a value left `undefined` is left out of the
 * URL rather than sent empty, and an array is written one `key=` per
 * element, which is how `/v1/expand` names its seeds. Numbers and booleans
 * go as the text they print as, which is the `true` / `false` the server
 * insists on for a flag.
 */
async function get<P extends Route>(
  path: P,
  params: Routes[P]["params"],
): Promise<Routes[P]["response"]> {
  const url = new URL(path, API_BASE)
  for (const [key, value] of Object.entries(params as Record<string, unknown>)) {
    if (value === undefined) continue
    for (const one of Array.isArray(value) ? value : [value]) {
      url.searchParams.append(key, String(one))
    }
  }

  let response: Response
  try {
    // No request headers: every route is a plain GET, which keeps the
    // browser from sending a preflight the server would rather not answer.
    response = await fetch(url)
  } catch {
    throw new ApiError(
      `Could not reach the ol-rht API at ${API_BASE}. Is it running, and is ` +
        `this origin (${window.location.origin}) one it allows?`,
      0,
    )
  }

  if (!response.ok) {
    const refused = (await response.json().catch(() => null)) as Refused | null
    throw new ApiError(
      refused?.message ?? `${response.status} ${response.statusText}`,
      response.status,
      refused?.code ?? null,
    )
  }

  return (await response.json()) as Routes[P]["response"]
}

export const api = {
  /** Which epoch and batch the server answers from, its counts, and the terms. */
  epoch: () => get("/v1/epoch", {}),

  /** Which work a name or public id is. */
  resolve: (text: string) => get("/v1/resolve", { text }),

  /** Everything the merge knows about one work. */
  work: (work: string) => get("/v1/work", { work }),

  /** One page of what a work cites. */
  references: (work: string, cursor = "") =>
    // The first page is asked for with no cursor at all: the server reads an
    // empty one the same way, but there is nothing to send, so nothing is.
    get("/v1/references", { work, page_size: PAGE_SIZE, cursor: cursor || undefined, hydrate: true }),

  /** One page of what cites a work. */
  citers: (work: string, cursor = "") =>
    get("/v1/citers", { work, page_size: PAGE_SIZE, cursor: cursor || undefined, hydrate: true }),

  /** Works matching a query. Hits carry a title and a score, nothing more. */
  search: (query: string, limit = 50) => get("/v1/search", { query, limit }),

  /**
   * Works one hop away through the graph, ordered by overlap. One call per
   * question — never two /v1/expand calls stitched together here, which would
   * seed from whatever page this client happened to have fetched and so give
   * a different answer to every caller.
   */
  related: (work: string, by: RelatedBy, limit = RELATED) =>
    // Without `hydrate` the rows come back as bare ids: it is off unless it
    // is asked for, in either literal `true` or `false` and nothing else.
    get("/v1/related", { work, by, limit, hydrate: true }),
}

/** The year, or null when nothing gives one. Zero in any position means absent. */
export function year(published: PubDate | null | undefined): number | null {
  return published && published.year > 0 ? published.year : null
}

/** A name as the merge writes it: `doi:10.1234/x`, `pmid:12345678`. */
export function nameText(name: Name): string {
  return `${name.namespace}:${name.value}`
}

/** How recognisable an identifier is, for choosing which one to lead with. */
const NAMESPACE_ORDER = ["doi", "pmid", "pmc", "openalex"]

/**
 * The name to put where a title would go, when there is no title. A DOI if
 * there is one, then a PMID, a PMC id, an OpenAlex id, and last a publisher's
 * own id (`reg1038:…`), which names the work to nobody outside that publisher.
 */
export function primaryName(names: Name[] | undefined): Name | null {
  if (!names || names.length === 0) return null
  const rank = (n: Name) => {
    const known = NAMESPACE_ORDER.indexOf(n.namespace)
    return (known === -1 ? NAMESPACE_ORDER.length : known) * 2 +
      (n.standing === "promoted" ? 0 : 1)
  }
  return names.reduce((best, n) => (rank(n) < rank(best) ? n : best))
}
