import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { createRoute, useNavigate } from "@tanstack/react-router"
import { api } from "@/api"
import { rootRoute } from "@/routes/root"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { WorkList } from "@/components/work-list"
import { Empty, ErrorBox, LoadingRows } from "@/components/states"

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

  // Search runs on submit, not on every keystroke: fewer requests, and the
  // results stop shifting under the cursor while you type. Submitting is a
  // navigation, so every search you ran is a history entry you can go back to.
  const search = useQuery({
    queryKey: ["search", q],
    queryFn: () => api.search(q, LIMIT),
    enabled: q.length > 0,
  })

  return (
    <div className="space-y-6">
      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          void navigate({ to: "/", search: { q: draft.trim() } })
        }}
      >
        <Input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="Search works…"
          autoFocus
        />
        <Button type="submit" disabled={draft.trim().length === 0}>
          Search
        </Button>
      </form>

      {q.length === 0 && <Empty>Type a query and press Search.</Empty>}

      {q.length > 0 && search.isPending && <LoadingRows />}
      {search.isError && <ErrorBox error={search.error} />}

      {search.isSuccess && (
        <>
          <p className="text-muted-foreground text-sm">
            {search.data.total} {search.data.total === 1 ? "work" : "works"} for{" "}
            <span className="font-mono">{q}</span>
            {search.data.total > search.data.hits.length &&
              ` · showing the first ${search.data.hits.length}`}
          </p>

          {search.data.hits.length === 0 ? (
            <Empty>Nothing matched.</Empty>
          ) : (
            /* A hit carries a title and an id; year and venue live on the
               work itself, one click away. */
            <WorkList
              rows={search.data.hits.map((hit) => ({
                public_id: hit.public_id,
                title: hit.title,
              }))}
            />
          )}
        </>
      )}
    </div>
  )
}
