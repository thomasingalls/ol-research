import { useEffect, useState } from "react"
import { CheckIcon, ChevronDownIcon, CopyIcon, XIcon } from "lucide-react"
import { nameRank, type Name, type PublicId } from "@/api"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

/** The last copy, and whether the clipboard took it. */
type Copied = { text: string; ok: boolean }

/** How long a copy's check (or cross) stays before the copy icon returns. */
const SHOWN_MS = 1500

/** What each namespace is called beside its value. */
const LABELS: Record<string, string> = {
  doi: "DOI",
  pmid: "PMID",
  pmc: "PMC",
  openalex: "OpenAlex",
}

/** One line of the list: what it is called, and what a click copies. */
type Entry = { label: string; text: string; name: Name | null }

/**
 * Every id a work goes by, in one list, each a click to copy. The trigger is
 * a ghost button showing the first of them — a DOI if there is one — and
 * the list holds all of them, the public id last.
 *
 * A name is copied without its namespace — `10.1234/x`, `12345678`,
 * `PMC3737249` — which is what a reference manager or PubMed's search box
 * takes; the namespace is its label. The public id is copied whole: `ol:` is
 * part of it, not a namespace, and the UI never takes one apart.
 *
 * The list stays open after a copy, so that the check lands beside what was
 * clicked and another can follow.
 */
export function CopyNames({
  names,
  publicId,
}: {
  names: Name[]
  publicId: PublicId
}) {
  const [copied, setCopied] = useState<Copied | null>(null)

  useEffect(() => {
    if (copied === null) return
    const shown = setTimeout(() => setCopied(null), SHOWN_MS)
    return () => clearTimeout(shown)
  }, [copied])

  const entries: Entry[] = [
    ...[...names]
      .sort((a, b) => nameRank(a) - nameRank(b))
      .map((n) => ({
        label: LABELS[n.namespace] ?? n.namespace,
        text: n.value,
        name: n,
      })),
    { label: "Public id", text: publicId, name: null },
  ]
  const [lead] = entries

  // Without a secure origin there is no `navigator.clipboard` at all; going
  // through a promise makes that a failed copy like any other.
  const copy = (text: string) =>
    Promise.resolve()
      .then(() => navigator.clipboard.writeText(text))
      .then(
        () => setCopied({ text, ok: true }),
        () => setCopied({ text, ok: false }),
      )

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="ghost"
            size="sm"
            className="max-w-full min-w-0"
            aria-label={`Identifiers: ${lead.label} ${lead.text}`}
          >
            <span className="text-muted-foreground">{lead.label}</span>
            {/* A DOI can run to 70 characters; the button ends in an
                ellipsis before it runs off the card. The list has it all. */}
            <span className="truncate font-mono text-xs">{lead.text}</span>
            <ChevronDownIcon data-icon="inline-end" aria-hidden />
          </Button>
        </DropdownMenuTrigger>
        {/* Opaque, as the Read button's menu is, and no wider than the
            screen: a long DOI wraps rather than running off it. */}
        <DropdownMenuContent
          align="start"
          className="bg-popover w-auto max-w-[calc(100vw-2rem)]"
        >
          {entries.map((e) => {
            const Mark =
              copied?.text !== e.text ? CopyIcon : copied.ok ? CheckIcon : XIcon
            return (
              <DropdownMenuItem
                key={`${e.label}:${e.text}`}
                title={`${e.name ? describe(e.name) : "this site's id"} · click to copy`}
                onSelect={(event) => {
                  event.preventDefault()
                  void copy(e.text)
                }}
              >
                <span className="text-muted-foreground w-16 shrink-0 text-xs">
                  {e.label}
                </span>
                <span className="font-mono text-xs break-all">{e.text}</span>
                {/* An alias may be a version of the work, and a shared
                    name names another work too: said, not only hovered. */}
                {e.name && e.name.standing !== "promoted" && (
                  <span className="text-muted-foreground text-xs">
                    {e.name.standing}
                  </span>
                )}
                <Mark className="ml-auto" aria-hidden />
              </DropdownMenuItem>
            )
          })}
        </DropdownMenuContent>
      </DropdownMenu>
      <span className="sr-only" aria-live="polite">
        {copied === null
          ? ""
          : copied.ok
            ? `Copied ${copied.text}`
            : `Could not copy ${copied.text}`}
      </span>
    </>
  )
}

/** A name's standing and who asserted it, as its tooltip. */
function describe(n: Name): string {
  return `${n.standing}${n.by.length ? ` · asserted by ${n.by.join(", ")}` : ""}`
}
