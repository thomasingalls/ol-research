//! The snapshot container: one work per line, with line and id context.

use std::io::BufRead;
use std::ops::ControlFlow;

use ingest::frame::{self, ReadError};
use serde::Deserialize;

use crate::error::Error;
use crate::record::Work;

/// Just enough of a work to name it in an error.
#[derive(Deserialize)]
struct JustId<'a> {
    #[serde(borrow)]
    id: std::borrow::Cow<'a, str>,
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

/// The works of a part file read as a stream, each parsed from a line
/// buffer reused for the next. Blank lines are skipped.
///
/// # Errors
///
/// The stream's, or the first line that does not parse.
pub fn records<R: BufRead, F>(input: R, mut each: F) -> Result<(), ReadError<Error>>
where
    F: FnMut(&Work<'_>) -> ControlFlow<()>,
{
    frame::lines(input, |number, text| {
        line(number, text).map(|work| each(&work))
    })
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
