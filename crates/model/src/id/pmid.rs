use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use super::{ParseError, parse_digits, strip_literal};

/// A PubMed identifier.
///
/// Observed forms (plan §4a; `pubmed26n1334.xml.gz` and the OpenAlex
/// snapshot of 2026-06-26):
///
/// * PubMed XML, `<PMID Version="1">41605286</PMID>`: bare digits —
///   [`Pmid::from_digits`], also [`FromStr`].
/// * OpenAlex `ids.pmid`, `https://pubmed.ncbi.nlm.nih.gov/41605286` —
///   [`Pmid::from_url`].
///
/// Positive and fits `u32`: the 2026 baseline tops out near 4.2 × 10^7,
/// a hundredfold below the limit. `Option<Pmid>` is four bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Pmid(NonZeroU32);

impl Pmid {
    /// The prefix OpenAlex puts in front of a PMID.
    pub const URL_PREFIX: &'static str = "https://pubmed.ncbi.nlm.nih.gov/";

    /// Wraps a raw value; `None` for zero.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(v) => Some(Self(v)),
            None => None,
        }
    }

    /// The numeric value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// Parses the bare-digits form emitted by PubMed XML.
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

    /// Parses the URL form used by OpenAlex.
    ///
    /// # Errors
    ///
    /// [`ParseError::BadPrefix`] unless the input starts with exactly
    /// [`Self::URL_PREFIX`]; then as [`Self::from_digits`], with offsets
    /// relative to the whole input.
    pub fn from_url(s: &str) -> Result<Self, ParseError> {
        let rest = strip_literal(s, Self::URL_PREFIX)?;
        Self::from_digits(rest).map_err(|e| e.shift(Self::URL_PREFIX.len()))
    }
}

impl FromStr for Pmid {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_digits(s)
    }
}

impl fmt::Display for Pmid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl From<Pmid> for u32 {
    fn from(pmid: Pmid) -> Self {
        pmid.get()
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseError, Pmid};

    /// First and last PMID in `pubmed26n1334.xml.gz`.
    const OBSERVED_XML: [&str; 2] = ["41605286", "41610285"];
    /// First three `ids.pmid` values in the OpenAlex part file.
    const OBSERVED_OPENALEX: [&str; 3] = [
        "https://pubmed.ncbi.nlm.nih.gov/30752826",
        "https://pubmed.ncbi.nlm.nih.gov/35773766",
        "https://pubmed.ncbi.nlm.nih.gov/37304899",
    ];

    #[test]
    fn parses_observed_xml_form() {
        for s in OBSERVED_XML {
            let pmid = Pmid::from_digits(s).unwrap();
            assert_eq!(pmid.to_string(), s);
            assert_eq!(s.parse::<Pmid>(), Ok(pmid));
        }
    }

    #[test]
    fn parses_observed_openalex_form() {
        for s in OBSERVED_OPENALEX {
            let pmid = Pmid::from_url(s).unwrap();
            assert_eq!(format!("{}{pmid}", Pmid::URL_PREFIX), s);
        }
    }

    #[test]
    fn forms_are_not_interchangeable() {
        assert_eq!(
            Pmid::from_digits(OBSERVED_OPENALEX[0]),
            Err(ParseError::NotDigit { offset: 0 })
        );
        assert_eq!(Pmid::from_url("30752826"), Err(ParseError::BadPrefix));
    }

    #[test]
    fn url_prefix_is_exact() {
        for s in [
            "http://pubmed.ncbi.nlm.nih.gov/1",
            "https://PubMed.ncbi.nlm.nih.gov/1",
            "https://www.ncbi.nlm.nih.gov/pubmed/1",
            "",
        ] {
            assert_eq!(Pmid::from_url(s), Err(ParseError::BadPrefix), "{s:?}");
        }
    }

    #[test]
    fn url_offsets_cover_the_whole_input() {
        let p = Pmid::URL_PREFIX;
        assert_eq!(Pmid::from_url(p), Err(ParseError::Empty));
        assert_eq!(
            Pmid::from_url(&format!("{p}1/")),
            Err(ParseError::NotDigit {
                offset: p.len() + 1
            })
        );
        assert_eq!(
            Pmid::from_url(&format!("{p}01")),
            Err(ParseError::LeadingZero)
        );
    }

    #[test]
    fn width_is_u32() {
        assert_eq!(Pmid::from_digits("4294967295").map(Pmid::get), Ok(u32::MAX));
        assert_eq!(Pmid::from_digits("4294967296"), Err(ParseError::Overflow));
        assert_eq!(Pmid::from_digits("0"), Err(ParseError::Zero));
    }

    #[test]
    fn option_is_niche_packed() {
        assert_eq!(size_of::<Option<Pmid>>(), 4);
        assert_eq!(size_of::<Pmid>(), 4);
    }

    #[test]
    fn new_and_get_roundtrip() {
        assert_eq!(Pmid::new(0), None);
        assert_eq!(Pmid::new(41_605_286).map(u32::from), Some(41_605_286));
    }
}
