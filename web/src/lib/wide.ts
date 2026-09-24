import { useSyncExternalStore } from "react"

/**
 * Tailwind's `xl`. From this width up, a work opens beside the search rather
 * than in place of it; below it, the work is a page of its own. The CSS says
 * `xl:`, and anything in script that has to agree with it asks here.
 */
const wide = window.matchMedia("(min-width: 80rem)")

export const isWide = () => wide.matches

export function useWide(): boolean {
  return useSyncExternalStore((onChange) => {
    wide.addEventListener("change", onChange)
    return () => wide.removeEventListener("change", onChange)
  }, isWide)
}
