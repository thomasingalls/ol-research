# AI chat UI libraries, September 2026

**Snapshot: 2026-09-26.** Recorded from the chat UI research conversation at
the user's request. This is a dated comparison of documentation, repository
activity, release notes and package manifests, not a benchmark or a promise
that the same versions and recommendations will still apply later. The
implementation notes in [the web README](../../web/README.md#chat-ui) describe
what this repo actually installed; the versions surveyed below are not its
lockfile.

The target is a full chat screen with modern model interactions: tools,
files, rich answers and eventually research workflows. Voice is not a
priority. Staying in the shadcn ecosystem, preferably using its installation
registry, is a strong preference. An “embedded copilot” means an assistant
inside another application that can read its current state and operate its
controls; that is a different emphasis from the chat screen being built here.

## The decision

Start with **assistant-ui**. It combines source-owned shadcn components with
a conversation runtime, so the project gets more than the appearance of a
chat. The user agreed with this recommendation and requested the first
integration on `feature/chat`, with no real model connections or credentials.

For a future model backend, **assistant-ui plus Vercel AI SDK** is the first
combination to evaluate. That is a recommendation, not a backend decision:
assistant-ui also supports custom runtimes and other integrations. The first
implementation uses a local mock and installs no provider SDK.

The strongest alternative is **official shadcn chat primitives plus selected
Vercel AI Elements**, with the application taking responsibility for more
conversation behavior. Add json-render or OpenUI only if the product needs
models to compose layouts from a component catalog. Ordinary tool results
can be rendered by ordinary React components.

## The distinctions that matter

“Supports shadcn” can mean three different things:

1. Uses a similar visual design or Tailwind tokens.
2. Gives you React source you can customize, possibly using shadcn internally.
3. Publishes components that the shadcn CLI installs through a registry.

Only the third directly answers the registry preference. None says who
owns message state, tool execution, stream transport or conversation storage.

There are also three different deliverables in this market: component
collections, conversation runtimes, and complete starter applications.
Comparing them as interchangeable chat widgets hides substantial work.
An attractive tool card is not a tool-call state machine; an approval button
is not a durable approval protocol; a scroll container is not chat history.

## The shortlist and the rest of the landscape

| Project | What it supplies | shadcn fit | Reading for this project |
|---|---|---|---|
| **assistant-ui** | Conversation runtime, primitives, copied UI components, backend adapters | Native registry; Radix and Base UI variants | Best starting point for a complete chat UX |
| **Vercel AI Elements** | Presentation components for AI SDK applications | Native registry and owned source | Strong visual toolkit; application still assembles conversation behavior |
| **Official shadcn chat components** | Message, attachment and scrolling primitives | Native ecosystem | Useful foundation; model/tool protocols live elsewhere |
| **Prompt Kit** | Polished chat and AI presentation components | Native registry | Selective component source; weaker recent maintenance evidence |
| **CopilotKit** | Agent/app state integration, tools, human interaction and chat UI | shadcn-compatible styling; native installation registry not verified | More attractive when acting inside the application is central |
| **Tambo** | Registered generative/interactable components, chat UI and agent backend | Own CLI copies components; native shadcn installation route not verified | A broader architectural commitment than the requested UI layer |
| **OpenAI ChatKit** | Embedded chat component, widgets and server protocol | Own theming; not source-owned shadcn | Different ownership model; not the preferred fit |
| **Ant Design X** | Substantial chat, markdown and agent UI ecosystem | Another design system | Credible alternative if the design-system preference changes |
| **LangChain agent-chat-ui** | Complete LangGraph chat application | Uses shadcn internally | Useful reference/starter, not a drop-in registry library |
| **LlamaIndex chat-ui** | Chat components and registry | Genuine registry | Distinguish the older chat project from the newer general UI project |
| **NLUX** | Conversational UI library | Not the preferred registry path | Weak evidence of recent substantive development |

### assistant-ui

Its important contribution is the runtime behind the components: message
state, editing, regeneration, branching and integration points for tools and
other capabilities. The [custom runtime guide](https://www.assistant-ui.com/docs/runtimes/custom/overview)
distinguishes a runtime that owns state from an external-store adapter that
connects state the application already owns. This offers a route from a small
local prototype to a custom backend without replacing the screen.

The [branching guide](https://www.assistant-ui.com/docs/guides/branching)
documents conversation branches rather than treating edits as destructive
overwrites. Backend adapters include AI SDK, LangGraph and agent-protocol
integrations, but capabilities are not identical across all adapters. Check
the chosen adapter, particularly for approvals, attachments and persistence.
The [adapter concepts](https://www.assistant-ui.com/docs/runtimes/concepts/adapters)
are a useful boundary between UI behavior and services the application must
provide.

The current [AI SDK 7 integration](https://www.assistant-ui.com/docs/runtimes/ai-sdk/v7)
was a particularly good freshness signal. The manifest inspected for
`@assistant-ui/ai-sdk` declared peers `ai ^7.0.101` and
`@ai-sdk/react ^4.0.104`. The old package names and older tutorials are not
necessarily the right starting point for a new integration.

This is still integration work. [Resumable streams](https://www.assistant-ui.com/docs/guides/resumable-streams)
need backend support. Cloud storage is optional, and a persistence adapter
is not itself a database. Rendering markdown has alternatives, including the
[Streamdown integration](https://www.assistant-ui.com/docs/guides/streamdown).

Maintenance evidence was unusually current: `@assistant-ui/react 0.15.22`
and `@assistant-ui/ai-sdk 0.0.8` were released on September 24, with further
main-branch activity on September 26. Recent work covered resolved approval
state, descriptors and stream/state fixes. That is evidence of attention to
current interaction patterns, accompanied by API churn and some explicitly
unstable APIs. Sources: [React release](https://github.com/assistant-ui/assistant-ui/releases/tag/%40assistant-ui/react%400.15.22),
[AI SDK adapter release](https://github.com/assistant-ui/assistant-ui/releases/tag/%40assistant-ui/ai-sdk%400.0.8),
[adapter manifest](https://github.com/assistant-ui/assistant-ui/blob/main/packages/ai-sdk/package.json).

### Vercel AI Elements

AI Elements is a collection of copied, composable UI components. Its range
includes conversation and input, attachments, reasoning, sources, tool
states, confirmations, plans, queues, artifacts, terminal output, file trees
and previews. It gives the application useful ways to display model activity;
it is not a replacement for the state and transport behind those displays.
Sources: [overview](https://elements.ai-sdk.dev/docs),
[tool component](https://elements.ai-sdk.dev/components/tool),
[confirmation component](https://elements.ai-sdk.dev/components/confirmation).

The setup is native to the shadcn registry. Documentation is Next-oriented
and the stack expects React 19 and Tailwind 4; this repo already has those
two, but uses Vite. That difference should be checked component by component,
not interpreted as an automatic incompatibility.
[Setup](https://elements.ai-sdk.dev/docs/setup).

An important version nuance: the inspected `packages/elements/package.json`
still used `ai ^6.0.105` and `@ai-sdk/react ^3.0.41`, while other Vercel
applications had moved to AI SDK 7. This does **not** establish that copied
Elements components fail with SDK 7. It establishes that compatibility needs
verification rather than inference from the common vendor.
[Manifest](https://github.com/vercel/ai-elements/blob/main/packages/elements/package.json).

The latest main commit observed was August 21, including a question
component. The latest CLI release observed, `ai-elements 1.9.0`, was March
12. A CLI package release and the live registry's content have different
cadences; the older CLI date alone is not evidence of abandonment. License:
Apache-2.0, checked against the repository's license rather than relying on
GitHub's classification. [Commits](https://github.com/vercel/ai-elements/commits/main/),
[CLI release](https://github.com/vercel/ai-elements/releases/tag/ai-elements%401.9.0).

### Official shadcn chat primitives

The June 2026 additions include MessageScroller, Message, Bubble,
Attachment and Marker, with headless behavior in `@shadcn/react` and
Radix/Base UI integration. They address real chat concerns such as streaming
scroll position, restoring anchors and prepending history. They do not
provide model transport, conversation persistence or the lifecycle of tool
requests. The announcement explicitly positions them alongside AI Elements.
[June changelog](https://ui.shadcn.com/docs/changelog/2026-06-chat-components).

The user's assessment was that these are fine components, but assistant-ui
is a better starting point for following evolving tool-call patterns. That
is a judgment about where the application should obtain behavior, not a
claim that shadcn's presentation components are defective.

### Prompt Kit

Prompt Kit is a real shadcn registry option, including `@prompt-kit` and
direct component URLs. It offers useful visual primitives, but leaves the
conversation engine to the application. Documentation examples inspected
still used AI SDK 5. The latest main activity observed was March 12,
primarily website/SEO work, and there were no GitHub releases. This supports
a more cautious maintenance rating, not a declaration that it is abandoned.
MIT licensed. [Installation](https://github.com/ibelick/prompt-kit/blob/main/app/docs/installation/page.mdx),
[documentation index](https://github.com/ibelick/prompt-kit/blob/main/llms.txt),
[commits](https://github.com/ibelick/prompt-kit/commits/main/).

### CopilotKit

CopilotKit is especially relevant when the agent shares application state,
operates app tools, renders interactive results and asks a human to intervene.
The v2 API is under `@copilotkit/react-core/v2`; that does not mean the npm
package itself has a 2.x version. AG-UI and agent integration are central to
its offering. Its current styling supports shadcn tokens and Tailwind 4,
but this survey did not verify an equivalent native shadcn installation
registry. [Docs](https://docs.copilotkit.ai/),
[styling](https://docs.copilotkit.ai/custom-look-and-feel/css).

Maintenance was strong: release `1.74.0` on September 25 included rendering
and per-message state fixes. The core is MIT; managed thread, memory and
analytics offerings belong to the separate Intelligence product surface.
Do not assume every advertised service is included in the open-source UI.
For this project it is a candidate if app actions become the main product,
rather than an obvious requirement for the initial chat screen.
[Repository](https://github.com/CopilotKit/CopilotKit),
[release](https://github.com/CopilotKit/CopilotKit/releases/tag/v1.74.0).

### Tambo

Tambo lets the application register components with schemas so a model can
select them and progressively fill their props. Interactable components
can be updated later in a conversation. It also supplies an agent and
conversation backend, using its cloud or a self-hosted deployment. That is
a larger adoption choice than installing presentation components.
Its own CLI copies UI into an application; native shadcn-registry installation
was not verified. MIT licensed. [Repository](https://github.com/tambo-ai/tambo),
[CLI](https://docs.tambo.co/reference/cli).

The React release observed was `1.3.0`, June 15. The latest default-branch
commit observed was August 5 and concerned dependencies. GitHub's later
September 23 `pushed_at` timestamp was not a main-branch commit date. These
are different maintenance signals and should not be substituted for one
another. [Release](https://github.com/tambo-ai/tambo/releases/tag/react-v1.3.0),
[commits](https://github.com/tambo-ai/tambo/commits/main/).

### ChatKit, Ant Design X and complete application starters

OpenAI ChatKit provides an embedded web component with a React wrapper,
theming and widgets. It is not source-owned shadcn UI. A custom Python
server can connect it to an agent service. A material current-docs change:
Agent Builder was deprecated, with shutdown scheduled for November 30,
2026; ChatKit remains available and the current direction for new apps is
a server implementation of their own. Older hosted quickstarts should not
be treated as the default without checking this transition.
[ChatKit](https://developers.openai.com/api/docs/guides/chatkit),
[custom integration](https://developers.openai.com/api/docs/guides/custom-chatkit),
[themes](https://developers.openai.com/api/docs/guides/chatkit-themes).

Ant Design X is a serious alternative with chat UI, streaming markdown,
SDK support and A2UI-related components. Docs showed `2.9.0` and the latest
main activity observed was September 20. The primary drawback here is
adopting another design system, not lack of scope.
[Introduction](https://x.ant.design/docs/react/introduce/).

LangChain's agent-chat-ui is a complete Next.js/LangGraph chat application
with shadcn components and an artifact side panel. It is useful as a
reference or a starter for that backend, but is not the same deliverable
as a registry library added to the existing Vite app.
[Repository](https://github.com/langchain-ai/agent-chat-ui).

The older LlamaIndex `chat-ui` does have a genuine registry, including
`https://ui.llamaindex.ai/r/chat.json`. Its latest main change observed was
December 16, 2025, a security dependency update, following August activity.
The separate `run-llama/llama-ui` project had September 21, 2026 activity
and broader document-preview/workflow components. That does not establish
maintenance of the older chat package; the newer project's React peer
requirements also deserve checking against React 19.
[Older chat UI](https://github.com/run-llama/chat-ui),
[newer UI project](https://github.com/run-llama/llama-ui).

NLUX's default `latest` branch showed a November 25, 2025 documentation
endpoint fix, with the preceding observed activity in September 2024.
That was insufficient current maintenance evidence to recommend it over
the shortlist. [History](https://github.com/nlkitai/nlux/commits/latest/).

## Vercel's products are different layers

| Product | Responsibility | What it does not settle |
|---|---|---|
| AI SDK Core | Models, providers, tool execution and structured generation | The visual chat experience |
| AI SDK UI / `useChat` | Client message state and stream handling | A finished screen or durable application storage |
| AI Elements | Visual components for those interactions | The whole conversation runtime and product |
| Streamdown | Rendering incomplete streamed markdown, with optional richer syntax | Thread management |
| json-render | Model-generated UI constrained to a registered catalog | A chat shell |
| Chatbot template | A complete Next.js application | A small, framework-neutral component installation |
| AI Gateway | Optional access/routing to models | A requirement imposed by the UI components |
| Chat SDK | Unified bots across messaging platforms | The same product as AI SDK UI or AI Elements |

Sources: [AI SDK](https://github.com/vercel/ai),
[AI frontend overview](https://elements.ai-sdk.dev/docs/vercel-ai-frontend),
[Streamdown](https://github.com/vercel/streamdown),
[Chat SDK](https://github.com/vercel/chat).
Chat SDK also has web-related integration; describing it as simply “no web”
would be too absolute. Its product emphasis remains distinct.

The [Chatbot template](https://github.com/vercel/chatbot) includes application
choices such as Auth.js, Postgres history, Blob attachments and artifacts.
Its inspected manifest already used `ai 7.0.15` and `@ai-sdk/react 4.0.16`,
despite the different versions in AI Elements. Its latest main commit
observed was July 8. It can use direct providers; AI Gateway is optional.
This is a useful reference, but replacing the current application with the
template would be a much broader change than this task.
[Manifest](https://github.com/vercel/chatbot/blob/main/package.json).

## Rich output: useful additions, not replacement chat runtimes

**Tool UI** supplies source-owned result widgets such as tables, charts,
forms and approvals. However, `assistant-ui/tool-ui` was archived when
checked. A September 26 assistant-ui commit folded it into the main
elements/generative-UI work, while shadcn's registry directory still listed
the older `@tool-ui` entry. Investigate that migration before installing
the old registry. A main-branch merge does not prove a corresponding npm
package has been published. [Old repository](https://github.com/assistant-ui/tool-ui),
[migration commit](https://github.com/assistant-ui/assistant-ui/commit/87e76e0070f1437c7f14f89bb43454e16efe0605),
[registry directory](https://github.com/shadcn-ui/ui/blob/main/apps/v4/registry/directory.json).

**json-render**, at `vercel-labs/json-render`, lets a model emit a JSON UI
specification drawn from an application-defined component/action catalog.
Rendering can progress as the specification arrives. Its
`@json-render/shadcn` package advertised 36 prebuilt Radix/Tailwind
components. A runtime component “registry” here is a different concept from
the shadcn installation registry. Apache-2.0; release `0.21.0` on September
18 and main activity on September 23 were observed.
[Docs](https://json-render.dev/docs),
[repository](https://github.com/vercel-labs/json-render),
[release](https://github.com/vercel-labs/json-render/releases/tag/v0.21.0).

**OpenUI** from Thesys (`@openuidev/react-lang`) supplies a UI description
language, schemas and progressive rendering. Its shadcn example includes
more than 40 components. This is worth comparing with json-render if
generated layouts become important; it is not needed just to render a
known search-results component. Do not conflate it with the older W&B
OpenUI project or assume that every OpenUI use requires the same hosted
service as Thesys C1.
[shadcn example](https://www.openui.com/docs/openui-lang/examples/design-systems/shadcn).

The community `shadcn.io` collection is also distinct from official
`ui.shadcn.com`. A third-party gallery or registry entry is evidence of
available components, not a guarantee of a maintained conversation engine.

## What “keeps up with model capabilities” should mean

Use concrete interactions to evaluate a candidate:

- **Interleaved parts:** text, exposed reasoning summaries, tool calls,
  results, sources and files in their original sequence. A reasoning panel
  can display only what the provider exposes; it does not reveal hidden
  chain of thought.
- **Tool lifecycle:** partial arguments, parallel calls, long-running work,
  results, errors and retries. Inspect behavior between the happy-path
  screenshots.
- **Human decisions:** allow, deny, custom choices, text answers and resume.
  Reload after a decision and verify that it is neither lost nor executed
  twice.
- **Conversation lifecycle:** edit an earlier message, switch branches,
  regenerate, navigate away, reload and restore the correct history.
- **Stream lifecycle:** disconnect, reconnect, resume and stop. UI stream
  cancellation is not necessarily cancellation of backend model work.
- **Provider escape hatches:** preserve useful provider metadata and custom
  parts instead of forcing every capability into plain text. AI SDK 7's
  evolving part protocol makes this a version-specific integration check.
- **Actual usability:** keyboard/IME input, focus, mobile layout, long
  histories, scroll anchoring, markdown while incomplete, code and citations.

The inspected AI SDK resume documentation specifically distinguished
disconnecting a browser stream from stopping the backend generation.
Resumption needs stored stream identifiers and server infrastructure; a real
stop may need a separate endpoint. This is why a checkbox saying “resumable”
does not settle application behavior.
[Stream protocol](https://github.com/vercel/ai/blob/main/content/docs/04-ai-sdk-ui/50-stream-protocol.mdx),
[resume documentation](https://github.com/vercel/ai/blob/main/content/docs/04-ai-sdk-ui/03-chatbot-resume-streams.mdx).

If a comparison becomes necessary, run assistant-ui and shadcn + AI Elements
against the **same** backend and the same recorded interaction sequences.
Measure missing behavior and integration effort. Package popularity, a
recent release and a large component gallery are useful signals, but none
substitutes for that exercise.

## Does assistant-ui require its own CLI?

**No.** Its documented manual setup registers this source in `components.json`:

```json
{
  "registries": {
    "@assistant-ui": "https://r.assistant-ui.com/styles/{style}/{name}.json"
  }
}
```

Then the existing shadcn CLI can add `@assistant-ui/thread` or other
components. The source files belong to the application after installation;
the runtime remains an ordinary npm dependency. The style-aware endpoint
selects Base UI for styles starting with `base-`, and Radix otherwise.
The repo's `radix-rhea` style belongs to the latter.
[Installation](https://www.assistant-ui.com/docs/installation).

No assistant-ui cloud service or provider key is required for the UI or a
local mock runtime. A real backend would be a separate integration. The
[LocalRuntime guide](https://www.assistant-ui.com/docs/runtimes/custom/local-runtime)
describes the custom adapter boundary used for this first implementation.

**Observed during installation:** the live registry had moved beyond the
API in the npm versions permitted by this repo's 14-day minimum release
age. Typechecking identified the mismatch. The copied assistant-ui sources
were aligned with upstream release commit
[`af746615`](https://github.com/assistant-ui/assistant-ui/commit/af7466155560aa3ff364abf57e89c7209882ab0d)
(September 10, `@assistant-ui/react 0.15.19`), with the Radix trigger
adaptations retained. This is concrete evidence that owning registry source
also means managing its compatibility with packaged runtime versions.
The dependency-age policy was not relaxed.
