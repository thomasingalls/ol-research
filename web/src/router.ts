import { createRouter, type ParsedLocation } from "@tanstack/react-router"
import { isWide } from "@/lib/wide"
import { rootRoute } from "@/routes/root"
import { browseRoute } from "@/routes/browse"
import { searchRoute } from "@/routes/search"
import { workRoute } from "@/routes/work-detail"
import { resolveRoute } from "@/routes/resolve"
import { citationRoute } from "@/routes/citation"
import { findRoute } from "@/routes/find"

/**
 * Code-based routes, not file-based. Three screens do not need a codegen step
 * and a generated route tree checked into the repo; each route is declared
 * beside the component it renders, and the tree is these few lines. Search
 * and work share a layout, `browse`, which holds the search panel so that it
 * survives a work opening beside it.
 *
 * Routing is the only thing in here. Fetching stays with TanStack Query in the
 * components — no route loaders, so there is exactly one path to the API.
 */
const routeTree = rootRoute.addChildren([
  browseRoute.addChildren([searchRoute, workRoute]),
  findRoute,
  // Find by ID's and Find by citation's old addresses, sent on to /find.
  resolveRoute,
  citationRoute,
])

/**
 * Opening a work from the centred search, and going back to it, animate: the
 * search moves aside to make room, or back to the middle. Nothing else does.
 * The transition is the browser's (View Transitions); this names which of the
 * two is running, and index.css says what each one looks like.
 *
 * Only on a wide screen, where the work opens beside the search — below that
 * the work replaces it, and there is nothing to move. Not for anyone who has
 * asked for less motion. And not at all in a browser that cannot tell the two
 * apart, which would otherwise run a transition on every navigation.
 */
function layoutChange({
  fromLocation,
  toLocation,
}: {
  fromLocation?: ParsedLocation
  toLocation: ParsedLocation
}): string[] | false {
  if (!fromLocation || !isWide() || matchMedia("(prefers-reduced-motion: reduce)").matches) {
    return false
  }
  const from = fromLocation.pathname
  const to = toLocation.pathname
  if (from === "/" && to.startsWith("/work/")) return ["open-work"]
  if (from.startsWith("/work/") && to === "/") return ["close-work"]
  return false
}

export const router = createRouter({
  routeTree,
  // Going back to a list should put you back where you were in it.
  scrollRestoration: true,
  defaultViewTransition: CSS.supports("selector(:active-view-transition-type(a))")
    ? { types: layoutChange }
    : false,
})

/** Makes every `to`, `params` and `search` in the app typed against the tree. */
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}
