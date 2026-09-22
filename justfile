# The commands that matter. `just` on its own lists them.
#
# Recipes are grouped by what they touch: nothing, the dev machine, or
# the store of record. The last group is the dangerous one and is marked.
#
# `just --list` shows only the single comment line directly above a
# recipe, so that line is the summary and anything longer sits above a
# blank line, where just ignores it.

store := env_var_or_default("OL_RHT_STORE", "data")
key := env_var_or_default("OL_RHT_PUBLIC_KEY", env_var("HOME") / ".config/ol-rht/public.key")
cli := "cargo run --release -p ol-rht --"

@default:
    just --list

# ---------------------------------------------------------------- code

# fmt, clippy and every test — what CI would run
check: fmt-check lint test

# Format every crate
fmt:
    cargo fmt --all

# Fail if anything is unformatted
fmt-check:
    cargo fmt --all -- --check

# Clippy over every target, warnings as errors
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# The store-backed harnesses skip themselves unless OL_RHT_STORE is set;
# see the holdout recipes for those.

# The workspace's unit and integration tests
test:
    cargo test --workspace

# One crate's tests, e.g. `just test-one merge`
test-one crate:
    cargo test -p {{crate}}

# Release binaries: the CLI and the server
build:
    cargo build --release -p ol-rht -p ol-rht-serve

# ----------------------------------------------------------------- dev

# gRPC on :50051, JSON on :50052, dev origins allowed. Read-only against
# the store. Run `just web` beside it in another shell.

# The query server, read-only
serve:
    cargo run --release -p ol-rht-serve -- --store {{store}}

# Expects `just serve` already running; the UI reads VITE_API_URL,
# defaulting to http://127.0.0.1:50052.

# The Vite dev server
web:
    cd web && pnpm dev

# pnpm-workspace.yaml holds packages back two weeks (minimumReleaseAge).
# Do not pass a flag that overrides it.

# Install the frontend's dependencies
web-install:
    cd web && pnpm install

# Typecheck and production-build the frontend
web-check:
    cd web && pnpm typecheck && pnpm build

# web/src/generated/serving.ts is the JSON API's response bodies, generated
# from the Rust types that serialize them (crates/ol-rht-serve/src/ts.rs).
# It is committed, so the frontend builds from a fresh checkout with no
# Rust toolchain; `just test` fails when it is stale, so `just check`
# already catches the drift and `ts-check` is only for asking on its own.

# Regenerate the TypeScript the browser client is written against
ts-types:
    cargo test -p ol-rht-serve --lib ts::write_the_bindings -- --ignored --exact

# Fail if the committed TypeScript is not what the protocol generates
ts-check:
    cargo test -p ol-rht-serve --lib ts::the_committed_bindings_are_current

# ------------------------------------------------- the store, read-only

# None of these writes. `--survey` is what makes the merge paths safe:
# without it, and without `--limit`, `merge identity` appends to the
# registry (`let complete = limit.is_none() && !opts.survey`).

# The identity pass and its report, writing nothing
report:
    {{cli}} merge identity --store {{store}} --survey

# Prints the PublisherIds report — the evidence for the registrant list
# in merge::publisher::ELSEVIER. Opens no writer: normalize builds one
# only with --out.

# The normalize survey over raw PubMed and Crossref
survey-publisher:
    {{cli}} pubmed normalize {{store}}/raw/pubmed/baseline/*.xml.gz \
        {{store}}/raw/pubmed/updatefiles/*.xml.gz \
        {{store}}/raw/pubmed/lookup/*/*.xml --survey
    {{cli}} crossref normalize {{store}}/raw/crossref/api/*.json \
        {{store}}/raw/crossref/lookup/*/*.json --survey

# The published epoch's own consistency check
epoch-check:
    {{cli}} epoch check --store {{store}}

# The graph's invariants over the held-only subgraph
graph-check:
    {{cli}} graph check --store {{store}} --held

# One work, by name, #id or public id
work name:
    {{cli}} query work '{{name}}' --store {{store}}

# Search the lexical index, e.g. `just search crispr screen`
search +words:
    {{cli}} query search {{words}} --store {{store}}

# Neighbours, by cites-same or cited-alongside
related name by="cites-same":
    {{cli}} query related '{{name}}' --by {{by}} --store {{store}}

# A directed path between two works, if one exists within the bounds
path from to:
    {{cli}} query path '{{from}}' '{{to}}' --store {{store}}

# ------------------------------------------------ the store, read-write

# These mutate the store of record. They are the user's to run.

# Both passes, the registry and the merged sets; hours on a real corpus.
# Since 2026-09-19 the registry is staged and promoted only after the
# merged sets are written, so a failed run leaves it untouched — watch
# that registry.jsonl does not grow until the run is nearly done, and
# that registry.jsonl.pending exists while it runs.

# The full merge — writes the store of record
[confirm("This writes the registry — the store of record. Run it? [y/N]")]
merge:
    cargo run --release -p ol-rht -- merge --store {{store}} --public-key {{key}}

# Apply update files as a batch through the incremental path
merge-batch +paths:
    cargo run --release -p ol-rht -- merge --batch {{paths}} --store {{store}} --public-key {{key}}

# The four serving artifacts, each separately runnable, then `publish`,
# which is what readers start seeing.

# The whole sequence after a merge: all four artifacts, then the epoch.
# This is what a merge-version bump needs — until it runs, a freshly
# started `serve` refuses the store as older than the binary, and so do
# `epoch-check` and every query recipe.

# Rebuild all four artifacts and cut an epoch
refresh: names works graph-build search-build publish

# Rebuild the names artifact
names:
    {{cli}} names build --store {{store}}

# Rebuild the works artifact
works:
    {{cli}} works build --store {{store}}

# Rebuild the citation graph
graph-build:
    {{cli}} graph build --store {{store}}

# Rebuild the lexical index
search-build:
    {{cli}} search build --store {{store}}

# Cut the epoch readers serve from
publish:
    {{cli}} epoch publish --store {{store}}

# ----------------------------------------------------------- harnesses

# Each builds a scratch store by hard link and never writes ./data. They
# hold a prefix out of a full merge, apply it as a batch, and demand the
# result equal the full merge exactly.
#
# The prefixes are easy to get wrong: pubmed26n1334 is the only BASELINE
# file; 1335-1339 are UPDATE files. A wrong prefix fails on setup with
# `nothing at <path>`, which reads like a code failure and is not.
#
# Do not `cargo build` while one of these runs — it invalidates the run.

# The 1334 baseline hold-out, ~5.5 minutes
holdout:
    OL_RHT_STORE={{store}} cargo test --release -p ol-rht --test incremental -- --nocapture

# One prefix, e.g. `just holdout-one pubmed/updatefiles/pubmed26n1339`
holdout-one prefix:
    OL_RHT_STORE={{store}} OL_RHT_HOLD_OUT={{prefix}} \
        cargo test --release -p ol-rht --test incremental -- --nocapture

# Every update file under pubmed/updatefiles, batched in path order
update-files:
    OL_RHT_STORE={{store}} cargo test --release -p ol-rht --test update_files -- --nocapture

# The baseline and all five update files in order — the longest of the three
holdout-all:
    OL_RHT_STORE={{store}} \
    OL_RHT_HOLD_OUT=pubmed/baseline/pubmed26n1334,pubmed/updatefiles/pubmed26n1335,pubmed/updatefiles/pubmed26n1336,pubmed/updatefiles/pubmed26n1337,pubmed/updatefiles/pubmed26n1338,pubmed/updatefiles/pubmed26n1339 \
        cargo test --release -p ol-rht --test incremental -- --nocapture
