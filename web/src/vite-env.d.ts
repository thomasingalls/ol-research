/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Base URL of the ol-rht HTTP API, e.g. http://localhost:8080 */
  readonly VITE_API_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
