import { useQuery } from "@tanstack/react-query"
import { Link, createRoute } from "@tanstack/react-router"
import { nameText, primaryName, year } from "@/api"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { CopyNames } from "@/components/copy-names"
import { ReadButton } from "@/components/read-button"
import {
  CitationLists,
  LIST_DEFAULT,
  LISTS,
  type List,
} from "@/components/citation-lists"
import { Footer } from "@/components/shell"
import { Empty, ErrorBox, LoadingRows } from "@/components/states"
import { browseRoute } from "@/routes/browse"
import { workQuery } from "@/queries"

/**
 * `/work/$workId`, where `workId` is a public id such as `ol:71D1MFYWNG8KEK`.
 * A public id never contains a slash, so it is safe as a path segment — which
 * a name like `doi:10.1000/xyz` would not be. Names are resolved on /resolve.
 */
export const workRoute = createRoute({
  getParentRoute: () => browseRoute,
  path: "/work/$workId",
  /**
   * Which list is open lives in the URL, like the search query does, so the
   * page you are looking at is the page you can send to someone. The default
   * one normalises to no parameter at all, and anything that is not one of
   * the four is dropped rather than passed to the API.
   */
  validateSearch: (search: Record<string, unknown>): { list?: List } => {
    const list = LISTS.find((l) => l.list === search.list)?.list
    return list && list !== LIST_DEFAULT ? { list } : {}
  },
  /**
   * Another work is another page, not this one with new props: its columns
   * start at the top, and nothing ("Show more", say) carries over from the
   * work before. Back still returns each column to where you left it.
   */
  remountDeps: ({ params }) => params.workId,
  component: WorkDetail,
})

/**
 * The work, and its lists beside it. On a wide screen the two are the middle
 * and right columns of the page, each scrolling on its own and held in view
 * while the search results to their left scroll with the page. Below that
 * width they are one column, work above lists, as a page of their own.
 */
function WorkDetail() {
  const { workId: id } = workRoute.useParams()
  const { list = LIST_DEFAULT } = workRoute.useSearch()

  return (
    // Held just under the header's bar (see Shell) and a margin above the
    // bottom of the screen, so both columns are whole on screen at once.
    <div className="space-y-8 xl:sticky xl:top-(--bar) xl:grid xl:h-[calc(100dvh-var(--bar)-1rem)] xl:grid-cols-[minmax(0,1fr)_24rem] xl:gap-8 xl:space-y-0 xl:self-start 2xl:grid-cols-[minmax(0,1fr)_32rem]">
      {/* The card's edge is a ring and a shadow drawn outside its box,
          which a scrolling column would cut off; the padding makes room for
          them and the negative margin gives it back to the layout. */}
      <div
        data-vt="paper"
        data-scroll-restoration-id="paper"
        className="xl:-m-1 xl:overflow-y-auto xl:p-1"
      >
        <Paper id={id} />
        <Footer className="hidden xl:block" />
      </div>

      {/* On a wide screen the lists are a surface like the work's card,
          and scroll inside it. */}
      <aside
        data-vt="lists"
        data-scroll-restoration-id="lists"
        className="xl:bg-card xl:overflow-y-auto xl:rounded-[min(var(--radius-4xl),24px)] xl:px-3 xl:pb-3 xl:shadow-sm xl:ring-1 xl:ring-foreground/5"
      >
        <CitationLists id={id} list={list} />
      </aside>
    </div>
  )
}

function Paper({ id }: { id: string }) {
  const work = useQuery(workQuery(id))
  const w = work.data?.work ?? null

  // With no title — a stub always, anything else only when its record has
  // none — the identifier stands where the title would. It is what we hold.
  const lead = w && !w.title ? primaryName(w.names) : null

  return (
    <div className="space-y-6">
      {/* Beside the results there is nothing to go back to: they are there. */}
      <Button asChild variant="ghost" size="sm" className="xl:hidden">
        <Link to="/" search={(prev) => ({ q: prev.q })}>
          ← Search
        </Link>
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
              {/* The names the work goes by elsewhere, and this site's id
                  for it, each a click to copy. */}
              <CopyNames names={w.names} publicId={w.public_id} />
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

            <ReadButton names={w.names} />
          </CardContent>
        </Card>
      )}
    </div>
  )
}
