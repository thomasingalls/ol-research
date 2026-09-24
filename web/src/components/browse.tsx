import { useEffect, useLayoutEffect, useRef } from "react"
import { Outlet, useParams, useRouter } from "@tanstack/react-router"
import { cn } from "@/lib/utils"
import { isWide } from "@/lib/wide"
import { SearchPanel } from "@/components/search-panel"

/**
 * The search and whatever work is open, laid out for the width there is.
 *
 * With no work open the search panel is the page. With one open, on a wide
 * screen, the panel is the left column and the work fills the rest; below
 * that width the work is a page of its own and the panel steps out of the
 * way. Either way the panel is the same element throughout, never rebuilt.
 *
 * Beside a work, the results still scroll with the page, as they did before
 * it opened. That keeps the router's own scroll memory in charge of the list,
 * and it is what lets a result clicked far down the page stay where it was.
 */
export function Browse() {
  const { workId } = useParams({ strict: false })
  const focused = workId !== undefined

  const panel = useRef<HTMLDivElement>(null)
  const clickedAt = useRef<number | null>(null)
  const wasFocused = useRef(focused)

  // Opening a work from the centred results narrows the panel, so its rows
  // wrap onto more lines and the one you clicked moves down the page. Scroll
  // by exactly that much, so the row stays under the pointer that clicked it.
  // A layout effect, so it lands before anything is painted — or pictured
  // for the transition — in between.
  useLayoutEffect(() => {
    const opened = focused && !wasFocused.current
    wasFocused.current = focused
    const before = clickedAt.current
    clickedAt.current = null
    if (!opened || before === null || !isWide()) return

    const row = panel.current?.querySelector('a[data-status="active"]')
    if (row) window.scrollBy(0, row.getBoundingClientRect().top - before)
  }, [focused])

  // How far the panel's top edge moves on screen, for index.css. Opening a
  // work, it moves up by however much the rows grew above the one clicked;
  // going back, by however far the router's scroll memory takes the list.
  // The transition moves the panel's box that far, and the pictures in it
  // are held against it, so the rows move sideways and not up or down.
  //
  // Measured before the router starts on the new page, and again once it
  // has rendered it and restored any scroll: both inside the transition's
  // window, before either picture is taken.
  const router = useRouter()
  useEffect(() => {
    let top = 0
    const measure = () => panel.current?.getBoundingClientRect().top ?? 0
    const before = router.subscribe("onBeforeLoad", () => {
      top = measure()
    })
    const after = router.subscribe("onRendered", () => {
      document.documentElement.style.setProperty(
        "--vt-shift",
        `${top - measure()}px`,
      )
    })
    return () => {
      before()
      after()
    }
  }, [router])

  return (
    <div
      className={cn(
        focused &&
          "xl:grid xl:grid-cols-[20rem_minmax(0,1fr)] xl:gap-8 2xl:grid-cols-[24rem_minmax(0,1fr)]",
      )}
    >
      <div
        ref={panel}
        data-vt="search"
        className={cn(focused && "hidden xl:block")}
        onClickCapture={(event) => {
          // Only a plain click navigates here; a modified one opens a tab.
          const plain =
            event.button === 0 &&
            !(event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
          const row = plain ? (event.target as Element).closest("a") : null
          clickedAt.current = row ? row.getBoundingClientRect().top : null
        }}
      >
        <SearchPanel focused={focused} />
      </div>
      <Outlet />
    </div>
  )
}
