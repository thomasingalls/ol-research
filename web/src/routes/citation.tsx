import { createRoute, redirect } from "@tanstack/react-router"
import { rootRoute } from "@/routes/root"

/**
 * `/citation?text=…`, Find by citation's address until 2026-09-28, when it
 * and Find by ID became one view, `/find`. Kept so that a link to it still
 * lands, on the same text.
 */
export const citationRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/citation",
  validateSearch: (search: Record<string, unknown>): { text?: string } => {
    const text = typeof search.text === "string" ? search.text.trim() : ""
    return text ? { text } : {}
  },
  beforeLoad: ({ search }) => {
    throw redirect({ to: "/find", search, replace: true })
  },
})
