//! The reports: the identity pass's — what it read, what it found, what
//! the registry did — and the merge's — every promoted field's agreement,
//! the references resolved, the edges' agreement, the relations, what
//! was written (merge plan §8).

use std::collections::BTreeMap;
use std::io::{self, Write};

use ingest::Key;

use crate::build::Merged;
use crate::layout::Summary;

use crate::identity::{Resolution, Standing, StubWorks};
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
    stub_works: &StubWorks,
    registry: Option<&Assignment>,
    changes: &[String],
) -> io::Result<()> {
    let s = &res.stats;
    writeln!(out, "identity")?;
    for source in SourceId::ALL {
        let records = s.records.get(&source).copied().unwrap_or(0);
        if records == 0 && !s.sets.contains_key(&source) {
            continue;
        }
        let n = |m: &BTreeMap<SourceId, u64>| m.get(&source).copied().unwrap_or(0);
        writeln!(
            out,
            "  {source}: {} sets, {records} records ({} superseded by a newer copy, {} withdrawn, {} repeated, {} names joined from another copy), {} references ({} of those copies skipped)",
            n(&s.sets),
            n(&s.superseded),
            n(&s.deleted),
            n(&s.repeated),
            n(&s.joined),
            n(&s.references),
            n(&s.silent_references),
        )?;
    }
    writeln!(out, "  assertions by source and field")?;
    for ((source, field), n) in &s.asserted {
        writeln!(out, "    {source:<9} {field:<20} {n:>10}")?;
    }
    writeln!(out, "  names: {} distinct", res.arena.len())?;
    if s.collided > 0 {
        writeln!(
            out,
            "  publisher ids and aliases one source claimed for more than one of its records, and so no article's id: {} values, {} assertions rejected",
            s.collided, s.collided_assertions
        )?;
        for example in &s.collided_examples {
            writeln!(out, "              {example}")?;
        }
    }
    if s.corrected > 0 {
        writeln!(
            out,
            "  aliases a record of their own work says it corrects, and so not its names: {} assertions rejected; the records giving most",
            s.corrected
        )?;
        for example in &s.corrected_examples {
            writeln!(out, "              {example}")?;
        }
    }
    if !s.listed_corrections.is_empty() {
        writeln!(
            out,
            "  records whose aliases their source's list says are what they correct, judged by hand: {}",
            s.listed_corrections.len()
        )?;
        for record in &s.listed_corrections {
            writeln!(out, "              {record}")?;
        }
    }
    if s.container_aliases > 0 {
        writeln!(
            out,
            "  aliases a container's record gives — a journal's, a volume's, an issue's, a series' — and so not its names: {} assertions rejected; the records giving most",
            s.container_aliases
        )?;
        for example in &s.container_examples {
            writeln!(out, "              {example}")?;
        }
    }
    if s.stars > 0 {
        writeln!(
            out,
            "  names every record of a large component carries under a title of its own, and so no article's id: {} values, {} assertions rejected",
            s.stars, s.star_assertions
        )?;
        for example in &s.star_examples {
            writeln!(out, "              {example}")?;
        }
    }
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
    if !s.related.is_empty() {
        writeln!(out, "  relation targets by source")?;
        for source in SourceId::ALL {
            let get = |what: &'static str| s.related.get(&(source, what)).copied().unwrap_or(0);
            if s.related.keys().any(|(src, _)| *src == source) {
                writeln!(
                    out,
                    "    {source:<9} held {:>9}  stub {:>9}  none {:>9}",
                    get("held"),
                    get("stub"),
                    get("none")
                )?;
            }
        }
    }
    let mut by_key = [0u64; Key::ALL.len()];
    for name in res.stubs.iter() {
        by_key[Key::ALL.iter().position(|k| *k == name.key()).unwrap_or(0)] += 1;
    }
    let mut sizes: BTreeMap<usize, u64> = BTreeMap::new();
    for names in stub_works.iter() {
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
    for ((source, what), n) in &s.unmapped {
        writeln!(out, "  UNMAPPED ({source}): {what}: {n} records")?;
    }
    if let Some(a) = registry {
        writeln!(
            out,
            "  registry, held works then stubs: {} minted, {} reused, {} merged, {} split, {} names attached, {} dropped",
            a.minted, a.reused, a.merged, a.split, a.attached, a.dropped
        )?;
    }
    if !changes.is_empty() {
        writeln!(
            out,
            "  ids of record merged and split, the first of each: the work each went into, its records then its names"
        )?;
        for change in changes {
            writeln!(out, "              {change}")?;
        }
    }
    Ok(())
}

/// Writes the report of one merge run after its identity report: the
/// promoted fields' agreement (the table of merge plan §5, measured), the
/// kinds, the status flags, the references and edges, the relations, the
/// unresolved, and what was written.
///
/// # Errors
///
/// I/O.
#[allow(clippy::too_many_lines, clippy::cast_precision_loss)] // reporting only
pub fn merge(out: &mut impl Write, m: &Merged, written: Option<&Summary>) -> io::Result<()> {
    let s = &m.stats;
    writeln!(out, "merge")?;
    writeln!(
        out,
        "  views: {}",
        SourceId::ALL
            .iter()
            .map(|src| format!("{src} {}", s.viewed.get(src).copied().unwrap_or(0)))
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    let filled: u64 = s.filled.values().sum();
    if filled > 0 {
        writeln!(
            out,
            "    {filled} of those are a second copy of a record, filling what the newest has none of"
        )?;
    }
    writeln!(out, "  works: {} held, {} stubs", m.held, m.stubs)?;
    writeln!(
        out,
        "  fields: works with a value; compared = two or more candidates; disputed = another source differs under the rule"
    )?;
    writeln!(
        out,
        "    {:<10} {:>8} {:>9} {:>9} {:>7}  promoted from",
        "field", "present", "compared", "disputed", "rate"
    )?;
    for (name, f) in &m.fields {
        let rate = if f.compared == 0 {
            0.0
        } else {
            100.0 * f.disputed as f64 / f.compared as f64
        };
        writeln!(
            out,
            "    {name:<10} {:>8} {:>9} {:>9} {rate:>6.1}%  {}",
            f.present,
            f.compared,
            f.disputed,
            f.from
                .iter()
                .map(|(src, n)| format!("{src} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        )?;
        for ((a, b), (compared, differ)) in &f.pairs {
            let rate = if *compared == 0 {
                0.0
            } else {
                100.0 * *differ as f64 / *compared as f64
            };
            writeln!(
                out,
                "      {a}-{b}: {differ} of {compared} differ ({rate:.1}%)"
            )?;
        }
    }
    let placed: u64 = m.joins.values().sum();
    writeln!(
        out,
        "  placements: {placed} works with one; how their records' placements joined (citation fields plan CF2)"
    )?;
    for (join, n) in &m.joins {
        writeln!(out, "    {:<40} {n:>9}", join.as_str())?;
    }
    let with_both: u64 = m.kinds.values().sum();
    writeln!(
        out,
        "  kinds: {with_both} works with a form and a genre, {} whose genre cannot live in its form",
        m.kind_conflicts
    )?;
    let mut kinds: Vec<_> = m.kinds.iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for ((form, genre), n) in kinds.iter().take(24) {
        let flag = if form.admits(*genre) {
            ""
        } else {
            "  <- conflict"
        };
        writeln!(out, "    {n:>8}  {form} / {genre}{flag}")?;
    }
    writeln!(out, "  status by flag and the sources asserting it")?;
    for ((flag, by), n) in &m.status {
        writeln!(out, "    {n:>8}  {flag} by {by}")?;
    }
    writeln!(
        out,
        "  references by source: held, stub, self, no name, shared, disagree, empty, unknown, citing record unknown"
    )?;
    for source in SourceId::ALL {
        if !s.references.keys().any(|(src, _)| *src == source) {
            continue;
        }
        let get = |what: &'static str| s.references.get(&(source, what)).copied().unwrap_or(0);
        writeln!(
            out,
            "    {source:<9} {:>9} {:>9} {:>6} {:>9} {:>6} {:>6} {:>7} {:>7} {:>7}",
            get("held"),
            get("stub"),
            get("self"),
            get("no name"),
            get("shared"),
            get("disagree"),
            get("empty"),
            get("unknown"),
            get("citing record unknown")
        )?;
    }
    writeln!(
        out,
        "  edges: {} distinct, {} of them self-citations",
        m.edge_count, m.self_citations
    )?;
    for (by, n) in &m.edges_by {
        writeln!(out, "    {n:>9}  by {by}")?;
    }
    writeln!(
        out,
        "  agreement, over edges whose citing work both sources hold and whose target is held"
    )?;
    for ((a, b), [both, only_a, only_b]) in &m.agreement {
        writeln!(
            out,
            "    {a}-{b}: both {both}, {a} alone {only_a}, {b} alone {only_b}"
        )?;
    }
    writeln!(
        out,
        "  relations: {} kept; targets by source: held, stub, not a name, shared, unknown",
        m.relation_count
    )?;
    for source in SourceId::ALL {
        if !s.relations.keys().any(|(src, _)| *src == source) {
            continue;
        }
        let get = |what: &'static str| s.relations.get(&(source, what)).copied().unwrap_or(0);
        writeln!(
            out,
            "    {source:<9} {:>9} {:>9} {:>10} {:>6} {:>7}",
            get("held"),
            get("stub"),
            get("not a name"),
            get("shared"),
            get("unknown")
        )?;
    }
    let mut kinds: Vec<_> = s.relation_kinds.iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (kind, n) in kinds.iter().take(30) {
        writeln!(out, "    {n:>9}  {kind}")?;
    }
    writeln!(
        out,
        "  unresolved: {} rows ({})",
        m.unresolved_by.values().sum::<u64>(),
        m.unresolved_by
            .iter()
            .map(|(why, n)| format!("{why} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    for ((source, what), n) in &s.unmapped {
        writeln!(out, "  UNMAPPED ({source}): {what}: {n} records")?;
    }
    if let Some(w) = written {
        writeln!(
            out,
            "  written: {} shards, {}, {:.1} MiB",
            w.shards,
            w.records
                .iter()
                .map(|(kind, n)| format!("{n} {kind}"))
                .collect::<Vec<_>>()
                .join(", "),
            w.bytes as f64 / (1024.0 * 1024.0)
        )?;
    }
    Ok(())
}
