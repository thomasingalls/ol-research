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
          Local model · No corpus connection · Messages aren’t saved
        </p>
        <div className="min-h-0 flex-1">
          <Thread components={{ Welcome: ChatWelcome }} />
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
          Chat with the local development model. It has no access to the
          literature database yet.
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  )
}
