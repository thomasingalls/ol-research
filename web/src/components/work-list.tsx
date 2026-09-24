import { useRef } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { Link } from "@tanstack/react-router"
import { Badge } from "@/components/ui/badge"
import { nameText, primaryName, year, type Name, type Rendered } from "@/api"
import { useWide } from "@/lib/wide"
import { workQuery } from "@/queries"

/**
 * How long the pointer has to rest on a row before its work is fetched: the
 * router's own measure of intent. Long enough that sweeping down a list does
 * not ask for every row on the way past.
 */
const INTENT_MS = 50

/** Render the names that point at a work, e.g. doi:10.1234/x, pmid:12345678. */
export function Names({ names, omit }: { names: Name[]; omit?: Name | null }) {
  // The name already standing as the row's headline is not repeated here.
  const rest = omit ? names.filter((n) => nameText(n) !== nameText(omit)) : names
  if (rest.length === 0) return null
  return (
    <div className="flex flex-wrap gap-1">
      {rest.map((n) => (
        <Badge
          key={nameText(n)}
          variant="secondary"
          /* A DOI can run to 70 characters. Let the badge wrap and grow
             rather than run off the edge of a column. */
          className="h-auto font-mono text-xs break-all whitespace-normal"
          title={`${n.standing}${n.by.length ? ` · asserted by ${n.by.join(", ")}` : ""}`}
        >
          {nameText(n)}
        </Badge>
      ))}
    </div>
  )
}

/**
 * A row is a summary — or as much of one as the server sent — plus whatever
 * its list adds: the asserters of an arc.
 */
export type Row = Rendered & {
  /** Who asserted the arc, on a references or citers list. */
  by?: string[]
}

/**
 * `identifiers` off leaves out the public id and the names under a row, for a
 * list read by title, as search results are. A row with no title still leads
 * with an identifier, since that is all it has.
 */
export function WorkList({
  rows,
  identifiers = true,
}: {
  rows: Row[]
  identifiers?: boolean
}) {
  return (
    <ul className="divide-border divide-y">
      {rows.map((row) => (
        <li key={row.public_id}>
          <WorkRow row={row} identifiers={identifiers} />
        </li>
      ))}
    </ul>
  )
}

/**
 * One row, whatever list it is in. Everything it shows comes from the single
 * summary shape, so references, citers and related works render identically
 * and a caller only has to supply what its context adds, as `children`.
 *
 * The row never renders empty. A stub has no title because there is no record
 * behind it — that is what a stub is, and it is most of what any reference
 * list points at — so the row leads with the identifier we do hold. An
 * identifier is a real answer, and is rendered as one: same weight, same
 * place, no apology.
 */
export function WorkRow({
  row,
  identifiers = true,
  children,
}: {
  row: Row
  identifiers?: boolean
  children?: React.ReactNode
}) {
  const wide = useWide()
  const queryClient = useQueryClient()
  const intent = useRef<number | undefined>(undefined)
  const warm = () => void queryClient.prefetchQuery(workQuery(row.public_id))
  const names = row.names ?? []
  const lead = row.title ? null : primaryName(names)
  const authors = row.authors ?? []
  const byline = [
    authors.length > 2 ? `${authors[0]} et al.` : authors.join(", "),
    row.venue,
    year(row.published)?.toString(),
  ].filter(Boolean)
  // What the work's sources say of its standing, warned of as the work's own
  // page warns of it, so a reader knows before opening it.
  const warnings = [
    { label: "retracted", by: row.retracted_by ?? [] },
    { label: "expression of concern", by: row.concern_by ?? [] },
  ].filter((w) => w.by.length > 0)

  // A real link, not a button: middle-click, open-in-new-tab and copy-link
  // all work, which is the point of the work detail having a URL at all.
  //
  // It takes the search and the open tab with it, so the results stay beside
  // the next work and the same list opens on it. On a wide screen the page's
  // scroll is the results' scroll, so it is left where it is; on a narrow one
  // the work is a new page and starts at the top. The open work is the row
  // marked active, wherever it appears, whatever its query string.
  //
  // Pointing at a row, focusing it or touching it fetches its work, so the
  // page usually has it by the time the click lands and opens on the work
  // rather than on a placeholder.
  //
  // Hover darkens whatever the row sits on, the page or the canvas. The open
  // work's row is raised as a card is, so it stands out on either.
  return (
    <Link
      to="/work/$workId"
      params={{ workId: row.public_id }}
      search={(prev) => ({ q: prev.q, list: prev.list })}
      resetScroll={!wide}
      activeOptions={{ includeSearch: false }}
      onMouseEnter={() => {
        intent.current = window.setTimeout(warm, INTENT_MS)
      }}
      onMouseLeave={() => window.clearTimeout(intent.current)}
      onFocus={warm}
      onTouchStart={warm}
      className="hover:bg-foreground/4 data-[status=active]:bg-card data-[status=active]:ring-foreground/10 block w-full rounded-lg px-2 py-3 text-left data-[status=active]:shadow-sm data-[status=active]:ring-1"
    >
      <div className="flex items-start justify-between gap-2">
        {row.title ? (
          <span className="font-medium">{row.title}</span>
        ) : lead ? (
          <span className="font-mono text-sm font-medium break-all">
            {nameText(lead)}
          </span>
        ) : (
          /* Not hydrated and not named: the public id is all there is. */
          <span className="font-mono text-sm font-medium">{row.public_id}</span>
        )}

        {(row.stub || warnings.length > 0) && (
          <div className="flex shrink-0 flex-col items-end gap-1">
            {row.stub && (
              <Badge
                variant="outline"
                className="text-[10px]"
                title="A stub: known only from a citation. We hold its identifiers, not a record — so there is no title or venue to show."
              >
                stub
              </Badge>
            )}
            {warnings.map((w) => (
              <Badge
                key={w.label}
                variant="destructive"
                className="text-[10px]"
                title={`${w.label}, according to ${w.by.join(", ")}`}
              >
                {w.label}
              </Badge>
            ))}
          </div>
        )}
      </div>

      {byline.length > 0 && (
        <p className="text-muted-foreground mt-1 text-xs">
          {byline.join(" · ")}
        </p>
      )}

      {children}

      {(identifiers || (row.by && row.by.length > 0)) && (
        <div className="text-muted-foreground mt-1 flex flex-wrap gap-x-3 text-xs">
          {identifiers && <span className="font-mono">{row.public_id}</span>}
          {row.by && row.by.length > 0 && <span>asserted by {row.by.join(", ")}</span>}
        </div>
      )}

      {identifiers && names.length > 0 && (
        <div className="mt-2">
          <Names names={names} omit={lead} />
        </div>
      )}
    </Link>
  )
}
