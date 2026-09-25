import { ArrowUpRightIcon, ChevronDownIcon } from "lucide-react"
import { nameUrl, type Name } from "@/api"
import { Button } from "@/components/ui/button"
import {
  ButtonGroup,
  ButtonGroupSeparator,
} from "@/components/ui/button-group"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"

/**
 * The places a work can be read, one per namespace whose promoted name has a
 * page elsewhere, in the order they are offered: the publisher's version of
 * record first, then PubMed Central's free full text, PubMed's abstract, and
 * OpenAlex's metadata.
 */
const PLACES = [
  { namespace: "doi", label: "Publisher" },
  { namespace: "pmc", label: "PubMed Central" },
  { namespace: "pmid", label: "PubMed" },
  { namespace: "openalex", label: "OpenAlex" },
] as const

type Place = { namespace: string; label: string; url: string }

function places(names: Name[]): Place[] {
  return PLACES.flatMap(({ namespace, label }) => {
    const url = names
      .filter((n) => n.namespace === namespace)
      .map(nameUrl)
      .find((u) => u !== null)
    return url ? [{ namespace, label, url }] : []
  })
}

/** Every way off this site opens a new tab and sends no referrer. */
const OUT = { target: "_blank", rel: "noopener noreferrer" } as const

/**
 * A split button. Its face is the first place the work has — the publisher,
 * when it has a DOI — and the menu beside it holds the rest. Each is a link:
 * choosing one in the menu goes there, as the face does. A work with one
 * place has no menu, and a work none of whose names has a page elsewhere
 * has nowhere to read, and shows no button.
 */
export function ReadButton({ names }: { names: Name[] }) {
  const [first, ...rest] = places(names)
  if (!first) return null

  return (
    <ButtonGroup>
      <Button asChild>
        <a
          href={first.url}
          {...OUT}
          title={`Opens ${new URL(first.url).host} in a new tab`}
        >
          {first.label}
          <ArrowUpRightIcon data-icon="inline-end" aria-hidden />
        </a>
      </Button>
      {rest.length > 0 && (
        <>
          <ButtonGroupSeparator />
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button size="icon" aria-label="Other places to read it">
                <ChevronDownIcon />
              </Button>
            </DropdownMenuTrigger>
            {/* Opaque, where the style's menus are translucent — 70% of the
                popover colour over a blur, which read as no background. As
                wide as its places, not as the trigger, which is an icon. */}
            <DropdownMenuContent align="end" className="bg-popover w-auto">
              {rest.map((p) => (
                <DropdownMenuItem key={p.namespace} asChild>
                  <a href={p.url} {...OUT}>
                    {p.label}
                    <ArrowUpRightIcon className="ml-auto" aria-hidden />
                  </a>
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </>
      )}
    </ButtonGroup>
  )
}
