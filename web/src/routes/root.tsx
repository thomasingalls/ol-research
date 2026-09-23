import { Link, createRootRoute } from "@tanstack/react-router"
import { Button } from "@/components/ui/button"
import { Shell } from "@/components/shell"

/**
 * The root every screen renders inside. Its component is the `Shell` —
 * header, outlet, footer — which lives in components/ so that it can be
 * edited without a full page reload.
 */
export const rootRoute = createRootRoute({
  component: Shell,
  notFoundComponent: NotFound,
})

/** A URL that names no screen says so, rather than quietly showing search. */
function NotFound() {
  return (
    <div className="space-y-3">
      <h1 className="font-medium">No such page</h1>
      <p className="text-muted-foreground text-sm">
        <span className="font-mono">{window.location.pathname}</span> is not a
        screen here.
      </p>
      <Button asChild variant="outline" size="sm">
        <Link to="/">← Search</Link>
      </Button>
    </div>
  )
}
