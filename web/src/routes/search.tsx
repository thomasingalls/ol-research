import { createRoute } from "@tanstack/react-router"
import { browseRoute } from "@/routes/browse"

/**
 * `/` and `/?q=…`: the search on its own, centred. There is nothing here but
 * the path — the panel, and the `q` it reads, belong to the layout above, so
 * that they are still there when a work opens beside them.
 */
export const searchRoute = createRoute({
  getParentRoute: () => browseRoute,
  path: "/",
})
