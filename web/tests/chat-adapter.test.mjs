import { test } from "node:test"
import assert from "node:assert/strict"
import { chatAdapter, readChatEvents } from "../src/lib/chat-adapter.ts"

const start = { type: "start", message_id: "reply" }
const finish = { type: "finish", reason: "stop" }
const delta = (part_id, kind, text) => ({ type: "delta", part_id, kind, text })
const wire = (events) => events.map(JSON.stringify).join("\n") + "\n"
function body(text, size = 1) {
  const bytes = new TextEncoder().encode(text)
  return new ReadableStream({
    start(controller) {
      for (let offset = 0; offset < bytes.length; offset += size) {
        controller.enqueue(bytes.slice(offset, offset + size))
      }
      controller.close()
    },
  })
}

test("NDJSON survives every tested byte boundary, including Unicode", async () => {
  const events = [start, delta("a", "text", "é λ 😀\nsecond line"), finish]
  for (let size = 1; size <= 80; size++) {
    assert.deepEqual(await Array.fromAsync(readChatEvents(body(wire(events), size))), events)
  }
})

test("truncated, unknown, malformed and oversized events fail", async () => {
  for (const text of ['{"type":"start"', '{"type":"unknown"}\n', '{"type":"delta","text":2}\n', "x".repeat(262145)]) {
    await assert.rejects(Array.fromAsync(readChatEvents(body(text, 4096))))
  }
})

const messages = [{ id: "question", role: "user", content: [{ type: "text", text: "hello" }], attachments: [] }]

test("adapter preserves interleaved parts, immutable updates and finish reason", async (t) => {
  const events = [start, delta("a", "reasoning", "Think"), delta("a", "reasoning", "…"), delta("b", "text", "Answer"), delta("c", "reasoning", "Check"), delta("d", "text", "Done"), { type: "finish", reason: "length" }]
  const abort = new AbortController()
  t.mock.method(globalThis, "fetch", async (url, options) => {
    assert.equal(url, "/v1/chat")
    assert.equal(options.signal, abort.signal)
    assert.equal(options.headers.Authorization, undefined)
    assert.deepEqual(JSON.parse(options.body), { messages: [{ id: "question", role: "user", content: messages[0].content }] })
    return new Response(body(wire(events)), { headers: { "content-type": "application/x-ndjson" } })
  })
  const updates = await Array.fromAsync(chatAdapter.run({ messages, abortSignal: abort.signal }))
  assert.deepEqual(updates[0].content, [{ type: "reasoning", text: "Think" }])
  assert.deepEqual(updates.at(-1).content, [
    { type: "reasoning", text: "Think…" }, { type: "text", text: "Answer" },
    { type: "reasoning", text: "Check" }, { type: "text", text: "Done" },
  ])
  assert.deepEqual(updates.at(-1).status, { type: "incomplete", reason: "length" })
  assert.deepEqual(updates.at(-2).metadata.custom.partIds, ["a", "b", "c", "d"])
})

test("adapter does not treat broken or out-of-order streams as completed answers", async (t) => {
  const bad = [
    [start, delta("a", "text", "partial")],
    [start, { type: "error", message: "upstream unavailable" }],
    [delta("a", "text", "early"), finish],
    [start, start, finish],
    [start, delta("a", "text", "first"), delta("a", "reasoning", "changed")],
    [start, delta("a", "text", "first"), delta("b", "reasoning", "next"), delta("a", "text", "old")],
    [start, finish, delta("a", "text", "late")],
  ]
  for (const events of bad) {
    const mock = t.mock.method(globalThis, "fetch", async () => new Response(body(wire(events)), { headers: { "content-type": "application/x-ndjson" } }))
    await assert.rejects(Array.fromAsync(chatAdapter.run({ messages, abortSignal: new AbortController().signal })))
    mock.mock.restore()
  }
})

test("Stop discards already-buffered deltas", async (t) => {
  const abort = new AbortController()
  t.mock.method(globalThis, "fetch", async () => new Response(
    body(wire([start, delta("a", "text", "first"), delta("a", "text", " second"), finish]), 4096),
    { headers: { "content-type": "application/x-ndjson" } },
  ))
  const run = chatAdapter.run({ messages, abortSignal: abort.signal })
  assert.equal((await run.next()).value.content[0].text, "first")
  abort.abort()
  assert.equal((await run.next()).done, true)
})

test("HTTP errors and wrong content types are visible; reader cancellation propagates", async (t) => {
  for (const response of [new Response('{"message":"invalid branch"}', { status: 400 }), new Response("<html>wrong endpoint</html>")]) {
    const mock = t.mock.method(globalThis, "fetch", async () => response)
    await assert.rejects(Array.fromAsync(chatAdapter.run({ messages, abortSignal: new AbortController().signal })))
    mock.mock.restore()
  }
  let cancelled = false
  const stream = new ReadableStream({
    start(controller) { controller.enqueue(new TextEncoder().encode(wire([start]))) },
    cancel() { cancelled = true },
  })
  for await (const event of readChatEvents(stream)) {
    assert.equal(event.type, "start")
    break
  }
  assert(cancelled)
})
