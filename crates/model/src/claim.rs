//! A value a source asserted to be something it might not be.

use serde::{Deserialize, Serialize};

/// What a source claimed a `T` to be.
///
/// Sources put non-identifiers in identifier slots: PubMed holds a
/// publisher's e-location string, or the literal word `DOI`, in fields typed
/// as DOIs, in about one record in five hundred. The strict type never holds
/// such a value; this wrapper records that the source asserted one and what
/// it actually was, so the record stays honest and the count stays visible.
/// Downstream code has to match on it — the malformed case cannot be
/// ignored by accident (plan §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Claim<'a, T, E> {
    /// It parsed.
    Valid(T),
    /// It did not; here is what the source wrote, and why it failed.
    Malformed {
        /// The value as the source serialized it.
        raw: &'a str,
        /// Why the strict parser rejected it.
        error: E,
    },
}

impl<'a, T, E> Claim<'a, T, E> {
    /// Wraps a parse result, keeping the raw text on failure.
    pub fn from_parse(raw: &'a str, result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Self::Valid(value),
            Err(error) => Self::Malformed { raw, error },
        }
    }

    /// The parsed value, if there was one.
    pub const fn valid(&self) -> Option<&T> {
        match self {
            Self::Valid(value) => Some(value),
            Self::Malformed { .. } => None,
        }
    }

    /// True when the source's value did not parse.
    #[must_use]
    pub const fn is_malformed(&self) -> bool {
        matches!(self, Self::Malformed { .. })
    }
}
