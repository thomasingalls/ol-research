import { useEffect } from "react"
import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"
import { Link } from "@tanstack/react-router"
import { api, ApiError, summaryOf, type PublicId } from "@/api"
import { cn } from "@/lib/utils"
import { workQuery } from "@/queries"
import { Button } from "@/components/ui/button"
import { WorkList } from "@/components/work-list"
import { RelatedWorks } from "@/components/related-works"
import { ErrorBox, LoadingRows, NoWorks } from "@/components/states"

/**
 * A work's four lists, one tab each, in the order they are offered. They come
 * in pairs, and the tabs are laid out as two rows of two so the pairs show:
 * what this work cites, and what else cites the same sources; what cites this
 * work, and what else those citers cite alongside it. The first of each pair
 * is an edge list, paged by cursor; the second is one question asked of the
 * graph, `/v1/related`, and its `list` is that route's `by`.
 *
 * `list` is the URL's parameter, so a tab is a link.
 *
 * `from` is which of the work's own two counts the list is built from. A
 * pair is only as full as its first half: a work that cites nothing has no
 * sources for anything else to share, and a work nothing cites has no citers
 * to cite anything alongside it. So a zero there means the list is empty,
 * known before anyone asks for it.
 *
 * `counted` is whether that count is the list's own length, which it is for
 * the edge lists, so their tabs can say it. A related list's length is only
 * known once `/v1/related` has been asked, which a tab does not do — see
 * "Nice to have" in the README for where those counts should come from.
 */
export const LISTS = [
  {
    list: "references",
    tab: "References",
    note: "Works this one cites.",
    from: "references",
    counted: true,
  },
  {
    list: "cites-same",
    tab: "Cites the same sources",
    note: "Papers that cite the same sources as this one.",
    from: "references",
    counted: false,
  },
  {
    list: "citers",
    tab: "Citers",
    note: "Works citing this one.",
    from: "citers",
    counted: true,
  },
  {
    list: "cited-alongside",
    tab: "Cited alongside",
    note: "Papers cited alongside this one.",
    from: "citers",
    counted: false,
  },
] as const

export type List = (typeof LISTS)[number]["list"]

export const LIST_DEFAULT: List = "references"

export function CitationLists({ id, list }: { id: PublicId; list: List }) {
  const open = LISTS.find((l) => l.list === list) ?? LISTS[0]

  // The same work the page beside this is showing, so it is already cached
  // or already on its way: this asks the API for nothing.
  const work = useQuery(workQuery(id)).data?.work ?? null

  return (
    <section className="space-y-3">
      {/* The tabs, and a line saying what the open one lists. Beside a work
          they stay at the top of their column while the list under them
          scrolls, and the rule is the edge it scrolls under. Tab labels,
          the line and the rows' text all start on one edge. */}
      <header className="bg-background xl:bg-card space-y-2 border-b pb-3 xl:sticky xl:top-0 xl:z-10 xl:pt-3">
        <nav aria-label="Which list to show" className="grid grid-cols-2 gap-1">
          {LISTS.map((l) => {
            // Empty for this work: drawn as a disabled button is, so it
            // reads as nothing there, but it is still a link. The panel
            // behind it says why it is empty, which is worth a click.
            const empty = work !== null && work[l.from] === 0
            const count = l.counted && work !== null ? work[l.from] : null
            return (
              <Button
                key={l.list}
                asChild
                size="sm"
                variant={l.list === list ? "secondary" : "ghost"}
                className={cn("justify-start px-2", empty && "opacity-50")}
              >
                {/* A tab is a link, so the open one is in the URL and the
                    page can be pasted to someone else as it stands.
                    `replace` keeps Back meaning the work you arrived from,
                    not the other tab. The page stays where it is; only the
                    list starts again from its top. */}
                <Link
                  to="/work/$workId"
                  params={{ workId: id }}
                  search={(prev) => ({
                    ...prev,
                    list: l.list === LIST_DEFAULT ? undefined : l.list,
                  })}
                  replace
                  resetScroll={false}
                  activeOptions={{ exact: true }}
                  onClick={(event) =>
                    event.currentTarget
                      .closest('[data-scroll-restoration-id="lists"]')
                      ?.scrollTo({ top: 0 })
                  }
                >
                  {l.tab}
                  {count !== null && (
                    <span className="text-muted-foreground tabular-nums">
                      ({count.toLocaleString()})
                    </span>
                  )}
                  {/* A count says "(0)" out loud; a muted tab without one
                      needs saying for anyone who cannot see it muted. */}
                  {empty && count === null && (
                    <span className="sr-only"> (none)</span>
                  )}
                </Link>
              </Button>
            )
          })}
        </nav>
        <p className="text-muted-foreground px-2 text-xs">{open.note}</p>
      </header>

      {/* Keyed, so "Show more" on one tab is not carried to the next. */}
      {list === "references" || list === "citers" ? (
        <Edges key={list} kind={list} id={id} />
      ) : (
        <RelatedWorks key={list} id={id} by={list} />
      )}
    </section>
  )
}

function Edges({ kind, id }: { kind: "references" | "citers"; id: PublicId }) {
  const queryClient = useQueryClient()

  const query = useInfiniteQuery({
    queryKey: [kind, id],
    queryFn: ({ pageParam }) => api[kind](id, pageParam),
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
    <div className="space-y-3">
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

      {query.isSuccess && arcs.length === 0 && (
        <NoWorks>
          {kind === "references"
            ? "This work cites nothing we hold."
            : "Nothing we hold cites this work."}
        </NoWorks>
      )}
    </div>
  )
}
