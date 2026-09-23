# Deployment plan

**Status:** draft 1, 2026-09-22 — planning only; nothing is built and
nothing in the homelab has changed. How ol-rht gets onto the box: the
image, the store on the box's disks, the runs that write it, the server
that reads it. Written from the hosting decision of 2026-09-17 (serving
plan §10, §11 item 28) and the walkthrough of 2026-09-22 (§9).
**Inputs:** [serving plan](serving-plan.md) §4 (the epoch), §5 (the
freshness path), §7 (the transports), §8 (machinery); [ingestion
plan](ingestion-plan.md) §3.5 (the store); the homelab repository
(`~/Code/ol-homelab`, Forgejo `oxidation-labs/monorepo`), read 2026-09-22
without touching the cluster.

## 1. The box, and the rules it comes with

The box is `talos-0mm-vhe`, the one amd64 node of the homelab's Talos
cluster; the other nodes are arm64 Raspberry Pis on SD cards, and nothing
of ol-rht runs on them. Talos runs from the tower's NVMe; the four 4 TB
SATA SSDs hold the ZFS pool `tank` (OpenEBS ZFS LocalPV, StorageClasses
`zfspv-db` and `zfspv-metrics`). Postgres, SeaweedFS, VictoriaMetrics
and Forgejo's data already live on the tower, so ol-rht shares its disks
and memory with them. The user's allowance, 2026-09-22: ol-rht may claim
up to 100 GB of memory, always less where it can.

The homelab's rules bind everything here. Every change to the cluster
goes through git and Flux — no `kubectl apply`, not for experiments;
node-level changes (a Talos patch) are committed first and rolled out
with the homelab's `just` recipes. Secrets are the user's, made with
SOPS; this plan names them and never writes them. Kyverno audits (it
does not block) for a memory limit and a CPU request on every container,
the `app.kubernetes.io/name` label, probes, an allowed registry (Forgejo's
is) and no `latest` tag; namespaces run the restricted pod security
profile.

The design lives in this plan; the manifests live in the homelab
repository under `infra/clusters/homelab-talos/ol-rht/`, which is where
the homelab keeps every workload's runtime configuration.

## 2. The image

One image carrying both binaries, `ol-rht` and `ol-rht-serve`, amd64
only, built by the homelab's Tekton from a push to this repository
(decided 2026-09-22, §9) and pushed to Forgejo's registry; Flux's image
automation writes the new tag into the homelab repository, as it does for
hn-favorites.

What the pipeline lacks today:

- **A trigger for this repository.** The event listener and its build
  targets know only the monorepo's paths.
- **A Rust buildpack in a builder.** The homelab's builder
  (`ol-paketo-builder-amd64:20260224124552`) was made by kpack, per the
  homelab's history: a `ClusterBuilder` whose order is
  `paketo-buildpacks/go` alone, on the homelab's own stack
  (`kpack-stacks/ol-base-build:jammy-amd64` to build,
  `ol-base-run:jammy-amd64` to run), from a buildpack mirrored into
  Forgejo. kpack was removed 2026-02-27 and the images of 2026-02-24
  stayed pinned, so the repository holds no recipe to rebuild it. The
  Tekton task runs the lifecycle inside the builder, which can only use
  the buildpacks the builder carries, so Rust needs a builder that
  carries it: proposed, a `builder.toml` in `infra/cnb/builders/` (empty
  today) naming the same stack and the Rust buildpack mirrored into
  Forgejo, made with `pack builder create` and pushed. A trial build with
  `pack build --buildpack` over the existing builder is the same parts
  assembled on the fly.
- **A build step sized for Rust.** The shared step is capped at one CPU
  and 2 GiB, and its build cache is an empty directory, so every build is
  cold. The ol-rht build step runs on the tower with room, and its cache
  on a volume of its own (below).

**The buildpack**, checked 2026-09-22 against its source and releases:
`paketo-community/rust` v0.90.0 (released 2026-09-09) composes rustup
v1.17.0, rust-dist v1.50.0, cargo v0.16.0 and procfile v5.13.7. It is
kept current mostly by automated dependency bumps that one maintainer
merges.

- **The workspace.** The cargo buildpack asks `cargo metadata` for the
  members and `BP_CARGO_WORKSPACE_MEMBERS=ol-rht,ol-rht-serve` picks the
  two by package name; each is `cargo install --locked --path <member>`:
  the release profile, the committed `Cargo.lock`. The setting goes in a
  `project.toml` at the repository root, the one file this repository
  gains. **It fails on this workspace** (found by the trial build,
  2026-09-22): since Rust 1.77 `cargo metadata` shortens a member's id
  to `path+file:///…/crates/ol-rht#0.1.0` when the package is named for
  its directory, as all eleven here are, and the buildpack's
  `ParseWorkspaceMember` takes the name from the last path segment but
  the path from its parent, so it runs `cargo install --path
  /workspace/crates`. `BP_CARGO_INSTALL_ARGS` may carry a `--path` of
  its own, which the buildpack then keeps, but `cargo install` takes one
  path, so that installs one binary. A second bug of the same cause:
  `ProjectTargets`, which names the process types, matches
  `BP_CARGO_WORKSPACE_MEMBERS` against the pre-1.77 id only, so with the
  filter set the image has no process types at all. No upstream issue
  names either (issues checked 2026-09-22; #293, the Rust 1.77 format
  change, is closed). Decided 2026-09-22 (§9): a patched build of the
  cargo buildpack, carried in the Rust builder until an upstream release
  has the fix; the issue, a minimal reproduction and the patch are
  drafted in `upstream-contributions/paketo-community-cargo/`, for the
  user to file.
- **The toolchain.** rustup reads `rust-toolchain.toml`, so the image is
  built by whatever stable is current at build time, rustfmt and clippy
  included, as on the laptop. Pinning a version there is what would make
  two builds of one commit use one compiler.
- **Native code.** `ring` and `zstd-sys` need a C compiler; Paketo's
  Jammy base build image carries `build-essential`. The protocol is
  compiled by `protox` in `build.rs`, so no `protoc`. Whether the
  homelab's builder uses that build image is to be confirmed.
- **Linking.** Dynamic against glibc: the buildpack builds static musl
  binaries only for a tiny or static run image, and the homelab's run
  image is a Jammy base image, the same release as the build image.
- **Processes.** One process type per binary, `ol-rht` and
  `ol-rht-serve`, each launched through `tini`; with no `web` process
  the first binary is the default. The binaries sit in
  `/workspace/bin/`. The buildpack declares API 0.7, under which the
  launcher appends a container's arguments after the process's own
  (`tini -g -- <binary>`) rather than replacing them, so a Job's
  `command` is `/cnb/process/ol-rht` and its `args` the subcommand;
  seen on the first patched image.
- **The cache.** The target directory and cargo's registry are cache
  layers, which is only useful if the lifecycle's `/cache` outlives the
  build: a volume on the tower for the ol-rht build step. The homelab's
  naming convention also reserves `monorepo/caches/` for cache images in
  Forgejo, but a Rust target directory is large to push on every build.

`pack`, `docker` and `podman` are on the laptop, so a trial build there
is possible before the builder changes; it pulls a builder, the
buildpack, a toolchain and every crate, so it is the user's to run.
Found on the first try (2026-09-22): Docker Desktop keeps images in its
containerd store, where a multi-platform image is an index with no
labels of its own, so pack finds no `io.buildpacks.buildpackage.metadata`
on `paketocommunity/rust:0.90.0`; the amd64 manifest pulled by its
digest carries the label. The trial names the buildpack (and a
multi-platform run image) by amd64 digest. The cluster's build runs the
lifecycle against registries, with no Docker daemon, so this is the
laptop's alone. Forgejo's registry answered the laptop 401 until logged
in. The second try got as far as the toolchain and stopped there:
Docker Desktop emulates amd64 with qemu, and `rustc --version`
segfaults under it. So the laptop's trial builds arm64 natively, which
tests everything but the architecture; amd64 is proven by the first
build on the tower, where it is native. The third try, arm64 on
Paketo's `builder-jammy-buildpackless-base` with the `--path`
workaround for `ol-rht-serve` alone, built: a 228 MB image whose
binary runs on the Jammy run image and takes its arguments
(`ol-rht-serve --store /nowhere` answers `no epoch published`), and
which has no process types, the second bug above. The homelab's own
builder is not tried yet (the laptop was not logged in to Forgejo).
The buildpacks already in Forgejo were mirrored per architecture with
`skopeo copy --override-arch` (the removed kpack runbook), which is
what fixed kpack's arm64 `exec format error`; none was patched in code.

**The image's size**, measured on the arm64 trial (2026-09-22): 69.6 MB
to pull, about 160 MB unpacked (Docker Desktop's "228 MB" counts the
compressed download and the unpacked copy together). The Jammy base run
image is about nine tenths of it (57 MB compressed); ours is the
binary (`ol-rht-serve`, 12.6 MB), the launcher, `tini` and the SBOM,
with the CLI to come (17 MB on the laptop). On the tower the base's
layers are shared with every image on the same run image, so an ol-rht
image on the homelab's `ol-base-run:jammy-amd64` costs about its own
layers. Smaller is possible: Paketo's tiny and static run images are
11.1 and 1.0 MB compressed, and the buildpack builds static musl
binaries for them. The costs: musl's allocator, which both binaries
would use (the CLI's counting allocator wraps the system's), is slow
under multi-threaded allocation, so it would want mimalloc or jemalloc
underneath; no shell in the image; and a second stack, builder and run
image in Forgejo. Static glibc is not the way round it: a statically
linked glibc resolves host names through shared libraries it expects at
run time, and the CLI fetches by host name. Decided 2026-09-22 (§9): the
Jammy base for the first deployment, for its shell; distroless or from
scratch is the eventual direction (§11).

**D1, the pipeline** (proposed 2026-09-22). What makes a push to this
repository's `main` produce an image in Forgejo:

- **This repository** gains `project.toml`, whose build environment sets
  `BP_CARGO_WORKSPACE_MEMBERS=ol-rht,ol-rht-serve`. The pipeline's
  existing step reads it, as it does hn-favorites'.
- **Artifacts in Forgejo**, made once from the laptop by the user,
  following a runbook in the homelab's `infra/cnb/`:
  - the patched cargo buildpack, packaged for amd64 (`create-package`,
    then `pack buildpack package … --target linux/amd64 --publish`);
  - `paketocommunity/rustup`, `paketocommunity/rust-dist`,
    `paketobuildpacks/syft` and `paketobuildpacks/procfile`, copied by
    their amd64 digests under `monorepo/buildpacks/` with `docker buildx
    imagetools create`, the per-architecture mirroring the Go buildpack
    got (skopeo is not on the laptop; buildx is);
  - a Rust builder, `monorepo/builders/ol-rust-builder-amd64`, made by
    `pack builder create --publish` from a committed
    `infra/cnb/builders/ol-rust-builder-amd64/builder.toml`: the
    homelab's stack (`ol-base-build` / `ol-base-run`, `jammy-amd64`) and
    the meta-buildpack's own order (rustup, rust-dist, syft, cargo,
    procfile optional), the patched cargo in cargo's place.
- **Tekton** (`tekton-ci`), all under git:
  - the event listener's existing trigger narrowed to the monorepo
    (`body.repository.full_name == 'oxidation-labs/monorepo'`), since it
    clones the monorepo whatever repository sent the push; a second
    trigger for `oxidation-labs/ol-rht`;
  - a trigger template, a pipeline `ol-rht-ci` (a tag step, then the
    build) and a task `ol-rht-image-build`: the shared build task's clone,
    `project.toml` and `creator` steps, with its build cache on a
    volume instead of an empty directory, `creator` run under `flock` on
    that volume so two runs never share it at once, the build step sized
    for Rust, and the image pushed as `oxidation-labs/ol-rht:main-<sha7>-<unix
    time>` with no per-architecture suffix, so no manifest merge;
  - the cache volume, on the tower, in the `local-path` class (deleted
    with its claim — a cache, which is what the storage policy keeps
    that class for).
- **By the user, outside git:** the Forgejo webhook on this repository
  pointing at the event listener with its HMAC secret; the CI clone
  token's read access to this repository and the registry token's write
  access to `oxidation-labs/ol-rht` (secrets are the user's).

Both of its choices were the user's, 2026-09-22 (§9): the build stays
inside `tekton-ci`'s existing limits — it asks for one CPU and 2 GiB and
may take the namespace's per-container ceiling of 3 CPU and 4 GiB, since
CI cannot have the machine — and `just check` waits. Flux's image
automation for the new image belongs to D5, with the Deployment that
uses it.

**Written 2026-09-22**, uncommitted, in the homelab unless marked:

| File | What |
|---|---|
| `project.toml` (this repository) | `BP_CARGO_WORKSPACE_MEMBERS` |
| `infra/cnb/builders/ol-rust-builder-amd64/builder.toml` | the builder: the homelab's stack, the meta-buildpack's order, lifecycle 0.21.20, the patched cargo |
| `…/ol-rust-builder-amd64/README.md` | the runbook: mirror four buildpacks by amd64 digest, package the patched cargo, `pack builder create --publish`, pin the tag |
| `infra/cnb/buildpacks/paketo-community-cargo/` | the patch and why it exists |
| `tekton-ci/eventlistener-monorepo-ci.yaml` | both existing triggers named to the monorepo; a third for `oxidation-labs/ol-rht` |
| `tekton-ci/triggertemplate-ol-rht-ci-run.yaml` | the run: amd64 node, 2 h timeout, source and cache workspaces |
| `tekton-ci/pipeline-ol-rht-ci.yaml` | one task, on builder `ol-rust-builder-amd64:20260923064544` |
| `tekton-ci/task-ol-rht-image-build.yaml` | clone and tag, `project.toml`, then `creator` under `flock` on the cache |
| `tekton-ci/pvc-ol-rht-build-cache.yaml` | the cache, `local-path-retain` on the tower |
| `tekton-ci/kustomization.yaml`, `tekton-pruner-namespace-config.yaml` | the new files; one run of history kept |

`kubectl kustomize` builds the namespace with all four new objects. Nothing
is committed, and no Tekton object has been applied or validated against a
cluster.

The web UI is a separate build and waits for the exposure plan (§7).

## 3. The store on the box

A fresh store, its registry the store of record from its first merge,
under a new public-id key (decided 2026-09-22, §9). The laptop's store
stays a development sample. The key is a separate one because the two
stores number their works independently: under one key the same
`WorkId` in each would give the same public id to two different papers.

Two ZFS datasets under `tank/ol-rht` (agreed 2026-09-22, §9), each with
its own Retain StorageClass, because the store's two halves are read two
different ways; the graph is proposed for the NVMe instead (below):

- **bulk** — `raw`, `normalized`, `merged`, `receipts`, `listings`.
  Written once or appended, read start to finish with `read()`, which on
  ZFS goes through ZFS's own cache (the ARC) and not the page cache. The
  default record size suits it; the raw layer is already gzip, so ZFS
  compression gains nothing there.
- **serving** — `names`, `works`, `cited`, `search`, `epochs`, and
  `graph` unless it goes to the NVMe.
  Every artifact here is memory-mapped (checked 2026-09-22: the names
  FST, the works blocks and offsets, the cited artifact, the graph's base
  files, Tantivy's directory). A mapped page lives in the page cache;
  ZFS keeps its own copy in the ARC as well. `primarycache=metadata` on
  this dataset keeps one copy, and a small record size keeps a 4 KiB page
  fault from reading a whole 128 KiB record. These artifacts are rebuilt
  every run, so a record size changed later reaches the next build's
  files at no cost.

Both datasets are created once by hand, as the homelab's ZFS runbook
does for `tank/k8s/*` (its one written exception to git-only changes),
and their properties are inherited by the volumes OpenEBS creates under
them. The serving volume is mounted into the store root one directory at
a time (`subPath`), so the store stays one root to the code. Nothing
renames or hard-links across that split (checked 2026-09-22: every
`rename` stays inside its artifact's own directory, and the harness falls
back from a hard link to a copy).

**Proposed: the graph on the NVMe.** Asked for by the user 2026-09-22.
The graph is the most random-access of the artifacts and is written only
by full runs; the NVMe reads at random far better than the SATA mirrors,
and Talos formats its `EPHEMERAL` partition XFS, which caches a mapped
page once, in the page cache. The mechanism exists: the homelab's
`local-path-retain` StorageClass (Retain, bound at first use) puts a
volume under `/opt/local-path-provisioner`, which on Talos is on
`EPHEMERAL`, so on the tower it is the NVMe. That volume mounted at the
store's `graph/` is the whole change: `graph build` stages and renames
inside `graph/`, and everything else reaches the graph through that path.
No code changes.

What comes with it:

- **The disk is the kubelet's.** Container images, logs and pod storage
  live on it too. When free space falls below the kubelet's threshold
  (`nodefs.available<10%`, set in the homelab) it evicts the tower's
  pods one after another, Postgres among them, and evicting them frees
  nothing the graph holds, so it keeps going. local-path ignores the size
  a volume asks for, so nothing but the graph's own size bounds it: a
  build holds the old graph and the new one side by side until the next
  epoch, and the held-only graph beside them. Watched with an alert on
  the tower's free space.
- **It survives upgrades, not resets.** Since Talos 1.8 an upgrade never
  wipes the system disk (the homelab runs 1.13); a reset does. The graph
  is derived, so a reset costs a `graph build` over the merged layer on
  ZFS, not data.
- **No mirror, and none needed.** If the NVMe fails, Talos fails with it
  and the tower is down whatever holds the graph.
- **First use of the path.** No workload in the homelab uses
  `local-path-retain` yet; the graph would be the first.
- **The cleaner version costs a wipe.** A Talos user volume on the NVMe
  would be a filesystem of its own, sized, which the kubelet does not
  count, so it could never evict anything. It needs space outside
  `EPHEMERAL`, which fills the disk, and XFS cannot shrink: carving it
  means wiping `EPHEMERAL` on the tower. Not worth it unless the
  measurement says so.

The same volume could take the other serving artifacts later, since all
of them are derived. Whether the graph earns its place there is measured
at D6: the same graph built on both, the first queries after an epoch
swap timed on each, since a cold start is where the disk shows.

ZFS snapshots of the bulk dataset give the store of record the property
HDFS gave at the user's previous job: a snapshot before each full merge
makes the merge reversible.

**The ARC's ceiling.** The ARC is kernel memory that no pod is charged
for and that Kubernetes does not schedule around, and by default it may
grow to a large share of RAM. Before ol-rht claims its share at corpus
scale, the tower's `zfs_arc_max` is capped so that ol-rht's claim, the
other workloads and the ARC fit together. It is one setting in the
tower's Talos patch (`infra/talos/patches/install-disk-0mm-vhe.yaml`,
which already loads the module); the exact key for Talos 1.13 is to be
confirmed. Not needed for the sample.

A pod's memory limit also bounds the page cache it is charged for, so
the server's limit is its cache budget for the mapped artifacts. Too low
a limit makes it slower, not dead: the kernel reclaims file pages before
it kills.

## 4. The runs

Plain Kubernetes Jobs for everything that writes the store (agreed
2026-09-22, §9),
committed to the homelab repository, so that the commit is what starts a
run — the justfile's confirmation before a full merge becomes a commit.

- **A full run** is one Job whose stages run as init containers in order
  — normalize, merge, then the justfile's `refresh` order (names, works,
  graph, search) — with publish as the main container; the cited-as
  lookup is built on the way when it is missing, as on the laptop. Init containers run one at a time and the pod
  stops at the first failure (`restartPolicy: Never`, `backoffLimit: 0`:
  fail fast, no retry). Each stage has its own memory limit, and the pod
  reserves the largest stage's, not the sum. Each stage's log is its own
  container's, kept by VictoriaLogs after the pod is gone.
- **A Job's spec cannot change**, so each run is a new file and name
  (`…-v1`, `…-v2`), as the homelab's storage migration Jobs are.
- **No `ttlSecondsAfterFinished`.** A finished Job deleted by its TTL is
  missing from the cluster but present in git, and Flux creates it again:
  the merge would run a second time. Removing the file from git is how a
  finished Job goes away.
- **Batches**, once origins exist (serving plan SP6), are a CronJob with
  `concurrencyPolicy: Forbid`, beside the store's own `merged/merge.lock`.
- **Downloads** for the corpus are Jobs too; they are the user's to start,
  and committing them is starting them.

The homelab's other two runners, and where each fits:

- **Tekton** builds the image (§2) and is the home for what a push
  triggers: `just check`, later the hold-out harnesses against the sample
  on the box, and the experiment runs of the homelab's experiment-tracking
  decision. The store's runs are not triggered by a push.
- **Temporal** earns its place at SP6: many origins, each with its own
  cursor and cadence, fetch → normalize → batch with retries on fetches
  and none on merges, and a visible history of every batch. hn-favorites
  already uses its schedules this way. Its cost: a worker process —
  likely Go, as hn-favorites' is — and a choice between running the
  binaries inside the worker's pod (one memory limit for everything) and
  starting Jobs and waiting on them. Its default is to retry forever, so
  every activity's policy is set by hand. Not for the sample.

## 5. The server

A Deployment on the tower: `ol-rht-serve --store /store --listen
0.0.0.0:50051 --http 0.0.0.0:50052` (its defaults bind the loopback
address only). It needs no secret: the names artifact carries the public
bits, and the key never leaves the merge Jobs. Probes use the gRPC health
service the server already runs, through the kubelet's own gRPC probe,
so the probe rule is met with no new code.

The server would ideally mount the store read-only; Tantivy's reader
takes a lock file when it reloads, which a read-only mount refuses. To be
checked before the mount is chosen.

The server and the runs share one machine, which the hosting decision
of 2026-09-17 did not intend (§9): memory limits and CPU requests are the
only fence between them, and the disks are shared with the rest of the
tower. The swap-in spare of that decision is already the epoch: a full
run builds a new epoch and the server moves to it when `epochs/current`
flips.

## 6. Getting the sample in

The laptop's raw layer, receipts and listings are copied to the box, and
everything after them is rebuilt there by a full run (§4), which proves
the image, the store, the Job and the server before any large download,
and asks nothing of the providers again. With no `kubectl cp` into a
throwaway pod, the bytes go through SeaweedFS: a tarball uploaded over a
port-forward into a bucket a committed Job creates, then a committed Job
that unpacks it into the bulk volume.

The normalize invocations that built the sample's sets are to be written
down once, as the full run's first stage.

## 7. Exposure

The user's, planned separately through kgateway (2026-09-22). The JSON
facade and the web UI go with it.

## 8. Milestones

- **D1 — the image.** Tekton builds and pushes it from a push to this
  repository; Flux's image automation follows the tag.
- **D2 — the namespace and the store.** Namespace, the two datasets, the
  StorageClasses and volumes; the key and the registry pull secret made
  by the user.
- **D3 — the sample in.** The tarball through SeaweedFS into the bulk
  volume.
- **D4 — the first full run on the box.** The Job of §4, then `epoch
  check` as a Job of its own.
- **D5 — the server.** The Deployment of §5, reachable inside the
  cluster; exposure is §7's.
- **D6 — the corpus.** The downloads, then the full run at scale — where
  graph plan G3, the ARC's ceiling and the serving dataset's record size
  are measured rather than guessed.

## 9. Decision log

- 2026-09-22 — the walkthrough, the user's answers:
  1. The image is built by the homelab's Tekton from this repository,
     not by moving the code into the monorepo and not on the laptop.
  2. ZFS stays on the tower (the user: "i'm not switching off of zfs on
     this machine"); proceed without a separate smoke test of the cache,
     since there is no other disk to test on.
  3. The sample goes first, then the corpus.
  4. Everything starts fresh on the box: a new registry, a new key.
  5. Building and serving on one machine departs from the hosting
     decision of 2026-09-17; accepted as what exists today. The user's
     reason: it teaches the full corpus and may bring real use without
     paying for large-memory cloud machines.
  6. Exposure is planned separately, through kgateway.

  Also recorded: the tower's disks are four 4 TB SATA SSDs under ZFS;
  ol-rht may claim up to 100 GB of the cluster's memory.
- 2026-09-22, later — the runs are Kubernetes Jobs (§4) and the store
  takes two ZFS datasets (§3), both as proposed. The user asked how
  possible it would be to put the graph on the NVMe's XFS instead of
  ZFS: possible with no code change, through the homelab's
  `local-path-retain` class, and proposed with its costs (§3). The
  buildpack was checked against its source and releases (§2): it builds
  a workspace like this one in principle; nothing has been built yet.
- 2026-09-22, the image — the Rust build goes through a patched cargo
  buildpack (§2), after the laptop's trial found two bugs in how it
  reads Rust 1.77+ workspace ids; the upstream issue, a minimal
  reproduction and the patch were drafted at the user's request in
  `upstream-contributions/paketo-community-cargo/`, unfiled.
- 2026-09-22, the image's base — the Jammy base for the first
  deployment, for the debugging a shell allows. Eventually distroless or
  from scratch, as far as it goes; the user's reason is not scanner
  noise but that a scanner like Trivy reacts, and a minimal image is
  secure by construction ("this approach has saved my bacon in the past,
  it will again in the future"). Not a goal for now (§11).
- 2026-09-22, D1's two choices — the Rust build stays within
  `tekton-ci`'s limits ("it's gonna be slow regardless, i can't just give
  the whole machine to ci"), and `just check` in the pipeline waits. D1's
  files were written the same day (§2), none committed.

## 10. Open items

1. ~~**Which runner writes the store.**~~ Kubernetes Jobs, 2026-09-22
   (§4).
2. ~~**The two datasets.**~~ Agreed 2026-09-22 (§3); the serving
   dataset's record size is measured at D6.
3. **The ARC's ceiling** (§3): its value, chosen with the user before
   corpus scale; the Talos key confirmed.
4. **The builder** (§2): a committed `builder.toml` for a Rust builder
   on the homelab's stack, proposed; whether `ol-base-build` carries a C
   compiler (the trial build answers it); the build step's size and its
   cache volume.
5. ~~**The cargo buildpack's workspace bugs.**~~ A patched build,
   2026-09-22 (§2). Left: `go test` on the patch and the reproduction
   run before and after it (both download; the user's), then the patched
   package built for amd64 and mirrored into Forgejo.
6. **The first image** (§2): the `command` form confirmed on a patched
   image; whether the server and the CLI want `tini`
   (`BP_CARGO_TINI_DISABLED`); the homelab's own builder tried, for its
   build image's C compiler.
7. ~~**The image's base.**~~ Jammy base for now, 2026-09-22 (§2);
   distroless or from scratch eventually (§11).
8. **The toolchain** (§2): `rust-toolchain.toml` says `stable`, so each
   build takes the stable of its day; pinning is the user's call.
9. **The store mount for the server** (§5): read-only if Tantivy allows.
10. **The graph on the NVMe** (§3): proposed, awaiting the user; measured
   at D6 against the same graph on ZFS. The tower's NVMe is inferred to
   be most of the cluster's 1951 GiB of allocatable ephemeral storage,
   the Pis' cards being small; to be confirmed.
11. ~~**D1's two choices.**~~ Decided 2026-09-22 (§2).
12. **D1's first run** (§2): the Forgejo webhook on this repository and
    the CI tokens' access to it, then the two pushes. The buildpacks and
    the builder were published 2026-09-23 and the builder's tag is
    pinned; Forgejo refused pack's multi-platform buildpack index, so the
    patched cargo buildpack is published per architecture, like the
    homelab's other mirrors.

## 11. Eventually

Not goals for now; recorded so they are not lost.

- **Distroless, or from scratch** (the user, 2026-09-22). Move the image
  toward as little as possible beyond the two binaries: static musl on
  Paketo's static run image or a scratch image, with an allocator under
  the counting one, since musl's is slow under many threads. The reason
  is construction, not scanning: what is absent cannot be exploited, and
  Trivy only reports what is already there. The first deployment keeps
  the Jammy base for its shell.
