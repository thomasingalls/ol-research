# Workbench Postgres storage

Implemented on `feature/chat`, 2026-09-26. This is the first storage slice
from the [workbench design](research-workbench-design.md): a local Postgres
container, SQLx pool and migrations, and Rust persistence APIs for projects,
sessions and native checkpoints.

The chat API and assistant-ui now use this store for a versioned conversation
history projection: messages, edit/regeneration branches and selected head
survive reloads. See [chat history](chat-api.md#persisted-conversations) for
its write boundaries and failure behavior. This is **not native agent state**
or durable execution. There is no rpi dependency, execution lease, run
scheduler, Temporal integration, VM allocation or artifact store. Database revisions prevent conflicting checkpoint commits;
they do not stop an obsolete executor from performing external tool calls.

## Local setup

Docker must be running. From the repository/worktree root:

```sh
just db-up
just db-migrate
just db-ping
just db-test
```

The Compose recipes use the current Docker context. On the development Mac,
that is Docker Desktop (`desktop-linux`); `DOCKER_CONTEXT=desktop-linux`
can select it explicitly. No native Postgres server or psql is required.

The [Containerfile](../dev/postgres/Containerfile) pins the official
multi-platform Postgres 18 image by digest, resolved to 18.6 on Debian
trixie. The inspected cluster manifest uses major 18 (18.1); nothing in the
cluster is changed. The [Compose configuration](../dev/postgres/compose.yaml)
publishes only `127.0.0.1:55432`. Override `OL_RHT_POSTGRES_PORT` consistently
for both Compose and the recipes if that port is occupied.

Local defaults are user/database `ol_rht`, password `local-dev-only`. These
are disposable development credentials, unrelated to model provider keys.
The image's initial role is a superuser so SQLx can create isolated test
databases; it is not a production application-role configuration.

The named volume `ol-rht-postgres_postgres-data` mounts at
`/var/lib/postgresql`, using Postgres 18's versioned data-directory layout.
The fixed Compose project name intentionally shares this development
database across worktrees. Stop it when unused:

```sh
just db-stop     # retain container and data
just db-down     # remove container/network; retain data volume
just db-up       # reuse the retained volume
```

`just db-status`, `just db-logs` and `just db-shell` expose status, logs and
containerized psql. **`just db-reset` deletes the named volume and all local
application data.** It is never invoked by startup, migration or tests.
After an intentional reset, run `just db-up db-migrate`.

An image update does not migrate an existing data directory across Postgres
major versions. Change the pin deliberately; a major upgrade needs its own
data migration procedure. See the
[official image documentation](https://github.com/docker-library/docs/blob/master/postgres/README.md).

## Rust boundary

The [`workbench-store` crate](../crates/workbench-store/src/lib.rs) owns the
pool, migration set and storage methods. It has no dependency on the chat
wire protocol or an agent SDK. `Store::connect(url)` configures five pooled
connections, a five-second acquisition deadline and 30-second statement and
idle-transaction deadlines. Statement logging is disabled. Connection does
not apply migrations; `Store::from_pool` supports caller-managed pools.

`workbench-db migrate` explicitly applies embedded SQLx migrations, including
their checksum validation and migration locking. `workbench-db check` tests
connectivity only; it does not certify that the schema is current. Both use
`OL_RHT_DATABASE_URL` and have no implicit database URL when invoked directly:

```sh
# Development default; the just recipes supply this automatically.
export OL_RHT_DATABASE_URL='postgres://ol_rht:local-dev-only@127.0.0.1:55432/ol_rht?sslmode=disable'
cargo run -p workbench-store --bin workbench-db -- migrate
```

The library supports PostgreSQL TLS through rustls and native trust roots;
connection settings remain explicit in the URL. The disabled TLS setting
above is for the loopback development container. Configure remote database
credentials and certificate verification for that deployment separately.
This database is application storage, separate from Temporal's internals.

## Schema and guarantees

The [initial migration](../crates/workbench-store/migrations/202609260001_workbench.sql)
creates three tables:

| Table | Purpose |
|---|---|
| `workbench_projects` | Stable UUID, title and creation time |
| `workbench_sessions` | Stable UUID, project reference, title and storage revision |
| `workbench_checkpoints` | Snapshot UUID, session/revision, reader envelope and exact native bytes |

Callers choose stable UUIDs before submitting writes. Repeating project or
session creation with the same ID and data returns the existing record;
reusing the ID for different data returns `Error::IdConflict`.

A checkpoint records `runner`, `runner_version`, `format`, `format_version`
and `native_session_id`. `payload` is `bytea`, preserving bytes exactly,
including JSONL whitespace and ordering. The store does not deserialize,
normalize, render or execute it. Select the appropriate native reader using
the stored envelope. A runner version can be an upstream revision plus a
local-patch identifier; recording a version does not supply a migration.

`save_checkpoint(project_id, session_id, checkpoint_id, expected_revision,
&state)` locks the session row and commits the new snapshot and revision in
one transaction. Sessions start at revision zero. Two different writes
expecting the same revision cannot both commit. A failed insert rolls back
the revision update. The unique `(session_id, revision)` constraint provides
an additional database invariant.

If the response is lost after commit, retry with the **same checkpoint ID,
expected revision and state**. The original checkpoint is returned even if
newer snapshots have since been saved, without moving the head backwards.
Changed data or a different expected revision with that ID is a conflict.
For an actual revision conflict, reload current state and decide how to
reconcile; do not blindly resubmit stale bytes with the newer revision.

`latest_checkpoint` reads the head; `checkpoint` retrieves an earlier
snapshot without changing it. Native conversation branches remain in the
native payload. A storage revision is not an agent turn, a conversation
branch, a run attempt or a workspace snapshot. Old snapshots are immutable
through this API, which offers no update or delete operation for them.

Session and checkpoint reads/writes require the owning project ID, and
database foreign keys prevent missing parents. This scoping is not user
authorization: the eventual application must check access before calling
the store. Session listing uses bounded UUID pagination and excludes payloads.
Titles are limited to 512 characters, envelope fields to 256, and snapshots
to 1 byte–8 MiB. Artifact blobs and larger runner state need a later storage
decision. Saving a full native snapshot per revision is deliberately simple;
append/delta storage can be introduced when the runner's real format is known.

No cascade deletion or retention policy is implemented. No assumptions about
model message roles, tool schemas, compaction or rpi's wire format are baked
into these tables. Run identities, execution ownership, handoff checkpoints,
artifact provenance and native runner integration remain subsequent slices.
The current chat projection is explicitly tagged `ol-rht-chat-history`,
version `1`, with runner `ol-rht-chat` and its package version. A future
native runner needs its own format and reader; the chat API refuses to
read or overwrite an unsupported envelope.

## Queries, migrations and tests

SQLx 0.9 checks the storage queries against Postgres. The generated
[`.sqlx` metadata](../.sqlx) is committed, and `.cargo/config.toml` defaults
`SQLX_OFFLINE=true`, so normal compilation, Clippy and workspace tests need
no database. Connectivity probes and test-only SQL use runtime queries.

When changing a migration or query:

```sh
just db-tools           # pinned SQLx CLI; installed under ignored target/
just db-up db-migrate
just db-prepare         # refresh metadata against the local schema
just db-prepare-check   # fail on metadata drift
just db-test
cargo clippy -p workbench-store --all-targets -- -D warnings
```

Add a new forward migration once a migration has been shared/applied; do
not rewrite its checksum. `build.rs` tracks the migration directory so newly
added migrations are embedded on rebuild. Metadata commands deliberately
use the local container URL, not `OL_RHT_DATABASE_URL`.

Database tests use `#[sqlx::test]` to create one database per test and clean
up successful tests. They are explicitly ignored by ordinary workspace
tests; `just db-test` runs them and fails if the database is unavailable.
`OL_RHT_TEST_DATABASE_URL` can point at a dedicated test Postgres with
database-creation privileges. It does not inherit `OL_RHT_DATABASE_URL`.
Failed-test databases may be retained by SQLx for diagnosis.

The recipe runs both the storage crate and chat API database suites.
Chat tests cover HTTP creation/listing, reload after API restart, full
branch-tree round trips, scoping, revision conflicts, delayed retries,
malformed/oversized history and unsupported formats.

Storage coverage includes migration repetition, idempotent creation, project scoping,
pagination, exact-byte restoration after reconnect, delayed and concurrent
retries, competing writers, atomic rollback, invalid inputs and a property
test over arbitrary binary payloads. These tests do not require a model,
corpus, VM or Temporal service.
