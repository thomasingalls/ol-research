import type { ChatModelAdapter, ThreadAssistantMessagePart, ThreadMessage } from "@assistant-ui/react"
import type { ChatEvent, ChatMessage, ChatRequest, ContentPart, SearchArgs, SearchResult, PaperArgs, PaperResult, ToolCall, ToolResult } from "@/generated/chat"

/** Read records independently of HTTP chunk boundaries, including split UTF-8. */
export async function* readChatEvents(body: ReadableStream<Uint8Array>) {
  const reader = body.getReader()
  const decoder = new TextDecoder("utf-8", { fatal: true })
  let buffer = ""
  try {
    while (true) {
      const { done, value } = await reader.read()
      buffer += decoder.decode(value, { stream: !done })
      let end: number
      while ((end = buffer.indexOf("\n")) !== -1) {
        if (end > 256 * 1024) throw new Error("Chat event exceeds the size limit.")
        const line = buffer.slice(0, end).trim()
        buffer = buffer.slice(end + 1)
        if (line) yield parseEvent(line)
      }
      if (buffer.length > 256 * 1024) throw new Error("Chat event exceeds the size limit.")
      if (done) {
        if (buffer.trim()) throw new Error("Chat stream ended inside an event.")
        return
      }
    }
  } finally {
    await reader.cancel().catch(() => {})
    reader.releaseLock()
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function strings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string")
}

function count(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0
}

export function isSearchArgs(value: unknown): value is SearchArgs {
  return isRecord(value) && typeof value.query === "string" && !!value.query.trim() &&
    new TextEncoder().encode(value.query).length <= 512 && count(value.limit) && value.limit >= 1 && value.limit <= 10
}

export function isPaperArgs(value: unknown): value is PaperArgs {
  return isRecord(value) && typeof value.id === "string" && !!value.id.trim() &&
    new TextEncoder().encode(value.id).length <= 2048
}

function isProvenance(value: unknown): boolean {
  return value === null || (isRecord(value) && count(value.epoch) && count(value.batch) &&
    typeof value.at === "string" && strings(value.sources) && strings(value.attribution))
}

export function isPaperResult(value: unknown): value is PaperResult {
  if (!isRecord(value)) return false
  if (value.status === "error") return typeof value.message === "string"
  if (value.status !== "ok" || !isProvenance(value.answered) || !isRecord(value.paper)) return false
  const p = value.paper
  return typeof p.public_id === "string" && !!p.public_id && typeof p.stub === "boolean" &&
    typeof p.title === "string" && strings(p.authors) && typeof p.authors_truncated === "boolean" &&
    typeof p.venue === "string" && typeof p.abstract_text === "string" && typeof p.abstract_truncated === "boolean" &&
    strings(p.retracted_by) && strings(p.corrected_by) && strings(p.concern_by) &&
    (p.open_access === null || typeof p.open_access === "boolean") &&
    (p.full_text_url === null || typeof p.full_text_url === "string") &&
    (p.publisher_url === null || typeof p.publisher_url === "string") &&
    (p.published === null || (isRecord(p.published) && Number.isSafeInteger(p.published.year) && count(p.published.month) && count(p.published.day))) &&
    Array.isArray(p.identifiers) && p.identifiers.every(n => isRecord(n) && typeof n.namespace === "string" && typeof n.value === "string" && typeof n.standing === "string")
}

function isToolCall(value: unknown): value is ToolCall {
  return isRecord(value) && ((value.name === "search" && isSearchArgs(value.args)) ||
    (value.name === "get_paper" && isPaperArgs(value.args)))
}

function isToolResult(value: unknown): value is ToolResult {
  return isRecord(value) && ((value.name === "search" && isSearchResult(value.data)) ||
    (value.name === "get_paper" && isPaperResult(value.data)))
}

export function isSearchResult(value: unknown): value is SearchResult {
  if (!isRecord(value)) return false
  if (value.status === "error") return typeof value.message === "string"
  if (value.status !== "ok" || typeof value.query !== "string" || !count(value.total) ||
    !Array.isArray(value.hits) || value.hits.length > 10) return false
  if (!isProvenance(value.answered)) return false
  return value.hits.every((hit) => isRecord(hit) && typeof hit.public_id === "string" && !!hit.public_id &&
    typeof hit.title === "string" && strings(hit.authors) && typeof hit.venue === "string" &&
    (hit.year === null || Number.isSafeInteger(hit.year)) &&
    strings(hit.retracted_by) && strings(hit.corrected_by) && strings(hit.concern_by))
}

function parseEvent(line: string): ChatEvent {
  const value: unknown = JSON.parse(line)
  if (typeof value !== "object" || value === null || !("type" in value)) {
    throw new Error("Invalid chat event.")
  }
  switch (value.type) {
    case "start":
      if ("message_id" in value && typeof value.message_id === "string" && value.message_id) {
        return { type: "start", message_id: value.message_id }
      }
      break
    case "delta":
      if (
        "part_id" in value && typeof value.part_id === "string" && value.part_id &&
        "kind" in value && (value.kind === "text" || value.kind === "reasoning") &&
        "text" in value && typeof value.text === "string"
      ) return { type: "delta", part_id: value.part_id, kind: value.kind, text: value.text }
      break
    case "tool_call":
      if ("part_id" in value && typeof value.part_id === "string" && value.part_id &&
        "call" in value && isToolCall(value.call)) {
        return { type: "tool_call", part_id: value.part_id, call: value.call }
      }
      break
    case "tool_result":
      if ("part_id" in value && typeof value.part_id === "string" && value.part_id &&
        "result" in value && isToolResult(value.result)) {
        return { type: "tool_result", part_id: value.part_id, result: value.result }
      }
      break
    case "finish":
      if ("reason" in value && (value.reason === "stop" || value.reason === "length")) {
        return { type: "finish", reason: value.reason }
      }
      break
    case "error":
      if ("message" in value && typeof value.message === "string") {
        return { type: "error", message: value.message }
      }
      break
  }
  throw new Error("Unsupported or malformed chat event.")
}

/**
 * One selected branch goes to our API. The browser has no provider URL or key.
 * Its abort signal closes the response; the API then drops the model request.
 */
export const chatAdapter: ChatModelAdapter = {
  async *run({ messages, abortSignal }) {
    const request: ChatRequest = { messages: messages.map(toChatMessage) }
    let response: Response
    try {
      response = await fetch("/v1/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/x-ndjson" },
        body: JSON.stringify(request),
        signal: abortSignal,
      })
    } catch (error) {
      if (abortSignal.aborted) throw error
      throw new Error("Could not reach the chat service. Start `just chat-api`.")
    }
    if (!response.ok) {
      const error = await response.json().catch(() => null)
      throw new Error(error?.message ?? `Chat service returned HTTP ${response.status}. Please try again later.`)
    }
    if (!response.body || !response.headers.get("content-type")?.startsWith("application/x-ndjson")) {
      await response.body?.cancel()
      throw new Error("The chat endpoint did not return a chat stream.")
    }

    let content: ThreadAssistantMessagePart[] = []
    const partIds: string[] = []
    let messageId: string | undefined
    let finished = false
    for await (const event of readChatEvents(response.body)) {
      // Stop also discards records already buffered in this browser.
      if (abortSignal.aborted) return
      if (finished) throw new Error("Chat sent an event after finishing.")
      switch (event.type) {
        case "start":
          if (messageId) throw new Error("Chat started the same reply twice.")
          messageId = event.message_id
          break
        case "delta": {
          if (!messageId) throw new Error("Chat sent content before starting.")
          const last = content.length - 1
          if (partIds[last] === event.part_id) {
            const previous = content[last]
            if ((previous.type !== "text" && previous.type !== "reasoning") || previous.type !== event.kind) throw new Error("Chat changed a part's type.")
            content = [...content.slice(0, last), { type: event.kind, text: previous.text + event.text }]
          } else {
            if (partIds.includes(event.part_id)) throw new Error("Chat resumed a part out of order.")
            partIds.push(event.part_id)
            content = [...content, { type: event.kind, text: event.text }]
          }
          yield { content, metadata: { custom: { messageId, partIds: [...partIds] } } }
          break
        }
        case "tool_call": {
          if (!messageId || partIds.includes(event.part_id)) throw new Error("Chat sent a duplicate or premature tool call.")
          partIds.push(event.part_id)
          content = [...content, { type: "tool-call", toolCallId: event.part_id, toolName: event.call.name, args: event.call.args, argsText: JSON.stringify(event.call.args) }]
          yield { content, metadata: { custom: { messageId, partIds: [...partIds] } } }
          break
        }
        case "tool_result": {
          const index = partIds.indexOf(event.part_id)
          const part = content[index]
          if (!part || part.type !== "tool-call" || part.result !== undefined || part.toolName !== event.result.name) throw new Error("Chat sent an unmatched or duplicate tool result.")
          content = content.map((item, i) => i === index ? { ...part, result: event.result.data, isError: event.result.data.status === "error" } : item)
          yield { content, metadata: { custom: { messageId, partIds: [...partIds] } } }
          break
        }
        case "finish":
          if (content.some((part) => part.type === "tool-call" && part.result === undefined)) throw new Error("Chat finished with an unresolved tool call.")
          if (!messageId) throw new Error("Chat finished before starting.")
          finished = true
          yield {
            content,
            status: event.reason === "length"
              ? { type: "incomplete", reason: "length" }
              : { type: "complete", reason: "stop" },
          }
          break
        case "error":
          throw new Error(event.message)
      }
    }
    if (!finished) throw new Error("Chat disconnected before finishing. The reply may be incomplete.")
  },
}

/** Shared by model requests and the versioned history codec. */
export function toChatMessage(message: ThreadMessage): ChatMessage {
  if (message.role !== "user" && message.role !== "assistant") {
    throw new Error("Only user and assistant messages are supported yet.")
  }
  if (message.role === "user" && message.attachments.length > 0) {
    throw new Error("File uploads are not connected yet.")
  }
  return {
    id: message.id,
    role: message.role,
    content: message.content.map((part): ContentPart => {
      if (part.type === "tool-call") {
        const call = { name: part.toolName, args: part.args }
        const result = part.result === undefined ? null : { name: part.toolName, data: part.result }
        if (!isToolCall(call) || (result !== null && !isToolResult(result))) throw new Error("Invalid stored tool call or result.")
        return { type: "tool_call", tool_call_id: part.toolCallId, call, result }
      }
      if (part.type !== "text" && part.type !== "reasoning") {
        throw new Error(`Sending ${part.type} parts is not supported yet.`)
      }
      return { type: part.type, text: part.text }
    }),
  }
}
