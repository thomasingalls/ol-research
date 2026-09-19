import { useEffect } from "react"
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query"
import { Link, createRoute } from "@tanstack/react-router"
import {
  api,
  ApiError,
  nameText,
  primaryName,
  summaryOf,
  year,
  type RelatedBy,
} from "@/api"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Names, WorkList } from "@/components/work-list"
import {
  RELATED_DEFAULT,
  RELATED_TABS,
  RelatedWorks,
} from "@/components/related-works"
import { Empty, ErrorBox, LoadingRows } from "@/components/states"
import { rootRoute } from "@/routes/root"

/**
 * `/work/$workId`, where `workId` is a public id such as `ol:71D1MFYWNG8KEK`.
 * A public id never contains a slash, so it is safe as a path segment — which
 * a name like `doi:10.1000/xyz` would not be. Names are resolved on /resolve.
 */
export const workRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/work/$workId",
  /**
   * Which related-works question is open lives in the URL, like the search
   * query does, so the page you are looking at is the page you can send to
   * someone. The default one normalises to no parameter at all, and anything
   * that is not one of the two is dropped rather than passed to the API.
   */
  validateSearch: (search: Record<string, unknown>): { related?: RelatedBy } => {
    const related = RELATED_TABS.find((t) => t.by === search.related)?.by
    return related && related !== RELATED_DEFAULT ? { related } : {}
  },
  component: WorkDetail,
})

function WorkDetail() {
  const { workId: id } = workRoute.useParams()
  const { related = RELATED_DEFAULT } = workRoute.useSearch()
  const work = useQuery({ queryKey: ["work", id], queryFn: () => api.work(id) })
  const w = work.data?.work ?? null

  // With no title — a stub always, anything else only when its record has
  // none — the identifier stands where the title would. It is what we hold.
  const lead = w && !w.title ? primaryName(w.names) : null

  return (
    <div className="space-y-6">
      <Button asChild variant="ghost" size="sm">
        <Link to="/">← Search</Link>
      </Button>

      {work.isPending && <LoadingRows rows={2} />}
      {work.isError && <ErrorBox error={work.error} />}

      {work.isSuccess && w === null && (
        <Empty>The server returned no work for this id.</Empty>
      )}

      {w !== null && (
        <Card>
          <CardHeader>
            <CardTitle className="flex flex-wrap items-start gap-2">
              {w.title ? (
                w.title
              ) : lead ? (
                <span className="font-mono text-base break-all">
                  {nameText(lead)}
                </span>
              ) : (
                <span className="text-muted-foreground italic">no title</span>
              )}
              {w.stub && <Badge variant="outline">stub</Badge>}
              {w.retracted_by.length > 0 && (
                <Badge variant="destructive">retracted</Badge>
              )}
              {w.concern_by.length > 0 && (
                <Badge variant="destructive">expression of concern</Badge>
              )}
            </CardTitle>
            <CardDescription>
              {/* A stub has no record behind it, so it has no year and no
                  venue to be missing. Say what it is, not what it lacks. */}
              {w.stub
                ? "Known only from a citation. Its identifiers are what we hold — there is no record to show."
                : [year(w.published), w.venue, w.genre]
                    .filter(Boolean)
                    .join(" · ") || "no year or venue"}
            </CardDescription>
          </CardHeader>

          <CardContent className="space-y-4">
            {w.authors.length > 0 && (
              <p className="text-sm">{w.authors.join(", ")}</p>
            )}

            {/* An empty abstract may be absent OR withheld by its source's
                terms. We cannot tell the two apart, so we claim neither. */}
            {w.abstract && (
              <p className="text-muted-foreground text-sm">{w.abstract}</p>
            )}

            <div className="space-y-1">
              <p className="text-muted-foreground text-xs">names</p>
              {w.names.length > 0 ? (
                <Names names={w.names} />
              ) : (
                <Empty>no names</Empty>
              )}
            </div>

            {w.held_by.length > 0 && (
              <p className="text-muted-foreground text-xs">
                held by {w.held_by.join(", ")}
              </p>
            )}

            <p className="text-muted-foreground font-mono text-xs">
              {w.public_id}
            </p>
          </CardContent>
        </Card>
      )}

      <RelatedWorks id={id} by={related} />

      <div className="grid gap-6 md:grid-cols-2">
        <Edges kind="references" id={id} />
        <Edges kind="citers" id={id} />
      </div>
    </div>
  )
}

const HEADINGS = {
  references: { heading: "References", note: "works this one cites" },
  citers: { heading: "Citers", note: "works citing this one" },
} as const

function Edges({ kind, id }: { kind: "references" | "citers"; id: string }) {
  const queryClient = useQueryClient()
  const { heading, note } = HEADINGS[kind]

  const query = useInfiniteQuery({
    queryKey: [kind, id],
    queryFn: ({ pageParam }) => api[kind](id, pageParam as string),
    initialPageParam: "",
    getNextPageParam: (last) => last.cursor || undefined,
  })

  const pages = query.data?.pages.length ?? 0
  const stale = query.error instanceof ApiError && query.error.staleCursor

  // The epoch moved under the list, so the cursor we hold is from an older
  // one. Start the list again instead of retrying a cursor that can never
  // work. Guarded on pages > 0 so a first-page failure cannot loop.
  useEffect(() => {
    if (stale && pages > 0) {
      void queryClient.resetQueries({ queryKey: [kind, id] })
    }
  }, [stale, pages, queryClient, kind, id])

  const arcs = query.data?.pages.flatMap((page) => page.arcs) ?? []
  const total = query.data?.pages[0]?.total ?? 0

  return (
    <section className="space-y-3">
      <div>
        <h2 className="font-medium">
          {heading}
          {query.isSuccess && (
            <span className="text-muted-foreground ml-2 text-sm font-normal">
              {total}
            </span>
          )}
        </h2>
        <p className="text-muted-foreground text-xs">{note}</p>
      </div>

      {query.isPending && <LoadingRows rows={3} />}
      {query.isError && <ErrorBox error={query.error} />}

      {arcs.length > 0 && (
        <>
          {/* One summary shape, one row component, whichever list it is in;
              the arc adds only who asserted it. */}
          <WorkList
            rows={arcs.map((arc) => ({ ...summaryOf(arc), by: arc.by }))}
          />
          {query.hasNextPage && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => void query.fetchNextPage()}
              disabled={query.isFetchingNextPage}
            >
              {query.isFetchingNextPage
                ? "Loading…"
                : `Load more (${arcs.length} of ${total})`}
            </Button>
          )}
        </>
      )}

      {query.isSuccess && arcs.length === 0 && <Empty>None.</Empty>}
    </section>
  )
}
