import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import {
  api,
  ApiError,
  RELATED,
  RELATED_MORE,
  summaryOf,
  type Neighbour,
  type PublicId,
  type RelatedBy,
} from "@/api"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { WorkRow } from "@/components/work-list"
import { ErrorBox, LoadingRows, NoWorks } from "@/components/states"

/**
 * One of the two questions `/v1/related` answers, as a tab's contents: what
 * came back, what it was measured from, and each row's overlap in words. The
 * tabs, and the note saying which question this is, are `CitationLists`'.
 */
export function RelatedWorks({ id, by }: { id: PublicId; by: RelatedBy }) {
  const [limit, setLimit] = useState<number>(RELATED)

  const query = useQuery({
    queryKey: ["related", id, by, limit],
    queryFn: () => api.related(id, by, limit),
    // Keep the rows up while "Show more" refetches, but not across a tab
    // change: the other question's rows under this question's heading would
    // be a different answer wearing the wrong label.
    placeholderData: (prev, previous) =>
      previous?.queryKey[2] === by ? prev : undefined,
  })

  const data = query.data ?? null
  const rows = data?.related ?? []

  return (
    <div className="space-y-3">
      {query.isPending && <LoadingRows rows={3} />}
      {query.isError && <RelatedError error={query.error} />}

      {data !== null && (
        <>
          <Caption by={by} data={data} shown={rows.length} />

          {rows.length === 0 ? (
            <NoWorks>{nothing(by, data.seeds_used)}</NoWorks>
          ) : (
            <ul className="divide-border divide-y">
              {rows.map((row) => (
                <li key={row.public_id}>
                  {/* One summary shape, one row component, whichever list. */}
                  <WorkRow row={summaryOf(row)}>
                    {/* The interpretable number, in words. The cosine beside
                        it only says why this row is where it is. */}
                    <p className="mt-1 text-sm">
                      {overlap(by, row)}
                      <span
                        className="text-muted-foreground ml-2 font-mono text-xs"
                        title="Salton's cosine: touches / √(seeds × degree). It sets the order. The count to its left is what it means."
                      >
                        {row.score.toFixed(2)}
                      </span>
                    </p>
                  </WorkRow>
                </li>
              ))}
            </ul>
          )}

          {data.total > rows.length && limit < RELATED_MORE && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setLimit(RELATED_MORE)}
              disabled={query.isFetching}
            >
              {query.isFetching ? "Loading…" : `Show more (${rows.length} of ${data.total})`}
            </Button>
          )}
        </>
      )}
    </div>
  )
}

/**
 * What the list is and is not: how much of it came back, what it was measured
 * from, and what put it in this order. `seeds_used` is the honest part — a
 * work with 882 citers may be seeded with 200 of them, and then this list is
 * the best answer from those 200 rather than the whole truth.
 */
function Caption({
  by,
  data,
  shown,
}: {
  by: RelatedBy
  data: { total: number; seeds_used: number; seeds_total: number }
  shown: number
}) {
  // The server's own two numbers, so the fraction is the one it measured and
  // not the work's arc count, which counts a slightly different thing.
  const from = seededFrom(by, data.seeds_used, data.seeds_total)

  const parts = [
    data.total > shown
      ? `${shown.toLocaleString()} of ${data.total.toLocaleString()}`
      : shown > 0
        ? `${shown.toLocaleString()} ${shown === 1 ? "work" : "works"}`
        : null,
    from,
    shown > 0 ? "ordered by overlap" : null,
  ].filter(Boolean)

  if (parts.length === 0) return null
  return (
    <p className="text-muted-foreground px-2 text-xs">{parts.join(" · ")}</p>
  )
}

/**
 * What the list was measured from. Three cases, and only one of them may say
 * "all": we know the work's own count, and every one of them was a seed.
 * Without that count we say what we were told and claim nothing more.
 */
function seededFrom(by: RelatedBy, seeds: number, held: number): string | null {
  if (seeds === 0) return null
  const n = seeds.toLocaleString()
  const capped = held > seeds
  if (by === "cites-same") {
    return capped
      ? `measured from ${n} of the ${held.toLocaleString()} works it cites`
      : `measured from all ${n} of the works it cites`
  }
  return capped
    ? `measured from ${n} of the ${held.toLocaleString()} works citing it`
    : `measured from all ${n} of the works citing it`
}

/**
 * The overlap in words. `touches` is what a reader can act on: the cosine
 * orders the list but does not tell anyone what it found.
 */
function overlap(by: RelatedBy, row: Neighbour): string {
  const n = row.touches
  // `degree` is what `touches` is out of. If it cannot be (a count from the
  // other side of the graph, and this row is what it is), say only the part
  // that stands on its own rather than print "7 of 3".
  if (row.degree < n || row.degree === 0) {
    return by === "cited-alongside"
      ? `cited by ${n} of the same ${n === 1 ? "work" : "works"} as this one`
      : `cites ${n} of the same ${n === 1 ? "source" : "sources"} as this one`
  }
  const degree = row.degree
  if (by === "cited-alongside") {
    if (degree === 1) return "the one work citing it also cites this one"
    if (n === degree)
      return `all ${n} of the works citing it also cite this one`
    return `${n} of the ${degree.toLocaleString()} works citing it also ${n === 1 ? "cites" : "cite"} this one`
  }
  if (degree === 1) return "the one work it cites is also cited by this one"
  if (n === degree)
    return `all ${n} of the works it cites are also cited by this one`
  return `${n} of the ${degree.toLocaleString()} works it cites ${n === 1 ? "is" : "are"} also cited by this one`
}

/** An empty list is an answer; say which answer it is. */
function nothing(by: RelatedBy, seeds: number): string {
  if (seeds === 0) {
    return by === "cites-same"
      ? "This work cites nothing we hold, so there is nothing to compare it by."
      : "Nothing we hold cites this work, so there is nothing to compare it by."
  }
  return by === "cites-same"
    ? "Nothing else we hold cites any of the same sources."
    : "Nothing else we hold is cited by any of the same works."
}

/**
 * A server that does not serve /v1/related is not a failed request: it is an
 * older server, and saying so is more use than its 404. Everything else is
 * shown as it came.
 */
function RelatedError({ error }: { error: unknown }) {
  const missing =
    error instanceof ApiError &&
    error.code === "not_found" &&
    error.message.includes("no such route")

  if (!missing) return <ErrorBox error={error} />

  return (
    <Alert>
      <AlertTitle>This server does not serve /v1/related</AlertTitle>
      <AlertDescription className="space-y-1">
        <span className="block">
          Related works are one call to that route. The API answering here does
          not have it, so there is nothing to show — not an empty result.
        </span>
        <span className="block font-mono text-xs break-all">
          {(error as ApiError).message}
        </span>
      </AlertDescription>
    </Alert>
  )
}
