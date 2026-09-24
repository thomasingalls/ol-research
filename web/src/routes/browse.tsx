import { createRoute } from "@tanstack/react-router"
import { Browse } from "@/components/browse"
import { rootRoute } from "@/routes/root"

/**
 * The search and the work you opened from it, as one screen. A layout route
 * with no path of its own: `/` and `/work/$workId` both render inside it, so
 * the search panel is one element that stays mounted when a work opens —
 * which is what lets it move aside instead of vanishing and being rebuilt.
 *
 * It owns the query. `q` is a validated search param rather than component
 * state, so a search is a link: paste the URL and land on the same results.
 * It rides along on a work's URL too, because the results beside the work
 * are part of what you are looking at. An empty or missing `q` normalises to
 * no parameter at all.
 */
export const browseRoute = createRoute({
  getParentRoute: () => rootRoute,
  id: "browse",
  validateSearch: (search: Record<string, unknown>): { q?: string } => {
    const q = typeof search.q === "string" ? search.q.trim() : ""
    return q ? { q } : {}
  },
  component: Browse,
})
