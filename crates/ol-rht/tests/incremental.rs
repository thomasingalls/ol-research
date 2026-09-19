//! The delta path on the sample store, one baseline file held out and
//! merged as a batch (see `harness`), when `OL_RHT_STORE` names the store;
//! skipped otherwise.
//!
//! `OL_RHT_HOLD_OUT` names the prefixes, comma-separated and relative to
//! the normalized layer's version directory, batched in the order given;
//! `pubmed/baseline/pubmed26n1334` by default.

mod harness;

#[test]
fn the_sample_batch_equals_the_full_merge() {
    let Ok(root) = std::env::var("OL_RHT_STORE") else {
        eprintln!("OL_RHT_STORE not set; skipping");
        return;
    };
    let hold_outs: Vec<String> = std::env::var("OL_RHT_HOLD_OUT")
        .unwrap_or_else(|_| "pubmed/baseline/pubmed26n1334".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    harness::batches_equal_the_full_merge("incremental", &root, &hold_outs);
}
