import { AssistantRuntimeProvider, useAuiState, useLocalRuntime } from "@assistant-ui/react"
import { useEffect, useRef, useState, useSyncExternalStore } from "react"
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query"
import { useBlocker } from "@tanstack/react-router"
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
import { Button } from "@/components/ui/button"
import { ChatHistoryStore, createSession, decodeHistory, listSessions, loadHistory } from "@/lib/chat-history"
import type { HistoryResponse } from "@/generated/chat"
import { chatRoute } from "@/routes/chat"

export function Chat() {
  const { session } = chatRoute.useSearch()
  const navigate = chatRoute.useNavigate()
  const client = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [creating, setCreating] = useState(false)
  const [createError, setCreateError] = useState<string>()
  const creation = useRef<{ id: string, title: string } | undefined>(undefined)
  const sessions = useInfiniteQuery({
    queryKey: ["chat-sessions"], initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) => listSessions(pageParam),
    getNextPageParam: page => page.next_cursor ?? undefined,
  })
  const history = useQuery({
    queryKey: ["chat-history", session], enabled: !!session, retry: false,
    refetchOnWindowFocus: false, refetchOnReconnect: false, refetchOnMount: "always",
    queryFn: async () => {
      const loaded = await loadHistory(session!)
      decodeHistory(loaded.history)
      if (!Number.isSafeInteger(loaded.revision) || loaded.revision < 0) throw new Error("Invalid history revision.")
      return loaded
    },
  })
  const choices = sessions.data?.pages.flatMap(page => page.sessions).sort((a, b) => b.created_at.localeCompare(a.created_at)) ?? []

  async function newChat() {
    setCreating(true)
    setCreateError(undefined)
    creation.current ??= { id: crypto.randomUUID(), title: `Chat · ${new Date().toLocaleString()}` }
    try {
      const created = await createSession(creation.current.id, creation.current.title)
      creation.current = undefined
      await client.invalidateQueries({ queryKey: ["chat-sessions"] })
      await navigate({ search: { session: created.id } })
    } catch (error) {
      setCreateError(error instanceof Error ? error.message : String(error))
    } finally { setCreating(false) }
  }

  return (
    <section aria-label="Chat" className="flex h-[calc(100dvh-12rem)] min-h-[40rem] flex-col xl:h-[calc(100dvh-7rem)]">
      <PublicChatNotice />
      <div className="flex flex-wrap items-center gap-2 border-b px-4 py-2">
        <label className="sr-only" htmlFor="chat-session">Conversation</label>
        <select id="chat-session" className="min-w-0 max-w-72 rounded-lg border bg-background px-2 py-1 text-sm"
          value={session ?? ""} disabled={busy || creating}
          onChange={event => void navigate({ search: { session: event.target.value || undefined } })}>
          <option value="">Choose a conversation</option>
          {session && !choices.some(choice => choice.id === session) && <option value={session}>Current conversation</option>}
          {choices.map(choice => <option key={choice.id} value={choice.id}>{choice.title}</option>)}
        </select>
        <Button variant="outline" disabled={busy || creating} onClick={() => void newChat()}>{creating ? "Creating…" : createError ? "Retry new chat" : "New chat"}</Button>
        {sessions.hasNextPage && <Button variant="ghost" disabled={sessions.isFetchingNextPage} onClick={() => void sessions.fetchNextPage()}>More conversations</Button>}
        <span className="text-muted-foreground ml-auto text-xs">Corpus tools</span>
      </div>
      {(sessions.error || createError) && <div role="alert" className="px-4 py-2 text-sm">
        {createError ?? sessions.error?.message}
        {sessions.error && <Button variant="link" onClick={() => void sessions.refetch()}>Retry list</Button>}
      </div>}
      {!session ? <ChatWelcome /> : history.isFetching ? <p role="status" className="p-4 text-sm">Loading conversation…</p>
        : history.error ? <div role="alert" className="p-4 text-sm">{history.error.message}<Button variant="link" onClick={() => void history.refetch()}>Retry load</Button></div>
        : history.data ? <SavedChat key={session} session={session} initial={history.data} onBusy={setBusy} reload={() => void history.refetch()} /> : null}
    </section>
  )
}

function PublicChatNotice() {
  return <aside aria-labelledby="public-chat-notice" className="shrink-0 border-2 border-amber-600 bg-amber-50 px-4 py-3 text-sm text-amber-950 dark:border-amber-400 dark:bg-amber-950/40 dark:text-amber-100">
    <h2 id="public-chat-notice" className="text-base font-bold">Public shared demo — your chats are visible to everyone</h2>
    <p className="mt-1 font-semibold">Anyone can read, continue or change any conversation. Do not enter private, personal or confidential information.</p>
    <p className="mt-2">The hosted demo uses NVIDIA’s free Nemotron endpoint through OpenRouter. NVIDIA’s notice:</p>
    <blockquote className="mt-1 border-l-2 border-current pl-3">
      <p>For the free endpoint, please do not upload any confidential information or personal data (such as voices or faces of people). Your use is logged for security purposes and to improve NVIDIA products and services. The logged session data for improvement purposes is not linked to your identity or any persistent identifier. For more information about NVIDIA&apos;s data processing practices, see <a className="underline underline-offset-2" href="https://www.nvidia.com/en-us/about-nvidia/privacy-policy/" target="_blank" rel="noreferrer">Privacy Policy</a>. By using this free endpoint, you consent to NVIDIA&apos;s collection, recording, and use of such information and the <a className="underline underline-offset-2" href="https://assets.ngc.nvidia.com/products/api-catalog/legal/NVIDIA%20API%20Trial%20Terms%20of%20Service.pdf" target="_blank" rel="noreferrer">NVIDIA API Trial Terms of Service</a>.</p>
    </blockquote>
    <p className="mt-2">Replies and saves have shared usage limits. <a className="underline underline-offset-2" href="https://openrouter.ai/privacy" target="_blank" rel="noreferrer">OpenRouter’s privacy policy</a> also applies. NVIDIA’s statement about identifiers does not make conversations on this site private.</p>
  </aside>
}

function SavedChat({ session, initial, onBusy, reload }: {
  session: string, initial: HistoryResponse, onBusy: (busy: boolean) => void, reload: () => void,
}) {
  const [history] = useState(() => new ChatHistoryStore(session, initial))
  const [model] = useState(() => history.modelAdapter(chatAdapter))
  const runtime = useLocalRuntime(model, { adapters: { history: history.adapter } })
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <HistoryStatus history={history} onBusy={onBusy} reload={reload} />
      <div className="min-h-0 flex-1">
        <Thread components={{ Welcome: ChatWelcome, ToolFallback: ChatTool, ToolGroup: ChatToolGroup }} />
      </div>
    </AssistantRuntimeProvider>
  )
}

function HistoryStatus({ history, onBusy, reload }: { history: ChatHistoryStore, onBusy: (busy: boolean) => void, reload: () => void }) {
  const state = useSyncExternalStore(history.subscribe, history.getSnapshot)
  const running = useAuiState(s => s.thread.isRunning)
  const loading = useAuiState(s => s.thread.isLoading)
  const head = useAuiState(s => s.thread.messages.at(-1)?.id ?? null)
  const busy = running || state.phase !== "saved"
  useEffect(() => { onBusy(busy); return () => onBusy(false) }, [busy, onBusy])
  useEffect(() => { if (!running && !loading) history.selectHead(head) }, [history, head, running, loading])
  const blocker = useBlocker({ shouldBlockFn: () => busy, enableBeforeUnload: busy, withResolver: true })

  function download() {
    const url = URL.createObjectURL(new Blob([history.export()], { type: "application/json" }))
    const anchor = document.createElement("a")
    anchor.href = url
    anchor.download = "chat-history.json"
    anchor.click()
    setTimeout(() => URL.revokeObjectURL(url), 0)
  }

  return <div className="px-4 py-2 text-sm">
    {state.phase === "error" ? <div role="alert" className="flex flex-wrap items-center gap-2">
      <span>Not saved: {state.message}</span>
      {!state.conflict && <Button variant="outline" onClick={() => void history.retry().catch(() => {})}>Retry save</Button>}
      <Button variant="outline" onClick={download}>Export local history</Button>
      <Button variant="ghost" disabled={running} onClick={reload}>Discard local changes and reload</Button>
    </div> : <p role="status" className="text-muted-foreground text-center text-xs">
      {running ? "Reply in progress · saved when finished or stopped" : state.phase === "saving" ? "Saving…" : "Saved"}
    </p>}
    {blocker.status === "blocked" && <div role="alert" className="mt-2 flex flex-wrap items-center gap-2">
      <span>This conversation has a reply in progress or unsaved changes. Leaving may lose them.</span>
      <Button variant="outline" onClick={blocker.reset}>Stay</Button>
      <Button variant="ghost" onClick={blocker.proceed}>Leave anyway</Button>
    </div>}
  </div>
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
          Explore the literature corpus in a public, shared conversation.
          Search for papers or look one up by ID for a citation and reading links.
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  )
}
