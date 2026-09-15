//! ORCID identifiers.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// An ORCID iD: sixteen characters in four hyphenated groups, the last a
/// checksum digit (`X` for ten) over the first fifteen under ISO 7064
/// MOD 11-2. Stored as the fifteen-digit number; the checksum is derived.
///
/// Sources write it three ways: `https://orcid.org/0000-0002-3118-2156`
/// (Crossref, always), `http://orcid.org/…` and bare `0000-…` (PubMed, as
/// authors typed it). One entry point per form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Orcid(u64);

/// Why a string is not an ORCID iD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// Not `NNNN-NNNN-NNNN-NNNC`.
    Shape,
    /// The check character does not match the digits.
    Checksum,
    /// Not the expected URL prefix.
    BadPrefix,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Shape => "not four hyphenated groups of four",
            Self::Checksum => "checksum does not match",
            Self::BadPrefix => "missing or wrong URL prefix",
        })
    }
}

impl std::error::Error for Error {}

/// ISO 7064 MOD 11-2 over the fifteen digits: `0..=10`, ten meaning `X`.
/// ISNI uses the same scheme; [`crate::isni`] shares it.
pub(crate) fn checksum(mut base: u64) -> u8 {
    let mut digits = [0u8; 15];
    for slot in digits.iter_mut().rev() {
        *slot = u8::try_from(base % 10).unwrap_or(0);
        base /= 10;
    }
    let total = digits.iter().fold(0u32, |t, &d| (t + u32::from(d)) * 2);
    u8::try_from((12 - total % 11) % 11).unwrap_or(0)
}

impl Orcid {
    /// The prefix Crossref writes.
    pub const HTTPS_PREFIX: &'static str = "https://orcid.org/";
    /// The prefix some PubMed records write.
    pub const HTTP_PREFIX: &'static str = "http://orcid.org/";

    /// Parses the bare hyphenated form, `0000-0002-3118-2156`.
    ///
    /// # Errors
    ///
    /// [`Error::Shape`] or [`Error::Checksum`].
    pub fn from_hyphenated(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        if b.len() != 19 || b[4] != b'-' || b[9] != b'-' || b[14] != b'-' {
            return Err(Error::Shape);
        }
        let mut base = 0u64;
        for (i, &c) in b.iter().enumerate() {
            if matches!(i, 4 | 9 | 14) {
                continue;
            }
            if i == 18 {
                let check = match c {
                    b'X' => 10,
                    d if d.is_ascii_digit() => d - b'0',
                    _ => return Err(Error::Shape),
                };
                if check != checksum(base) {
                    return Err(Error::Checksum);
                }
            } else if c.is_ascii_digit() {
                base = base * 10 + u64::from(c - b'0');
            } else {
                return Err(Error::Shape);
            }
        }
        Ok(Self(base))
    }

    /// Parses the `https://orcid.org/…` form.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`], then as [`Self::from_hyphenated`].
    pub fn from_https_url(s: &str) -> Result<Self, Error> {
        let rest = s.strip_prefix(Self::HTTPS_PREFIX).ok_or(Error::BadPrefix)?;
        Self::from_hyphenated(rest)
    }

    /// Parses the `http://orcid.org/…` form.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`], then as [`Self::from_hyphenated`].
    pub fn from_http_url(s: &str) -> Result<Self, Error> {
        let rest = s.strip_prefix(Self::HTTP_PREFIX).ok_or(Error::BadPrefix)?;
        Self::from_hyphenated(rest)
    }

    /// Parses whichever of the three forms a free-text field holds:
    /// `https://orcid.org/…`, `http://orcid.org/…` or bare. PubMed's
    /// `Identifier Source="ORCID"` carries all three, as authors typed them.
    ///
    /// # Errors
    ///
    /// As [`Self::from_hyphenated`] after any prefix is removed.
    pub fn from_url_or_bare(s: &str) -> Result<Self, Error> {
        let rest = s
            .strip_prefix(Self::HTTPS_PREFIX)
            .or_else(|| s.strip_prefix(Self::HTTP_PREFIX))
            .unwrap_or(s);
        Self::from_hyphenated(rest)
    }

    /// The fifteen-digit number without the check character.
    #[must_use]
    pub const fn base(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Orcid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!("{:015}", self.0);
        let check = checksum(self.0);
        write!(f, "{}-{}-{}-{}", &s[..4], &s[4..8], &s[8..12], &s[12..15])?;
        if check == 10 {
            f.write_str("X")
        } else {
            write!(f, "{check}")
        }
    }
}

impl Serialize for Orcid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self, self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Orcid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "ORCID",
            |n: u64| (n < 1_000_000_000_000_000).then_some(Self(n)),
            |s| Self::from_hyphenated(s).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Orcid};

    #[test]
    fn observed_forms_and_checksums() {
        for s in [
            "0000-0002-3118-2156",
            "0000-0002-7803-9664",
            "0000-0001-6538-9406",
            "0000-0002-1825-0097",
        ] {
            let o = Orcid::from_hyphenated(s).unwrap();
            assert_eq!(o.to_string(), s);
        }
        // The canonical X example from the ORCID documentation.
        let o = Orcid::from_hyphenated("0000-0002-1694-233X").unwrap();
        assert_eq!(o.to_string(), "0000-0002-1694-233X");
        assert_eq!(
            Orcid::from_https_url("https://orcid.org/0000-0002-3118-2156"),
            Orcid::from_hyphenated("0000-0002-3118-2156")
        );
        assert_eq!(
            Orcid::from_http_url("http://orcid.org/0000-0002-3118-2156"),
            Orcid::from_hyphenated("0000-0002-3118-2156")
        );
    }

    #[test]
    fn rejections() {
        assert_eq!(
            Orcid::from_hyphenated("0000-0002-3118-2157"),
            Err(Error::Checksum)
        );
        assert_eq!(
            Orcid::from_hyphenated("0000000231182156"),
            Err(Error::Shape)
        );
        assert_eq!(
            Orcid::from_hyphenated("0000-0002-3118-215"),
            Err(Error::Shape)
        );
        assert_eq!(
            Orcid::from_hyphenated("0000-0002-3118-215x"),
            Err(Error::Shape)
        );
        assert_eq!(
            Orcid::from_https_url("http://orcid.org/0000-0002-3118-2156"),
            Err(Error::BadPrefix)
        );
        assert_eq!(
            Orcid::from_http_url("https://orcid.org/0000-0002-3118-2156"),
            Err(Error::BadPrefix)
        );
        assert_eq!(
            Orcid::from_https_url("0000-0002-3118-2156"),
            Err(Error::BadPrefix)
        );
    }

    #[test]
    fn free_text_fields_carry_three_forms() {
        for form in [
            "https://orcid.org/0000-0002-3118-2156",
            "http://orcid.org/0000-0002-3118-2156",
            "0000-0002-3118-2156",
        ] {
            assert_eq!(
                Orcid::from_url_or_bare(form),
                Orcid::from_hyphenated("0000-0002-3118-2156"),
                "{form}"
            );
        }
        // PubMed's truncated ones.
        assert_eq!(
            Orcid::from_url_or_bare("0000-0002-7368-498"),
            Err(Error::Shape)
        );
    }
}
