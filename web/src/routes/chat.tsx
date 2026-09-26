import { createRoute, lazyRouteComponent } from "@tanstack/react-router"
import { rootRoute } from "@/routes/root"

export const chatRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat",
  validateSearch: (search: Record<string, unknown>): { session?: string } => ({
    session: typeof search.session === "string" ? search.session : undefined,
  }),
  component: lazyRouteComponent(() => import("@/components/chat"), "Chat"),
})
