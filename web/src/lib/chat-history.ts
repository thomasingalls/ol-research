import type {
  ChatModelAdapter, ExportedMessageRepository, ExportedMessageRepositoryItem,
  ThreadAssistantMessagePart, ThreadHistoryAdapter,
} from "@assistant-ui/react"
import type {
  ChatHistory, ChatSession, HistoryResponse, SavedMessage, SavedRevision,
  SavedStatus, SaveHistory, SessionPage,
} from "@/generated/chat"
import { toChatMessage } from "./chat-adapter.ts"

export class HistoryError extends Error {
  readonly conflict: boolean
  constructor(message: string, conflict = false) {
    super(message)
    this.conflict = conflict
  }
}

async function request<T>(url: string, body?: unknown): Promise<T> {
  let response: Response
  try {
    response = await fetch(url, {
      method: body === undefined ? "GET" : "PUT",
      headers: { Accept: "application/json", ...(body === undefined ? {} : { "Content-Type": "application/json" }) },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
      signal: AbortSignal.timeout(15_000),
    })
  } catch {
    throw new HistoryError("Could not reach chat storage. Start the database and chat service, then retry.")
  }
  const data = await response.json().catch(() => null)
  if (!response.ok) {
    throw new HistoryError(data?.message ?? `Chat storage returned HTTP ${response.status}.`, response.status === 409)
  }
  if (data === null) throw new HistoryError("Chat storage returned an invalid response.")
  return data as T
}

const path = (id: string) => `/v1/chat/sessions/${encodeURIComponent(id)}`
export const listSessions = (after?: string) => request<SessionPage>(`/v1/chat/sessions${after ? `?after=${encodeURIComponent(after)}` : ""}`)
export const createSession = (id: string, title: string) => request<ChatSession>(path(id), { title })
export const loadHistory = (id: string) => request<HistoryResponse>(`${path(id)}/history`)

function errorText(error: unknown): string | null {
  if (typeof error === "string") return error
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message
  return error == null ? null : JSON.stringify(error)
}

export function encodeMessage({ message, parentId }: ExportedMessageRepositoryItem): SavedMessage {
  let status: SavedStatus | null = null
  if (message.role === "assistant") {
    const s = message.status
    if (s.type === "complete") status = s
    else if (s.type === "incomplete" && ["length", "cancelled", "error"].includes(s.reason)) {
      status = { type: "incomplete", reason: s.reason as "length" | "cancelled" | "error", error: errorText(s.error) }
    } else {
      throw new Error("Only completed or stopped replies can be saved by this history format.")
    }
  }
  const custom = message.metadata.custom
  return {
    message: toChatMessage(message), parent_id: parentId, created_at: message.createdAt.toISOString(), status,
    model_message_id: typeof custom.messageId === "string" ? custom.messageId : null,
    part_ids: Array.isArray(custom.partIds) && custom.partIds.every(id => typeof id === "string") ? custom.partIds : [],
  }
}

export function decodeHistory(history: ChatHistory): ExportedMessageRepository {
  const ids = new Set<string>()
  const parents = new Set<string>()
  const messages = history.messages.map((item): ExportedMessageRepositoryItem => {
    const { message, parent_id: parentId } = item
    if (!message.id || ids.has(message.id) || (parentId !== null && !ids.has(parentId))) throw new Error("Invalid stored conversation tree.")
    ids.add(message.id)
    if (parentId !== null) parents.add(parentId)
    const createdAt = new Date(item.created_at)
    if (!Number.isFinite(createdAt.valueOf())) throw new Error("Invalid stored message date.")
    const custom = { ...(item.model_message_id ? { messageId: item.model_message_id } : {}), partIds: item.part_ids }
    const common = { id: message.id, createdAt }
    const content: ThreadAssistantMessagePart[] = message.content.map(part => part.type === "tool_call"
      ? { type: "tool-call", toolCallId: part.tool_call_id, toolName: part.call.name, args: part.call.args,
          argsText: JSON.stringify(part.call.args), ...(part.result === null ? {} : { result: part.result.data, isError: part.result.data.status === "error" }) }
      : part)
    if (message.role === "user") {
      if (item.status !== null || content.some(part => part.type !== "text")) throw new Error("Invalid stored user message.")
      return { parentId, message: { ...common, role: "user", content: content as { type: "text", text: string }[], attachments: [], metadata: { custom } } }
    }
    if (message.role !== "assistant" || !item.status) throw new Error("Invalid stored assistant message.")
    const status = item.status.type === "complete" ? item.status : {
      type: "incomplete" as const, reason: item.status.reason,
      ...(item.status.error === null ? {} : { error: { code: "unknown", message: item.status.error } }),
    }
    return { parentId, message: { ...common, role: "assistant", content, status,
      metadata: { custom, unstable_state: null, unstable_annotations: [], unstable_data: [], steps: [] } } }
  })
  if (history.head_id === null ? messages.length !== 0 : !ids.has(history.head_id) || parents.has(history.head_id)) throw new Error("Invalid stored conversation head.")
  return { headId: history.head_id, messages }
}

export type SaveState = { phase: "saved" | "saving" | "error", message?: string, conflict?: boolean }

/** One serialized write queue per mounted conversation. A failed commit retains
 * its exact ID and bytes for retry, even if further local changes have arrived. */
export class ChatHistoryStore {
  private id: string
  private revision: number
  private history: ChatHistory
  private saved: string
  private pending: SaveHistory | undefined
  private inFlight: Promise<void> | undefined
  private failure: Error | undefined
  private state: SaveState = { phase: "saved" }
  private listeners = new Set<() => void>()
  readonly adapter: ThreadHistoryAdapter

  constructor(id: string, initial: HistoryResponse) {
    const repository = decodeHistory(initial.history)
    if (!Number.isSafeInteger(initial.revision) || initial.revision < 0) throw new Error("Invalid history revision.")
    this.id = id
    this.revision = initial.revision
    this.history = initial.history
    this.saved = JSON.stringify(this.history)
    this.adapter = {
      load: async () => repository,
      append: item => this.append(item),
      update: item => this.append(item),
    }
  }

  subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener) } }
  getSnapshot = () => this.state
  export = () => JSON.stringify({ format: "ol-rht-chat-history", format_version: "1", history: this.history }, null, 2)

  private setState(state: SaveState) {
    this.state = state
    this.listeners.forEach(listener => listener())
  }

  private async append(item: ExportedMessageRepositoryItem) {
    try {
      const saved = encodeMessage(item)
      const messages = [...this.history.messages]
      const index = messages.findIndex(m => m.message.id === saved.message.id)
      if (index < 0) messages.push(saved)
      else messages[index] = saved
      this.history = { messages, head_id: saved.message.id }
      await this.flush()
    } catch (error) {
      this.fail(error)
      throw error
    }
  }

  selectHead(id: string | null) {
    if (id === this.history.head_id || !this.history.messages.some(m => m.message.id === id)) return
    this.history = { ...this.history, head_id: id }
    void this.flush().catch(() => {})
  }

  private fail(error: unknown) {
    this.failure = error instanceof Error ? error : new Error(String(error))
    this.setState({ phase: "error", message: this.failure.message, conflict: error instanceof HistoryError && error.conflict })
  }

  async flush(): Promise<void> {
    if (this.failure) throw this.failure
    if (this.inFlight) {
      await this.inFlight
      // A caller may have changed the tree just as the previous write settled.
      return this.flush()
    }
    if (this.saved === JSON.stringify(this.history)) return
    this.inFlight = this.write().catch(error => { this.fail(error); throw error }).finally(() => { this.inFlight = undefined })
    await this.inFlight
    return this.flush()
  }

  private async write() {
    this.setState({ phase: "saving" })
    while (this.pending || this.saved !== JSON.stringify(this.history)) {
      this.pending ??= { checkpoint_id: crypto.randomUUID(), expected_revision: this.revision, history: this.history }
      const committed = await request<SavedRevision>(`${path(this.id)}/history`, this.pending)
      if (!Number.isSafeInteger(committed.revision) || committed.revision !== this.pending.expected_revision + 1) throw new Error("Invalid saved history revision.")
      this.revision = committed.revision
      this.saved = JSON.stringify(this.pending.history)
      this.pending = undefined
    }
    this.setState({ phase: "saved" })
  }

  retry = async () => {
    if (this.state.conflict || (this.failure && !this.pending)) throw this.failure
    this.failure = undefined
    await this.flush()
  }

  /** assistant-ui starts history.append and run concurrently. Explicitly gate
   * the model on history so a failed user-message save never starts tool work. */
  modelAdapter(model: ChatModelAdapter): ChatModelAdapter {
    const history = this
    return {
      async *run(options) {
        await history.flush()
        options.abortSignal.throwIfAborted()
        const result = model.run(options)
        if (Symbol.asyncIterator in result) yield* result
        else yield await result
      },
    }
  }
}
