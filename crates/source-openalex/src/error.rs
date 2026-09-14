//! A record that did not fit the model, with enough context to find it.

use std::fmt;

/// A parse failure: the line, the work if its id was readable, and serde's
/// reason — `unknown field`, `unknown variant`, `invalid type`.
#[derive(Debug)]
pub struct Error {
    /// The line of the JSON-lines file, from one.
    pub line: usize,
    /// The work's `id`, when the line was intact enough to read it.
    pub id: Option<String>,
    /// What serde said.
    pub source: serde_json::Error,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}", self.line)?;
        if let Some(id) = &self.id {
            write!(f, " ({id})")?;
        }
        write!(f, ": {}", self.source)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
