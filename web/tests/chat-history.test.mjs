import { test } from "node:test"
import assert from "node:assert/strict"
import { ChatHistoryStore, decodeHistory, encodeMessage } from "../src/lib/chat-history.ts"
import { chatAdapter } from "../src/lib/chat-adapter.ts"

const date = new Date("2026-09-26T12:00:00Z")
const user = (id = "u1", parentId = null) => ({ parentId, message: {
  id, role: "user", createdAt: date, content: [{ type: "text", text: "papers about λ" }], attachments: [], metadata: { custom: {} },
} })
const reply = (id = "a1", parentId = "u1", status = { type: "complete", reason: "stop" }) => ({ parentId, message: {
  id, role: "assistant", createdAt: date, status, content: [
    { type: "reasoning", text: "Find sources" },
    { type: "tool-call", toolCallId: `${id}-search`, toolName: "search", args: { query: "λ", limit: 5 }, argsText: '{"query":"λ","limit":5}',
      result: { status: "ok", query: "λ", total: 0, hits: [], answered: { epoch: 3, batch: 2, at: "2026-09-26", sources: ["pubmed"], attribution: ["NLM"] } }, isError: false },
    { type: "text", text: "Nothing found" },
  ], metadata: { custom: { messageId: `server-${id}`, partIds: [`${id}-think`, `${id}-search`, `${id}-answer`] }, unstable_state: null, unstable_annotations: [], unstable_data: [], steps: [] },
} })
const empty = () => new ChatHistoryStore("session", { revision: 0, history: { head_id: null, messages: [] } })
const ok = revision => Response.json({ revision })
const deferred = () => {
  let resolve, reject
  const promise = new Promise((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

test("history codec restores dates, branches, selected head, reasoning, tools and provenance", () => {
  const nodes = [user(), reply(), reply("a2"), user("edited"), reply("edited-a", "edited")]
  const history = JSON.parse(JSON.stringify({ head_id: "a1", messages: nodes.map(encodeMessage) }))
  const restored = decodeHistory(history)
  assert.equal(restored.headId, "a1")
  assert.deepEqual(restored.messages, nodes.map(node => ({ ...node, message: { ...node.message,
    metadata: { ...node.message.metadata, custom: { ...node.message.metadata.custom, partIds: node.message.metadata.custom.partIds ?? [] } },
  } })))
  assert.deepEqual(restored.messages.map(encodeMessage), history.messages)
})

test("stopped pending tools and error replies never reload as running or completed", () => {
  const stopped = reply("a1", "u1", { type: "incomplete", reason: "cancelled" })
  delete stopped.message.content[1].result
  delete stopped.message.content[1].isError
  const failed = reply("a2", "u1", { type: "incomplete", reason: "error", error: { code: "unknown", message: "Disconnected" } })
  const tree = decodeHistory({ head_id: "a2", messages: [user(), stopped, failed].map(encodeMessage) })
  assert.equal(tree.messages[1].message.status.reason, "cancelled")
  assert.equal(tree.messages[1].message.content[1].result, undefined)
  assert.deepEqual(tree.messages[2].message.status, failed.message.status)
  assert.throws(() => encodeMessage(reply("a", "u1", { type: "running" })), /stopped/)
  assert.throws(() => decodeHistory({ head_id: "u1", messages: [user(), reply()].map(encodeMessage) }), /head/)
  assert.throws(() => decodeHistory({ head_id: "a1", messages: [reply(), user()].map(encodeMessage) }), /tree/)
})

test("writes serialize, preserve the full tree and persist branch selection", async t => {
  const first = deferred()
  const requests = []
  t.mock.method(globalThis, "fetch", async (_url, options) => {
    const body = JSON.parse(options.body)
    requests.push(body)
    if (requests.length === 1) await first.promise
    return ok(body.expected_revision + 1)
  })
  const store = empty()
  const one = store.adapter.append(user())
  const two = store.adapter.append(reply())
  assert.equal(requests.length, 1)
  first.resolve()
  await Promise.all([one, two])
  assert.deepEqual(requests.map(r => r.expected_revision), [0, 1])
  assert.equal(requests[0].history.messages.length, 1)
  assert.equal(requests[1].history.messages.length, 2)
  await store.adapter.append(reply("a2"))
  store.selectHead("a1")
  await store.flush()
  const last = requests.at(-1)
  assert.equal(last.history.head_id, "a1")
  assert.equal(last.history.messages.length, 3)
  assert.equal(store.getSnapshot().phase, "saved")
})

test("a lost save acknowledgement retries the same ID and bytes before newer changes", async t => {
  const requests = []
  let offline = true
  t.mock.method(globalThis, "fetch", async (_url, options) => {
    const request = JSON.parse(options.body)
    requests.push(request)
    if (offline) throw new Error("Lost acknowledgement")
    return ok(request.expected_revision + 1)
  })
  const store = empty()
  await assert.rejects(store.adapter.append(user()))
  await assert.rejects(store.adapter.append(reply()))
  assert.equal(requests.length, 1)
  assert.equal(store.getSnapshot().phase, "error")
  assert.equal(JSON.parse(store.export()).history.messages.length, 2)
  offline = false
  await store.retry()
  assert.deepEqual(requests[0], requests[1])
  assert.notEqual(requests[1].checkpoint_id, requests[2].checkpoint_id)
  assert.equal(requests[2].expected_revision, 1)
  assert.equal(requests[2].history.messages.length, 2)
})

test("a change arriving as a write settles is included in the save barrier", async t => {
  const requests = []
  t.mock.method(globalThis, "fetch", async (_url, options) => {
    const body = JSON.parse(options.body)
    requests.push(body)
    return ok(body.expected_revision + 1)
  })
  const store = empty()
  let appended = false
  let next
  store.subscribe(() => {
    if (store.getSnapshot().phase === "saved" && !appended) {
      appended = true
      next = store.adapter.append(reply())
    }
  })
  await store.adapter.append(user())
  await next
  assert.equal(requests.length, 2)
  assert.equal(requests.at(-1).history.messages.length, 2)
})

test("revision conflicts remain visible, stop automatic retries, and keep local export", async t => {
  let requests = 0
  t.mock.method(globalThis, "fetch", async () => { requests++; return Response.json({ message: "Changed elsewhere" }, { status: 409 }) })
  const store = empty()
  await assert.rejects(store.adapter.append(user()))
  assert.equal(store.getSnapshot().conflict, true)
  await assert.rejects(store.retry())
  assert.equal(requests, 1)
  assert.equal(JSON.parse(store.export()).history.messages[0].message.id, "u1")
})

test("model starts only after the prompt is saved; failed saves and early Stop prevent generation", async t => {
  const saved = deferred()
  let calls = 0
  const model = { async *run() { calls++; yield { content: [{ type: "text", text: "answer" }] } } }
  t.mock.method(globalThis, "fetch", async () => { await saved.promise; return ok(1) })
  const store = empty()
  const append = store.adapter.append(user())
  const run = Array.fromAsync(store.modelAdapter(model).run({ abortSignal: new AbortController().signal }))
  await Promise.resolve()
  assert.equal(calls, 0)
  saved.resolve()
  await Promise.all([append, run])
  assert.equal(calls, 1)
  const abort = new AbortController()
  abort.abort()
  await assert.rejects(Array.fromAsync(store.modelAdapter(model).run({ abortSignal: abort.signal })))
  assert.equal(calls, 1)
  t.mock.method(globalThis, "fetch", async () => Response.json({}, { status: 503 }))
  const broken = empty()
  await assert.rejects(broken.adapter.append(user()))
  await assert.rejects(Array.fromAsync(broken.modelAdapter(model).run({ abortSignal: new AbortController().signal })))
  assert.equal(calls, 1)
})

test("restored selected branch feeds the existing transport without executing saved tools", async t => {
  const repository = decodeHistory({ head_id: "a1", messages: [user(), reply(), reply("unused")].map(encodeMessage) })
  let body
  t.mock.method(globalThis, "fetch", async (_url, options) => {
    body = JSON.parse(options.body)
    return new Response('{"type":"start","message_id":"new"}\n{"type":"finish","reason":"stop"}\n', { headers: { "content-type": "application/x-ndjson" } })
  })
  await Array.fromAsync(chatAdapter.run({ messages: [repository.messages[0].message, repository.messages[1].message, user("followup").message], abortSignal: new AbortController().signal }))
  assert.deepEqual(body.messages.map(m => m.id), ["u1", "a1", "followup"])
  assert.equal(body.messages[1].content[1].result.data.answered.epoch, 3)
})
