import { createRoute, redirect } from "@tanstack/react-router"
import { rootRoute } from "@/routes/root"

/**
 * `/resolve?text=…`, Find by ID's address until 2026-09-28, when it and Find
 * by citation became one view, `/find`, which reads every identifier the old
 * page did and goes straight to the paper when that is all it was given.
 * Kept so that a link to it still lands, on the same text. The API's own
 * `/v1/resolve` is unchanged, for any client that wants a name and nothing
 * else.
 */
export const resolveRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/resolve",
  validateSearch: (search: Record<string, unknown>): { text?: string } => {
    const text = typeof search.text === "string" ? search.text.trim() : ""
    return text ? { text } : {}
  },
  beforeLoad: ({ search }) => {
    throw redirect({ to: "/find", search, replace: true })
  },
})
