import path from "node:path"
import { defineConfig } from "vite"
import react from "@vitejs/plugin-react"
import tailwindcss from "@tailwindcss/vite"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      "/v1/chat": {
        target: "http://127.0.0.1:50053",
        // The response stays streamed. Aborting it must also close the
        // proxy's upstream request, including while waiting for headers.
        configure(proxy) {
          proxy.on("proxyReq", (proxyReq, _req, res) => {
            res.on("close", () => proxyReq.destroy())
          })
        },
      },
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
})
