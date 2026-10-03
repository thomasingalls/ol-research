//! The report every build prints: the documents and their text, the
//! facet distribution, the index's bytes, the mapping in force — marked
//! provisional — and a handful of fixed queries so relevance is something
//! to look at rather than guess.

use std::io::{self, Write};

use merge::document::Stats;

use crate::index::Opened;

/// Queries every build report runs, so two builds can be compared.
pub const SAMPLE_QUERIES: [&str; 6] = [
    "randomized controlled trial",
    "systematic review",
    "covid-19 vaccine",
    "crispr",
    "machine learning",
    "prisma",
];

/// Prints the build report: with the assembly's counts for a build that
/// assembled its documents, and for one built from saved documents
/// (tokenizer plan T1.3), which has none, what it was built from.
///
/// # Errors
///
/// Writing to `out`, or the engine while it counts.
///
/// # Panics
///
/// Never: every sample query is within the query parser's caps.
pub fn print(
    out: &mut impl Write,
    opened: &Opened,
    stats: Option<&Stats>,
) -> Result<(), crate::Error> {
    let m = opened.manifest();
    writeln!(out, "search {} {}", m.engine, crate::version())?;
    if let Some(stats) = stats {
        assembled(out, stats)?;
    }
    if let Some(subset) = &m.subset {
        writeln!(
            out,
            "  documents: the {} of {} (blake3 {}), the works {} names (blake3 {})",
            subset.works, subset.documents, subset.blake3, subset.list, subset.list_blake3
        )?;
    }
    writeln!(out, "  facets, top values")?;
    for (root, values) in opened.facets_of_all(8)? {
        writeln!(
            out,
            "    {root}: {}",
            values
                .iter()
                .map(|(v, n)| format!("{v} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        )?;
    }
    #[allow(clippy::cast_precision_loss)]
    let mib = m.bytes as f64 / (1024.0 * 1024.0);
    writeln!(
        out,
        "  index: {} documents, {} segments, {mib:.1} MiB",
        m.documents, m.segments
    )?;
    writeln!(out, "  mapping: {}", m.mapping)?;
    writeln!(out, "  sample queries, top three by score")?;
    for query in SAMPLE_QUERIES {
        let asked = query_parser::parse(query).expect("a sample query is within the caps");
        let hits = opened.search(&asked, 3, 1, None)?;
        writeln!(out, "    {query:?}: {} hits", hits.total)?;
        for hit in &hits.hits {
            writeln!(
                out,
                "      {:>6.2}  {}  {}",
                hit.score,
                hit.id,
                hit.title.as_deref().unwrap_or("(no title)")
            )?;
        }
    }
    Ok(())
}

/// What the assembly counted of the documents and their text.
fn assembled(out: &mut impl Write, stats: &Stats) -> io::Result<()> {
    writeln!(
        out,
        "  documents: {} held works; {} stubs skipped",
        stats.documents, stats.stubs
    )?;
    writeln!(
        out,
        "  text: title {}, abstract {} ({}), authors {} works with {} names, venue {}",
        stats.with_title,
        stats.abstract_from.values().sum::<u64>(),
        stats
            .abstract_from
            .iter()
            .map(|(s, n)| format!("{s} {n}"))
            .collect::<Vec<_>>()
            .join(", "),
        stats.with_authors,
        stats.author_names,
        stats.with_venue
    )?;
    writeln!(
        out,
        "  records read: {}; supplied text: {}",
        stats
            .records
            .iter()
            .map(|(s, n)| format!("{s} {n}"))
            .collect::<Vec<_>>()
            .join(", "),
        stats
            .supplied
            .iter()
            .map(|(s, n)| format!("{s} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

impl From<io::Error> for crate::Error {
    fn from(e: io::Error) -> Self {
        Self::Io {
            path: std::path::PathBuf::from("stdout"),
            source: e,
        }
    }
}
