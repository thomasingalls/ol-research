//! The report every build prints: the documents and their text, the
//! facet distribution, the index's bytes, the mapping in force — marked
//! provisional — and a handful of fixed queries so relevance is something
//! to look at rather than guess.

use std::io::{self, Write};

use merge::document::Stats;

use crate::index::Opened;
use crate::schema;

/// Queries every build report runs, so two builds can be compared.
pub const SAMPLE_QUERIES: [&str; 6] = [
    "randomized controlled trial",
    "systematic review",
    "covid-19 vaccine",
    "crispr",
    "machine learning",
    "prisma",
];

/// Prints the build report.
///
/// # Errors
///
/// Writing to `out`, or the engine while it counts.
pub fn print(out: &mut impl Write, opened: &Opened, stats: &Stats) -> Result<(), crate::Error> {
    let m = opened.manifest();
    writeln!(out, "search {} {}", m.engine, crate::version())?;
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
    )?;
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
    writeln!(out, "  mapping: {}", schema::DESCRIPTION)?;
    writeln!(out, "  sample queries, top three by score")?;
    for query in SAMPLE_QUERIES {
        let hits = opened.search(query, 3, 1)?;
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

impl From<io::Error> for crate::Error {
    fn from(e: io::Error) -> Self {
        Self::Io {
            path: std::path::PathBuf::from("stdout"),
            source: e,
        }
    }
}
