import { queryOptions } from "@tanstack/react-query"
import { api, type PublicId } from "@/api"

/**
 * A work, as the work page reads it and as a row warms it before the click.
 * One key and one function for both, so the page finds exactly what the row
 * fetched, and the two cannot drift apart.
 *
 * Warming asks for nothing twice. A work already cached and fresh (see
 * `staleTime` in App.tsx) is not asked for again, and one already on its way
 * is waited for, not asked for a second time.
 */
export const workQuery = (id: PublicId) =>
  queryOptions({ queryKey: ["work", id], queryFn: () => api.work(id) })
