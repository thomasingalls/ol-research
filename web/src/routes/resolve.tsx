import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { createRoute, useNavigate } from "@tanstack/react-router"
import { api, type Shared, type Split, type ResolveOutcome } from "@/api"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { WorkList } from "@/components/work-list"
import { Empty, ErrorBox, LoadingRows } from "@/components/states"
import { rootRoute } from "@/routes/root"

/** The protocol's three outcomes, narrowed. */
const one = (o: ResolveOutcome): o is { public_id: string } => "public_id" in o
const shared = (o: ResolveOutcome): o is { shared: Shared } => "shared" in o
const split = (o: ResolveOutcome): o is { split: Split } => "split" in o

/**
 * `/resolve` and `/resolve?text=…`. The pasted name lives in the URL for the
 * same reason the search query does: so the answer is a link. Same shape as
 * the search screen — the box holds a draft, the URL holds what was submitted.
 */
export const resolveRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/resolve",
  validateSearch: (search: Record<string, unknown>): { text?: string } => {
    const text = typeof search.text === "string" ? search.text.trim() : ""
    return text ? { text } : {}
  },
  component: Resolve,
})

function Resolve() {
  const { text = "" } = resolveRoute.useSearch()
  const navigate = useNavigate()
  const [draft, setDraft] = useState(text)

  useEffect(() => setDraft(text), [text])

  const query = useQuery({
    queryKey: ["resolve", text],
    queryFn: () => api.resolve(text),
    enabled: text.length > 0,
    retry: false,
  })

  const outcome = query.data?.outcome ?? null

  // One work, one place to be. The other two outcomes need a decision from
  // the reader, so they stay on this page. The jump replaces this entry rather
  // than pushing one: otherwise Back would land here and resolve forward again.
  const single = outcome !== null && one(outcome) ? outcome.public_id : null
  useEffect(() => {
    if (single) {
      void navigate({
        to: "/work/$workId",
        params: { workId: single },
        replace: true,
      })
    }
  }, [single, navigate])

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-medium">Resolve a name</h1>
        <p className="text-muted-foreground text-sm">
          Paste a name that points at a work — a DOI, a{" "}
          <span className="font-mono">pmid:12345678</span>, a{" "}
          <span className="font-mono">pmc:PMC…</span>, an{" "}
          <span className="font-mono">openalex:W…</span> — or a public id.
        </p>
      </div>

      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          void navigate({ to: "/resolve", search: { text: draft.trim() } })
        }}
      >
        <Input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="doi:10.1038/nature12373  ·  pmid:23851394  ·  pmc:PMC3737249"
          autoFocus
          className="font-mono"
        />
        <Button type="submit" disabled={draft.trim().length === 0}>
          Resolve
        </Button>
      </form>

      {text.length === 0 && <Empty>Paste a name and press Resolve.</Empty>}

      {text.length > 0 && query.isPending && <LoadingRows rows={1} />}
      {query.isError && <ErrorBox error={query.error} />}

      {query.isSuccess && outcome === null && (
        <Empty>
          <span className="font-mono">{text}</span> names no work we hold.
        </Empty>
      )}

      {outcome !== null && shared(outcome) && (
        <div className="space-y-3">
          <Alert>
            <AlertTitle>A shared name</AlertTitle>
            <AlertDescription>
              <span className="font-mono">{text}</span> was accepted into{" "}
              {outcome.shared.public_ids.length} works. Pick one.
            </AlertDescription>
          </Alert>
          <WorkList
            rows={outcome.shared.public_ids.map((public_id) => ({
              public_id,
              title: "",
            }))}
          />
        </div>
      )}

      {outcome !== null && split(outcome) && (
        <Alert>
          <AlertTitle>A retired id</AlertTitle>
          <AlertDescription>
            <span className="font-mono">{text}</span> was retired by a split, so
            no survivor answers for it. It was{" "}
            <span className="font-mono">{outcome.split.public_id}</span>.
          </AlertDescription>
        </Alert>
      )}
    </div>
  )
}
