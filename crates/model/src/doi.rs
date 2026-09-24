//! Digital Object Identifiers.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

/// A DOI, lowercased.
///
/// The rule (plan §4): `10.` then four to nine ASCII digits with no leading
/// zero (the registrant), `/`, then a non-empty suffix of printable ASCII
/// with no whitespace. DOIs are case-insensitive by definition, so the
/// canonical form is lowercase; the value is borrowed from the input unless
/// it contained uppercase, in which case a lowercased copy is made (about
/// 7 % of PubMed's DOIs).
///
/// Non-ASCII suffixes are permitted by the DOI Handbook and rejected here
/// until one is observed; that is a modelling decision waiting on evidence.
/// So far the only non-ASCII seen is a Unicode hyphen where an ASCII one
/// belongs, which is the data being wrong, not the rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Doi<'a>(#[serde(borrow)] Cow<'a, str>);

/// A registrant code: the digits between `10.` and `/`, `1001` in
/// `10.1001/jama.2025.15703`. Four to nine digits, no leading zero.
///
/// Also the type of Crossref's `prefix`, which names the DOI's *current
/// owner* and differs from the DOI's own registrant on 7 % of works, because
/// DOIs move between publishers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Registrant(u32);

/// Why a string is not a DOI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// Nothing to parse.
    Empty,
    /// Does not start with `10.`.
    BadPrefix,
    /// The registrant code is not four to nine digits without a leading zero.
    Registrant,
    /// No `/` after the registrant code.
    MissingSlash,
    /// Nothing after the `/`.
    EmptySuffix,
    /// A byte that is not printable ASCII, at this offset into the input.
    InvalidChar {
        /// Byte offset into the whole input.
        offset: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty"),
            Self::BadPrefix => f.write_str("does not start with 10."),
            Self::Registrant => {
                f.write_str("registrant code is not 4-9 digits without a leading zero")
            }
            Self::MissingSlash => f.write_str("no / after the registrant code"),
            Self::EmptySuffix => f.write_str("empty suffix"),
            Self::InvalidChar { offset } => {
                write!(f, "non-printable or non-ASCII byte at {offset}")
            }
        }
    }
}

impl std::error::Error for Error {}

impl Registrant {
    /// Parses a prefix, `10.1001`.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`] or [`Error::Registrant`].
    pub fn parse(s: &str) -> Result<Self, Error> {
        let digits = s.strip_prefix("10.").ok_or(Error::BadPrefix)?;
        Self::from_digits(digits)
    }

    fn from_digits(digits: &str) -> Result<Self, Error> {
        if !(4..=9).contains(&digits.len())
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || digits.starts_with('0')
        {
            return Err(Error::Registrant);
        }
        digits.parse().map(Self).map_err(|_| Error::Registrant)
    }

    /// The code.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// The registrant with code `code`, as [`Registrant::get`] gives it:
    /// four to nine digits, no leading zero.
    #[must_use]
    pub const fn from_code(code: u32) -> Option<Self> {
        if code >= 1_000 && code <= 999_999_999 {
            Some(Self(code))
        } else {
            None
        }
    }
}

impl fmt::Display for Registrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "10.{}", self.0)
    }
}

impl<'a> Doi<'a> {
    /// The prefix OpenAlex and others put in front of a DOI.
    pub const URL_PREFIX: &'static str = "https://doi.org/";

    /// Parses a bare DOI such as `10.1001/jama.2025.15703`.
    ///
    /// # Errors
    ///
    /// See [`Error`].
    pub fn parse(s: &'a str) -> Result<Self, Error> {
        if s.is_empty() {
            return Err(Error::Empty);
        }
        let rest = s.strip_prefix("10.").ok_or(Error::BadPrefix)?;
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        Registrant::from_digits(&rest[..digits])?;
        let suffix = rest[digits..]
            .strip_prefix('/')
            .ok_or(Error::MissingSlash)?;
        if suffix.is_empty() {
            return Err(Error::EmptySuffix);
        }
        let suffix_start = s.len() - suffix.len();
        if let Some(bad) = suffix.bytes().position(|b| !(0x21..=0x7e).contains(&b)) {
            return Err(Error::InvalidChar {
                offset: suffix_start + bad,
            });
        }
        Ok(Self(if suffix.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(s.to_ascii_lowercase())
        } else {
            Cow::Borrowed(s)
        }))
    }

    /// Parses the URL form, `https://doi.org/10.…`.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`] unless the input starts with exactly
    /// [`Self::URL_PREFIX`]; then as [`Self::parse`], with offsets relative
    /// to the whole input.
    pub fn from_url(s: &'a str) -> Result<Self, Error> {
        let rest = s.strip_prefix(Self::URL_PREFIX).ok_or(Error::BadPrefix)?;
        Self::parse(rest).map_err(|e| match e {
            Error::InvalidChar { offset } => Error::InvalidChar {
                offset: offset + Self::URL_PREFIX.len(),
            },
            other => other,
        })
    }

    /// The canonical, lowercased DOI.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The registrant code.
    #[must_use]
    pub fn registrant(&self) -> Registrant {
        let s = self.as_str();
        Registrant::from_digits(&s[3..s.find('/').unwrap_or(s.len())]).unwrap_or(Registrant(0))
    }

    /// True when no lowercasing copy was needed.
    #[must_use]
    pub const fn is_borrowed(&self) -> bool {
        matches!(self.0, Cow::Borrowed(_))
    }
}

impl fmt::Display for Doi<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{Doi, Error, Registrant};

    #[test]
    fn parses_observed_forms_borrowing_lowercase_ones() {
        for s in [
            "10.1001/jama.2025.15703",
            "10.1093/bioinformatics/btu170",
            "10.1038/s41586-021-03819-2",
            "10.82227/repository.uwtsd.ac.uk.00001267",
            "10.1016/s0140-6736(25)01234-5",
        ] {
            let doi = Doi::parse(s).unwrap();
            assert_eq!(doi.as_str(), s);
            assert!(doi.is_borrowed(), "{s}");
        }
        assert_eq!(
            Doi::parse("10.1001/jama.2025.15703")
                .unwrap()
                .registrant()
                .get(),
            1001
        );
        assert_eq!(Registrant::parse("10.1037").unwrap().to_string(), "10.1037");
        assert_eq!(Registrant::parse("10.0123"), Err(Error::Registrant));
        assert_eq!(Registrant::parse("1037"), Err(Error::BadPrefix));
    }

    #[test]
    fn uppercase_is_lowercased_by_copy() {
        let doi = Doi::parse("10.1097/MEG.0000000000003129").unwrap();
        assert_eq!(doi.as_str(), "10.1097/meg.0000000000003129");
        assert!(!doi.is_borrowed());
        assert_eq!(doi, Doi::parse("10.1097/meg.0000000000003129").unwrap());
    }

    #[test]
    fn rejects_what_sources_put_in_doi_slots() {
        assert_eq!(Doi::parse("DOI"), Err(Error::BadPrefix));
        assert_eq!(Doi::parse("e25.00642"), Err(Error::BadPrefix));
        assert_eq!(
            Doi::parse("10.10.1080/09638288.2024.2381616"),
            Err(Error::Registrant)
        );
        assert_eq!(
            Doi::parse("%9210.1046/j.1532-5415.2003.51185.x"),
            Err(Error::BadPrefix)
        );
        assert_eq!(
            Doi::parse("https://www.ajronline.org/doi/10.2214/AJR.07.2123"),
            Err(Error::BadPrefix)
        );
        // Crossref's Unicode hyphen: the data is wrong, not the rule.
        assert_eq!(
            Doi::parse("10.1111/1755\u{2010}0998.13324"),
            Err(Error::InvalidChar { offset: 12 })
        );
    }

    #[test]
    fn rule_edges() {
        assert_eq!(Doi::parse(""), Err(Error::Empty));
        assert_eq!(Doi::parse("10.123/x"), Err(Error::Registrant));
        assert_eq!(Doi::parse("10.0123/x"), Err(Error::Registrant));
        assert_eq!(Doi::parse("10.1234567890/x"), Err(Error::Registrant));
        assert_eq!(Doi::parse("10.1234"), Err(Error::MissingSlash));
        assert_eq!(Doi::parse("10.1234x/y"), Err(Error::MissingSlash));
        assert_eq!(Doi::parse("10.1234/"), Err(Error::EmptySuffix));
        assert_eq!(
            Doi::parse("10.1234/a b"),
            Err(Error::InvalidChar { offset: 9 })
        );
        assert_eq!(
            Doi::parse("10.1234/a\u{e9}"),
            Err(Error::InvalidChar { offset: 9 })
        );
        assert_eq!(Doi::parse(" 10.1234/a"), Err(Error::BadPrefix));
        assert!(
            Doi::parse("10.1234/a.").is_ok(),
            "trailing period is printable ASCII"
        );
    }

    #[test]
    fn url_form() {
        let doi = Doi::from_url("https://doi.org/10.25675/3.026947").unwrap();
        assert_eq!(doi.as_str(), "10.25675/3.026947");
        assert_eq!(
            Doi::from_url("http://doi.org/10.1/x"),
            Err(Error::BadPrefix)
        );
        assert_eq!(Doi::from_url("10.25675/3.026947"), Err(Error::BadPrefix));
        assert_eq!(
            Doi::from_url("https://doi.org/10.1234/a b"),
            Err(Error::InvalidChar { offset: 16 + 9 })
        );
    }
}
