import { Link, Outlet, createRootRoute } from "@tanstack/react-router"
import { useQuery } from "@tanstack/react-query"
import { api, API_BASE } from "@/api"
import { Button } from "@/components/ui/button"

/**
 * The shell every screen renders inside: header, outlet, footer. Nothing else
 * lives here — there is no auth, no nested layout, and three screens do not
 * want one.
 */
export const rootRoute = createRootRoute({
  component: Shell,
  notFoundComponent: NotFound,
})

function Shell() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <header className="mb-8 flex items-baseline gap-4">
        <Link to="/" className="text-lg font-semibold">
          ol-rht
        </Link>
        <nav className="flex gap-1">
          <Button asChild variant="ghost" size="sm">
            <Link to="/">Search</Link>
          </Button>
          <Button asChild variant="ghost" size="sm">
            <Link to="/resolve">Resolve</Link>
          </Button>
        </nav>
      </header>

      <main>
        <Outlet />
      </main>

      <Footer />
    </div>
  )
}

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

/**
 * Which epoch answered, how current it is, and the attribution its sources
 * require. The sources ask that these be shown, so they are shown.
 */
function Footer() {
  const epoch = useQuery({ queryKey: ["epoch"], queryFn: api.epoch })
  const answered = epoch.data?.answered ?? null

  return (
    <footer className="text-muted-foreground mt-12 space-y-2 border-t pt-4 text-xs">
      <p className="font-mono">api: {API_BASE}</p>

      {epoch.isError && (
        <p>
          No epoch info:{" "}
          {epoch.error instanceof Error ? epoch.error.message : "unknown error"}
        </p>
      )}

      {epoch.isSuccess && answered !== null && (
        <>
          <p className="font-mono">
            epoch {answered.epoch} · batch {answered.batch} · as of{" "}
            {answered.at} · {epoch.data.works.toLocaleString()} works ·{" "}
            {epoch.data.arcs.toLocaleString()} arcs
          </p>
          {answered.attribution.map((line) => (
            <p key={line}>{line}</p>
          ))}
        </>
      )}
    </footer>
  )
}
