use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use super::{ParseError, parse_digits, strip_literal};

/// A Wikidata item: `Q` and a positive decimal, `Q182973` (the University
/// of Iowa).
///
/// Crossref writes it as `https://www.wikidata.org/entity/Q182973` on an
/// institution or an affiliation — the third kind of organisation id its
/// deposit schema allows (`institution_id@type`, 5.3.1 on), beside ROR and
/// ISNI. The largest among the records the March 2026 file's survey
/// stopped at is `Q128137502` (2026-09-26); `u32` has thirty times the
/// headroom. `Option<Qid>` is four bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Qid(NonZeroU32);

impl Qid {
    /// The letter that makes the number an item's.
    pub const PREFIX: &'static str = "Q";
    /// What Crossref writes before it.
    pub const ENTITY_URL_PREFIX: &'static str = "https://www.wikidata.org/entity/";

    /// The number, without the `Q`.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Parses `Q182973`.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with exactly `Q`;
    /// then anything other than a positive decimal with no sign, no
    /// leading zero and no surrounding whitespace, or a value above
    /// `u32::MAX`, with offsets relative to the whole input.
    pub fn from_prefixed(s: &str) -> Result<Self, ParseError> {
        let digits = strip_literal(s, Self::PREFIX)?;
        parse_digits(digits, u64::from(u32::MAX))
            .map_err(|e| e.shift(Self::PREFIX.len()))
            .and_then(|v| {
                u32::try_from(v)
                    .ok()
                    .and_then(NonZeroU32::new)
                    .ok_or(ParseError::Overflow)
            })
            .map(Self)
    }

    /// Parses `https://www.wikidata.org/entity/Q182973`.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with exactly that
    /// URL; then as [`Self::from_prefixed`], offsets relative to the whole
    /// input.
    pub fn from_entity_url(s: &str) -> Result<Self, ParseError> {
        let item = strip_literal(s, Self::ENTITY_URL_PREFIX)?;
        Self::from_prefixed(item).map_err(|e| e.shift(Self::ENTITY_URL_PREFIX.len()))
    }
}

impl FromStr for Qid {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_prefixed(s)
    }
}

impl fmt::Display for Qid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::PREFIX, self.0)
    }
}

impl Serialize for Qid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self, self.get(), serializer)
    }
}

impl<'de> Deserialize<'de> for Qid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "Wikidata item",
            |n: u32| NonZeroU32::new(n).map(Self),
            |s| Self::from_prefixed(s).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, Qid};

    #[test]
    fn the_forms_crossref_writes() {
        let iowa = Qid::from_entity_url("https://www.wikidata.org/entity/Q182973").unwrap();
        assert_eq!(iowa, Qid::from_prefixed("Q182973").unwrap());
        assert_eq!(iowa.get(), 182_973);
        assert_eq!(iowa.to_string(), "Q182973");
        assert_eq!("Q182973".parse::<Qid>(), Ok(iowa));
        for s in ["Q1", "Q128137502", "Q4294967295"] {
            assert_eq!(Qid::from_prefixed(s).unwrap().to_string(), s);
        }
        assert_eq!(size_of::<Option<Qid>>(), 4);
    }

    #[test]
    fn rejections() {
        let cases: &[(&str, ParseError)] = &[
            ("Q0", ParseError::Zero),
            ("Q01", ParseError::LeadingZero),
            ("Q", ParseError::Empty),
            ("Q4294967296", ParseError::Overflow),
            ("q42", ParseError::BadPrefix),
            ("P31", ParseError::BadPrefix),
            ("L1", ParseError::BadPrefix),
            (" Q42", ParseError::BadPrefix),
            ("Q42 ", ParseError::NotDigit { offset: 3 }),
            ("Q-1", ParseError::NotDigit { offset: 1 }),
        ];
        for (s, e) in cases {
            assert_eq!(Qid::from_prefixed(s), Err(*e), "{s:?}");
        }
        let url_cases: &[(&str, ParseError)] = &[
            ("https://www.wikidata.org/wiki/Q42", ParseError::BadPrefix),
            ("http://www.wikidata.org/entity/Q42", ParseError::BadPrefix),
            ("https://wikidata.org/entity/Q42", ParseError::BadPrefix),
            ("Q42", ParseError::BadPrefix),
            (
                "https://www.wikidata.org/entity/Q42/",
                ParseError::NotDigit { offset: 35 },
            ),
        ];
        for (s, e) in url_cases {
            assert_eq!(Qid::from_entity_url(s), Err(*e), "{s:?}");
        }
    }
}
