import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { useNavigate, useSearch } from "@tanstack/react-router"
import { SearchIcon } from "lucide-react"
import { api, summaryOf } from "@/api"
import { cn } from "@/lib/utils"
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
 * The search box and its results. Centred on `/`; beside the work on a work's
 * page, on a wide screen. It reads `q` from whichever of the two it is on.
 */
export function SearchPanel({ focused }: { focused: boolean }) {
  const { q = "" } = useSearch({ strict: false })
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
          text field. An empty box submits to no query at all. Submitting
          changes the query and nothing else: beside a work, the work stays
          open and the results change around it. */}
      <form
        // Beside a work, the results scroll with the page and the box stays
        // at the top of the screen, so a new search is always one step away.
        // The padding is what the rows scroll under; the margins give it
        // back — all of it above, and below all but the gap the panel's own
        // spacing would have left — so nothing moves when the layout changes.
        // It sticks under the header's bar, its top padding tucked beneath
        // the bar, so the box lines up with the tops of the other columns.
        className={cn(
          focused &&
            "xl:bg-canvas xl:sticky xl:top-[calc(var(--bar)-1rem)] xl:z-10 xl:-mt-4 xl:mb-2 xl:py-4",
        )}
        onSubmit={(event) => {
          event.preventDefault()
          void navigate({
            to: ".",
            search: (prev) => ({ ...prev, q: draft.trim() }),
          })
        }}
      >
        {/* On the canvas, the box is a surface of its own, as the cards are. */}
        <InputGroup className={cn(focused && "xl:bg-card xl:border-border")}>
          <InputGroupInput
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Search works…"
            // Only where searching is what the page is for.
            autoFocus={!focused}
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

            {/* A hit is a row as a list's is: the same summary, so the
                same byline and the same warnings. Results are read by
                title, so they leave the identifiers to the work's page. */}
            <WorkList
              rows={search.data.hits.map(summaryOf)}
              identifiers={false}
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
