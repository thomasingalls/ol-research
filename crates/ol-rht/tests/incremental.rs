//! The delta path on the sample store (serving plan §5), when
//! `OL_RHT_STORE` names one; skipped otherwise. One normalized prefix is
//! held out of a scratch store built by hard links, merged in full and
//! published; the held-out sets are then merged as a batch; a full merge
//! over everything follows and must append nothing to the registry; and
//! the core over the first epoch plus the batch must answer exactly as
//! the core over the epoch built after. Freshness is measured from the
//! batch's end to a reader's fold, polling as the server does.
//!
//! `OL_RHT_HOLD_OUT` names the prefix, relative to the normalized layer's
//! version directory; `pubmed/baseline/pubmed26n1334` by default.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use ingest::{Kind, Store};
use serve::{Serving, compare};

fn link_tree(from: &Path, to: &Path, skip: &Path) -> std::io::Result<u64> {
    let mut linked = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            linked += link_tree(&source, &target, skip)?;
        } else if Kind::prefix_of(&source).is_none_or(|p| p != skip) {
            if std::fs::hard_link(&source, &target).is_err() {
                std::fs::copy(&source, &target)?;
            }
            linked += 1;
        }
    }
    Ok(linked)
}

fn link_in(source_root: &Path, store: &Store, file: &Path) {
    let target = store.root().join(file.strip_prefix(source_root).unwrap());
    if std::fs::hard_link(file, &target).is_err() {
        std::fs::copy(file, &target).unwrap();
    }
}

fn run(what: &str, args: &[&str]) {
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_ol-rht"))
        .args(args)
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!(
        "--- {what}: {:.1} s\n{}{}",
        started.elapsed().as_secs_f64(),
        stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .take(40)
            .collect::<Vec<_>>()
            .join("\n"),
        stderr
            .lines()
            .filter(|l| l.contains("merge") || l.contains("epoch"))
            .fold(String::new(), |mut s, l| {
                s.push_str("\n  ");
                s.push_str(l);
                s
            })
    );
    assert!(output.status.success(), "{what} failed:\n{stderr}");
}

#[test]
#[allow(clippy::too_many_lines)]
fn the_sample_batch_equals_the_full_merge() {
    let Ok(root) = std::env::var("OL_RHT_STORE") else {
        eprintln!("OL_RHT_STORE not set; skipping");
        return;
    };
    let hold_out =
        std::env::var("OL_RHT_HOLD_OUT").unwrap_or_else(|_| "pubmed/baseline/pubmed26n1334".into());
    let source = Store::new(&root);
    let normalized = source.root().join("normalized");
    let scratch = std::env::temp_dir().join(format!("ol-rht-incremental-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let store = Store::new(&scratch);
    let held: PathBuf = normalized
        .join(format!("v{}", model::VERSION))
        .join(&hold_out);
    let linked = link_tree(&normalized, &store.root().join("normalized"), &held).unwrap();
    eprintln!(
        "scratch store {} with {linked} sets linked, {} held out",
        scratch.display(),
        held.display()
    );
    let key = scratch.join("public.key");
    std::fs::write(&key, [42u8; 32]).unwrap();
    let store_arg = scratch.to_str().unwrap();
    let key_arg = key.to_str().unwrap();
    run(
        "full merge without the held-out sets",
        &["merge", "--store", store_arg, "--public-key", key_arg],
    );
    run("epoch 1", &["epoch", "publish", "--store", store_arg]);
    let a = Serving::open(&store).unwrap();
    assert_eq!((a.epoch().epoch, a.watermark().batch), (1, 1));
    let held_files: Vec<PathBuf> = [Kind::Works, Kind::References, Kind::Deletions]
        .iter()
        .map(|k| k.path(&held))
        .filter(|p| p.exists())
        .collect();
    assert!(!held_files.is_empty(), "nothing at {}", held.display());
    // The batch, and how long until a reader polling every second folds it.
    let started = Instant::now();
    let mut args = vec!["merge", "--batch"];
    args.extend(held_files.iter().map(|p| p.to_str().unwrap()));
    args.extend(["--store", store_arg, "--public-key", key_arg]);
    run("the batch", &args);
    let batched = Instant::now();
    let folded_in = loop {
        std::thread::sleep(Duration::from_millis(1000));
        let before = Instant::now();
        a.refresh().unwrap();
        if a.watermark().batch == 2 {
            break before.elapsed();
        }
        assert!(batched.elapsed() < Duration::from_secs(60), "never folded");
    };
    eprintln!(
        "freshness: the batch took {:.1} s; a reader polling every second answered at batch 2 {:.2} s after it ended, the fold itself {:.3} s",
        (batched - started).as_secs_f64(),
        batched.elapsed().as_secs_f64(),
        folded_in.as_secs_f64()
    );
    let line = merge::journal::last(&store.journal()).unwrap().unwrap();
    assert_eq!(line.kind, merge::journal::Kind::Incremental);
    eprintln!(
        "batch 2: {} records, {} minted, {} merged, {} names dropped",
        line.records, line.minted, line.merged, line.dropped
    );
    // Everything in: the full merge appends nothing.
    for file in &held_files {
        link_in(source.root(), &store, file);
    }
    let symbols = Kind::Symbols.path(&held);
    if symbols.exists() {
        link_in(source.root(), &store, &symbols);
    }
    run(
        "full merge over everything",
        &["merge", "--store", store_arg, "--public-key", key_arg],
    );
    let after = merge::journal::last(&store.journal()).unwrap().unwrap();
    assert_eq!(after.batch, 3);
    assert_eq!(after.kind, merge::journal::Kind::Full);
    assert_eq!(
        after.registry, line.registry,
        "the full merge appended registry lines"
    );
    assert_eq!(
        (after.minted, after.merged, after.split, after.dropped),
        (0, 0, 0, 0)
    );
    run("epoch 2", &["epoch", "publish", "--store", store_arg]);
    run("epoch check", &["epoch", "check", "--store", store_arg]);
    let b = Serving::open(&store).unwrap();
    assert_eq!((b.epoch().epoch, b.watermark().batch), (2, 3));
    let started = Instant::now();
    let differences = compare::differences(&a, &b, 30).unwrap();
    eprintln!(
        "compared {} ids in {:.1} s: {} differences",
        b.minted(),
        started.elapsed().as_secs_f64(),
        differences.len()
    );
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    let _ = std::fs::remove_dir_all(&scratch);
}
