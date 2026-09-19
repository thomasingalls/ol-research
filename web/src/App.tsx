import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { RouterProvider } from "@tanstack/react-router"
import { ApiError } from "@/api"
import { router } from "@/router"

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // The corpus only changes when a merge publishes an epoch, so nothing
      // we fetch goes stale mid-session. Don't refetch behind the user's back.
      refetchOnWindowFocus: false,
      staleTime: 5 * 60 * 1000,
      // A refusal is a decision, not a hiccup: the server has told us why,
      // and asking again cannot change the answer. Only a server we never
      // reached, or one that broke, is worth a second try.
      retry: (count, error) => {
        if (error instanceof ApiError && !error.unreachable) {
          return error.code === "internal" && count < 1
        }
        return count < 1
      },
    },
  },
})

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  )
}
