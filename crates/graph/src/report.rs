//! The report every build prints (graph plan §8): the counts, the degree
//! distributions, who asserted what, agreement per pair of sources — the
//! merge's table again, this time from the graph — and the bytes.

use std::collections::BTreeMap;
use std::io::{self, Write};

use merge::{SourceId, WorkId};

use crate::base::{Base, Lists};
use crate::layout::{
    FORWARD_LABELS, FORWARD_NEIGHBOURS, FORWARD_OFFSETS, INV, PERM, TRANSPOSE_LABELS,
    TRANSPOSE_NEIGHBOURS, TRANSPOSE_OFFSETS,
};
use crate::nodes::Nodes;

/// Prints the report for an open base over the works it was built on.
///
/// # Errors
///
/// Writing to `out`.
pub fn print(out: &mut impl Write, base: &Base, nodes: &Nodes) -> io::Result<()> {
    let m = base.manifest();
    writeln!(
        out,
        "graph {}",
        base.dir()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
    )?;
    writeln!(
        out,
        "  nodes: {}: {} works ({} held, {} stubs), {} retired ids",
        m.nodes,
        m.works,
        m.held,
        m.stubs,
        u64::from(m.nodes) - m.works
    )?;
    let dropped = if m.held_only {
        format!(", {} dropped outside held works", m.dropped)
    } else {
        String::new()
    };
    writeln!(
        out,
        "  arcs: {} distinct, {} self-citations{dropped}",
        m.arcs, m.self_loops
    )?;
    degrees(out, base, nodes, &base.forward(), "references")?;
    degrees(out, base, nodes, &base.transpose(), "citers")?;
    most_cited(out, base)?;
    isolated(out, base, nodes)?;
    held_subgraph(out, base, nodes)?;
    asserters(out, base)?;
    agreement(out, base, nodes)?;
    files(out, base)
}

fn degrees(
    out: &mut impl Write,
    base: &Base,
    nodes: &Nodes,
    lists: &Lists<'_>,
    what: &str,
) -> io::Result<()> {
    let mut histogram: BTreeMap<u64, u64> = BTreeMap::new();
    let (mut max, mut at, mut with) = (0u64, None, 0u64);
    for pos in 0..lists.rows() {
        let Some(work) = base.work_at(pos) else {
            continue;
        };
        if !nodes.present(work) {
            continue;
        }
        let d = lists.degree(pos);
        if d == 0 {
            continue;
        }
        with += 1;
        *histogram.entry(d).or_default() += 1;
        if d > max {
            max = d;
            at = Some(work);
        }
    }
    let p = |q| percentile(&histogram, with, q);
    writeln!(
        out,
        "  {what}: {with} works have any; p50 {}, p90 {}, p99 {}, p99.9 {}, max {max}{}",
        p(0.5),
        p(0.9),
        p(0.99),
        p(0.999),
        at.map_or_else(String::new, |w| format!(" ({w})"))
    )
}

/// The smallest degree at or above which `q` of the works fall.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn percentile(histogram: &BTreeMap<u64, u64>, total: u64, q: f64) -> u64 {
    let target = ((q * total as f64).ceil() as u64).max(1);
    let mut seen = 0;
    for (d, n) in histogram {
        seen += n;
        if seen >= target {
            return *d;
        }
    }
    0
}

fn most_cited(out: &mut impl Write, base: &Base) -> io::Result<()> {
    let transpose = base.transpose();
    let mut top: Vec<(u64, WorkId)> = Vec::new();
    for pos in 0..transpose.rows() {
        let d = transpose.degree(pos);
        if d == 0 || top.len() == 10 && d <= top[9].0 {
            continue;
        }
        let Some(work) = base.work_at(pos) else {
            continue;
        };
        let i = top.partition_point(|(n, w)| *n > d || (*n == d && *w < work));
        top.insert(i, (d, work));
        top.truncate(10);
    }
    writeln!(
        out,
        "  most cited: {}",
        top.iter()
            .map(|(n, w)| format!("{w} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn isolated(out: &mut impl Write, base: &Base, nodes: &Nodes) -> io::Result<()> {
    let (forward, transpose) = (base.forward(), base.transpose());
    let mut alone = 0u64;
    for pos in 0..forward.rows() {
        if forward.degree(pos) == 0
            && transpose.degree(pos) == 0
            && base.work_at(pos).is_some_and(|w| nodes.present(w))
        {
            alone += 1;
        }
    }
    writeln!(out, "  isolated: {alone} works with no arc either way")
}

/// The subgraph induced on the **held** works, which is the one a
/// traversal really moves through: a stub has citers and no references
/// (plan §11, item 6), so it ends any walk that reaches it and no
/// directed path has one in its interior ([`crate::path`]). Its arc
/// count and its components are what say whether a path between two
/// works can be expected to exist at all, which is why they are a report
/// line and not a one-off measurement — on the sample of 2026-09-19 a
/// mean out-degree of 0.63, an order below percolation, and a largest
/// component holding a quarter of the held works.
///
/// Weakly connected components only: the directed ones are all but
/// trivial in a citation graph, time being a direction — measured on the
/// same epoch, 148,122 strong components over 148,210 held works, the
/// largest of size 3.
#[allow(clippy::cast_precision_loss)]
fn held_subgraph(out: &mut impl Write, base: &Base, nodes: &Nodes) -> io::Result<()> {
    /// Union-find with path halving, over the compacted held ids.
    fn root(parent: &mut [u32], mut at: u32) -> u32 {
        while parent[at as usize] != at {
            parent[at as usize] = parent[parent[at as usize] as usize];
            at = parent[at as usize];
        }
        at
    }
    let n = base.nodes();
    // Held ids compacted, so the union-find is over the held works alone.
    let mut ix = vec![u32::MAX; n as usize + 1];
    let mut held: Vec<WorkId> = Vec::new();
    for raw in 1..=n {
        let Some(id) = WorkId::new(raw) else { continue };
        if nodes.is_held(id) {
            ix[raw as usize] = u32::try_from(held.len()).unwrap_or(u32::MAX);
            held.push(id);
        }
    }
    let mut parent: Vec<u32> = (0..u32::try_from(held.len()).unwrap_or(u32::MAX)).collect();
    let (mut arcs, mut loops) = (0u64, 0u64);
    let mut degree: Vec<u32> = vec![0; held.len()];
    for (from, id) in held.iter().enumerate() {
        let from = u32::try_from(from).unwrap_or(u32::MAX);
        for arc in base.references(*id) {
            let to = ix[arc.work.get() as usize];
            if to == u32::MAX {
                continue;
            }
            if to == from {
                loops += 1;
                continue;
            }
            arcs += 1;
            degree[from as usize] = degree[from as usize].saturating_add(1);
            degree[to as usize] = degree[to as usize].saturating_add(1);
            let (a, b) = (root(&mut parent, from), root(&mut parent, to));
            if a != b {
                parent[a as usize] = b;
            }
        }
    }
    let mut sizes: BTreeMap<u32, u64> = BTreeMap::new();
    for i in 0..u32::try_from(held.len()).unwrap_or(u32::MAX) {
        *sizes.entry(root(&mut parent, i)).or_default() += 1;
    }
    let mut components: Vec<u64> = sizes.into_values().collect();
    components.sort_unstable_by(|a, b| b.cmp(a));
    let touched = degree.iter().filter(|d| **d > 0).count() as u64;
    let works = held.len() as u64;
    let share = |part: u64, whole: u64| {
        if whole == 0 {
            0.0
        } else {
            100.0 * part as f64 / whole as f64
        }
    };
    writeln!(
        out,
        "  held subgraph: {works} works, {arcs} arcs between them ({loops} self-citations left out), \
         mean out-degree {:.3}, {touched} works with any arc ({:.2}%)",
        if works == 0 {
            0.0
        } else {
            arcs as f64 / works as f64
        },
        share(touched, works)
    )?;
    writeln!(
        out,
        "  held components: {} weak, largest {} ({:.2}% of held works, {:.2}% of those with an arc), then {:?}",
        components.len(),
        components.first().copied().unwrap_or(0),
        share(components.first().copied().unwrap_or(0), works),
        share(components.first().copied().unwrap_or(0), touched),
        &components[1..components.len().min(6)]
    )
}

fn asserters(out: &mut impl Write, base: &Base) -> io::Result<()> {
    let forward = base.forward();
    let mut counts = vec![0u64; base.asserters().len()];
    for pos in 0..forward.rows() {
        for (_, label) in forward.raw(pos) {
            counts[usize::from(label)] += 1;
        }
    }
    writeln!(out, "  arcs by asserters")?;
    for (set, n) in base.asserters().iter().zip(&counts) {
        writeln!(out, "    {n:>9}  by {set}")?;
    }
    Ok(())
}

/// Per pair of sources: arcs both assert, arcs only the first asserts,
/// arcs only the second asserts — corpus-wide, and then over arcs whose
/// citing work both sources hold and whose target is held, which is the
/// merge plan's §6 table.
fn agreement(out: &mut impl Write, base: &Base, nodes: &Nodes) -> io::Result<()> {
    let pairs: Vec<(SourceId, SourceId)> = SourceId::ALL
        .iter()
        .enumerate()
        .flat_map(|(i, a)| SourceId::ALL[i + 1..].iter().map(move |b| (*a, *b)))
        .collect();
    let mut wide = vec![[0u64; 3]; pairs.len()];
    let mut held = vec![[0u64; 3]; pairs.len()];
    let forward = base.forward();
    for pos in 0..forward.rows() {
        let Some(u) = base.work_at(pos) else {
            continue;
        };
        let u_by = nodes.held_by(u);
        for arc in forward.list(pos) {
            let target_held = nodes.is_held(arc.work);
            for (k, (a, b)) in pairs.iter().enumerate() {
                let (has_a, has_b) = (arc.by.contains(*a), arc.by.contains(*b));
                if !(has_a || has_b) {
                    continue;
                }
                let slot = match (has_a, has_b) {
                    (true, true) => 0,
                    (true, false) => 1,
                    (false, true) => 2,
                    (false, false) => unreachable!(),
                };
                wide[k][slot] += 1;
                if u_by.contains(*a) && u_by.contains(*b) && target_held {
                    held[k][slot] += 1;
                }
            }
        }
    }
    for (title, counts) in [
        ("agreement, corpus-wide", &wide),
        (
            "agreement, over arcs whose citing work both sources hold and whose target is held",
            &held,
        ),
    ] {
        writeln!(out, "  {title}")?;
        for ((a, b), [both, only_a, only_b]) in pairs.iter().zip(counts) {
            writeln!(
                out,
                "    {a}-{b}: both {both}, {a} alone {only_a}, {b} alone {only_b}"
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::cast_precision_loss)]
fn files(out: &mut impl Write, base: &Base) -> io::Result<()> {
    let m = base.manifest();
    let bytes = |name: &str| m.files[name].bytes;
    let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
    let bits = |b: u64| {
        if m.arcs == 0 {
            0.0
        } else {
            b as f64 * 8.0 / m.arcs as f64
        }
    };
    let forward = bytes(FORWARD_OFFSETS) + bytes(FORWARD_NEIGHBOURS) + bytes(FORWARD_LABELS);
    let transpose =
        bytes(TRANSPOSE_OFFSETS) + bytes(TRANSPOSE_NEIGHBOURS) + bytes(TRANSPOSE_LABELS);
    let tables = bytes(PERM) + bytes(INV);
    writeln!(
        out,
        "  files: forward {:.1} MiB (offsets {:.1}, neighbours {:.1}, labels {:.1}), transpose {:.1} MiB, perm and inv {:.1} MiB",
        mib(forward),
        mib(bytes(FORWARD_OFFSETS)),
        mib(bytes(FORWARD_NEIGHBOURS)),
        mib(bytes(FORWARD_LABELS)),
        mib(transpose),
        mib(tables)
    )?;
    writeln!(
        out,
        "  bits an arc: forward {:.1}, transpose {:.1}, both with the permutation {:.1}",
        bits(forward),
        bits(transpose),
        bits(forward + transpose + tables)
    )
}

#[cfg(test)]
mod tests {
    use super::print;
    use crate::base::Base;
    use crate::build::tests::{sample, scratch};
    use crate::build::{Options, build};
    use crate::layout::{self, Input};

    #[test]
    fn the_report_reads_the_base() {
        let (nodes, edges) = sample();
        let root = scratch("report");
        let dir = root.join(layout::name(false));
        let input = Input {
            merged: "v1.1".into(),
            shards: 1,
        };
        build(&dir, &nodes, input, Options::default(), edges).unwrap();
        let base = Base::open(&dir).unwrap();
        let mut out = Vec::new();
        print(&mut out, &base, &nodes).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("nodes: 6: 5 works (3 held, 2 stubs), 1 retired ids"),
            "{text}"
        );
        assert!(
            text.contains("arcs: 7 distinct, 1 self-citations"),
            "{text}"
        );
        assert!(
            text.contains("most cited: #3 2, #4 2, #1 1, #2 1, #6 1"),
            "{text}"
        );
        // #2 -> #3 is pubmed+crossref; both hold #2 and #3 is held: counted.
        // #1 -> #2 is all three: counted for every pair.
        assert!(
            text.contains("pubmed-crossref: both 2, pubmed alone 2, crossref alone 2"),
            "{text}"
        );
        assert!(text.contains("isolated: 0 works"), "{text}");
        // Held to held: 1 -> 2, 2 -> 1 and 2 -> 3, with 3 -> 3 left out
        // and the three arcs to the stubs 4 and 6 outside the subgraph.
        assert!(
            text.contains(
                "held subgraph: 3 works, 3 arcs between them (1 self-citations left out), \
                 mean out-degree 1.000, 3 works with any arc (100.00%)"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "held components: 1 weak, largest 3 (100.00% of held works, \
                 100.00% of those with an arc), then []"
            ),
            "{text}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
