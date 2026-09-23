import { Link, Outlet } from "@tanstack/react-router"
import { useQuery } from "@tanstack/react-query"
import { api, API_BASE } from "@/api"
import { Button } from "@/components/ui/button"
import { Logo } from "@/components/logo"

/**
 * The shell every screen renders inside: header, outlet, footer. Nothing else
 * lives here — there is no auth, no nested layout, and three screens do not
 * want one.
 *
 * It lives apart from the root route because a file that exports a route is
 * not a components-only module, and Vite's Fast Refresh reloads the whole page
 * on every edit to one. Kept here, a change to the header refreshes in place.
 */
export function Shell() {
  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <header className="mb-8 flex items-center gap-4">
        {/* The mark and wordmark are one lockup, as on the homepage, set back
            so that what this site is — Research — reads first. */}
        <Link to="/" className="flex items-center gap-2">
          <span className="text-muted-foreground flex items-center gap-2">
            <Logo className="size-8" />
            <span className="font-brand text-xl leading-none">
              oxidation labs
            </span>
          </span>
        </Link>
        <nav className="flex gap-1">
          <Button asChild variant="ghost" size="sm">
            <Link to="/">Search</Link>
          </Button>
          <Button asChild variant="ghost" size="sm">
            <Link to="/resolve">Find by ID</Link>
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
