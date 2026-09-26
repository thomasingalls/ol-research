import { createRoute, lazyRouteComponent } from "@tanstack/react-router"
import { rootRoute } from "@/routes/root"

export const chatRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat",
  component: lazyRouteComponent(() => import("@/components/chat"), "Chat"),
})
