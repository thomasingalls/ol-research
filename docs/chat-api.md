# Local chat API

First implementation, 2026-09-26, on `feature/chat`. The library comparison
remains a [dated research snapshot](inbox/ai-chat-ui-landscape.md); this file
describes the running implementation.

The [research workbench design](research-workbench-design.md) records the
subsequent persistence and execution discussion: Postgres/SQLx, a possible
vendored rpi, a Pi baseline, API-to-VM handoff, Temporal, durable workspaces
and artifact provenance. The [Postgres storage foundation](workbench-storage.md)
now backs the chat history API and browser history adapter. Native runner
integration, handoff and the rest remain proposals.

The application can now talk to a local model through a small Rust service:

```text
assistant-ui LocalRuntime
    → POST /v1/chat (same origin, Vite proxy during development)
    → ol-rht-chat on 127.0.0.1:50053
    → POST /v1/chat/completions on llama.cpp, 127.0.0.1:8081
    → GET /v1/search or /v1/work on ol-rht-serve, 127.0.0.1:50052
    → return tool results to llama.cpp, then stream its answer
```

`ol-rht-chat` calls the existing corpus HTTP API; it does not open the store
or index itself. Its tools are `search` and `get_paper`, executed by Rust.
Conversations and edit/regeneration branches are stored in Postgres.
There is no authentication system, upload service, approval workflow,
run reconnection protocol or cloud provider configuration. Generation is
still owned by the browser's HTTP connection.

## Start it

Start the local database and apply migrations explicitly first:

```sh
just db-up db-migrate
```

Then, from this workspace, in four terminals:

```sh
just serve
just chat-model
just chat-api
just web
```

Open the frontend's `/chat` route and choose **New chat**, or reopen a
conversation from the selector. Its session ID lives in `?session=…`, so
reloading or reopening that URL restores it. The corpus service (`just serve`) is
required for both tools. Without it, plain conversation still works, and a
corpus request produces a visible tool error that is also returned to the model.
The existing footer queries the corpus service's `/v1/epoch` separately.
The corpus defaults to permitting browser origins `http://localhost:5173`
and `http://127.0.0.1:5173`; a different frontend port needs a corresponding
corpus origin setting for the footer and paper detail pages.

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

`just chat-api` supplies the local `OL_RHT_DATABASE_URL` unless explicitly
overridden. Calling the binary directly requires this variable; there is no
silent fallback to browser-only history and startup never applies migrations.
The chat server also accepts explicit settings:

```sh
cargo run -p ol-rht-chat -- \
  --listen 127.0.0.1:50053 \
  --upstream http://127.0.0.1:8081/v1 \
  --model local-gemma \
  --corpus http://127.0.0.1:50052
```

Its listener, model URL and corpus URL are restricted to loopback for this
development step. The client disables environment proxies and redirects.
The only authorization value is the deliberately inert `local-dev-only`,
shared with the llama-server recipe. It reads no model provider credentials. `OL_RHT_DATABASE_URL` is a separate
application database setting, used only by Rust. No key is sent to or bundled in the browser.
Real provider access remains a separate decision.

**Vite is only the development proxy, not a production requirement.**
`pnpm build` produces static frontend assets. In production, serve those
assets normally and route `/v1/chat` and its subpaths to the Rust chat service using the
application server or a reverse proxy. The proxy must stream responses
without buffering and propagate disconnects. Neither Vite nor a Node
frontend server needs to run in production. The browser uses a relative
`/v1/chat` URL and has no provider hostname or key. This documents the
routing shape; production authentication and deployment are separate work.

## Corpus tools

The server advertises `search({ query, limit? })` and `get_paper({ id })`.
Search accepts nonempty keywords of at most 512 UTF-8 bytes, defaults to five hits, and
permits limits from one to ten. It requests hydrated `/v1/search` results
from the configured corpus, with URL-encoded query parameters. The model
cannot select the corpus URL, an HTTP method, or tools outside this pair.

A successful result contains the query, total matches, ranked paper IDs,
titles, up to six authors per paper, venue, year, and source-specific
retraction, correction and concern flags. It also retains the responding
epoch, batch, timestamp, sources and attribution. These are metadata search
results, **not abstracts, full text or evidence of study findings**. A zero
count refers to this corpus, not all literature. Corpus failures are
structured error results, visible in the UI and sent back to the model.
Upstream error bodies are not exposed.

`get_paper` requests `/v1/work?work=…`, which already resolves a public ID or
supported namespace-qualified name. Prefer the `ol:` ID returned by search.
Otherwise use `doi:…`, `pmid:…`, `pmc:PMC…`, `openalex:W…`, or `reg<digits>:…`
in canonical spelling. The model is instructed to convert a bare DOI/DOI
URL to a lowercase `doi:` name before calling. Input is limited to 2048
UTF-8 bytes; it is always a query parameter sent to the configured corpus,
never a URL to fetch. Absent, shared and retired identifiers return explicit
tool errors; the tool never guesses which work a shared name means.

A successful lookup returns the canonical public ID, title, authors, venue,
publication date, identifiers, warning flags, open-access metadata and the
abstract that the corpus is allowed to serve, together with provenance.
Authors are capped at 50 and abstracts at 8000 characters, with explicit
truncation flags. A missing abstract may be absent or withheld by source
terms. Citation-only stubs remain identified as stubs; no missing fields
are invented. The current corpus contract does not provide volume, issue
or page numbers.

Citation formatting is deliberately left to the model for now. The tool
returns structured metadata, and the system prompt asks for a citation
with authors, year, linked title, venue and DOI where available. It supplies
`full_text_url` from a **promoted PMC identifier**, and `publisher_url` from
a **promoted DOI**, using the same identifier destinations as the existing
Read button. Aliases/shared names are not used to derive links. The model
is instructed to link the title to PMC when available, otherwise the
publisher URL with an access caveat, or the local work page if neither
exists. A DOI landing page is not proof of accessible full text, and these
links are not fetched or availability-checked by the tool. The model does
not receive full text. The UI renders the model's Markdown with new-tab
links; there is no deterministic citation formatter or citation component.

The server accumulates fragmented tool names/arguments, validates complete
calls, and waits for `finish_reason: tool_calls` plus `[DONE]` before
executing anything. It emits a tool-call part, runs the requested tool, emits the result,
then calls the model again with the standard assistant `tool_calls` and
matching `role: tool` messages. Partial or token-truncated calls never run.
Tool arguments are not streamed to the browser in this first version; the
tool indicator appears once a complete call is ready to execute.

Each reply permits at most six tool calls, executed sequentially. This
allows one search followed by lookups for its five default results. Once
the budget is used, the final model request sets `tool_choice: none`; a model
that ignores that setting receives a stream error. Each corpus request has a
15-second deadline, a 1 MiB response-body cap and a 24 KiB metadata result
cap. The entire model/tool loop has a three-minute deadline.

The chat renders search progress, result cards, empty results and failures,
plus a compact paper-lookup status. Citation prose stays in the model answer.
Paper links and answer citations open a new tab so the current conversation
survives. Source attribution is available in each result card. The browser
only renders these tool parts; it does not execute tools or call the corpus
on the model's behalf.

## Persisted conversations

The browser uses assistant-ui's `ThreadHistoryAdapter`. A small codec maps
its branch repository to application-owned Rust/TypeScript types, independent
of the SQLx storage crate. It saves the whole tree, not just the branch sent
to the model. This includes message IDs and parents, selected head, dates,
text/reasoning, search/paper arguments and results with provenance, stream
message/part IDs, and complete, token-limited, cancelled or error status.
An unanswered tool remains unanswered on reload; loading never executes it.

The current envelope is `runner=ol-rht-chat`, `runner_version=<package-version>`, `format=ol-rht-chat-history`, `format_version=1`, with the storage
session UUID as `native_session_id`. These bytes are **a chat UI projection**,
not a native rpi/Pi session, exact model context, execution journal or
recovery checkpoint. That distinction remains part of the workbench design.
No new tables or schema migration were needed for this integration.

All endpoints below are under `/v1/chat/sessions` and send `Cache-Control: no-store`:

| Method/path | Behavior |
|---|---|
| `GET /v1/chat/sessions?after=<uuid>` | Up to 50 summaries and a nullable `next_cursor`; UUID keyset pagination |
| `PUT /v1/chat/sessions/<session-uuid>` | Create with `{ "title": "…" }`; identical ID/title retries are idempotent |
| `GET /v1/chat/sessions/<session-uuid>/history` | `{ "revision": n, "history": { "head_id": "…", "messages": […] } }`; an empty session is revision 0 |
| `PUT /v1/chat/sessions/<session-uuid>/history` | Save `{ "checkpoint_id": "uuid", "expected_revision": n, "history": … }`; return the committed revision |

The loopback binary provisions one stable **local development project**.
It has no users or project picker. Internal reads/writes are project-scoped;
that does not provide authentication or authorization for deployment. The
server refuses unknown runner/format envelopes rather than guessing or
replacing their contents. Future changes to this format need an explicit
reader/migration decision, not just a dependency upgrade.

History requests are capped at 2 MiB, with at most 2048 messages and 128
parts per message. Parents must precede children; IDs are unique and the
selected head must be a leaf. Dates, roles, statuses and tool pairings are
validated. These history limits are separate from the smaller selected-model
context limits below; no automatic compaction or retention has been added.
The browser lists loaded summaries by creation date and can fetch more pages.
New chat titles currently use the creation time; rename/delete are later work.

**Save boundaries and errors:**

- The user message is committed before the browser starts a model request.
  Assistant-ui invokes append and generation concurrently, so the model
  adapter explicitly waits for the history write.
- Completed replies and partial replies after Stop or a handled stream error
  are saved with their terminal status. Switching branches saves the selected
  head. The UI displays `Saving…`, `Saved`, or a visible failure.
- Writes are serialized per mounted conversation. Retries retain the exact
  checkpoint UUID, expected revision and snapshot, including after an uncertain
  network failure. A newer local snapshot follows only after that write is
  acknowledged. Saves have a 15-second client deadline.
- HTTP 409 leaves local changes intact and blocks further saves/generation.
  The UI offers a JSON export and explicit discard/reload. It never silently
  replaces local history or retries stale bytes with a newer revision.
  Other failures offer **Retry save**; the JSON export also works offline.
- Navigation warns while a reply is running or changes are unsaved. A new
  conversation/switch is disabled during that interval. Unsaved composer
  drafts are not part of history.

**Limits of this slice:** a reply is not checkpointed token by token. Closing
or crashing the browser before its terminal save can lose that reply,
including completed tools within it; the previously saved user message remains.
A failed save stays in browser memory until retried/exported, not in a durable
local outbox. The model may finish before its browser save succeeds. Reloading
does not resume generation. There is no execution lease, background run,
server-authoritative tool journal, multi-user collaboration or automatic
conflict merge. Optimistic revisions prevent lost history updates; they do
not guarantee only one executor. Generation and context selection still use
the current transport below. Durable runs belong to the subsequent runner/
execution slice, without treating this projection as its native state.

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

Roles are `user` and `assistant`; parts are `text`, `reasoning` and
`tool_call`. Reasoning and tool calls are permitted only on assistant
messages. Message ids must be distinct within the branch. The last message must be a user message. Unknown fields
and unsupported part types are refused, so adding future tools/files
requires an explicit contract change. The server owns the system prompt,
model choice, tool definitions, one-choice setting and 1024-token output cap
per model completion.

The browser sends all supported parts. When constructing the model's chat
history, the service concatenates consecutive text around tool calls and
leaves out previous reasoning. This avoids replaying reasoning as visible
answer text to a template that may not support reasoning history. A stopped
assistant message containing only reasoning contributes no upstream answer.
The original parts stay in the conversation and its saved history.

A history tool part has `type: "tool_call"`, `tool_call_id`, a validated
`call` (`name` and `args`) and a `result` (`name` and `data`), or `null` for
an interrupted tool. Call and result tool names must match. Completed calls
and results are replayed as paired assistant/tool messages, never executed
again merely because they are in history. Unresolved calls are omitted
from model history. Editing or regenerating can request fresh tool calls,
as it generates a new assistant branch. This is client-supplied history,
persisted as a UI history projection, not an authenticated execution log.

Limits are 128 messages, 128 parts per message, 64 KiB of total text
(including reasoning and serialized tool arguments/results), and a 256 KiB
JSON request body. These are transport limits, not a tokenizer or context-window estimator. If llama.cpp refuses
an oversized context, the UI reports the refusal; it does not silently
discard older messages. Starting a new chat creates a separate session; earlier sessions remain available.

A valid request gets `application/x-ndjson`: one JSON record per line.
HTTP chunk boundaries have no meaning. Example:

```jsonl
{"type":"start","message_id":"chat-example"}
{"type":"delta","part_id":"chat-example-0","kind":"reasoning","text":"Considering the question."}
{"type":"delta","part_id":"chat-example-1","kind":"text","text":"Hello"}
{"type":"delta","part_id":"chat-example-1","kind":"text","text":" there."}
{"type":"finish","reason":"stop"}
```

Search adds two event types, paired by `part_id`:

```jsonl
{"type":"tool_call","part_id":"chat-example-2","call":{"name":"search","args":{"query":"protein folding","limit":5}}}
{"type":"tool_result","part_id":"chat-example-2","result":{"name":"search","data":{"status":"ok","query":"protein folding","total":0,"hits":[],"answered":null}}}
```

The browser maps these to assistant-ui's `tool-call` parts, using the
`search` or `get_paper` name,
with the same call ID, arguments and eventual result. Tool errors set its
`isError` flag. A final `finish` requires all emitted calls to have results.

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
events, invalid/unknown tool calls and early EOF are **stream errors**, even
though the HTTP status is already 200. Upstream success requires both a
supported finish reason and `[DONE]`. Browser success requires `finish`;
EOF alone is never a completed answer. Read timeouts are 30 seconds of
inactivity, with a 120-second deadline per model request and a 5-second
connection timeout, within the three-minute run deadline. Failed upstream
bodies are not copied into the UI.

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
model/tool loop, including the wait for upstream headers; dropping the
body drops the active model or corpus HTTP request and its socket. It does
not schedule the next model turn. Closing the corpus socket does not promise
to interrupt search work already scheduled inside the separate corpus service.

This is deliberately coupled to the tested llama.cpp behavior: a closed
stream cancels its generation. A different OpenAI-compatible server is not
guaranteed to do that merely because it implements chat completions. Verify
cancellation for any replacement backend. Introducing background runs,
reconnection or approvals would require explicit run ownership and a stop
operation instead of relying on this connection lifetime.

The UI still supports edits, response regeneration and branch navigation.
An edit sends the newly selected history, and a regeneration sends history
up to the user message being answered. No abandoned branch is sent merely
because it remains in the saved branch tree.

## Checks

```sh
just chat-check
just db-test  # isolated Postgres databases; requires db-up
cargo clippy -p ol-rht-chat --all-targets -- -D warnings
cargo fmt -p ol-rht-chat -- --check
cd web && pnpm build
```

The Rust tests use a fake model over real loopback sockets. They cover
history conversion, ordered text/reasoning deltas, errors, truncation,
invalid tools, fragmented tool arguments, result provenance and warning
flags, identifier forwarding, promoted reading links, metadata truncation, empty/error searches, tool budgets, replay of completed results,
omission of cancelled calls, request limits, loopback configuration, and
socket closure before model headers, during generation and during search. A property test
varies UTF-8 content, SSE line endings and network chunk boundaries. No
model is needed for these tests.

The Node tests exercise the actual frontend adapter with fragmented streams,
interleaved parts, immutable updates, finish reasons, error records, bad
ordering, HTTP failures, reader cancellation, tool lifecycles, result
validation, matched tool names and tool history on follow-ups. They use Node's built-in
TypeScript stripping (Node 22.18+; development tested on Node 26), with no
additional test dependency. History tests exercise codec round trips, branch
selection, stopped/error status, serialized writes, uncertain retries,
conflicts, the save-before-model barrier and restored tool history. `just db-test` additionally runs the chat API against real Postgres, including API
restart, pagination/scoping, concurrent writes, malformed input, body limits,
unsupported formats and unavailable storage.

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

The subsequent corpus-tool check passed a real Gemma search, rendered source
attribution, local paper-detail navigation, and a follow-up answered from
retained search results. A real search→`get_paper` follow-up selected the
`ol:` ID when both that ID and a DOI were given, and generated a citation
whose title linked to the paper's PMC page. A separate PMID lookup resolved
to the same canonical work as its `ol:` ID and used the DOI publisher link
when no PMC identifier was present. Both title links had new-tab attributes;
external sites were not fetched during this check. Citation wording and
completeness remain model-dependent; these smoke checks do not establish
citation-style compliance. The mobile check remained within 320px and
showed no browser exceptions. Stop after search released the model slot in
about 171 ms in the observed run, retaining the completed search card.

Upstream reference: [llama.cpp server documentation](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md).
The application API is intentionally smaller than that server's API.

The persistence browser check on September 26 used a separate local test
database and the existing cached Gemma server. It passed save/reload, a
follow-up using restored context, edited/regenerated branches, restoration of
a previously selected branch, Stop with a persisted cancelled reply, and a
stale second-tab conflict that prevented its model request. Additional
browser checks covered a lost commit acknowledgement, identical retry,
restored tool cards, switching conversations, and retry after a failed load.
A 320px layout check passed with no browser exceptions. These checks validate conversation
history, not durable execution or crash recovery.
