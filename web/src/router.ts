import { createRouter } from "@tanstack/react-router"
import { rootRoute } from "@/routes/root"
import { searchRoute } from "@/routes/search"
import { workRoute } from "@/routes/work-detail"
import { resolveRoute } from "@/routes/resolve"

/**
 * Code-based routes, not file-based. Three screens do not need a codegen step
 * and a generated route tree checked into the repo; each route is declared
 * beside the component it renders, and the tree is these four lines.
 *
 * Routing is the only thing in here. Fetching stays with TanStack Query in the
 * components — no route loaders, so there is exactly one path to the API.
 */
const routeTree = rootRoute.addChildren([searchRoute, workRoute, resolveRoute])

export const router = createRouter({
  routeTree,
  // Going back to a list should put you back where you were in it.
  scrollRestoration: true,
})

/** Makes every `to`, `params` and `search` in the app typed against the tree. */
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}
