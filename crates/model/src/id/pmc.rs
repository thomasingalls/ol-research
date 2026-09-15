use serde::{Deserialize, Serialize};
use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use super::{ParseError, parse_digits, strip_literal};

/// A PubMed Central identifier.
///
/// One identifier, two serializations by role (plan §2; `pubmed26n1334.xml.gz`):
///
/// * A record's own PMC ID, `<ArticleId IdType="pmc">PMC12834818</ArticleId>`
///   — [`PmcId::from_prefixed`], also [`FromStr`].
/// * A reference's PMC ID, `<ArticleId IdType="pmcid">5349499</ArticleId>`
///   — [`PmcId::from_digits`].
///
/// The largest observed is `PMC12846865`; `u32` has three hundred times the
/// headroom. `Option<PmcId>` is four bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct PmcId(NonZeroU32);

impl PmcId {
    /// The prefix of the record-level form.
    pub const PREFIX: &'static str = "PMC";

    /// Wraps a raw value; `None` for zero.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// The numeric value, without the `PMC`.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Parses the bare-digits form used inside reference lists.
    ///
    /// # Errors
    ///
    /// Anything other than a positive decimal with no sign, no leading zero
    /// and no surrounding whitespace, or a value above `u32::MAX`.
    pub fn from_digits(s: &str) -> Result<Self, ParseError> {
        let value = parse_digits(s, u64::from(u32::MAX))?;
        u32::try_from(value)
            .ok()
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or(ParseError::Overflow)
    }

    /// Parses the `PMC`-prefixed form a record uses for its own identifier.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with exactly `PMC`;
    /// then as [`Self::from_digits`], with offsets relative to the whole
    /// input.
    pub fn from_prefixed(s: &str) -> Result<Self, ParseError> {
        let digits = strip_literal(s, Self::PREFIX)?;
        Self::from_digits(digits).map_err(|e| e.shift(Self::PREFIX.len()))
    }
}

impl FromStr for PmcId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_prefixed(s)
    }
}

impl fmt::Display for PmcId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}

impl From<PmcId> for u32 {
    fn from(id: PmcId) -> Self {
        id.get()
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, PmcId};

    #[test]
    fn two_forms_one_identifier() {
        let own = PmcId::from_prefixed("PMC12834818").unwrap();
        let referenced = PmcId::from_digits("12834818").unwrap();
        assert_eq!(own, referenced);
        assert_eq!(own.to_string(), "PMC12834818");
        assert_eq!("PMC12834818".parse::<PmcId>(), Ok(own));
    }

    #[test]
    fn forms_are_not_interchangeable() {
        assert_eq!(
            PmcId::from_digits("PMC1"),
            Err(ParseError::NotDigit { offset: 0 })
        );
        assert_eq!(PmcId::from_prefixed("1"), Err(ParseError::BadPrefix));
        assert_eq!(PmcId::from_prefixed("pmc1"), Err(ParseError::BadPrefix));
        assert_eq!(PmcId::from_prefixed("PMC"), Err(ParseError::Empty));
        assert_eq!(PmcId::from_prefixed("PMC01"), Err(ParseError::LeadingZero));
        assert_eq!(
            PmcId::from_prefixed("PMC1 "),
            Err(ParseError::NotDigit { offset: 4 })
        );
    }

    #[test]
    fn observed_maximum_fits_comfortably() {
        assert_eq!(
            PmcId::from_prefixed("PMC12846865").map(PmcId::get),
            Ok(12_846_865)
        );
        assert_eq!(size_of::<Option<PmcId>>(), 4);
    }
}
