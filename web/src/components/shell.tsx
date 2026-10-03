import { Link, Outlet, useParams } from "@tanstack/react-router"
import { useQuery } from "@tanstack/react-query"
import { api, API_BASE } from "@/api"
import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { Logo } from "@/components/logo"

/**
 * The shell every screen renders inside: header, outlet, footer. Nothing else
 * lives here — there is no auth, and the one nested layout (search beside a
 * work) is `Browse`'s.
 *
 * It is a centred column, except with a work open on a wide screen, when it
 * gives the three columns there the whole width, on a canvas a shade darker
 * than the work and its lists, so the two stand off it as surfaces and the
 * search reads as the sidebar beside them. The footer moves into the middle
 * column, under the work, so it is not stranded beneath the longest.
 *
 * On a wide screen the header is a bar that stays at the top, `--bar` tall,
 * on either layout. The work and its lists can then always start just under
 * it and end just above the bottom of the screen, however far the results
 * have been scrolled. And the bar is laid out the same on both, so that when
 * a work opens or closes it has only to slide sideways, from over the centred
 * column to the far left and back: a header that changed height or scrolled
 * out of sight in between would roll up or drop in instead.
 *
 * It lives apart from the root route because a file that exports a route is
 * not a components-only module, and Vite's Fast Refresh reloads the whole page
 * on every edit to one. Kept here, a change to the header refreshes in place.
 */
export function Shell() {
  const { workId } = useParams({ strict: false })
  const focused = workId !== undefined

  return (
    <div
      className={cn(
        "mx-auto max-w-4xl px-4 py-8 xl:pt-0 xl:[--bar:4rem]",
        focused && "xl:bg-canvas xl:min-h-dvh xl:max-w-none xl:px-6",
      )}
    >
      <header
        data-vt="header"
        className={cn(
          "mb-8 flex flex-wrap items-center gap-4",
          "xl:bg-background xl:sticky xl:top-0 xl:z-20 xl:mb-0 xl:h-(--bar)",
          focused && "xl:bg-canvas",
        )}
      >
        {/* The mark and wordmark are one lockup, as on the homepage, set back
            so that what this site is — Research — reads first. */}
        <Link to="/" className="flex shrink-0 items-center gap-2">
          <span className="text-muted-foreground flex items-center gap-2">
            <Logo className="size-8" />
            <span className="font-brand text-xl leading-none">
              oxidation labs
            </span>
          </span>
        </Link>
        <nav className="flex gap-1">
          <Button asChild variant="ghost" size="sm">
            {/* Back to the search you ran, if there is one. The mark, beside
                it, is the way to a fresh start. */}
            <Link to="/" search={(prev) => ({ q: prev.q })}>
              Search
            </Link>
          </Button>
          <Button asChild variant="ghost" size="sm">
            <Link to="/find">Find a paper</Link>
          </Button>
          <Button asChild variant="ghost" size="sm">
            <Link to="/chat">Chat</Link>
          </Button>
        </nav>
      </header>

      <main>
        <Outlet />
      </main>

      <Footer className={cn(focused && "xl:hidden")} />
    </div>
  )
}

/**
 * Which epoch answered, how current it is, and the attribution its sources
 * require. The sources ask that these be shown, so they are shown.
 */
export function Footer({ className }: { className?: string }) {
  const epoch = useQuery({ queryKey: ["epoch"], queryFn: api.epoch })
  const answered = epoch.data?.answered ?? null

  return (
    <footer
      className={cn(
        "text-muted-foreground mt-12 space-y-2 border-t pt-4 text-xs",
        className,
      )}
    >
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
