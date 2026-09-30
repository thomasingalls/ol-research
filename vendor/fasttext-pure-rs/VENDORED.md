# fasttext-pure-rs, vendored

`fasttext-pure-rs` 0.1.0 (MIT, <https://github.com/silvermine/fasttext-pure-rs>),
copied from the crates.io package on 2026-09-30 — `src/`, `LICENSE`,
`README.md` — and put in place of the published crate by the workspace's
`[patch.crates-io]`.

**The one change**: `src/io.rs`'s `MAX_ALLOC_BYTES`, a guard against
malformed files, is 4 GiB here instead of 1 GiB. GlotLID v3's input
matrix is 1,673,585,664 bytes, and the published guard refused it
(`ol-rht-language-d13-v1`, 2026-09-30: "matrix data too large: 1673585664
bytes exceeds 1073741824 byte limit"). Nothing else differs; the
manifest is trimmed of the test and bench targets whose files are not
copied.

**Why here**: the tokenizer plan's D13 measures GlotLID through this
reader (`crates/ol-rht-language`); the compromise is entered in the
plan's §7. **What lifts it**: an upstream release that lets a caller set
the limit, or raises it, after which this directory and the patch go.
