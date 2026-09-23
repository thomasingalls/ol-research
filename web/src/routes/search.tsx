import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { createRoute, useNavigate } from "@tanstack/react-router"
import { SearchIcon } from "lucide-react"
import { api } from "@/api"
import { rootRoute } from "@/routes/root"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { WorkList } from "@/components/work-list"
import { ErrorBox, LoadingRows } from "@/components/states"

const LIMIT = 50

/**
 * `/` and `/?q=…`. The query is a validated search param rather than component
 * state, so a search is a link: paste the URL and land on the same results.
 * An empty or missing `q` normalises to no parameter at all.
 */
export const searchRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  validateSearch: (search: Record<string, unknown>): { q?: string } => {
    const q = typeof search.q === "string" ? search.q.trim() : ""
    return q ? { q } : {}
  },
  component: Search,
})

function Search() {
  const { q = "" } = searchRoute.useSearch()
  const navigate = useNavigate()
  const [draft, setDraft] = useState(q)

  // Back, forward, or a pasted link moves the URL under us; the box follows it.
  useEffect(() => setDraft(q), [q])

  // Search runs on Enter, not on every keystroke: fewer requests, and the
  // results stop shifting under the cursor while you type. Submitting is a
  // navigation, so every search you ran is a history entry you can go back to.
  const search = useQuery({
    queryKey: ["search", q],
    queryFn: () => api.search(q, LIMIT),
    enabled: q.length > 0,
  })

  return (
    <div className="space-y-6">
      {/* No submit button: Enter submits, since this is the form's only
          text field. An empty box submits to the bare `/`. */}
      <form
        onSubmit={(event) => {
          event.preventDefault()
          void navigate({ to: "/", search: { q: draft.trim() } })
        }}
      >
        <InputGroup>
          <InputGroupInput
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Search works…"
            autoFocus
          />
          <InputGroupAddon>
            <SearchIcon />
          </InputGroupAddon>
        </InputGroup>
      </form>

      {q.length === 0 && <Corpus />}

      {q.length > 0 && search.isPending && <LoadingRows />}
      {search.isError && <ErrorBox error={search.error} />}

      {search.isSuccess &&
        (search.data.hits.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <SearchIcon />
              </EmptyMedia>
              <EmptyTitle>No matches</EmptyTitle>
            </EmptyHeader>
          </Empty>
        ) : (
          <>
            <p className="text-muted-foreground text-sm">
              {search.data.total} {search.data.total === 1 ? "work" : "works"}{" "}
              for <span className="font-mono">{q}</span>
              {search.data.total > search.data.hits.length &&
                ` · showing the first ${search.data.hits.length}`}
            </p>

            {/* A hit carries a title and an id; year and venue live on the
                work itself, one click away. */}
            <WorkList
              rows={search.data.hits.map((hit) => ({
                public_id: hit.public_id,
                title: hit.title,
              }))}
            />
          </>
        ))}
    </div>
  )
}

/**
 * Before any query: what there is to search, not how to search it. The counts
 * are the epoch the footer already asked for, so this costs no request; until
 * they arrive, or if they never do, the icon stands alone.
 */
function Corpus() {
  const epoch = useQuery({ queryKey: ["epoch"], queryFn: api.epoch })

  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <SearchIcon />
        </EmptyMedia>
        {epoch.isSuccess && (
          <EmptyDescription className="tabular-nums">
            <p>{epoch.data.works.toLocaleString()} works</p>
            <p>{epoch.data.arcs.toLocaleString()} citations</p>
          </EmptyDescription>
        )}
      </EmptyHeader>
    </Empty>
  )
}
