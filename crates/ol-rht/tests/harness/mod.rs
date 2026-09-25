//! The delta path on the sample store (serving plan §5), shared by the
//! sample harnesses. Some normalized prefixes are held out of a scratch
//! store built by hard links, the rest merged in full and published; the
//! held-out sets are merged as batches, one per prefix in path order; a
//! full merge over everything follows and must append nothing to the
//! registry; and the core over the first epoch plus the batches must
//! answer exactly as the core over the epoch built after. Freshness is
//! measured from each batch's end to a reader's fold, polling as the
//! server does. Run in release: a debug build takes five times as long.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use ingest::{Kind, Store};
use serve::{Serving, compare};

fn link_tree(from: &Path, to: &Path, skip: &[PathBuf]) -> std::io::Result<u64> {
    let mut linked = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&target)?;
            linked += link_tree(&source, &target, skip)?;
        } else if Kind::prefix_of(&source).is_none_or(|p| !skip.contains(&p)) {
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

fn run(what: &str, args: &[&str]) -> String {
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
    stdout.into_owned()
}

/// The prefixes under `dir`, relative to the normalized layer's version
/// directory, in path order.
#[allow(dead_code)] // one harness uses it
pub(crate) fn prefixes_under(root: &str, dir: &str) -> Vec<String> {
    let version = Store::new(root)
        .root()
        .join("normalized")
        .join(format!("v{}", model::VERSION));
    let mut found: Vec<String> = merge::sets::find(&version.join(dir), Kind::Works)
        .unwrap_or_default()
        .iter()
        .filter_map(|p| Kind::prefix_of(p))
        .filter_map(|p| {
            p.strip_prefix(&version)
                .ok()
                .map(|r| r.to_string_lossy().into_owned())
        })
        .collect();
    found.sort();
    found
}

/// Holds `hold_outs` out of a scratch copy of the store at `root`, merges
/// them as batches in order, and checks the batches against a full merge.
#[allow(clippy::too_many_lines)]
pub(crate) fn batches_equal_the_full_merge(label: &str, root: &str, hold_outs: &[String]) {
    let source = Store::new(root);
    let normalized = source.root().join("normalized");
    let version = normalized.join(format!("v{}", model::VERSION));
    let scratch = std::env::temp_dir().join(format!("ol-rht-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let store = Store::new(&scratch);
    let held: Vec<PathBuf> = hold_outs.iter().map(|h| version.join(h)).collect();
    let linked = link_tree(&normalized, &store.root().join("normalized"), &held).unwrap();
    eprintln!(
        "scratch store {} with {linked} sets linked, {} prefixes held out",
        scratch.display(),
        held.len()
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
    let mut all_files: Vec<PathBuf> = Vec::new();
    for (k, prefix) in held.iter().enumerate() {
        let batch_number = k as u64 + 2;
        let files: Vec<PathBuf> = [Kind::Works, Kind::References, Kind::Deletions]
            .iter()
            .map(|kind| kind.path(prefix))
            .filter(|p| p.exists())
            .collect();
        assert!(!files.is_empty(), "nothing at {}", prefix.display());
        // The batch's sets must sit where the full merge reads them.
        for file in &files {
            link_in(source.root(), &store, file);
        }
        let symbols = Kind::Symbols.path(prefix);
        if symbols.exists() {
            link_in(source.root(), &store, &symbols);
        }
        let in_scratch: Vec<PathBuf> = files
            .iter()
            .map(|f| store.root().join(f.strip_prefix(source.root()).unwrap()))
            .collect();
        let started = Instant::now();
        let mut args = vec!["merge", "--batch"];
        args.extend(in_scratch.iter().map(|p| p.to_str().unwrap()));
        args.extend(["--store", store_arg, "--public-key", key_arg]);
        run(&format!("batch {batch_number}"), &args);
        let batched = Instant::now();
        let folded_in = loop {
            std::thread::sleep(Duration::from_millis(1000));
            let before = Instant::now();
            a.refresh().unwrap();
            if a.watermark().batch == batch_number {
                break before.elapsed();
            }
            assert!(batched.elapsed() < Duration::from_secs(60), "never folded");
        };
        eprintln!(
            "freshness: batch {batch_number} took {:.1} s; a reader polling every second answered at it {:.2} s after it ended, the fold itself {:.3} s",
            (batched - started).as_secs_f64(),
            batched.elapsed().as_secs_f64(),
            folded_in.as_secs_f64()
        );
        let line = merge::journal::last(&store.journal()).unwrap().unwrap();
        assert_eq!(line.kind, merge::journal::Kind::Incremental);
        assert_eq!(line.batch, batch_number);
        all_files.extend(files);
    }
    let last_batch = merge::journal::last(&store.journal()).unwrap().unwrap();
    // The unresolved rows as the batches left them: the full merge next
    // replaces the merged directory they are read from.
    let unresolved_a = compare::unresolved(&store).unwrap();
    run(
        "full merge over everything",
        &["merge", "--store", store_arg, "--public-key", key_arg],
    );
    let after = merge::journal::last(&store.journal()).unwrap().unwrap();
    assert_eq!(after.batch, last_batch.batch + 1);
    assert_eq!(after.kind, merge::journal::Kind::Full);
    assert_eq!(
        after.registry, last_batch.registry,
        "the full merge appended registry lines"
    );
    assert_eq!(
        (after.minted, after.merged, after.split, after.dropped),
        (0, 0, 0, 0)
    );
    run("epoch 2", &["epoch", "publish", "--store", store_arg]);
    run("epoch check", &["epoch", "check", "--store", store_arg]);
    let b = Serving::open(&store).unwrap();
    assert_eq!(
        (b.epoch().epoch, b.watermark().batch),
        (2, last_batch.batch + 1)
    );
    let started = Instant::now();
    let mut differences = compare::differences(&a, &b, 30).unwrap();
    let unresolved_b = compare::unresolved(&store).unwrap();
    differences.extend(compare::unresolved_differences(
        &unresolved_a,
        &unresolved_b,
        30,
    ));
    eprintln!(
        "compared {} ids in {:.1} s: {} differences",
        b.minted(),
        started.elapsed().as_secs_f64(),
        differences.len()
    );
    assert!(differences.is_empty(), "{}", differences.join("\n"));
    let _ = std::fs::remove_dir_all(&scratch);
}
