//! The two containers, and per-item error context.

use std::borrow::Cow;

use serde::Deserialize;
use serde_json::value::RawValue;

use crate::error::{Context, Error};
use crate::record::Work;

/// The REST API envelope around a list of works. Items are kept as raw
/// slices so each can be parsed on its own, with its index and DOI in any
/// error, rather than the whole page failing at a byte column.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Envelope<'a> {
    #[serde(borrow)]
    status: Cow<'a, str>,
    #[serde(borrow)]
    message_type: Cow<'a, str>,
    #[serde(borrow)]
    message_version: Cow<'a, str>,
    #[serde(borrow)]
    message: Message<'a>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Message<'a> {
    /// Validated as empty; never read.
    #[serde(rename = "facets")]
    _facets: Facets,
    total_results: u64,
    items_per_page: u64,
    #[serde(borrow)]
    query: Query<'a>,
    #[serde(borrow)]
    items: Vec<&'a RawValue>,
}

/// Empty unless facets were requested, which this pipeline never does.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Facets {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Query<'a> {
    start_index: u64,
    #[serde(borrow)]
    search_terms: Option<Cow<'a, str>>,
}

/// Just enough of an item to name it in an error.
#[derive(Deserialize)]
struct JustDoi<'a> {
    #[serde(rename = "DOI", borrow)]
    doi: Cow<'a, str>,
}

/// One page of the REST API: `{"status":"ok","message-type":"work-list",…}`.
#[derive(Debug)]
pub struct Page<'a> {
    envelope: Envelope<'a>,
}

impl<'a> Page<'a> {
    /// Parses the envelope; the items inside are parsed by [`Page::works`].
    ///
    /// # Errors
    ///
    /// A malformed envelope, or one with unexpected keys or a status other
    /// than `ok` / `work-list`.
    pub fn parse(input: &'a str) -> Result<Self, Error> {
        let envelope: Envelope<'a> = serde_json::from_str(input).map_err(|source| Error {
            context: Context::Envelope,
            doi: None,
            source,
        })?;
        if envelope.status != "ok" || envelope.message_type != "work-list" {
            return Err(Error {
                context: Context::Envelope,
                doi: None,
                source: serde::de::Error::custom(format!(
                    "status {:?}, message-type {:?}",
                    envelope.status, envelope.message_type
                )),
            });
        }
        Ok(Self { envelope })
    }

    /// `total-results`, as the API reported it.
    #[must_use]
    pub const fn total_results(&self) -> u64 {
        self.envelope.message.total_results
    }

    /// `items-per-page`, as the API reported it — not the number of items.
    #[must_use]
    pub const fn items_per_page(&self) -> u64 {
        self.envelope.message.items_per_page
    }

    /// `query.start-index`.
    #[must_use]
    pub const fn start_index(&self) -> u64 {
        self.envelope.message.query.start_index
    }

    /// `query.search-terms`, when the page came from a search.
    #[must_use]
    pub fn search_terms(&self) -> Option<&str> {
        self.envelope.message.query.search_terms.as_deref()
    }

    /// `message-version`, `1.0.0` so far.
    #[must_use]
    pub fn message_version(&self) -> &str {
        &self.envelope.message_version
    }

    /// How many items the page holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.envelope.message.items.len()
    }

    /// True when the page holds no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.envelope.message.items.is_empty()
    }

    /// The works, each parsed on demand, borrowing from the input.
    pub fn works(&self) -> impl Iterator<Item = Result<Work<'a>, Error>> + '_ {
        self.envelope
            .message
            .items
            .iter()
            .enumerate()
            .map(|(index, raw)| work(raw.get(), Context::Item(index)))
    }
}

/// Parses one work with context for the error.
fn work<'a>(json: &'a str, context: Context) -> Result<Work<'a>, Error> {
    serde_json::from_str::<Work<'a>>(json).map_err(|source| Error {
        context,
        doi: serde_json::from_str::<JustDoi<'_>>(json)
            .ok()
            .map(|j| j.doi.into_owned()),
        source,
    })
}

/// The public data file's container: one work per line.
pub fn jsonl(input: &str) -> impl Iterator<Item = Result<Work<'_>, Error>> + '_ {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| work(line, Context::Line(index + 1)))
}

#[cfg(test)]
mod tests {
    use super::{Page, jsonl};
    use crate::error::Context;

    const WORK: &str = r#"{"DOI":"10.1000/x","URL":"https://doi.org/10.1000/x","type":"journal-article","source":"Crossref","prefix":"10.1000","member":"1","score":0.0,"created":{"date-parts":[[2020,1,2]],"date-time":"2020-01-02T00:00:00Z","timestamp":1577923200000},"deposited":{"date-parts":[[2020,1,2]],"date-time":"2020-01-02T00:00:00Z","timestamp":1577923200000},"indexed":{"date-parts":[[2020,1,2]],"date-time":"2020-01-02T00:00:00Z","timestamp":1577923200000},"issued":{"date-parts":[[2020]]},"publisher":"P","resource":{"primary":{"URL":"https://example.org/x"}}}"#;

    fn envelope(items: &str) -> String {
        format!(
            r#"{{"status":"ok","message-type":"work-list","message-version":"1.0.0","message":{{"facets":{{}},"total-results":1,"items":[{items}],"items-per-page":20,"query":{{"start-index":0,"search-terms":null}}}}}}"#
        )
    }

    #[test]
    fn a_minimal_work_round_trips_through_both_containers() {
        let input = envelope(WORK);
        let page = Page::parse(&input).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page.message_version(), "1.0.0");
        assert_eq!(page.search_terms(), None);
        let works: Vec<_> = page.works().map(Result::unwrap).collect();
        assert_eq!(works[0].doi, "10.1000/x");
        let lines = format!("{WORK}\n\n{WORK}\n");
        let works: Vec<_> = jsonl(&lines).map(Result::unwrap).collect();
        assert_eq!(works.len(), 2);
    }

    #[test]
    fn an_unknown_field_names_the_item_and_its_doi() {
        let bad = WORK.replacen("\"publisher\"", "\"publisherr\"", 1);
        let input = envelope(&bad);
        let page = Page::parse(&input).unwrap();
        let err = page.works().next().unwrap().unwrap_err();
        assert_eq!(err.context, Context::Item(0));
        assert_eq!(err.doi.as_deref(), Some("10.1000/x"));
        assert!(
            err.to_string().contains("unknown field `publisherr`"),
            "{err}"
        );
        let err = jsonl(&bad).next().unwrap().unwrap_err();
        assert_eq!(err.context, Context::Line(1));
    }

    #[test]
    fn the_envelope_is_strict_too() {
        let with_extra =
            envelope(WORK).replacen("\"facets\":{}", "\"facets\":{},\"next-cursor\":\"x\"", 1);
        assert!(Page::parse(&with_extra).is_err());
        let not_ok = envelope(WORK).replacen("\"status\":\"ok\"", "\"status\":\"failed\"", 1);
        assert!(Page::parse(&not_ok).is_err());
    }
}
