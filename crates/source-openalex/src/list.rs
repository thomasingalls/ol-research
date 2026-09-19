//! What the snapshot offers: the works manifest, one entry per part file,
//! with the byte and record counts the manifest declares (plan §2). And,
//! for seeding the POC sample only (plan §8, item 8), the API's answer to
//! a batch of ids.

use std::time::Duration;

use ingest::keys::Key;
use ingest::list::{Error, Index, Lister, Lookup, encode};
use ingest::listing::{Entry, Kind};
use serde::Deserialize;

use crate::OpenAlex;

/// The works manifest of the current snapshot.
pub const MANIFEST: &str = "https://openalex.s3.amazonaws.com/data/jsonl/works/manifest.json";
/// The works API, for lookups.
pub const API: &str = "https://api.openalex.org/works";
/// Ids per lookup request: a batch of 50 DOIs in one `filter` answered
/// on 2026-09-15.
const BATCH: usize = 50;
/// The page size, twice the batch: OpenAlex holds more than one work for
/// some DOIs (42 of 698 batches of 50 answered 51 to 53 on 2026-09-15),
/// and a page equal to the batch drops the rest; the parser refuses a
/// page smaller than its count, so the loss is loud, not silent.
const PAGE: usize = 2 * BATCH;
/// Where the manifest's `s3://` URLs are served over HTTPS.
const HTTPS: &str = "https://openalex.s3.amazonaws.com/data/jsonl/";
const S3: &str = "s3://openalex/data/jsonl/";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    date: String,
    format: String,
    entity: String,
    record_count: u64,
    content_length: u64,
    files: Vec<File>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    url: String,
    meta: Meta,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta {
    content_length: u64,
    record_count: u64,
}

/// The entries of a manifest's text: every part, with its counts, its
/// URL over HTTPS, and its path below `data/jsonl/`.
///
/// # Errors
///
/// A manifest that is not the works JSON-lines manifest, whose totals do
/// not add up, or with a URL outside the snapshot's prefix.
pub fn entries(text: &str) -> Result<Vec<Entry>, Error> {
    let index = |reason: String| Error::Index {
        name: "manifest.json".to_owned(),
        reason,
    };
    let manifest: Manifest = serde_json::from_str(text).map_err(|e| index(e.to_string()))?;
    if manifest.format != "jsonl" || manifest.entity != "works" {
        return Err(index(format!(
            "format {:?}, entity {:?}; expected jsonl works",
            manifest.format, manifest.entity
        )));
    }
    let (bytes, records) = manifest.files.iter().fold((0u64, 0u64), |(b, r), f| {
        (b + f.meta.content_length, r + f.meta.record_count)
    });
    if bytes != manifest.content_length || records != manifest.record_count {
        return Err(index(format!(
            "totals do not add up: {bytes} bytes / {records} records in files, {} / {} declared",
            manifest.content_length, manifest.record_count
        )));
    }
    manifest
        .files
        .iter()
        .map(|file| {
            let path = file
                .url
                .strip_prefix(S3)
                .ok_or_else(|| index(format!("url outside {S3}: {}", file.url)))?;
            Ok(Entry {
                url: format!("{HTTPS}{path}"),
                path: path.to_owned(),
                kind: Kind::Https,
                bytes: Some(file.meta.content_length),
                records: Some(file.meta.record_count),
                md5_url: None,
                release: Some(manifest.date.clone()),
            })
        })
        .collect()
}

impl Lister for OpenAlex {
    /// A tenth of the published 10 a second; the daily budget is the
    /// binding limit (1,000 requests keyless).
    const PACE: Duration = Duration::from_secs(1);
    const CONTACT: Option<&'static str> = Some("mailto");

    fn list(index: &mut dyn Index) -> Result<Vec<Entry>, Error> {
        entries(&index.document(MANIFEST, "manifest.json")?)
    }

    fn batch(key: Key) -> Option<usize> {
        matches!(key, Key::Doi | Key::Pmid | Key::OpenAlex).then_some(BATCH)
    }

    fn lookup(key: Key, ids: &[&str]) -> Result<Lookup, Error> {
        let filter = match key {
            Key::Doi => "doi",
            Key::Pmid => "pmid",
            Key::OpenAlex => "ids.openalex",
            Key::Pmc | Key::Publisher => return Err(Error::Unsupported(key)),
        };
        let values: Vec<String> = ids.iter().map(|id| encode(id)).collect();
        Ok(Lookup {
            url: format!("{API}?filter={filter}:{}&per-page={PAGE}", values.join("|")),
            extension: "json",
        })
    }
}

#[cfg(test)]
mod tests {
    use ingest::keys::Key;
    use ingest::list::Lister;

    use super::entries;
    use crate::OpenAlex;

    const TEXT: &str = r#"{"date":"2026-06-26","format":"jsonl","entity":"works","record_count":2440,"content_length":3038474,
      "files":[{"url":"s3://openalex/data/jsonl/works/updated_date=2016-06-24/part_0000.gz","meta":{"content_length":3038473,"record_count":2439}},
               {"url":"s3://openalex/data/jsonl/works/updated_date=2026-06-26/part_0001.gz","meta":{"content_length":1,"record_count":1}}]}"#;

    #[test]
    fn the_manifest_becomes_https_entries_with_counts() {
        let entries = entries(TEXT).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].url,
            "https://openalex.s3.amazonaws.com/data/jsonl/works/updated_date=2016-06-24/part_0000.gz"
        );
        assert_eq!(
            entries[0].path,
            "works/updated_date=2016-06-24/part_0000.gz"
        );
        assert_eq!(
            (entries[0].bytes, entries[0].records),
            (Some(3_038_473), Some(2439))
        );
        assert_eq!(entries[1].release.as_deref(), Some("2026-06-26"));
        assert!(entries.iter().all(|e| e.check().is_ok()));
    }

    #[test]
    fn a_manifest_that_does_not_add_up_or_look_right_is_refused() {
        assert!(
            entries(&TEXT.replacen("\"record_count\":2440", "\"record_count\":2441", 1))
                .unwrap_err()
                .to_string()
                .contains("add up")
        );
        assert!(
            entries(&TEXT.replacen("\"entity\":\"works\"", "\"entity\":\"authors\"", 1))
                .unwrap_err()
                .to_string()
                .contains("expected jsonl works")
        );
        assert!(
            entries(&TEXT.replacen(
                "s3://openalex/data/jsonl/works/updated_date=2016",
                "s3://other/works/x",
                1
            ))
            .unwrap_err()
            .to_string()
            .contains("outside")
        );
        assert!(entries(&TEXT.replacen("\"date\"", "\"dated\"", 1)).is_err());
    }

    #[test]
    fn a_lookup_is_one_filter_of_encoded_ids() {
        let lookup = OpenAlex::lookup(Key::Doi, &["10.1000/a", "10.1000/b,c"]).unwrap();
        assert_eq!(
            lookup.url,
            "https://api.openalex.org/works?filter=doi:10.1000/a|10.1000/b%2Cc&per-page=100"
        );
        assert_eq!(lookup.extension, "json");
        assert_eq!(
            OpenAlex::lookup(Key::Pmid, &["1", "2"]).unwrap().url,
            "https://api.openalex.org/works?filter=pmid:1|2&per-page=100"
        );
        assert_eq!(
            OpenAlex::lookup(Key::OpenAlex, &["W1"]).unwrap().url,
            "https://api.openalex.org/works?filter=ids.openalex:W1&per-page=100"
        );
        assert_eq!(OpenAlex::batch(Key::Doi), Some(50));
        assert_eq!(OpenAlex::batch(Key::Pmc), None);
        assert!(OpenAlex::lookup(Key::Pmc, &["PMC1"]).is_err());
    }
}
