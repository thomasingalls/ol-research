//! The identity report: what the pass read, what it found, what the
//! registry did.

use std::collections::BTreeMap;
use std::io::{self, Write};

use ingest::Key;

use crate::name::OwnedName;

use crate::identity::{Resolution, Standing};
use crate::registry::Assignment;
use crate::work::SourceId;

/// Writes the report of one identity run: one section per part of the
/// pass, in the order the plan's §8 lists them.
///
/// # Errors
///
/// I/O.
#[allow(clippy::too_many_lines)]
pub fn identity(
    out: &mut impl Write,
    res: &Resolution,
    stub_works: &[Vec<OwnedName>],
    held: Option<&Assignment>,
    stubs: Option<&Assignment>,
) -> io::Result<()> {
    let s = &res.stats;
    writeln!(out, "identity")?;
    for source in SourceId::ALL {
        let records = s.records.get(&source).copied().unwrap_or(0);
        if records == 0 && !s.sets.contains_key(&source) {
            continue;
        }
        writeln!(
            out,
            "  {source}: {} sets, {records} records ({} repeated), {} references",
            s.sets.get(&source).copied().unwrap_or(0),
            s.repeated.get(&source).copied().unwrap_or(0),
            s.references.get(&source).copied().unwrap_or(0),
        )?;
    }
    writeln!(out, "  assertions by source and field")?;
    for ((source, field), n) in &s.asserted {
        writeln!(out, "    {source:<9} {field:<20} {n:>10}")?;
    }
    writeln!(out, "  names: {} distinct", res.arena.len())?;
    let c = &res.components;
    writeln!(
        out,
        "  components: {} works from {} components, {} of them conflicts",
        res.works.len(),
        c.total,
        c.conflict
    )?;
    writeln!(out, "  works by the sources holding them")?;
    for (pattern, n) in &c.patterns {
        writeln!(out, "    {n:>8}  {pattern}")?;
    }
    writeln!(out, "  conflict components by shape")?;
    for (shape, n) in &c.shapes {
        writeln!(out, "    {n:>8}  {shape}")?;
        for example in c.examples.get(shape).map_or(&[][..], Vec::as_slice) {
            writeln!(out, "              {example}")?;
        }
    }
    for (what, n) in &c.unresolved {
        writeln!(
            out,
            "  UNRESOLVED ({what}): {n} components the rules could not resolve"
        )?;
        for example in c
            .unresolved_examples
            .get(what)
            .map_or(&[][..], Vec::as_slice)
        {
            writeln!(out, "              {example}")?;
        }
    }
    writeln!(
        out,
        "  verdicts: {} names accepted, {} assertions rejected, {} works joined on an untestable title",
        c.accepted, c.rejected, c.uncorroborated
    )?;
    writeln!(out, "  standings by namespace")?;
    for key in Key::ALL {
        let get = |st: Standing| c.standings.get(&(key, st)).copied().unwrap_or(0);
        let (p, a, sh) = (
            get(Standing::Promoted),
            get(Standing::Alias),
            get(Standing::Shared),
        );
        if p + a + sh > 0 {
            writeln!(
                out,
                "    {key:<9} promoted {p:>8}  alias {a:>6}  shared {sh:>4}"
            )?;
        }
    }
    writeln!(out, "  reference targets by source")?;
    for source in SourceId::ALL {
        let get = |what: &'static str| s.targets.get(&(source, what)).copied().unwrap_or(0);
        if s.references.get(&source).copied().unwrap_or(0) > 0 {
            writeln!(
                out,
                "    {source:<9} held {:>9}  stub {:>9}  none {:>9}  disagree {:>5}  names beside a held one {:>7}",
                get("held"),
                get("stub"),
                get("none"),
                get("disagree"),
                get("alias beside a held name")
            )?;
        }
    }
    let mut by_key = [0u64; 4];
    for name in res.stubs.iter() {
        by_key[Key::ALL.iter().position(|k| *k == name.key()).unwrap_or(0)] += 1;
    }
    let mut sizes: BTreeMap<usize, u64> = BTreeMap::new();
    for names in stub_works {
        *sizes.entry(names.len().min(4)).or_default() += 1;
    }
    writeln!(
        out,
        "  stubs: {} works from {} names cited and held by no work ({}); by names per stub: {}",
        stub_works.len(),
        res.stubs.len(),
        Key::ALL
            .iter()
            .zip(by_key)
            .filter(|(_, n)| *n > 0)
            .map(|(k, n)| format!("{k} {n}"))
            .collect::<Vec<_>>()
            .join(", "),
        sizes
            .iter()
            .map(|(k, n)| format!(
                "{}: {n}",
                if *k == 4 {
                    "4+".to_owned()
                } else {
                    k.to_string()
                }
            ))
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    if let Some(a) = held {
        writeln!(
            out,
            "  registry, held works: {} minted, {} reused, {} merged, {} split, {} names attached, {} dropped",
            a.minted, a.reused, a.merged, a.split, a.attached, a.dropped
        )?;
    }
    if let Some(a) = stubs {
        writeln!(
            out,
            "  registry, stubs: {} minted, {} reused, {} merged, {} split, {} names attached, {} dropped",
            a.minted, a.reused, a.merged, a.split, a.attached, a.dropped
        )?;
    }
    Ok(())
}
