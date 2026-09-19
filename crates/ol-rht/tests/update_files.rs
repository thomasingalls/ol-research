//! The delta path on PubMed's update files (see `harness`): every set
//! under `pubmed/updatefiles` held out and merged as batches in path order
//! against the rest of the sample — records delivered again, versioned
//! citations, deletions — when `OL_RHT_STORE` names the store and it has
//! update files; skipped otherwise.

mod harness;

#[test]
fn the_sample_update_files_equal_the_full_merge() {
    let Ok(root) = std::env::var("OL_RHT_STORE") else {
        eprintln!("OL_RHT_STORE not set; skipping");
        return;
    };
    let hold_outs = harness::prefixes_under(&root, "pubmed/updatefiles");
    if hold_outs.is_empty() {
        eprintln!("no normalized update files under {root}; skipping");
        return;
    }
    harness::batches_equal_the_full_merge("update-files", &root, &hold_outs);
}
