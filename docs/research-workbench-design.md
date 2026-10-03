# Research workbench: persistence, agent execution and provenance

**Recorded: 2026-09-26. Status: design discussion and dated research.**
This document preserves the reasoning from the chat-storage discussion so
implementation can proceed in small steps without losing the intended
product. It distinguishes the user's requirements, inspected implementation,
external documentation, and proposed architecture. The first
[Postgres/SQLx storage slice](workbench-storage.md) is now implemented:
local container, migrations, project/session records and native checkpoints.
A subsequent slice connects assistant-ui's branch history to the store through
a versioned chat API. It saves a UI projection, not native agent execution
state. rpi/Pi integration, Temporal workflows and VM handoff are not built. The concrete architecture still
needs implementation trials; recording it does not settle every choice below.

**Latest clarification, 2026-09-26:** prefer evaluating a vendored, customized
`rpi` with upstream Pi as a comparison baseline. Start with API execution for
bounded conversations; provision a VM when needed and support a controlled
handoff. A VM is not required for every model request or conversation.

The implementation inspected was `feature/chat` at `dac0cc7`. See
[the chat API](chat-api.md) for the running system and
[the web README](../web/README.md#chat-ui) for its frontend. The earlier
[UI survey](inbox/ai-chat-ui-landscape.md) records the assistant-ui choice.
The broader product history is in
[product shapes, September 25 clarification](inbox/product-shapes.md#10-the-workbench-is-part-of-the-product--2026-09-25),
the [workbench comparison](inbox/competition-list.md#research-workbenches--2026-09-25),
and the [science-assistant landscape](inbox/science-assistant-landscape.md).
This discussion develops the workbench direction already described there.
It does not reorder the corpus ingestion, merge, graph or serving plans.

## 1. Requirements and preferences from the user

- **The eventual target has requirements comparable to Claude Science.**
  The present search-and-chat screen is an early capability, not the scope
  against which to optimize the whole architecture. This is a requirements
  reference, not a commitment to reproduce every feature or scientific domain.
- **Keep the shadcn ecosystem and assistant-ui as the UI starting point.**
  Voice is not a priority. The workbench will need intentionally designed
  interfaces around scientific tools and outputs as well as conversation.
- **Postgres is available on the project server; SQLx is the requested Rust
  storage boundary.** Local Postgres should run in a container, with a
  `Containerfile`, rather than be installed as a native database service.
- **Temporal is already installed in the cluster.** It is an available
  execution platform to evaluate, not a requirement to route every operation
  through a workflow.
- **Reusing substantial existing machinery matters.** The concern is spending
  months rediscovering solutions to agent state, recovery and history.
- **Framework churn counts toward total maintenance cost.** The user is
  sceptical of general agent frameworks and especially of abstractions that
  need repeated adaptation whenever model providers introduce new APIs.
  A TypeScript or Python service remains possible for a clear maintenance
  win; adding one is not the default recommendation.
- **Prefer vendoring and patching rpi, with a maintained Pi baseline.** The
  user accepts owning customization rather than requiring an unchanged agent.
  This refers to `bigfish1913/pi-rust` and its `rpi-*` crates, not the other
  similarly named Rust ports. Goose was explicitly excluded from consideration.
  No runner has been installed or vendored at this checkpoint.
- **Execution can start in the API service and move to a temporary VM.** Many
  chats may finish with a handful of tools and need no separate compute
  environment. For more involved work, API execution could provide useful
  progress while a VM starts and later takes responsibility. Exact routing,
  warm-up and handoff policy remain proposals to test, not settled thresholds.
- **Local model testing remains sufficient for infrastructure development.**
  llama.cpp and the cached Gemma model are available. Use inert development
  credentials for now; real provider access and credential policy are a
  separate discussion. Success with this model would validate plumbing, not
  scientific capability or frontier-model compatibility.
- **Vite is development infrastructure.** Production must be able to serve
  static frontend assets and route API requests directly to the backend.

## 2. What exists now

The React chat uses assistant-ui through `useLocalRuntime`. It sends the
selected conversation branch to the standalone Rust `ol-rht-chat` service,
which calls a local OpenAI-compatible llama.cpp endpoint. The service runs
the model/tool loop and streams structured NDJSON events to the browser.

The Rust tools are `search` and `get_paper`, backed by the existing corpus
HTTP service. Paper lookup prefers an `ol:` public ID, with other supported
identifier namespaces as fallbacks. Tool results contain structured metadata
and corpus provenance. The model formats citations; titles link to a supplied
full-text destination when available, with publisher/local fallbacks. Links
open in a new tab. Full text is not returned by the current tool contract.

History and edit/regeneration branches are now persisted in Postgres through
a versioned chat-history API. The browser still owns the runtime and commits
terminal replies; an in-flight reply can be lost before its save. There is
no durable run service, sandbox runner, artifact store or reconnection protocol. The HTTP response owns generation; Stop or
disconnect drops the work. The current three-minute run limit and six-tool
budget are prototype limits, not workbench requirements. The repository has
an independent SQLx storage crate with a local Postgres container, described
in the [storage implementation](workbench-storage.md). The history integration
does not change generation's lifecycle or make its UI representation a native
agent session. [Chat history](chat-api.md#persisted-conversations) documents
save boundaries, optimistic conflicts and the missing recovery guarantees.

Do not confuse these representations:

| Record | Purpose |
|---|---|
| Conversation shown in the UI | What the user said, saw, edited and selected |
| Agent session | Native messages, tools, branches, compaction and context state used by a runner |
| Execution history | Attempts, completed operations, failures, waits and recovery decisions |
| Research workspace | Code, files, environment, datasets, kernels and jobs |
| Artifact provenance | How a particular result was produced and which evidence supports it |
| Model input | The context actually supplied for one model request |

One record may contribute to several of these views, but they have different
lifetimes and recovery rules. Assistant-ui supplies UI integration points;
it does not choose an effective scientific memory or context policy for us.

## 3. The Claude Science reference

Anthropic launched Claude Science on June 30, 2026. Its public documentation
describes a research workbench combining literature work, code execution,
specialist agents, session forks, scientific viewers and remote compute.
Artifacts retain links to generating code, environment and conversation;
a reviewer checks aspects such as citations and calculations.
[Announcement](https://www.anthropic.com/news/claude-science-ai-workbench).

The product page additionally describes persistent Python/R kernels,
iteration on figures and manuscripts, reusable scientific skills, and jobs
on existing machines or clusters. These are vendor-described capabilities,
not measurements of reliability or proof of the internal architecture.
[Product capabilities](https://claude.com/product/claude-science).

**Our design inference:** a durable research project must outlive individual
chat requests and workers. Results need identities and histories of their
own. Conversation branching eventually interacts with filesystem revisions,
datasets and external jobs. We should evaluate the backend against those
requirements even while the first implementation remains small.

Scientific validity is an additional concern: retaining an execution record
helps inspect a result but does not establish that its method or conclusion
is sound. Reviewer findings should be inspectable records, not a blanket
claim that the output is verified.

## 4. What the off-the-shelf survey established

**Sources were read on 2026-09-26.** These are documented capabilities and
fit assessments, not comparative benchmarks or evidence that the industry
has converged on one architecture. Recheck release status and integration
support before selecting versions.

| Candidate | Documented contribution | Assessment for this project |
|---|---|---|
| **Mastra + Postgres, optionally Temporal** | Threads, messages, memory, workflow snapshots and observability storage; a Temporal workflow integration | Substantial potential replacement for custom backend code, at the cost of a TypeScript service and adopting its abstractions. Its fit with native coding-agent sessions and research forks is untested. |
| **LangGraph + Postgres** | Thread checkpoints, state history, forks, interrupts and recovery from saved steps | A substantive implementation of state machinery. Adopting graph/checkpoint semantics remains an architectural commitment; a checkpointer alone is not a complete deployed job supervisor. |
| **Vercel AI SDK + Temporal** | Model/tool abstractions, durable model activities and live streaming via Workflow Streams | A narrower TypeScript composition, with application conversation storage still to integrate. |
| **Pydantic AI + Temporal** | Durable model/tool execution and frontend stream integration; separate Harness step persistence | Useful Python candidate. The inspected Harness step store has no built-in Postgres adapter, so it would not remove all storage work. |
| **Temporal Rust SDK + SQLx** | Durable workflows/activities while retaining Rust application storage | Fits the existing language and cluster; leaves agent-specific behavior and session integration to us or a runner. |
| **assistant-cloud** | Hosted thread/message persistence, titles, attachments and run reporting, including custom-backend integration | A real outsourcing option, but changes the premise of storing the conversation in our Postgres. |
| **Pi** | Coding-agent loop, native sessions, branching, compaction, tools and structured process control | Reference baseline for the proposed customized Rust runner. Keep a pinned, runnable comparison rather than assuming feature parity. |
| **rpi (`bigfish1913/pi-rust`)** | Rust libraries for providers, agent loop, tools and sessions, with a CLI using those libraries | User's preferred candidate to vendor and customize. Evaluate both bounded API execution and isolated workers; maturity, published-version features and upstream compatibility need validation. |

Primary sources for the comparison:

- Mastra [storage](https://mastra.ai/docs/storage) and
  [Temporal deployment integration](https://mastra.ai/integrations/deploy/temporal).
- LangGraph [persistence](https://docs.langchain.com/oss/python/langgraph/persistence)
  and [checkpointers](https://docs.langchain.com/oss/python/langgraph/checkpointers).
- Temporal's [AI SDK integration](https://docs.temporal.io/develop/typescript/integrations/ai-sdk).
- Pydantic AI [persistence](https://pydantic.dev/docs/ai/core-concepts/persistence/),
  [Temporal integration](https://pydantic.dev/docs/ai/capabilities/durable_execution/temporal/)
  and [step persistence](https://pydantic.dev/docs/ai/harness/step-persistence/).
- Temporal [Rust SDK](https://docs.rs/temporalio-sdk/latest/temporalio_sdk/)
  and [1.0.0 release](https://github.com/temporalio/sdk-rust/releases/tag/v1.0.0).
- [assistant-cloud integration matrix](https://www.assistant-ui.com/docs/cloud).
- Pi [CLI integration](https://pi.dev/docs/latest/cli-integration),
  [SDK](https://pi.dev/docs/latest/sdk) and
  [session format](https://pi.dev/docs/latest/session-format).
- rpi [repository](https://github.com/bigfish1913/pi-rust),
  [published crate documentation](https://docs.rs/crate/rpi-agent/latest) and
  [upstream compatibility audit](https://github.com/bigfish1913/pi-rust/blob/main/docs/native-pi-missing-features.md).

The Rust follow-up found several candidates, not an established drop-in
equivalent to Pi. rpi's published crate history inspected here starts in
August 2026; its repository and released crate documentation differ in
feature coverage. The preference to vendor it accepts evaluation and patch
maintenance, not a claim that it already matches Pi. The exact upstream
commit and available APIs must be inspected before implementation.

Other execution platforms examined were
[DBOS](https://docs.dbos.dev/architecture), which builds durable execution on
Postgres, [Restate](https://docs.restate.dev/), and
[Vercel Workflow SDK](https://github.com/vercel/workflow), which offers a
self-hosted Postgres backend. They are alternatives, not additional layers
proposed for this application. Existing Temporal operations weigh in its
favor, without settling the integration cost.

**Correction retained for future readers:** an indexed Temporal page still
called its Rust SDK public preview. The current published release was 1.0.0,
dated September 4, 2026. Do not carry the preview claim forward from old
search results. A recent 1.0 also does not establish years of production
experience for that language SDK.

The survey established useful reusable mechanisms. It did not establish
that every framework is merely templating, or that adopting a framework
would lower our total cost. Version upgrades, provider feature lag, stored
format migrations and eventual replacement all belong in that comparison.

## 5. Proposed division of responsibility

This is the current proposal, not a deployed design.

| Component | Owns or supplies |
|---|---|
| Rust service + SQLx + application Postgres | Project/conversation ownership, stable run identities, user decisions, artifact metadata and provenance relationships |
| Vendored rpi, subject to integration trials | Agent loop, model interaction, tool execution, native session state and context construction; Pi remains the comparison baseline |
| Bounded API executor | Short conversations using explicitly supplied service tools, without allocating a VM |
| Sandbox/VM worker | Working environment, scientific packages, processes and kernels for work needing those capabilities |
| Durable filesystem/object storage | Workspace revisions, datasets, native session checkpoints and artifact bytes |
| Temporal | Research-job lifecycle, coordination, waits, cancellation and recovery at defined boundaries |
| assistant-ui plus workbench components | Conversation, progress, review interactions and access to files/artifacts |
| Existing corpus services | Search, identity resolution, metadata and source provenance |

Application Postgres and Temporal's own persistence are separate logical
stores, even if hosted by the same database server. Application code should
use Temporal's APIs rather than query or modify its internal tables.

The proposed runner boundary needs only operations the product uses:
start or resume a session, observe progress, steer or queue input, cancel,
inspect status, and collect outputs. Pin a runner version and test those
operations. Avoid building a universal agent framework as the adapter.

The original two corpus tools can remain useful through Rust tool adapters
in the API executor and explicit integrations in a worker. Whether worker
access means native tools, a CLI, MCP, or a combination is open. The existing
HTTP corpus contract need not be replaced.

### API execution and promotion to a VM

The proposed starting point is a bounded executor in or alongside the Rust
API service, using only its configured tools and budgets. Search, paper
lookup and synthesis can complete there. Arbitrary scientific code, packages,
working files, kernels or longer computation motivate a separate worker.
Using the same pinned rpi core in both environments is an implementation
hypothesis to test; the current prototype loop need not be replaced in one
step, and not all model work must eventually use this runner.

VM preparation may overlap useful API work when escalation is expected.
Preparation alone does not give the VM ownership of the agent session. If
the API finishes before compute is needed, cancel or release the unused
allocation. Provisioning failure can leave API tools available while the
run reports that the requested compute capability is unavailable.

Keep the conversation, logical run and browser event interface stable across
the transition. Represent API and VM execution as separate stages/attempts
with explicit identities. A proposed handoff is:

1. Request an environment using a stable allocation identity; continue only
   work that the API executor is equipped to finish or checkpoint.
2. Reach a supported checkpoint boundary. Finish or explicitly interrupt the
   current model/tool step and persist its status, native session position,
   tool results, pending work, evidence and workspace references. An in-flight
   model stream or process stack is not automatically transferable.
3. Atomically transfer ownership of the run to the ready worker, using an
   owner/generation record and checks that reject stale writes. Only one
   executor may advance the active session. Already dispatched external work
   still needs explicit cancellation or reconciliation.
4. Restore with the matching runner version and acknowledge the checkpoint
   before continuing. Add the worker's capabilities to subsequent model input;
   do not re-execute completed tools merely to reconstruct context. Persist
   progress through the same API so the UI can keep observing and steering.

The handoff must be recoverable if either process fails between these steps.
Record its intent and checkpoint so recovery can determine who owns the run
and what actually completed. A shared transcript alone cannot provide this.
The first implementation can hand off between completed turns; transferring
partially completed steps requires additional runner-specific support.

Temporal can supervise provisioning and longer stages. A successful bounded
API conversation does not need a VM allocation or a Temporal workflow merely
to fit the architecture. Durable run state must still be independent of the
browser connection once the prototype's request-owned lifecycle is replaced.

## 6. Preserve native sessions and control framework churn

rpi is the preferred customization candidate; the following Pi contracts
describe the baseline, not a guarantee that its Rust port implements the
same APIs or persisted formats. Pi documents a long-lived JSONL RPC mode for
bidirectional process control,
including integrations from languages other than TypeScript. This provides
a plausible Rust-to-runner boundary without embedding a Node SDK in the
application service. [RPC documentation](https://pi.dev/docs/latest/rpc).

Its persistent session is an entry tree, including more than visible chat:
messages, model changes, compaction and branch-related state. Its session
manager selects the active branch and constructs model context.
[Session lifecycle](https://pi.dev/docs/latest/sdk),
[persisted format](https://pi.dev/docs/latest/session-format).

**Proposed storage rule:** retain the native record, its format version,
runner version and identifiers. Derive a browser-friendly transcript from
it. Do not reduce it to rendered Markdown or assume the UI representation
can reconstruct everything the runner needs to continue. A generic JSON
field is not sufficient by itself: restoration requires the matching
reader and a migration policy.

There must be one writer for a native session at a time and a defined mapping
between its IDs and application IDs. The database can index and retain
native entries or refer to durable checkpoints; that physical choice is
still open. Loading a transcript must not execute its historical tools.

Provider churn remains real. The intended advantage is concentrating it in
the runner and its adapter rather than duplicating model protocols across
the product. The trial must examine feature loss during rendering and
restoration, compatibility across upgrades, and the cost of pinning an old
runner when a new provider capability is needed.

### Vendoring and the Pi comparison

Record the rpi upstream commit, license, local patch history and session
format alongside the application integration. Keep product-specific tools
and policy in adapters where practical, and make required core changes
explicit. Updating upstream must account for stored sessions and the ability
to roll back; owning the source does not remove those compatibility costs.

Maintain a separately pinned upstream Pi baseline with comparable models,
tool contracts, inputs and budgets. Compare task outcomes and operational
behavior: evidence retention, context/compaction, interruption, restoration,
tool errors and event rendering, as well as resource use and latency. Test
the application's API-to-VM handoff separately. Exact wording need not match,
and shared native-session files or interchangeable extension APIs must not
be assumed. Define the concrete comparison protocol with the first trial.

## 7. A disposable worker with a durable workspace

The project must survive destruction of its VM. Candidate durable records
include code revisions, input manifests, environment specifications, agent
sessions, result files and external job IDs. Object storage and workspace
volume technology have not been selected for this work.

Working files, published artifacts and kernel memory have different
properties. A filesystem checkpoint does not automatically capture a live
Python or R kernel. Restoring a machine may require rerunning recorded code
or loading explicit computational checkpoints. Keeping an environment warm
across an analysis is a separate lifetime/cost policy from retaining it
forever or starting a new VM for every message.

Isolation is part of the runner contract: which files and resources are
mounted, what network access is permitted, how resource use is bounded,
and how access to external services is supplied. These choices belong to
the execution environment; an LLM prompt cannot enforce them. The concrete
VM technology, image contents and credential policy remain open.

**Research forks need more than a message parent.** A fork should identify
an agent-session point, a corresponding workspace/code revision and the
input/result versions it inherits. It must also define what happens to
already submitted jobs. Switching a conversational branch does not undo
filesystem writes or external effects. Forking a running kernel may be
unsupported initially; the UI should say what a fork actually restores.

## 8. Durability and recovery contracts

Temporal is a candidate supervisor for operations such as:

1. Allocate or reconnect to an execution environment.
2. Restore the chosen workspace and agent-session checkpoint.
3. Start or attach to the requested agent work.
4. Wait for user input, an external job or a completed stage.
5. Register durable outputs and their provenance.
6. Release temporary resources according to policy.

If a runner owns its internal loop, wrapping its whole run in one Temporal
activity does not checkpoint every internal model request or shell command.
The activity boundary determines what Temporal can recover. A retry must
find and reconcile existing work before deciding to launch a new attempt.
This is different from a framework integration that schedules each model
call and tool call as its own activity.

Temporal retries can re-execute activities whose completion was not durably
recorded. Recorded results can be reused during replay, but arbitrary
external side effects still need idempotency or reconciliation.
[Temporal retry semantics](https://docs.temporal.io/encyclopedia/retry-policies).

The implementation needs explicit behavior for these cases:

| Situation | Required distinction or proposed response |
|---|---|
| Browser disconnects | Detach from observation versus cancel work must be a product decision; current chat cancels on disconnect |
| User presses Stop | An explicit cancellation request with a recorded outcome, including jobs that cannot be stopped immediately |
| Rust API restarts | Reconnect using stable run/session identities; do not start another run merely to restore the UI |
| API-to-VM handoff is interrupted | Reconcile allocation, checkpoint and recorded owner/generation before either executor resumes; keep the same logical run |
| Runner dies | Restore a known checkpoint and reconcile ambiguous work; report what could not be recovered |
| Model stream dies | A new request is a new attempt; do not append its output blindly to a failed prefix |
| Compute job was submitted before failure | Inspect the recorded external job identity before resubmitting |
| Database commit succeeds but workflow start fails | Use a reliable handoff, such as an outbox with stable workflow IDs; do not imply a cross-system transaction exists |
| Output exists but registration failed | Reconcile or retry publication using stable output identities |

Application intent, worker status and observed stream progress should remain
distinguishable. A closed socket or process exit is not sufficient evidence
that a scientific run completed successfully. A restored session is not
proof that an interrupted external command was harmless to repeat.

Live output also needs a transport contract. Temporal documents Workflow
Streams for [Python](https://docs.temporal.io/develop/python/workflows/workflow-streams)
and its [TypeScript AI SDK integration](https://docs.temporal.io/develop/typescript/integrations/ai-sdk).
This is evidence of reusable transport support, not proof of a ready-made
Rust/rpi integration. Stream retention and completion differ from permanent
artifact/history storage. Exact token replay, reconnect offsets, attempt
boundaries and partial-output durability remain to be specified.

## 9. Scientific artifacts and evidence

Proposed first-class objects include figures, tables, notebooks, reports,
code and evidence collections. Store their metadata and relationships in
Postgres; large bytes can live in a durable file/object store. Their
identities should survive moving or renaming a path.

For each published artifact version, aim to retain:

- Producing run/attempt and relevant session position.
- Exact input references and immutable versions or content hashes where
  available, including the corpus epoch/batch for retrieved evidence.
- Code revision or script, invocation parameters, and applicable seeds.
- Environment/image/package information and relevant compute settings.
- Output identity and hash, plus a human-readable account of the operation.
- Evidence references, review findings, and links to preceding versions.

These are proposed provenance requirements, not a promise of bit-for-bit
reproduction: external services, unavailable inputs and nondeterministic
computation can prevent it. Record known gaps.

Preserve the exact `search`/`get_paper` results used by an analysis, with
canonical `ol:` IDs where available, other identifiers and provenance.
Opening an old project must not silently replace its evidence with today's
corpus response. Metadata search hits are not findings from a study; a
title link is not proof that the agent read the full text.

Model-written citations remain the current implementation. Deterministic
citation formatting and richer evidence UI are deferred design questions,
not prerequisites for recording the sources already returned.

## 10. How this revises the first SQLx proposal

The initial proposal was an end-to-end persisted chat with four tables:
`chat_threads`, `chat_messages`, `chat_runs` and `chat_model_calls`. It included
message-parent branches, exact tool-result snapshots, model-input recording,
idempotent submissions, periodic text checkpoints, run expiry/recovery, and
a server-authoritative history API.

The user considered it a good plan but questioned how much repeated existing
work. The subsequent framework survey first suggested a Mastra trial. The
user then clarified the cost of framework churn, the Pi-in-a-VM candidate,
and the Claude Science target. A Pi runner trial was then proposed. The
latest clarification prefers vendored rpi with a Pi baseline, bounded API
execution and optional VM handoff, with Temporal available for supervision.

**Retain:** Postgres/SQLx, stable IDs, explicit migrations, structured data,
historical evidence snapshots, concurrency checks, idempotent commands,
and preservation of edits/alternatives.

**Reconsider before fixing the schema:** treating a thread as the outermost
product object; independently reimplementing the runner's session tree;
using the UI transcript as model state; making `chat_runs` into a homegrown scheduler;
and storing each model request in a new table when native runner records
may already carry some of the information. The completeness of model-input
capture must be measured, not assumed from either representation.

**Still open:** which native records belong in Postgres versus checkpoint
objects, what indexes/read models the UI needs, and how project, workspace,
session, run and artifact identities relate. These can be settled through
a small vertical trial without implementing the eventual full workbench.

### Local Postgres foundation

The first [storage slice](workbench-storage.md) implements the following
foundation independently of chat, runner and workflow integration:

- A small `Containerfile` pinned to the official multi-platform Postgres 18
  digest (18.6). The inspected homelab manifest declares PostgreSQL 18.1;
  the local image matches its major without changing the cluster.
- Compose configuration for loopback-only access, development
  credentials, readiness, and a persistent named volume.
- `just` recipes for start/stop, logs, shell, migrations and database tests.
  Stopping preserves data; resetting the volume is an explicit operation.
- SQLx migrations in source control and checked-query offline metadata so
  ordinary compilation does not require a running database.
- Integration tests against real Postgres, with isolated test databases.

PostgreSQL 18's official-image volume layout differs from older images;
use the [image documentation](https://github.com/docker-library/docs/blob/master/postgres/README.md).
SQLx documents [offline query checking](https://docs.rs/sqlx/latest/sqlx/macro.query.html)
and [database tests](https://docs.rs/sqlx/latest/sqlx/attr.test.html).
Schema migrations should be explicit application operations rather than
only first-initialization container scripts.

This local database container does not require containerizing every local
service or change the production frontend's independence from Vite.
Remote infrastructure changes belong to the existing
[deployment workflow](deployment-plan.md), separately from this design record.

## 11. Proposed path to a first demo

The next trial should answer architectural questions with one small piece of
scientific work. It should not become a broad framework bake-off or require
every prospective scientific domain.

1. **Define the project/run/session boundary.** Decide stable IDs, the native
   records to preserve, and what one writer, cancellation and restoration
   mean. Establish the local Postgres/SQLx foundation as needed.
2. **Trial vendored rpi in the API path, keeping a Pi baseline.** Inspect and
   pin upstream, isolate local patches, retain native sessions and versions,
   and translate events to the existing UI. Verify that search/paper chats
   can finish without allocating a VM. Compare the required contracts to Pi.
3. **Add a VM stage with a controlled handoff.** Provision an isolated
   workspace, transfer responsibility at a supported checkpoint boundary,
   and keep observing the same logical run. Initially use a completed-turn
   boundary; overlap warm-up with API work after the basic transfer works.
4. **Produce one real artifact.** Choose a small, appropriate dataset and
   analysis, generate a figure, and retain the input/code/environment manifest.
   Literature metadata alone is not a substitute for analysis data.
5. **Reopen and revise it.** Restore the project, make an intentional change,
   keep the earlier artifact, and show how both versions were produced.
6. **Exercise recovery and a research fork.** Add the Temporal lifecycle
   integration at the point it supervises real work; interrupt workers and
   observation, restore or report the checkpoint boundary, and demonstrate
   a fork with defined workspace semantics.

Record the outcome against these acceptance questions:

- Can the UI reopen a project with the same evidence, branches and artifacts?
- Can a short conversation complete without provisioning compute?
- Does handoff preserve completed work and enforce one active session writer?
- Can provisioning failure or an interrupted handoff be reconciled explicitly?
- Can we restore the agent from native state without losing required content?
- Can the browser reconnect without duplicating work or mixing attempts?
- Can Stop reach the runner and account for outstanding external jobs?
- Does a worker crash cause a controlled recovery or an honest interruption?
- Can we inspect the code, inputs and environment behind a figure version?
- Does a fork preserve the original and use the intended workspace revision?
- Which custom components remain, and did reuse actually reduce maintenance?

Measure latency and infrastructure behavior separately from scientific task
quality. The local small model is suitable for exercising contracts; a later
capability evaluation will need representative tasks and models. No
scientific demo result or framework compatibility test is claimed here.

## 12. Open decisions to carry forward

- rpi vendoring: upstream revision, local patches, embedded API execution
  versus a separate process, native restoration and upgrade/rollback strategy.
- Pi baseline version, comparable tools/inputs and the operational behaviors
  each runner update must preserve.
- API-to-VM promotion: triggers, budgets, speculative warm-up, checkpoint
  boundaries, capability changes, ownership transfer and return/reuse policy.
- Sandbox technology and lifecycle: per job or reusable environment, workspace
  mounts, checkpoints, kernel recovery, limits and external service access.
- Temporal granularity: job/session/stage boundaries, how to attach to live
  work, retry policy, cancellation and ambiguous external effects.
- Durable stream mechanism and the explicit meaning of "saved" for partial
  text, native session entries and scientific outputs.
- Storage layout and ownership: native records, UI projections, artifact
  manifests, blob retention and deletion behavior.
- Research fork semantics while a kernel or remote compute job is active.
- Context policies: initial runner defaults, scientific project state beyond
  a transcript, retrieval/compaction experiments and how to evaluate them.
- First scientific demo task, dataset, artifact and review criteria.
- Authentication, collaboration, approvals and real model credentials as
  concrete product requirements emerge.

On resuming this work, inspect the implementation and update this checkpoint
before assuming any proposal has landed. Keep dated findings and corrections,
and promote choices to implementation decisions only when actually made.
