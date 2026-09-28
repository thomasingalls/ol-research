import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { createRoute, useNavigate } from "@tanstack/react-router"
import { api, summaryOf, type CitationMatch, type CitedName } from "@/api"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import { WorkRow } from "@/components/work-list"
import { Empty, ErrorBox, LoadingRows, NoWorks } from "@/components/states"
import { rootRoute } from "@/routes/root"

/**
 * `/citation` and `/citation?text=…`: one citation — a line from a reference
 * list — pasted, and the works it may be, each with what led to it
 * (docs/citation-lookup-plan.md). The citation lives in the URL for the same
 * reason a search's query does: so that the answer is a link.
 *
 * Two paths, every time: the DOIs, PMIDs and PMC ids written in it are looked
 * up, and its words are searched; each work either path reaches is checked
 * against it. Pre-v1: the check's field-by-field verdicts and the near misses
 * are in the answer and not yet on this page (the plan's CL4).
 */
export const citationRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/citation",
  validateSearch: (search: Record<string, unknown>): { text?: string } => {
    const text = typeof search.text === "string" ? search.text.trim() : ""
    return text ? { text } : {}
  },
  component: Citation,
})

/** What an identifier's name turned out to be, as a reader would say it. */
function named(id: CitedName): string {
  switch (id.resolved) {
    case "one":
      return "names one work"
    case "shared":
      return `a name shared by ${id.public_ids.length} works`
    case "split":
      return `an id retired by a split into ${id.public_ids.length} works`
    default:
      return "names nothing we hold"
  }
}

/**
 * Whether the citation's identifiers lead to different works: matches reached
 * by different names. A shared name reaching two works is one name, and its
 * line above says so on its own. Whether its identifiers and its words lead
 * to different works is the server's to say (`differ`).
 */
function disagrees(matches: CitationMatch[]): boolean {
  const named = matches.filter((m) => m.named_by.length > 0)
  return new Set(named.map((m) => m.named_by.join(" "))).size > 1
}

/** Why nothing is listed, in the reader's words. */
function couldNot(reason: string): string {
  switch (reason) {
    case "nothing-to-go-on":
      return "There is nothing in the citation to go on."
    case "nothing-agreed":
      return "Its words turned up works, and none agreed with it on every field the work has."
    default:
      return "Nothing the citation names or says led to a work we hold."
  }
}

function Citation() {
  const { text = "" } = citationRoute.useSearch()
  const navigate = useNavigate()
  const [draft, setDraft] = useState(text)

  useEffect(() => setDraft(text), [text])

  const query = useQuery({
    queryKey: ["citation", text],
    queryFn: () => api.citation(text),
    enabled: text.length > 0,
    retry: false,
  })

  const submit = () => {
    void navigate({ to: "/citation", search: { text: draft.trim() } })
  }
  const answer = query.data ?? null

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-medium">Find by citation</h1>
        <p className="text-muted-foreground text-sm">
          Paste one citation, as a reference list writes it. The DOI, PMID or
          PMC id in it is looked up, its words are searched, and every work
          found is checked against it.
        </p>
      </div>

      <form
        className="space-y-2"
        onSubmit={(event) => {
          event.preventDefault()
          submit()
        }}
      >
        <Textarea
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            // Enter is a new line, since a pasted citation may wrap; ⌘ or
            // Ctrl with it looks the citation up.
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault()
              submit()
            }
          }}
          placeholder="Lowry OH, Rosebrough NJ, Farr AL, Randall RJ. Protein measurement with the Folin phenol reagent. J Biol Chem. 1951;193(1):265-75. doi:10.1016/S0021-9258(19)52451-6"
          autoFocus
          rows={3}
        />
        <Button type="submit" disabled={draft.trim().length === 0}>
          Look up
        </Button>
      </form>

      {text.length === 0 && <Empty>Paste a citation and press Look up.</Empty>}

      {text.length > 0 && query.isPending && <LoadingRows rows={2} />}
      {query.isError && <ErrorBox error={query.error} />}

      {answer !== null && (
        <div className="space-y-6">
          <section className="space-y-2">
            <h2 className="text-sm font-medium">Identifiers in the citation</h2>
            {answer.identifiers.length === 0 ? (
              <Empty>No DOI, PMID or PMC id was found in it.</Empty>
            ) : (
              <ul className="space-y-1 text-sm">
                {answer.identifiers.map((id) => (
                  <li key={`${id.start}:${id.name}`}>
                    <span className="font-mono break-all">{id.written}</span>
                    <span className="text-muted-foreground">
                      {" "}
                      — read as <span className="font-mono">{id.name}</span>,{" "}
                      {named(id)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          {(answer.differ || disagrees(answer.matches)) && (
            <Alert>
              <AlertTitle>The citation disagrees with itself</AlertTitle>
              <AlertDescription>
                {answer.differ
                  ? "An identifier in it names one work, and its words match another. "
                  : "Its identifiers name different works. "}
                Each is listed below with what led to it.
              </AlertDescription>
            </Alert>
          )}

          <section className="space-y-2">
            <h2 className="text-sm font-medium">Works it may be</h2>
            {answer.matches.length === 0 ? (
              <NoWorks>{couldNot(answer.could_not)}</NoWorks>
            ) : (
              <ul className="divide-border divide-y">
                {answer.matches.map((match) => (
                  <li key={match.public_id}>
                    <WorkRow row={summaryOf(match)}>
                      <p className="text-muted-foreground mt-1 text-xs">
                        {match.named_by.length > 0 && (
                          <>
                            named by{" "}
                            <span className="font-mono">{match.named_by.join(", ")}</span>
                          </>
                        )}
                        {match.named_by.length > 0 && match.by_text && " · "}
                        {match.by_text && "found by its words"}
                      </p>
                    </WorkRow>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      )}
    </div>
  )
}
