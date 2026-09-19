/**
 * The whole contract with the Rust API lives in this file.
 *
 * It mirrors crates/ol-rht-serve/proto/olrht/v1/serving.proto, which the
 * HTTP surface serves as JSON (same messages, same shape, snake_case field
 * names — the server's build.rs renames nothing). If the protocol moves,
 * this file moves with it and nothing else has to.
 *
 * Vocabulary, the same as the pipeline's:
 *   work   an entity — one thing that was published, carrying a public id
 *   name   an identifier that points at a work, as `namespace:value`
 *          (doi:…, pmid:…, pmc:PMC…, openalex:W…, and `reg1038:…` and the
 *          rest of the `reg<digits>:…` ids a publisher gives its own works)
 *   stub   a work we know only because something cited it: it has names but
 *          no record behind it, so there is no title or venue to show
 */

export const API_BASE: string =
  import.meta.env.VITE_API_URL ?? "http://127.0.0.1:50052"

/** A public id, e.g. "ol:…". Opaque to the UI — never parse it. */
export type PublicId = string

/** On every response: which epoch answered, and the terms it answered under. */
export interface Answered {
  epoch: number
  batch: number
  /** The batch's timestamp: how current this answer is. */
  at: string
  sources: string[]
  /** One attribution line per source. Sources require these be shown. */
  attribution: string[]
}

export interface EpochInfo {
  answered: Answered | null
  merged: string
  nodes: number
  works: number
  arcs: number
}

export interface Name {
  namespace: string
  value: string
  /** promoted | alias | shared */
  standing: string
  by: string[]
  native: boolean
}

export interface SourceRecord {
  source: string
  native: string
  promoted: boolean
  revised: string
}

/** Zero in any position means "not given". */
export interface PubDate {
  year: number
  month: number
  day: number
}

export interface Disputed {
  field: string
  promoted_from: string
  others: string[]
}

export interface Work {
  public_id: PublicId
  stub: boolean
  names: Name[]
  records: SourceRecord[]
  held_by: string[]
  title: string
  authors: string[]
  venue: string
  published: PubDate | null
  language: string
  form: string
  genre: string
  retracted_by: string[]
  corrected_by: string[]
  concern_by: string[]
  open_access: boolean | null
  /** Empty when absent, or withheld by the terms of its source. */
  abstract: string
  disputed: Disputed[]
  references: number
  citers: number
}

export interface GetWorkResponse {
  answered: Answered | null
  work: Work | null
}

/**
 * Resolving a name has three outcomes, not two: it names one work, or it is
 * a name shared by several, or it is an id a split retired.
 */
export type ResolveOutcome =
  | { public_id: PublicId }
  | { shared: { public_ids: PublicId[] } }
  | { split: { public_id: PublicId } }

export interface ResolveResponse {
  answered: Answered | null
  /** null when the text names nothing. */
  outcome: ResolveOutcome | null
}

/** A search hit carries no year or venue — only the work itself has those. */
export interface Hit {
  public_id: PublicId
  score: number
  title: string
}

export interface FacetValue {
  value: string
  count: number
}

export interface Facet {
  root: string
  values: FacetValue[]
}

export interface SearchResponse {
  answered: Answered | null
  total: number
  hits: Hit[]
  facets: Facet[]
}

/**
 * The one shape every hydrated row comes back in: a row of references, of
 * citers, and of related works are all this, plus whatever that context adds.
 *
 * A stub carries names and nothing else — no title, no authors, no venue. That
 * is not missing data; it is the whole of what the corpus holds for it, and it
 * is 98% of what a reference list points at. The UI renders a stub as its
 * identifiers, which are a real answer, never as a blank row.
 *
 * The optional fields are the ones a server older than this contract does not
 * send yet, so read them defensively rather than assuming they arrived.
 */
export interface Summary {
  public_id: PublicId
  /** Known only from a citation: names, no record behind them. */
  stub?: boolean
  /** Empty on a stub, and on anything the server did not hydrate. */
  title: string
  names?: Name[]
  authors?: string[]
  venue?: string
  published?: PubDate | null
}

/**
 * What every row of every list arrives in: an id, the context's own fields,
 * and the summary — which is absent when the request did not hydrate, or when
 * nothing behind the id has a row. Read it with `summaryOf`, never directly.
 */
interface RowEnvelope {
  public_id: PublicId
  summary?: Summary | null
  /**
   * A server from before the summary was one message put these on the row
   * itself; the protocol has since reserved both field numbers. Read through
   * `summaryOf`, which prefers `summary` and falls back to these, so a screen
   * does not empty out against a server that has not been rebuilt yet.
   */
  title?: string
  names?: Name[]
}

/** One arc of a citation list: a summary, plus who asserted the arc. */
export interface Arc extends RowEnvelope {
  /** Who asserted the arc. */
  by: string[]
}

/**
 * The summary a row renders. Falls back to the id alone, which is still a row
 * and still a link — never an empty line.
 */
export function summaryOf(row: RowEnvelope): Summary {
  if (row.summary) return row.summary
  // `stub` is left unset rather than guessed: an unhydrated row does not say
  // whether there is a record behind it, and we will not say so for it.
  return { public_id: row.public_id, title: row.title ?? "", names: row.names ?? [] }
}

export interface WorkPage {
  answered: Answered | null
  arcs: Arc[]
  /** Empty on the last page. */
  cursor: string
  total: number
}

/**
 * Which question a related list answers. One hop through the graph, but two
 * different hops:
 *
 *   cites-same        seeded with this work's references, walking their citers
 *                     — papers that cite the same sources as this one.
 *   cited-alongside   seeded with this work's citers, walking their references
 *                     — papers cited alongside this one.
 *
 * The same two strings go in the URL and on the wire, so a tab is a link.
 */
export type RelatedBy = "cites-same" | "cited-alongside"

/** A related row: a summary, plus how it overlaps and how it was ordered. */
export interface Neighbour extends RowEnvelope {
  /** The raw overlap: how many seeds reach it. The interpretable number. */
  touches: number
  /**
   * What `touches` is out of — the neighbour's own count in the orientation
   * opposite the one walked: its citers for `cited-alongside`, the works it
   * cites for `cites-same`. "7 of its 9 citers also cite this one."
   */
  degree: number
  /** Salton's cosine, in 0..=1. It orders the list; it is not the answer. */
  score: number
}

export interface RelatedResponse {
  answered: Answered | null
  /** Best-scored first, and never the work itself. */
  related: Neighbour[]
  /** Seeds actually walked: the `|S|` every score divides by. */
  seeds_used: number
  /** Works the hop reached, against the `related` that `limit` left. */
  total: number
  /**
   * Seeds the work had before the cap. Equal to `seeds_used` unless its list
   * was longer than the server will walk — a work with 882 citers is not
   * necessarily seeded with 882 — and then this is what says the answer is
   * bounded. The UI shows both rather than implying the list is exhaustive.
   */
  seeds_total: number
}

/** The server's refusal body: `{ code, message }` alongside an HTTP status. */
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
 * Arcs per page — `page` is the page SIZE, not a page number.
 * The caps are refused, never clamped: page <= 100, limit <= 100.
 */
export const PAGE = 100

/** Related works asked for at first, and after "Show more". The cap is 1000. */
export const RELATED = 25
export const RELATED_MORE = 100

/** The server refuses any parameter a route does not read, so send only these. */
async function get<T>(path: string, params?: Record<string, string>): Promise<T> {
  const url = new URL(path, API_BASE)
  for (const [key, value] of Object.entries(params ?? {})) {
    url.searchParams.set(key, value)
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

  return (await response.json()) as T
}

export const api = {
  /** Which epoch and batch the server answers from, its counts, and the terms. */
  epoch: () => get<EpochInfo>("/v1/epoch"),

  /** Which work a name or public id is. */
  resolve: (text: string) => get<ResolveResponse>("/v1/resolve", { text }),

  /** Everything the merge knows about one work. */
  work: (work: string) => get<GetWorkResponse>("/v1/work", { work }),

  /** One page of what a work cites. */
  references: (work: string, cursor = "") =>
    get<WorkPage>("/v1/references", {
      work,
      page: String(PAGE),
      hydrate: "true",
      ...(cursor ? { cursor } : {}),
    }),

  /** One page of what cites a work. */
  citers: (work: string, cursor = "") =>
    get<WorkPage>("/v1/citers", {
      work,
      page: String(PAGE),
      hydrate: "true",
      ...(cursor ? { cursor } : {}),
    }),

  /** Works matching a query. Hits carry a title and a score, nothing more. */
  search: (query: string, limit = 50) =>
    get<SearchResponse>("/v1/search", { query, limit: String(limit) }),

  /**
   * Works one hop away through the graph, ordered by overlap. One call per
   * question — never two /v1/expand calls stitched together here, which would
   * seed from whatever page this client happened to have fetched and so give
   * a different answer to every caller.
   */
  related: (work: string, by: RelatedBy, limit = RELATED) =>
    get<RelatedResponse>("/v1/related", {
      work,
      by,
      limit: String(limit),
      // Without this the rows come back as bare ids: `hydrate` is off unless
      // it is asked for, in either literal `true` or `false` and nothing else.
      hydrate: "true",
    }),
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
