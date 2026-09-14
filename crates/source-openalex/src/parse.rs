//! The snapshot container: one work per line, with line and id context.

use serde::Deserialize;

use crate::error::Error;
use crate::record::Work;

/// Just enough of a work to name it in an error.
#[derive(Deserialize)]
struct JustId<'a> {
    #[serde(borrow)]
    id: std::borrow::Cow<'a, str>,
}

/// The works of a JSON-lines part file, each parsed on demand and borrowed
/// from the input. Blank lines are skipped.
pub fn jsonl(input: &str) -> impl Iterator<Item = Result<Work<'_>, Error>> + '_ {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str::<Work<'_>>(line).map_err(|source| Error {
                line: index + 1,
                id: serde_json::from_str::<JustId<'_>>(line)
                    .ok()
                    .map(|j| j.id.into_owned()),
                source,
            })
        })
}
