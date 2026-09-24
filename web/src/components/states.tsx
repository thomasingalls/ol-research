import { QuoteIcon } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Empty as EmptyFrame,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { ApiError } from "@/api"

export function LoadingRows({ rows = 4 }: { rows?: number }) {
  return (
    <div className="space-y-3" aria-busy="true" aria-label="Loading">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="space-y-2">
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-3 w-1/3" />
        </div>
      ))}
    </div>
  )
}

/**
 * Show the failure as it actually was. An unreachable API is not an empty
 * result, and we never pretend otherwise.
 */
export function ErrorBox({ error }: { error: unknown }) {
  const isApi = error instanceof ApiError
  const unreachable = isApi && error.unreachable
  const code = isApi ? error.code : null
  const message = error instanceof Error ? error.message : "Unknown error"

  return (
    <Alert variant="destructive">
      <AlertTitle>
        {unreachable ? "API unreachable" : (code ?? "Request failed")}
      </AlertTitle>
      <AlertDescription>
        {/* The server's message says why. Show it as it was written. */}
        <span className="font-mono text-xs break-all">{message}</span>
      </AlertDescription>
    </Alert>
  )
}

export function Empty({ children }: { children: React.ReactNode }) {
  return <p className="text-muted-foreground text-sm">{children}</p>
}

/**
 * A list of works with nothing in it, and why. An empty list is an answer,
 * so it gets the same weight the search gives "No matches", not a line of
 * grey text where the rows would be. `children` says why.
 */
export function NoWorks({ children }: { children: React.ReactNode }) {
  return (
    <EmptyFrame>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <QuoteIcon />
        </EmptyMedia>
        <EmptyTitle>No works</EmptyTitle>
        <EmptyDescription>{children}</EmptyDescription>
      </EmptyHeader>
    </EmptyFrame>
  )
}
