# Local chat API

First implementation, 2026-09-26, on `feature/chat`. The library comparison
remains a [dated research snapshot](inbox/ai-chat-ui-landscape.md); this file
describes the running implementation.

The application can now talk to a local model through a small Rust service:

```text
assistant-ui LocalRuntime
    → POST /v1/chat (same origin, Vite proxy during development)
    → ol-rht-chat on 127.0.0.1:50053
    → POST /v1/chat/completions on llama.cpp, 127.0.0.1:8081
```

`ol-rht-chat` is independent of `ol-rht-serve` and does not open the corpus.
The model has no search tools or corpus connection yet. There is no stored
conversation, authentication system, upload service, tool execution,
approval workflow, reconnection protocol or cloud provider configuration.
The browser keeps the selected conversation and its edit/regeneration
branches until the route is left or reloaded.

## Start it

From this workspace, in three terminals:

```sh
just chat-model
just chat-api
just web
```

Open the frontend's `/chat` route. The corpus service (`just serve`) is
optional for chat; the existing footer still queries its `/v1/epoch`.
Its absence does not prevent conversation with the model.

The model recipe uses the installed `llama-server`, the cached
`unsloth/gemma-4-E2B-it-qat-GGUF:Q4_K_XL`, an 8192-token context and one
generation slot. `--offline` prevents downloads. It binds to loopback,
disables its built-in web UI and unused multimodal projector, and starts
with reasoning off to keep the initial smoke test short. The API can
nevertheless transport exposed
`reasoning_content` if the model server is started with reasoning enabled.

The first machine tested had llama.cpp build 11146, commit `7fe450e19`.
The cached file used for the initial test was
`gemma-4-E2B-it-qat-UD-Q4_K_XL.gguf` under Hugging Face snapshot
`66a399f68ddd113b06dff02fca9523e55465d11d`. That absolute cache path is
machine-specific; the recipe uses the repository/quantization cache lookup.

The chat server also accepts explicit settings:

```sh
cargo run -p ol-rht-chat -- \
  --listen 127.0.0.1:50053 \
  --upstream http://127.0.0.1:8081/v1 \
  --model local-gemma
```

Both its listener and model URL are restricted to loopback for this
development step. The client disables environment proxies and redirects.
The only authorization value is the deliberately inert `local-dev-only`,
shared with the llama-server recipe. It reads no provider credentials or
secret environment variables. No key is sent to or bundled in the browser.
Real provider access remains a separate decision.

Vite proxies `/v1/chat` to port 50053. The frontend has no separate chat
hostname or key setting. A deployed frontend would need an explicit route
to a chat service; `pnpm build` does not create that service or deploy it.

## Contract

`POST /v1/chat` accepts JSON describing the **selected branch**:

```json
{
  "messages": [
    {
      "id": "user-message-1",
      "role": "user",
      "content": [{ "type": "text", "text": "Reply with a short greeting." }]
    }
  ]
}
```

Roles are `user` and `assistant`; parts are `text` and `reasoning`, with
reasoning permitted only on assistant messages. Message ids must be distinct
within the branch. The last message must be a user message. Unknown fields
and unsupported part types are refused, so adding future tool/file support
requires an explicit contract change. The server owns the system prompt,
model choice, one-choice setting and 1024-token output cap.

The browser sends all supported parts. When constructing the model's chat
history, the service concatenates the text parts of each message and leaves
out previous reasoning. This avoids replaying reasoning as visible answer
text to a template that may not support reasoning history. A stopped
assistant message containing only reasoning contributes no upstream answer.
The original parts stay in the browser's conversation.

Limits are 128 messages, 128 parts per message, 64 KiB of total text
(including reasoning), and a 256 KiB JSON request body. These are transport
limits, not a tokenizer or context-window estimator. If llama.cpp refuses
an oversized context, the UI reports the refusal; it does not silently
discard older messages. Starting a new chat clears the history.

A valid request gets `application/x-ndjson`: one JSON record per line.
HTTP chunk boundaries have no meaning. Example:

```jsonl
{"type":"start","message_id":"chat-example"}
{"type":"delta","part_id":"chat-example-0","kind":"reasoning","text":"Considering the question."}
{"type":"delta","part_id":"chat-example-1","kind":"text","text":"Hello"}
{"type":"delta","part_id":"chat-example-1","kind":"text","text":" there."}
{"type":"finish","reason":"stop"}
```

Each new consecutive part gets its own id. Deltas append to the current
part; a text → reasoning → text sequence remains three ordered parts.
Part and response ids remain stable within the response, and the adapter
keeps them in assistant-ui's message metadata. They are not persisted run
handles and cannot be used to reconnect later.

`finish.reason` is `stop` or `length`. A token-limited answer is marked
incomplete in assistant-ui and displays a token-limit notice. A failed run
ends with an `error` record containing a user-readable `message`.

Invalid requests receive an HTTP 4xx JSON error before streaming starts.
Once streaming has started, model connection errors, HTTP refusals, malformed
events, unsupported tool calls and early EOF are **stream errors**, even
though the HTTP status is already 200. Upstream success requires both a
supported finish reason and `[DONE]`. Browser success requires `finish`;
EOF alone is never a completed answer. Read timeouts are 30 seconds of
inactivity, with a 120-second overall request deadline and a 5-second
connection timeout. Failed upstream bodies are not copied into the UI.

The canonical Rust request/event types are in
`crates/ol-rht-chat/src/protocol.rs`. The checked-in browser types,
`web/src/generated/chat.ts`, are generated from those declarations:

```sh
just chat-types
```

A test fails if they drift. The frontend also validates incoming events at
runtime, including their order and part identity. Types alone cannot
validate bytes from a network connection.

## Stop and disconnect

There is no detached worker and no explicit stop endpoint in this version.
The browser's abort signal closes its request. Vite explicitly destroys its
upstream request on response close. The Rust response body owns the entire
model request, including the wait for upstream headers; dropping the body
drops that request and socket.

This is deliberately coupled to the tested llama.cpp behavior: a closed
stream cancels its generation. A different OpenAI-compatible server is not
guaranteed to do that merely because it implements chat completions. Verify
cancellation for any replacement backend. Introducing background runs,
reconnection or approvals would require explicit run ownership and a stop
operation instead of relying on this connection lifetime.

The UI still supports edits, response regeneration and branch navigation.
An edit sends the newly selected history, and a regeneration sends history
up to the user message being answered. No abandoned branch is sent merely
because it remains in the browser's branch tree.

## Checks

```sh
just chat-check
cargo clippy -p ol-rht-chat --all-targets -- -D warnings
cargo fmt -p ol-rht-chat -- --check
cd web && pnpm build
```

The Rust tests use a fake model over real loopback sockets. They cover
history conversion, ordered text/reasoning deltas, errors, truncation,
unsupported tools, request limits, loopback configuration, and upstream
socket closure both before headers and during generation. A property test
varies UTF-8 content, SSE line endings and network chunk boundaries. No
model is needed for these tests.

The Node tests exercise the actual frontend adapter with fragmented streams,
interleaved parts, immutable updates, finish reasons, error records, bad
ordering, HTTP failures and reader cancellation. They use Node's built-in
TypeScript stripping (Node 22.18+; development tested on Node 26), with no
additional test dependency.

For a live smoke check, send a short prompt in `/chat`, follow up with a
question about it, edit it, and regenerate a response. Then request a long
answer and press Stop while it streams. Inspect llama.cpp's `/slots` with
the dummy authorization header: `is_processing` should become false
promptly, and the browser's partial answer should stop changing.

The live browser check on September 26 passed real Gemma replies,
follow-up context, edited branches, regeneration, readable errors and a
320px layout. Stop released the actual generation slot in about 110 ms in
that run. The offline cache recipe was also launched successfully. These
are observations on this machine, not latency guarantees for other models.

Upstream reference: [llama.cpp server documentation](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md).
The application API is intentionally smaller than that server's API.
