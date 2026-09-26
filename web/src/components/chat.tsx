import { AssistantRuntimeProvider, useLocalRuntime } from "@assistant-ui/react"
import { MessagesSquareIcon } from "lucide-react"
import { Thread } from "@/components/assistant-ui/elements/thread.aui"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { chatAdapter } from "@/lib/chat-adapter"
import { ChatTool, ChatToolGroup } from "@/components/chat-tools"

/** A route-scoped runtime: leaving Chat discards its in-memory conversation. */
export function Chat() {
  const runtime = useLocalRuntime(chatAdapter)

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <section
        aria-label="Chat"
        className="flex h-[calc(100dvh-12rem)] min-h-80 flex-col xl:h-[calc(100dvh-7rem)]"
      >
        <p className="text-muted-foreground px-4 py-2 text-center text-xs">
          Local model · Corpus tools · Messages aren’t saved
        </p>
        <div className="min-h-0 flex-1">
          <Thread components={{ Welcome: ChatWelcome, ToolFallback: ChatTool, ToolGroup: ChatToolGroup }} />
        </div>
      </section>
    </AssistantRuntimeProvider>
  )
}

function ChatWelcome() {
  return (
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <MessagesSquareIcon />
        </EmptyMedia>
        <EmptyTitle>Chat</EmptyTitle>
        <EmptyDescription>
          Find papers in the literature corpus with the local development model.
          Search for papers or look one up by ID for a citation and reading links.
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  )
}
