//! The two containers: the snapshot's JSON lines, one work per line, and
//! the API's list page, one lookup's answer (plan §8, item 8) — with
//! line or item and id context.

use std::io::BufRead;
use std::ops::ControlFlow;

use ingest::frame::{self, ReadError};
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::error::Error;
use crate::record::Work;

/// Just enough of a work to name it in an error.
#[derive(Deserialize)]
struct JustId<'a> {
    #[serde(borrow)]
    id: std::borrow::Cow<'a, str>,
}

/// The API's list page: `{"meta":…,"results":[…],"group_by":[]}`. Strict:
/// every key known, every result present (`count` is the page), no
/// groups.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page<'a> {
    #[serde(borrow)]
    meta: Meta<'a>,
    #[serde(borrow)]
    results: Vec<&'a RawValue>,
    #[serde(borrow)]
    group_by: Vec<&'a RawValue>,
}

/// Validated as present, mostly never read: `count`, `page` and
/// `per_page` are checked against the results.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta<'a> {
    count: u64,
    #[serde(rename = "db_response_time_ms")]
    _db_response_time_ms: u64,
    page: u64,
    per_page: u64,
    #[serde(rename = "groups_count")]
    _groups_count: Option<u64>,
    /// The query as the API read it back; on some responses.
    #[serde(borrow, default, rename = "x_query")]
    _x_query: Option<&'a RawValue>,
    /// What the request cost against the daily budget.
    #[serde(default, rename = "cost_usd")]
    _cost_usd: Option<f64>,
}

/// One line, numbered from one, as a work borrowed from it.
///
/// # Errors
///
/// The line does not parse; the error names the line and the work's id.
pub fn line(number: usize, text: &str) -> Result<Work<'_>, Error> {
    serde_json::from_str::<Work<'_>>(text).map_err(|source| Error {
        line: number,
        id: serde_json::from_str::<JustId<'_>>(text)
            .ok()
            .map(|j| j.id.into_owned()),
        source,
    })
}

/// The works of a raw file in either container, read as a stream and told
/// apart by the first bytes: an API page starts with `{"meta"`, a part
/// file with a work. A page is one document, held whole while its
/// results are parsed one by one (numbered from one, as lines are); lines
/// are read one at a time from a buffer reused for the next. Blank lines
/// are skipped. A result or line that does not parse is handed to `each`
/// as its error, and the next is read if `each` goes on: each is framed
/// before it is parsed, so one cannot spoil the next.
///
/// # Errors
///
/// The stream's, or a page that is not one page of works.
pub fn records<R: BufRead, F>(mut input: R, mut each: F) -> Result<(), ReadError<Error>>
where
    F: FnMut(Result<&Work<'_>, Error>) -> ControlFlow<()>,
{
    let mut one = |work: Result<Work<'_>, Error>| match work {
        Ok(work) => each(Ok(&work)),
        Err(e) => each(Err(e)),
    };
    if frame::starts_with(&mut input, b"{\"meta\"")? {
        let mut buf = Vec::new();
        input.read_to_end(&mut buf)?;
        let page = page(frame::utf8(&buf)?).map_err(ReadError::Parse)?;
        for (index, raw) in page.results.iter().enumerate() {
            if one(line(index + 1, raw.get())).is_break() {
                break;
            }
        }
        Ok(())
    } else {
        frame::lines(input, |number, text| Ok(one(line(number, text))))
    }
}

/// One API page, checked: the first page, as many results as `meta.count`
/// says and no more than it holds, and no groups — a shorter page would
/// lose works silently.
fn page(text: &str) -> Result<Page<'_>, Error> {
    let envelope = |source| Error {
        line: 0,
        id: None,
        source,
    };
    let page: Page<'_> = serde_json::from_str(text).map_err(envelope)?;
    if page.meta.page != 1 || page.meta.count > page.meta.per_page {
        return Err(envelope(serde::de::Error::custom(format!(
            "page {} of {} results, {} per page: not one lookup's answer",
            page.meta.page, page.meta.count, page.meta.per_page
        ))));
    }
    if u64::try_from(page.results.len()).ok() != Some(page.meta.count) {
        return Err(envelope(serde::de::Error::custom(format!(
            "meta.count {} but {} results on the page",
            page.meta.count,
            page.results.len()
        ))));
    }
    if !page.group_by.is_empty() {
        return Err(envelope(serde::de::Error::custom("group_by is not empty")));
    }
    Ok(page)
}

/// The works of a part file's text, each parsed on demand and borrowed
/// from it. Blank lines are skipped. For tests and eyes; the runner
/// streams through [`records`].
pub fn jsonl(input: &str) -> impl Iterator<Item = Result<Work<'_>, Error>> + '_ {
    input
        .lines()
        .enumerate()
        .filter(|(_, text)| !text.trim().is_empty())
        .map(|(index, text)| line(index + 1, text))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::ops::ControlFlow;

    use ingest::ReadError;

    use super::records;

    const WORK: &str = include_str!("../fixtures/smallest.json");

    fn page(count: u64, items: &[&str], extra: &str) -> String {
        format!(
            r#"{{"meta":{{"count":{count},"db_response_time_ms":9,"page":1,"per_page":50,"groups_count":null{extra}}},"results":[{}],"group_by":[]}}"#,
            items.join(",")
        )
    }

    fn count(text: &str) -> Result<usize, ReadError<crate::Error>> {
        let mut n = 0;
        records(Cursor::new(text), |_| {
            n += 1;
            ControlFlow::Continue(())
        })?;
        Ok(n)
    }

    #[test]
    fn an_api_page_is_read_like_lines_and_checked_whole() {
        let work = WORK.trim();
        assert_eq!(count(&page(2, &[work, work], "")).unwrap(), 2);
        assert_eq!(count(&page(0, &[], ",\"cost_usd\":0.0001")).unwrap(), 0);
        assert_eq!(count(&format!("{work}\n{work}\n")).unwrap(), 2);
        let short = count(&page(2, &[work], "")).unwrap_err();
        assert!(short.to_string().contains("meta.count 2"), "{short}");
        let extra = count(&page(1, &[work], ",\"next_cursor\":\"x\"")).unwrap_err();
        assert!(extra.to_string().contains("unknown field"), "{extra}");
        let deep =
            count(&page(1, &[work], "").replacen("\"page\":1", "\"page\":2", 1)).unwrap_err();
        assert!(deep.to_string().contains("page 2"), "{deep}");
        let mut seen = 0;
        records(Cursor::new(page(2, &[work, work], "")), |_| {
            seen += 1;
            ControlFlow::Break(())
        })
        .unwrap();
        assert_eq!(seen, 1, "break stops before the next result");
    }

    #[test]
    fn a_line_or_result_that_does_not_parse_is_handed_over_and_the_next_read() {
        let work = WORK.trim();
        let bad = work.replacen('{', "{\"no_such_field\":1,", 1);
        let lines = format!("{work}\n{bad}\n{work}\n");
        let page = page(3, &[work, &bad, work], "");
        for input in [lines, page] {
            let mut seen = Vec::new();
            records(Cursor::new(&input), |work| {
                seen.push(work.map(|_| ()).map_err(|e| e.line));
                ControlFlow::Continue(())
            })
            .unwrap();
            assert_eq!(seen, [Ok(()), Err(2), Ok(())], "{input}");
        }
    }
}
