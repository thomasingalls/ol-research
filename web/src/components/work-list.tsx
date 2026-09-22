import { Link } from "@tanstack/react-router"
import { Badge } from "@/components/ui/badge"
import { nameText, primaryName, year, type Name, type Rendered } from "@/api"

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

export function WorkList({ rows }: { rows: Row[] }) {
  return (
    <ul className="divide-border divide-y">
      {rows.map((row) => (
        <li key={row.public_id}>
          <WorkRow row={row} />
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
  children,
}: {
  row: Row
  children?: React.ReactNode
}) {
  const names = row.names ?? []
  const lead = row.title ? null : primaryName(names)
  const authors = row.authors ?? []
  const byline = [
    authors.length > 2 ? `${authors[0]} et al.` : authors.join(", "),
    row.venue,
    year(row.published)?.toString(),
  ].filter(Boolean)

  // A real link, not a button: middle-click, open-in-new-tab and copy-link
  // all work, which is the point of the work detail having a URL at all.
  return (
    <Link
      to="/work/$workId"
      params={{ workId: row.public_id }}
      className="hover:bg-muted/50 block w-full rounded px-2 py-3 text-left"
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

        {row.stub && (
          <Badge
            variant="outline"
            className="shrink-0 text-[10px]"
            title="A stub: known only from a citation. We hold its identifiers, not a record — so there is no title or venue to show."
          >
            stub
          </Badge>
        )}
      </div>

      {byline.length > 0 && (
        <p className="text-muted-foreground mt-1 text-xs">
          {byline.join(" · ")}
        </p>
      )}

      {children}

      <div className="text-muted-foreground mt-1 flex flex-wrap gap-x-3 text-xs">
        <span className="font-mono">{row.public_id}</span>
        {row.by && row.by.length > 0 && <span>asserted by {row.by.join(", ")}</span>}
      </div>

      {names.length > 0 && (
        <div className="mt-2">
          <Names names={names} omit={lead} />
        </div>
      )}
    </Link>
  )
}
