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
 * `/find` and `/find?text=…`: whatever a visitor has that points at a paper
 * — a DOI, a PMID or any other identifier, with its namespace or without, a
 * link, a title, a whole citation — and the papers it may be
 * (docs/citation-lookup-plan.md). One view for what were two, Find by ID and
 * Find by citation, since a visitor asks both the same thing (the user,
 * 2026-09-28); underneath it is one call, the citation lookup, which reads
 * identifiers of every kind and searches the words when there are any. What
 * was typed lives in the URL, as a search's query does, so that the answer
 * is a link.
 *
 * Text that is identifiers and nothing more, naming one paper, goes straight
 * to it, as Find by ID did: a DOI pasted alone is a paper asked for. Anything
 * with words in it stays here, with the list and what led to each paper.
 */
export const findRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/find",
  validateSearch: (search: Record<string, unknown>): { text?: string } => {
    const text = typeof search.text === "string" ? search.text.trim() : ""
    return text ? { text } : {}
  },
  component: Find,
})

/** What an identifier's name turned out to be, as a reader would say it. */
function named(id: CitedName): string {
  switch (id.resolved) {
    case "one":
      return "names one paper"
    case "shared":
      return `a name shared by ${id.public_ids.length} papers`
    case "split":
      return `an id retired by a split into ${id.public_ids.length} papers`
    default:
      return "names nothing we hold"
  }
}

/**
 * Whether the identifiers lead to different papers: matches reached by
 * different names. A shared name reaching two is one name, and its line says
 * so on its own. Whether the identifiers and the words lead to different
 * papers is the server's to say (`differ`).
 */
function disagrees(matches: CitationMatch[]): boolean {
  const named = matches.filter((m) => m.named_by.length > 0)
  return new Set(named.map((m) => m.named_by.join(" "))).size > 1
}

/** The one paper the matches are, when they are one; otherwise null. */
function onlyPaper(matches: CitationMatch[]): string | null {
  const ids = new Set(matches.map((m) => m.public_id))
  return ids.size === 1 ? [...ids][0] : null
}

/** Why nothing is listed, in the reader's words. */
function couldNot(reason: string): string {
  switch (reason) {
    case "nothing-to-go-on":
      return "There is nothing in it to go on."
    case "nothing-agreed":
      return "Its words turned up papers, and none agreed with it on everything the paper says."
    default:
      return "Nothing in it led to a paper we hold."
  }
}

function Find() {
  const { text = "" } = findRoute.useSearch()
  const navigate = useNavigate()
  const [draft, setDraft] = useState(text)

  useEffect(() => setDraft(text), [text])

  const query = useQuery({
    queryKey: ["find", text],
    queryFn: () => api.citation(text),
    enabled: text.length > 0,
    retry: false,
  })

  const submit = () => {
    void navigate({ to: "/find", search: { text: draft.trim() } })
  }
  const answer = query.data ?? null

  // Identifiers alone, naming one paper: one place to be. The jump replaces
  // this entry, so that Back does not land here and jump forward again.
  const straight = answer?.only_names ? onlyPaper(answer.matches) : null
  useEffect(() => {
    if (straight) {
      void navigate({
        to: "/work/$workId",
        params: { workId: straight },
        replace: true,
      })
    }
  }, [straight, navigate])

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-medium">Find a paper</h1>
        <p className="text-muted-foreground text-sm">
          Paste whatever points at it: a DOI, a PMID or any other identifier, a
          link, its title, or a whole citation as a reference list writes it.
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
            // Ctrl with it finds.
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault()
              submit()
            }
          }}
          placeholder="10.1038/nature12373  ·  PMID 23851394  ·  a title  ·  Lowry OH, Rosebrough NJ, Farr AL, Randall RJ. Protein measurement with the Folin phenol reagent. J Biol Chem. 1951;193(1):265-75."
          autoFocus
          rows={3}
        />
        <Button type="submit" disabled={draft.trim().length === 0}>
          Find
        </Button>
      </form>

      {text.length === 0 && <Empty>Paste something that points at a paper, and press Find.</Empty>}

      {text.length > 0 && (query.isPending || straight !== null) && <LoadingRows rows={2} />}
      {query.isError && <ErrorBox error={query.error} />}

      {answer !== null && straight === null && (
        <div className="space-y-6">
          {answer.identifiers.length > 0 && (
            <section className="space-y-2">
              <h2 className="text-sm font-medium">Identifiers in it</h2>
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
            </section>
          )}

          {(answer.differ || disagrees(answer.matches)) && (
            <Alert>
              <AlertTitle>What you pasted disagrees with itself</AlertTitle>
              <AlertDescription>
                {answer.differ
                  ? "An identifier in it names one paper, and its words match another. "
                  : "Its identifiers name different papers. "}
                Each is listed below with what led to it.
              </AlertDescription>
            </Alert>
          )}

          <section className="space-y-2">
            <h2 className="text-sm font-medium">Papers it may be</h2>
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
