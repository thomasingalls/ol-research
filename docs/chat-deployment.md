# Public chat deployment

Prepared 2026-09-27. The first release is a public, shared demo, not a private
workspace. Anyone can list, read, continue or modify its conversations. The
UI keeps the public-history warning and the NVIDIA free-endpoint notice
visible above every conversation, including direct links to saved sessions.
It has no dismissal or remembered acknowledgment that could hide the warning.

The release uses the existing Tekton builds and Flux image automation. The
Rust image packages `ol-rht`, `ol-rht-serve`, `ol-rht-chat` and `workbench-db`.
The chat service calls the existing corpus HTTP API and does not mount its
PVCs. No corpus rebuild, GPU, Temporal or model VM is needed for this slice.

## Runtime

```sh
/cnb/process/ol-rht-chat --mode public-demo \
  --listen 0.0.0.0:50053 --corpus http://ol-rht-serve:50052
```

| Environment variable | Deployment value |
| --- | --- |
| `OL_RHT_CHAT_PROVIDER` | `openrouter` |
| `OL_RHT_CHAT_MODEL` | `nvidia/nemotron-3-ultra-550b-a55b:free` |
| `OL_RHT_OPENROUTER_PROVIDER` | `nvidia` (provider allowlist; no fallback) |
| `OPENROUTER_API_KEY` | Kubernetes Secret; never in a frontend build |
| `OL_RHT_DATABASE_URL` | Dedicated database/role with verified TLS; Kubernetes Secret |
| `OL_RHT_CHAT_MAX_CONCURRENT` | `2` |
| `OL_RHT_CHAT_REPLIES_PER_MINUTE` | `6` |
| `OL_RHT_CHAT_WRITES_PER_MINUTE` | `60` |

The public mode explicitly permits an operator-configured HTTP(S) corpus
service address. Redirects remain disabled, credentials/query/fragment in
that URL are rejected, and the OpenRouter key is sent only to OpenRouter.
Development mode still requires loopback for the listener and corpus.
The local model endpoint always requires loopback. The binary does not load
`.env`; only the local `just` recipes do.

Reply concurrency is held for the whole response stream, including provider
queueing and tools, and released on completion, failure or disconnect. New
replies and history PUTs have independent sliding one-minute limits. Refusals
return HTTP 429, a `Retry-After` header and a message the UI displays. History
reads and health probes do not consume those allowances. Limits are global
per process, reset on restart and multiply with replicas (including a rolling
update's temporary second pod). They are resource bounds, not per-user quotas
or fairness guarantees. Provider quotas still apply independently.

The existing 30 tool calls, 65,536 output tokens per model completion and
180-second overall generation deadline remain. Proxies have 240-second
request/backend deadlines for `/v1/chat`; other traffic keeps its old timeout.
SIGTERM/SIGINT stops accepting requests and drains existing responses; the
Deployment allows 210 seconds before termination. A lost browser connection
still cancels generation; there is no durable/background run recovery.

`GET /health` checks process liveness. `GET /ready` checks the database and
the checksums/completion of this binary's embedded migrations, with a two-second
deadline. Startup requires that schema too. Neither readiness nor serving
applies migrations; `workbench-db migrate` is a separate release Job. Later
successful migrations are allowed by readiness, so rollback still requires
checking that those migrations are backward compatible.

## Disclosure and endpoint suitability

The notice supplied by the user matches the free endpoint's
[OpenRouter disclosure](https://openrouter.ai/nvidia/nemotron-3-ultra-550b-a55b:free),
checked 2026-09-27. It links to [NVIDIA's privacy policy](https://www.nvidia.com/en-us/about-nvidia/privacy-policy/)
and [API Trial Terms of Service](https://assets.ngc.nvidia.com/products/api-catalog/legal/NVIDIA%20API%20Trial%20Terms%20of%20Service.pdf).
The endpoint collects/logs content for security and improvement; NVIDIA's
statement about identifiers does not make this application's history private.
Requests also pass through OpenRouter, whose privacy policy is linked in the UI.

The linked trial terms (§§1.2, 1.4 and 4.2) restrict production use and making
the API/output available to others. Confirm that the applicable provider
agreement permits this public demo before enabling its route, or select an
endpoint that permits it and update the disclosure and provider pin together.
A privacy notice alone does not establish permission for public deployment.
The prepared manifests stay suspended until the release steps are performed.

## First release

The operational runbook is in `~/Code/ol-homelab` at
`infra/clusters/homelab-talos/ol-rht/CHAT-ROLLOUT.md`.

1. Land the infrastructure preparation first: frontend image pinned, new
   chat/migration Flux Kustomizations suspended, database/role and secrets ready.
   Secrets are supplied manually under that repo's `AGENTS.md`; do not copy `.env`.
2. Merge this branch into main and let Tekton build both image lanes. Check
   the actual amd64 image contains all four CNB processes. CI builds images;
   it currently does not run the Rust or browser tests.
3. Pin the migration Job to that release, enable its Kustomization through
   Git, and wait for success. Never add an image-policy marker or TTL to a Job.
4. Apply the upstream `ol-infra` chat timeout change; enable the chat
   Deployment/Service/route through Git only after migration and endpoint review.
5. Verify public streaming, tool results, cancellation and history persistence.
   Then publish the matching frontend image and restore its image-policy marker.

The two application projects have separate stable IDs for development and
public-demo mode. Local history is not copied into production.

## Validation and operation

Run `just check`, `just chat-check`, `just db-test` and `just web-check` before
release; database tests use isolated local test databases. Validate the native
amd64 build on the production builder and inspect `/cnb/process` in its image.
If the local package-manager launcher cannot run, the already-installed
tools check the frontend without changing dependency versions or release-age rules:

```sh
cd web
./node_modules/.bin/tsc --noEmit
./node_modules/.bin/vite build
node --test tests/chat-*.test.mjs
```

Public smoke tests should send only synthetic/public content. Create a shared
conversation, generate a reply using corpus tools, stop a second reply, reload
history, then verify the same history after a Git-driven pod replacement.
Exercise a reply longer than 60 seconds to check both proxies. Verify 429
responses and recovery without hammering the real provider (use fake-provider
transport tests for repeated-limit checks).

History snapshots are append-only and have no retention policy yet. Monitor
the shared Postgres volume and backup status before inviting sustained traffic.
Rollback images through Git after disabling the relevant image setter; do not
roll back the database by deleting it or undoing applied migration files.
Removing the chat route and setting chat replicas to zero through Git is the
kill switch; database/history and the corpus can remain intact.

### Preparation checks (2026-09-27)

- Workspace formatting and Clippy passed; 549 workspace tests passed (15
  explicitly ignored). The parallel corpus fixtures needed a process-local
  file-descriptor limit of 4096 on macOS; the default limit was exhausted.
- All 13 isolated Postgres integration tests passed, including schema
  readiness and persistence across API restarts.
- Frontend typecheck, production build and all 19 adapter/history tests passed
  with the installed tools. The pnpm launcher would not install its pinned
  version under the repository's minimum-release-age rule; that rule was preserved.
- New Kustomize subtrees rendered successfully, changed YAML parsed, and the
  homelab remote-resource check passed. Secret files and the live cluster were
  not read or changed.
- The actual amd64 CNB build was attempted locally but QEMU crashed executing
  `rustc --version`, before application compilation. Native amd64 CI validation
  and public end-to-end smoke tests remain release gates. A browser visual
  check was unavailable because computer-use access to Safari was not granted.
