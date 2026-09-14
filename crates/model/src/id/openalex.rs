use std::fmt;
use std::num::NonZeroU64;
use std::str::FromStr;

use super::{ParseError, parse_digits, strip_literal};

/// An OpenAlex work identifier — the `W` entity only.
///
/// Observed forms (plan §4a; `updated_date=2025-07-25/part_0000.gz` of the
/// 2026-06-26 snapshot):
///
/// * `id`, `ids.openalex`, `referenced_works[]`:
///   `https://openalex.org/W3029093293` — [`OpenAlexId::from_url`].
///   All 10,873 records in the part file use it; none are bare.
/// * Bare `W3029093293`, the API's short form — [`FromStr`].
///
/// The largest value in that file is `W4410348178`, above `u32::MAX`, so
/// this is a `u64`. `Option<OpenAlexId>` is eight bytes.
///
/// Other OpenAlex entities (`A` authors, `S` sources, `I` institutions, …)
/// are different types if and when a normalizer needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OpenAlexId(NonZeroU64);

impl OpenAlexId {
    /// The entity prefix of a work identifier.
    pub const ENTITY_PREFIX: &'static str = "W";
    /// The prefix OpenAlex puts in front of every identifier it emits.
    pub const URL_PREFIX: &'static str = "https://openalex.org/";

    /// Wraps a raw value; `None` for zero.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// The numeric value, without the `W`.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// Parses the bare `W…` form.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with an uppercase
    /// `W`; then anything other than a positive decimal with no sign, no
    /// leading zero and no surrounding whitespace, or a value above
    /// `u64::MAX`. Offsets are relative to the whole input.
    pub fn from_bare(s: &str) -> Result<Self, ParseError> {
        let digits = strip_literal(s, Self::ENTITY_PREFIX)?;
        parse_digits(digits, u64::MAX)
            .map_err(|e| e.shift(Self::ENTITY_PREFIX.len()))
            .and_then(|v| NonZeroU64::new(v).map(Self).ok_or(ParseError::Zero))
    }

    /// Parses the URL form the snapshot and API emit.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with exactly
    /// [`Self::URL_PREFIX`]; then as [`Self::from_bare`], with offsets
    /// relative to the whole input.
    pub fn from_url(s: &str) -> Result<Self, ParseError> {
        let rest = strip_literal(s, Self::URL_PREFIX)?;
        Self::from_bare(rest).map_err(|e| e.shift(Self::URL_PREFIX.len()))
    }
}

impl FromStr for OpenAlexId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_bare(s)
    }
}

impl fmt::Display for OpenAlexId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", Self::ENTITY_PREFIX, self.0)
    }
}

impl From<OpenAlexId> for u64 {
    fn from(id: OpenAlexId) -> Self {
        id.get()
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenAlexId, ParseError};

    /// First three work `id`s in the OpenAlex part file.
    const OBSERVED: [&str; 3] = [
        "https://openalex.org/W3029093293",
        "https://openalex.org/W3025147924",
        "https://openalex.org/W3025437713",
    ];
    /// The largest work `id` in the part file: above `u32::MAX`.
    const OBSERVED_MAX: &str = "https://openalex.org/W4410348178";

    #[test]
    fn parses_observed_url_form() {
        for s in OBSERVED {
            let id = OpenAlexId::from_url(s).unwrap();
            assert_eq!(format!("{}{id}", OpenAlexId::URL_PREFIX), s);
        }
    }

    #[test]
    fn observed_maximum_needs_u64() {
        let id = OpenAlexId::from_url(OBSERVED_MAX).unwrap();
        assert_eq!(id.get(), 4_410_348_178);
        assert!(id.get() > u64::from(u32::MAX));
    }

    #[test]
    fn parses_bare_form() {
        let id: OpenAlexId = "W3029093293".parse().unwrap();
        assert_eq!(id.to_string(), "W3029093293");
        assert_eq!(id, OpenAlexId::from_url(OBSERVED[0]).unwrap());
    }

    #[test]
    fn forms_are_not_interchangeable() {
        assert_eq!(
            OpenAlexId::from_bare(OBSERVED[0]),
            Err(ParseError::BadPrefix)
        );
        assert_eq!(OpenAlexId::from_url("W1"), Err(ParseError::BadPrefix));
    }

    #[test]
    fn entity_prefix_is_exact() {
        for s in ["", "w1", "A123", "S1", "1", " W1"] {
            assert_eq!(
                OpenAlexId::from_bare(s),
                Err(ParseError::BadPrefix),
                "{s:?}"
            );
        }
        assert_eq!(
            OpenAlexId::from_url("https://openalex.org/w1"),
            Err(ParseError::BadPrefix)
        );
        assert_eq!(
            OpenAlexId::from_url("http://openalex.org/W1"),
            Err(ParseError::BadPrefix)
        );
    }

    #[test]
    fn offsets_cover_the_whole_input() {
        assert_eq!(OpenAlexId::from_bare("W"), Err(ParseError::Empty));
        assert_eq!(OpenAlexId::from_bare("W0"), Err(ParseError::Zero));
        assert_eq!(OpenAlexId::from_bare("W01"), Err(ParseError::LeadingZero));
        assert_eq!(
            OpenAlexId::from_bare("W1 "),
            Err(ParseError::NotDigit { offset: 2 })
        );
        let p = OpenAlexId::URL_PREFIX;
        assert_eq!(
            OpenAlexId::from_url(&format!("{p}W1/")),
            Err(ParseError::NotDigit {
                offset: p.len() + 2
            })
        );
    }

    #[test]
    fn width_is_u64() {
        assert_eq!(
            OpenAlexId::from_bare("W18446744073709551615").map(OpenAlexId::get),
            Ok(u64::MAX)
        );
        assert_eq!(
            OpenAlexId::from_bare("W18446744073709551616"),
            Err(ParseError::Overflow)
        );
    }

    #[test]
    fn option_is_niche_packed() {
        assert_eq!(size_of::<Option<OpenAlexId>>(), 8);
        assert_eq!(size_of::<OpenAlexId>(), 8);
    }

    #[test]
    fn new_and_get_roundtrip() {
        assert_eq!(OpenAlexId::new(0), None);
        assert_eq!(
            OpenAlexId::new(4_410_348_178).map(u64::from),
            Some(4_410_348_178)
        );
    }
}
