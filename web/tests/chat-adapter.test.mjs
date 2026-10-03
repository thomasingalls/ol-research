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

const searchArgs = { query: "protein λ", limit: 2 }
const toolCall = { type: "tool_call", part_id: "search-1", call: { name: "search", args: searchArgs } }
const searchResult = { status: "ok", query: searchArgs.query, total: 1, hits: [{
  public_id: "ol:one", title: "Protein λ", authors: ["Author"], venue: "Journal", year: 2026,
  retracted_by: ["pubmed"], corrected_by: [], concern_by: [],
}], answered: { epoch: 8, batch: 7, at: "2026-09-19", sources: ["pubmed"], attribution: ["NLM attribution"] } }
const toolResult = { type: "tool_result", part_id: "search-1", result: { name: "search", data: searchResult } }

test("search parts retain identity, results and immutable snapshots across a follow-up", async (t) => {
  const first = [start, delta("before", "text", "Looking"), toolCall, toolResult, delta("after", "text", "Found it"), finish]
  let sent
  t.mock.method(globalThis, "fetch", async (_url, options) => {
    sent = JSON.parse(options.body)
    return new Response(body(wire(first)), { headers: { "content-type": "application/x-ndjson" } })
  })
  const updates = await Array.fromAsync(chatAdapter.run({ messages, abortSignal: new AbortController().signal }))
  const tool = updates.at(-1).content[1]
  assert.equal(tool.type, "tool-call")
  assert.equal(tool.toolName, "search")
  assert.equal(tool.toolCallId, "search-1")
  assert.deepEqual(tool.args, searchArgs)
  assert.deepEqual(tool.result, searchResult)
  assert.equal(tool.isError, false)
  assert.equal(updates[1].content[1].result, undefined)
  assert.deepEqual(updates.at(-2).metadata.custom.partIds, ["before", "search-1", "after"])
  const history = [...messages, { id: "a1", role: "assistant", content: updates.at(-1).content },
    { ...messages[0], id: "u2" }]
  await Array.fromAsync(chatAdapter.run({ messages: history, abortSignal: new AbortController().signal }))
  assert.deepEqual(sent.messages[1].content[1], { type: "tool_call", tool_call_id: "search-1", call: { name: "search", args: searchArgs }, result: { name: "search", data: searchResult } })
})

test("search errors are visible tool results; cancelling a pending tool freezes it", async (t) => {
  const failure = { ...toolResult, result: { name: "search", data: { status: "error", message: "Corpus unavailable" } } }
  const mock = t.mock.method(globalThis, "fetch", async () => new Response(body(wire([start, toolCall, failure, finish])), { headers: { "content-type": "application/x-ndjson" } }))
  const updates = await Array.fromAsync(chatAdapter.run({ messages, abortSignal: new AbortController().signal }))
  assert.equal(updates.at(-1).content[0].isError, true)
  mock.mock.restore()
  const abort = new AbortController()
  t.mock.method(globalThis, "fetch", async () => new Response(body(wire([start, toolCall, toolResult, finish]), 4096), { headers: { "content-type": "application/x-ndjson" } }))
  const run = chatAdapter.run({ messages, abortSignal: abort.signal })
  const pending = (await run.next()).value
  assert.equal(pending.content[0].result, undefined)
  abort.abort()
  assert.equal((await run.next()).done, true)
  assert.equal(pending.content[0].result, undefined)
})

test("malformed search data and mismatched tool lifecycles fail explicitly", async (t) => {
  for (const events of [
    [toolCall], [start, toolResult], [start, toolCall, toolCall], [start, toolCall, finish],
    [start, toolCall, toolResult, toolResult],
    [start, delta("search-1", "text", "not a tool"), toolResult],
    [start, toolCall, { ...toolResult, result: { name: "search", data: { ...searchResult, hits: [{ title: "missing id" }] } } }],
    [start, { ...toolCall, call: { name: "search", args: { query: "x", limit: 11 } } }],
    [start, toolCall, delta("search-1", "text", "changed type")],
  ]) {
    const mock = t.mock.method(globalThis, "fetch", async () => new Response(body(wire(events)), { headers: { "content-type": "application/x-ndjson" } }))
    await assert.rejects(Array.fromAsync(chatAdapter.run({ messages, abortSignal: new AbortController().signal })))
    mock.mock.restore()
  }
})

const paperData = { status: "ok", answered: null, paper: {
  public_id: "ol:paper", stub: false, title: "Paper title", authors: ["Author"], authors_truncated: false,
  venue: "Journal", published: { year: 2026, month: 9, day: 1 }, identifiers: [{ namespace: "pmc", value: "PMC123", standing: "promoted" }],
  abstract_text: "Abstract", abstract_truncated: false, retracted_by: [], corrected_by: [], concern_by: [], open_access: true,
  full_text_url: "https://pmc.ncbi.nlm.nih.gov/articles/PMC123/", publisher_url: null,
} }
const paperCall = { type: "tool_call", part_id: "paper-1", call: { name: "get_paper", args: { id: "ol:paper" } } }
const paperResult = { type: "tool_result", part_id: "paper-1", result: { name: "get_paper", data: paperData } }

test("search and paper lookups share a reply and replay with the correct tool names", async (t) => {
  let sent
  const events = [start, toolCall, toolResult, paperCall, paperResult, delta("answer", "text", "A citation"), finish]
  t.mock.method(globalThis, "fetch", async (_url, options) => {
    sent = JSON.parse(options.body)
    return new Response(body(wire(events)), { headers: { "content-type": "application/x-ndjson" } })
  })
  const updates = await Array.fromAsync(chatAdapter.run({ messages, abortSignal: new AbortController().signal }))
  const parts = updates.at(-1).content
  assert.equal(parts[0].toolName,"search")
  assert.equal(parts[1].toolName,"get_paper")
  assert.deepEqual(parts[1].args,{id:"ol:paper"})
  assert.deepEqual(parts[1].result,paperData)
  await Array.fromAsync(chatAdapter.run({ messages:[...messages,{id:"a",role:"assistant",content:parts},{...messages[0],id:"next"}], abortSignal:new AbortController().signal }))
  assert.deepEqual(sent.messages[1].content[1],{type:"tool_call",tool_call_id:"paper-1",call:paperCall.call,result:paperResult.result})
})

test("tools reject mismatched result names and malformed paper metadata", async (t) => {
  for (const events of [
    [start,toolCall,{...paperResult,part_id:"search-1"}],
    [start,paperCall,{...paperResult,result:{name:"get_paper",data:{status:"ok",paper:{public_id:"ol:paper"},answered:null}}}],
    [start,{...paperCall,call:{name:"get_paper",args:{id:" "}}}],
  ]) {
    const mock=t.mock.method(globalThis,"fetch",async()=>new Response(body(wire(events)),{headers:{"content-type":"application/x-ndjson"}}))
    await assert.rejects(Array.fromAsync(chatAdapter.run({messages,abortSignal:new AbortController().signal})))
    mock.mock.restore()
  }
})
