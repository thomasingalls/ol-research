//! What NCBI offers: the baseline and update-file directory indexes, one
//! entry per `pubmedNNnNNNN.xml.gz` with its `.md5` sidecar (plan §2).
//! The index is an HTML directory listing; it is read strictly — every
//! link must be a data file, its `.md5` sidecar, its `_stats.html` page
//! (update files have one each, baseline files none), or one of the few
//! known others, and the file numbers must be contiguous — so a change in
//! what NCBI publishes is a failed listing, not a silently shorter one.
//! It fired on first contact: the update index's stats pages.
//!
//! And, for seeding the POC sample only (plan §8, item 8), E-utilities'
//! `efetch`, which answers a batch of PMIDs with the baseline files' own
//! `PubmedArticleSet` XML.

use std::collections::BTreeSet;
use std::time::Duration;

use ingest::keys::Key;
use ingest::list::{Error, Index, Lister, Lookup};
use ingest::listing::{Entry, Kind};

use crate::Pubmed;

/// The FTP site over HTTPS.
pub const BASE: &str = "https://ftp.ncbi.nlm.nih.gov/pubmed/";
/// E-utilities' `efetch`, for lookups.
pub const EFETCH: &str = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi";
/// PMIDs per lookup request, well inside what a GET carries.
const BATCH: usize = 200;
/// The two directories listed, in order.
const DIRECTORIES: [&str; 2] = ["baseline", "updatefiles"];
/// Links an index may hold besides data files and sidecars.
const KNOWN: [&str; 3] = [
    "/pubmed/",
    "README.txt",
    "https://www.hhs.gov/vulnerability-disclosure-policy/index.html",
];

/// The entries of one directory index's HTML.
///
/// # Errors
///
/// A link that is none of the known kinds, a file without its sidecar
/// or a sidecar or stats page without its file, files from more than
/// one year, or numbers that are not contiguous.
pub fn entries(html: &str, directory: &str) -> Result<Vec<Entry>, Error> {
    let index = |reason: String| Error::Index {
        name: format!("{directory}.html"),
        reason,
    };
    let mut files = BTreeSet::new();
    let mut sidecars = BTreeSet::new();
    let mut stats = BTreeSet::new();
    for href in hrefs(html) {
        if KNOWN.contains(&href) {
            continue;
        }
        if let Some(name) = href.strip_suffix(".md5") {
            if number(name).is_some() {
                sidecars.insert(name.to_owned());
                continue;
            }
        } else if let Some(stem) = href.strip_suffix("_stats.html") {
            let name = format!("{stem}.xml.gz");
            if number(&name).is_some() {
                stats.insert(name);
                continue;
            }
        } else if number(href).is_some() {
            files.insert(href.to_owned());
            continue;
        }
        return Err(index(format!("unexpected link {href:?}")));
    }
    if files != sidecars {
        let odd: Vec<&String> = files.symmetric_difference(&sidecars).take(5).collect();
        return Err(index(format!(
            "files and .md5 sidecars do not pair up; first few odd ones: {odd:?}"
        )));
    }
    if !stats.is_empty() && stats != files {
        let odd: Vec<&String> = files.symmetric_difference(&stats).take(5).collect();
        return Err(index(format!(
            "files and _stats.html pages do not pair up; first few odd ones: {odd:?}"
        )));
    }
    let numbers: Vec<(u32, u32)> = files.iter().filter_map(|f| number(f)).collect();
    let years: BTreeSet<u32> = numbers.iter().map(|(y, _)| *y).collect();
    if years.len() > 1 {
        return Err(index(format!("files from more than one year: {years:?}")));
    }
    let contiguous = numbers.windows(2).all(|w| w[1].1 == w[0].1 + 1);
    if !contiguous {
        return Err(index("file numbers are not contiguous".to_owned()));
    }
    let release = years.first().map(|y| format!("20{y:02}"));
    Ok(files
        .iter()
        .map(|name| Entry {
            url: format!("{BASE}{directory}/{name}"),
            path: format!("{directory}/{name}"),
            kind: Kind::Https,
            bytes: None,
            records: None,
            md5_url: Some(format!("{BASE}{directory}/{name}.md5")),
            release: release.clone(),
        })
        .collect())
}

/// `pubmed26n1334.xml.gz` → `(26, 1334)`.
fn number(name: &str) -> Option<(u32, u32)> {
    let rest = name.strip_prefix("pubmed")?.strip_suffix(".xml.gz")?;
    let (year, n) = rest.split_once('n')?;
    if year.len() != 2 || n.len() != 4 {
        return None;
    }
    Some((year.parse().ok()?, n.parse().ok()?))
}

/// The `href` values of an HTML index, in order.
fn hrefs(html: &str) -> impl Iterator<Item = &str> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
}

impl Lister for Pubmed {
    /// A third of the three a second NCBI allows without a key.
    const PACE: Duration = Duration::from_secs(1);
    /// NCBI's `email`, registered with `tool` before it will unblock an
    /// address; `tool` is always sent, the address only when given.
    const CONTACT: Option<&'static str> = Some("email");

    fn batch(key: Key) -> Option<usize> {
        (key == Key::Pmid).then_some(BATCH)
    }

    fn lookup(key: Key, ids: &[&str]) -> Result<Lookup, Error> {
        if key != Key::Pmid {
            return Err(Error::Unsupported(key));
        }
        Ok(Lookup {
            url: format!(
                "{EFETCH}?db=pubmed&retmode=xml&tool=ol-rht&id={}",
                ids.join(",")
            ),
            extension: "xml",
        })
    }

    fn list(index: &mut dyn Index) -> Result<Vec<Entry>, Error> {
        let mut all = Vec::new();
        for directory in DIRECTORIES {
            let html =
                index.document(&format!("{BASE}{directory}/"), &format!("{directory}.html"))?;
            all.extend(entries(&html, directory)?);
        }
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::{entries, number};

    fn index(names: &[&str]) -> String {
        let mut html = String::from(
            "<html><body><a href=\"/pubmed/\">Parent</a><a href=\"README.txt\">README.txt</a>",
        );
        for n in names {
            use std::fmt::Write as _;
            let _ = writeln!(html, "<a href=\"{n}\">{n}</a>  2026-01-29 16:14  13M");
        }
        html.push_str("<a href=\"https://www.hhs.gov/vulnerability-disclosure-policy/index.html\">x</a></body></html>");
        html
    }

    #[test]
    fn files_pair_with_sidecars_and_become_entries() {
        let html = index(&[
            "pubmed26n0002.xml.gz.md5",
            "pubmed26n0001.xml.gz",
            "pubmed26n0001.xml.gz.md5",
            "pubmed26n0002.xml.gz",
        ]);
        let entries = entries(&html, "baseline").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].url,
            "https://ftp.ncbi.nlm.nih.gov/pubmed/baseline/pubmed26n0001.xml.gz"
        );
        assert_eq!(entries[0].path, "baseline/pubmed26n0001.xml.gz");
        assert_eq!(
            entries[0].md5_url.as_deref(),
            Some("https://ftp.ncbi.nlm.nih.gov/pubmed/baseline/pubmed26n0001.xml.gz.md5")
        );
        assert_eq!(entries[1].release.as_deref(), Some("2026"));
        assert!(entries.iter().all(|e| e.check().is_ok()));
        assert_eq!(number("pubmed26n1334.xml.gz"), Some((26, 1334)));
        assert_eq!(number("pubmed26n1334.xml"), None);
    }

    #[test]
    fn anything_surprising_in_the_index_fails_the_listing() {
        let e = |names: &[&str]| entries(&index(names), "baseline").unwrap_err().to_string();
        assert!(
            e(&[
                "pubmed26n0001.xml.gz",
                "pubmed26n0001.xml.gz.md5",
                "stats.html"
            ])
            .contains("unexpected link")
        );
        assert!(e(&["pubmed26n0001.xml.gz"]).contains("pair up"));
        assert!(
            e(&[
                "pubmed26n0001.xml.gz",
                "pubmed26n0001.xml.gz.md5",
                "pubmed26n0002.xml.gz",
                "pubmed26n0002.xml.gz.md5",
                "pubmed26n0001_stats.html"
            ])
            .contains("_stats.html pages")
        );
        let with_stats = index(&[
            "pubmed26n1335.xml.gz",
            "pubmed26n1335.xml.gz.md5",
            "pubmed26n1335_stats.html",
        ]);
        assert_eq!(
            entries(&with_stats, "updatefiles").unwrap()[0].path,
            "updatefiles/pubmed26n1335.xml.gz"
        );
        assert!(
            e(&[
                "pubmed26n0001.xml.gz",
                "pubmed26n0001.xml.gz.md5",
                "pubmed26n0003.xml.gz",
                "pubmed26n0003.xml.gz.md5"
            ])
            .contains("contiguous")
        );
        assert!(
            e(&[
                "pubmed26n0001.xml.gz",
                "pubmed26n0001.xml.gz.md5",
                "pubmed27n0002.xml.gz",
                "pubmed27n0002.xml.gz.md5"
            ])
            .contains("more than one year")
        );
    }
}
