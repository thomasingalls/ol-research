import type { ChatModelAdapter } from "@assistant-ui/react"
import type { ChatEvent, ChatRequest, ContentPart } from "@/generated/chat"

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
    const request: ChatRequest = {
      messages: messages.map((message) => {
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
            if (part.type !== "text" && part.type !== "reasoning") {
              throw new Error(`Sending ${part.type} parts is not supported yet.`)
            }
            return { type: part.type, text: part.text }
          }),
        }
      }),
    }
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
      throw new Error(error?.message ?? `Chat service returned HTTP ${response.status}. Start \`just chat-api\` if it is not running.`)
    }
    if (!response.body || !response.headers.get("content-type")?.startsWith("application/x-ndjson")) {
      await response.body?.cancel()
      throw new Error("The chat endpoint did not return a chat stream.")
    }

    let content: ContentPart[] = []
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
            if (content[last].type !== event.kind) throw new Error("Chat changed a part's type.")
            content = [...content.slice(0, last), { type: event.kind, text: content[last].text + event.text }]
          } else {
            if (partIds.includes(event.part_id)) throw new Error("Chat resumed a part out of order.")
            partIds.push(event.part_id)
            content = [...content, { type: event.kind, text: event.text }]
          }
          yield { content, metadata: { custom: { messageId, partIds: [...partIds] } } }
          break
        }
        case "finish":
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
