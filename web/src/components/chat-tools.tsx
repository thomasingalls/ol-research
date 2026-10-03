import type { ToolCallMessagePartProps } from "@assistant-ui/react"
import { Link } from "@tanstack/react-router"
import { BookOpenIcon, LoaderIcon, SearchIcon } from "lucide-react"
import { ToolFallback } from "@/components/assistant-ui/elements/tool-fallback.aui"
import { Badge } from "@/components/ui/badge"
import { isPaperArgs, isPaperResult, isSearchArgs, isSearchResult } from "@/lib/chat-adapter"
import type { PropsWithChildren } from "react"

/** Keep the papers visible instead of hiding them in the generic tool group. */
export function ChatToolGroup({ children }: PropsWithChildren) {
  return <>{children}</>
}

/** Display only: the Rust service owns execution and supplies the result. */
export function ChatTool(props: ToolCallMessagePartProps) {
  if (props.toolName === "get_paper" && isPaperArgs(props.args)) {
    const result = isPaperResult(props.result) ? props.result : undefined
    const running = !result && props.status.type === "running"
    return <section aria-label="Paper lookup" className="my-2 rounded-lg border p-3 text-sm">
      <p role="status" className="flex items-center gap-2">
        {running ? <LoaderIcon aria-hidden className="size-4 shrink-0 animate-spin motion-reduce:animate-none" /> : <BookOpenIcon aria-hidden className="size-4 shrink-0" />}
        {running ? "Retrieving paper…" : !result ? "Paper lookup stopped" : result.status === "error" ? "Paper lookup failed" : "Paper retrieved"}
      </p>
      <p className="text-muted-foreground mt-1 break-all text-xs">{result?.status === "ok" ? result.paper.public_id : props.args.id}</p>
      {result?.status === "error" && <p className="text-destructive mt-2">{result.message}</p>}
    </section>
  }
  return <ChatSearch {...props} />
}

function ChatSearch(props: ToolCallMessagePartProps) {
  if (props.toolName !== "search" || !isSearchArgs(props.args)) return <ToolFallback {...props} />
  const result = isSearchResult(props.result) ? props.result : undefined
  const running = !result && props.status.type === "running"
  const label = running ? "Searching the corpus…" : !result ? "Search stopped" : result.status === "error" ? "Search failed" : "Corpus search"

  return (
    <section aria-label="Corpus search" className="my-2 min-w-0 rounded-lg border p-4 text-sm">
      <p role="status" className="flex items-center gap-2 font-medium">
        {running ? <LoaderIcon aria-hidden className="size-4 shrink-0 animate-spin motion-reduce:animate-none" /> : <SearchIcon aria-hidden className="size-4 shrink-0" />}
        {label}
      </p>
      <p className="text-muted-foreground mt-1 break-words">{props.args.query}</p>
      {result?.status === "error" && <p className="text-destructive mt-2">{result.message}</p>}
      {result?.status === "ok" && <>
        <p className="text-muted-foreground mt-2 text-xs">
          {result.total === 0 ? "No matches in this corpus." : `Showing ${result.hits.length} of ${result.total.toLocaleString()} matches · Metadata only`}
        </p>
        <ol className="mt-3 space-y-3">
          {result.hits.map((paper) => <li key={paper.public_id} className="space-y-1">
            <Link to="/work/$workId" params={{ workId: paper.public_id }} target="_blank" rel="noopener noreferrer"
              className="font-medium break-words underline underline-offset-4">
              {paper.title || paper.public_id}<span className="sr-only"> (opens in a new tab)</span>
            </Link>
            <p className="text-muted-foreground text-xs break-words">
              {[paper.authors.join(", "), paper.venue, paper.year].filter(Boolean).join(" · ")}
            </p>
            <div className="flex flex-wrap gap-1">
              {paper.retracted_by.length > 0 && <Badge variant="destructive">Retracted</Badge>}
              {paper.corrected_by.length > 0 && <Badge variant="secondary">Corrected</Badge>}
              {paper.concern_by.length > 0 && <Badge variant="destructive">Expression of concern</Badge>}
            </div>
          </li>)}
        </ol>
        {result.answered && <details className="text-muted-foreground mt-3 text-xs">
          <summary className="cursor-pointer">Sources: {result.answered.sources.join(", ")} · Epoch {result.answered.epoch}</summary>
          <p className="mt-1">Batch {result.answered.batch} · {result.answered.at}</p>
          {result.answered.attribution.map((line) => <p key={line} className="mt-1">{line}</p>)}
        </details>}
      </>}
    </section>
  )
}
