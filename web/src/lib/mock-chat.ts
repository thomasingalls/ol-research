import type { ChatModelAdapter } from "@assistant-ui/react"

const previewReply =
  "This is a **local preview**. No agent is connected yet, so I can’t search " +
  "the literature or answer your question.\n\n" +
  "You can try sending another message, editing this conversation, or " +
  "stopping a reply while it streams."

/**
 * The only chat adapter wired into the app. Deliberately has no transport,
 * provider SDK, environment lookup, or credentials; even a real key in the
 * environment cannot turn this preview into a model request.
 *
 * LocalRuntime owns message state, edits, branches, regeneration and stop.
 * Each yield is cumulative content, as required by ChatModelAdapter.
 */
export const mockChatAdapter: ChatModelAdapter = {
  async *run({ abortSignal }) {
    let text = ""
    for (const word of previewReply.split(/(?<=\s)/)) {
      if (abortSignal.aborted) return
      await new Promise<void>((resolve) => setTimeout(resolve, 40))
      if (abortSignal.aborted) return
      text += word
      yield { content: [{ type: "text", text }] }
    }
  },
}
