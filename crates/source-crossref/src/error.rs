//! A record that did not fit the model, with enough context to find it.

use std::fmt;

/// Where in the input the failure was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// The API envelope around the items.
    Envelope,
    /// The n-th item of an envelope, from zero.
    Item(usize),
    /// The n-th line of a JSON-lines file, from one.
    Line(usize),
}

/// A parse failure: where, which work if known, and serde's reason —
/// `unknown field`, `unknown variant`, `invalid type`, with the byte column.
#[derive(Debug)]
pub struct Error {
    /// Where.
    pub context: Context,
    /// The item's DOI, when the item was intact enough to read it.
    pub doi: Option<String>,
    /// What serde said.
    pub source: serde_json::Error,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.context {
            Context::Envelope => f.write_str("envelope")?,
            Context::Item(i) => write!(f, "item {i}")?,
            Context::Line(n) => write!(f, "line {n}")?,
        }
        if let Some(doi) = &self.doi {
            write!(f, " (DOI {doi})")?;
        }
        write!(f, ": {}", self.source)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
