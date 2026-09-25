import { useState } from "react"
import { createRoute } from "@tanstack/react-router"
import { ArrowUpIcon, MessagesSquareIcon } from "lucide-react"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupTextarea,
} from "@/components/ui/input-group"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { rootRoute } from "@/routes/root"

/**
 * `/chat`. The shell of a chat screen, with nothing behind it yet: what you
 * send is shown, and nothing answers, and the page says so rather than
 * pretending. The conversation lives in this component, not the URL, and is
 * gone when you leave the tab.
 */
export const chatRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/chat",
  component: Chat,
})

function Chat() {
  const [sent, setSent] = useState<string[]>([])
  const [draft, setDraft] = useState("")

  const send = () => {
    const text = draft.trim()
    if (text.length === 0) return
    setSent((prev) => [...prev, text])
    setDraft("")
  }

  return (
    <div className="space-y-6">
      {sent.length === 0 ? (
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <MessagesSquareIcon />
            </EmptyMedia>
            <EmptyTitle>Chat</EmptyTitle>
            <EmptyDescription>
              No agent is connected yet, so nothing will answer.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <>
          <ol className="space-y-3">
            {sent.map((text, i) => (
              <li
                key={i}
                className="bg-muted ml-auto w-fit max-w-[80%] rounded-2xl px-4 py-2 text-sm whitespace-pre-wrap"
              >
                {text}
              </li>
            ))}
          </ol>
          <p className="text-muted-foreground text-sm">
            Nothing answers yet: no agent is connected.
          </p>
        </>
      )}

      {/* Stays at the bottom of the screen as the conversation grows. Enter
          sends, Shift+Enter is a new line, and neither fires mid-composition
          in an input method. */}
      <form
        className="bg-background sticky bottom-0 pb-4"
        onSubmit={(event) => {
          event.preventDefault()
          send()
        }}
      >
        <InputGroup>
          <InputGroupTextarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (
                event.key === "Enter" &&
                !event.shiftKey &&
                !event.nativeEvent.isComposing
              ) {
                event.preventDefault()
                send()
              }
            }}
            placeholder="Ask about the literature…"
            autoFocus
          />
          <InputGroupAddon align="block-end">
            <InputGroupButton
              type="submit"
              variant="default"
              size="icon-xs"
              className="ml-auto"
              disabled={draft.trim().length === 0}
              aria-label="Send"
            >
              <ArrowUpIcon />
            </InputGroupButton>
          </InputGroupAddon>
        </InputGroup>
      </form>
    </div>
  )
}
