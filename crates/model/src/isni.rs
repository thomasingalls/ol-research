//! ISNI identifiers.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::orcid::checksum;

/// An International Standard Name Identifier: sixteen characters, the
/// last a checksum (`X` for ten) over the first fifteen under ISO 7064
/// MOD 11-2 — the scheme ORCID borrowed. Stored as the fifteen-digit
/// number; the checksum is derived.
///
/// Crossref writes it as `https://www.isni.org/0000000404370279` on
/// affiliations (40 of them across 34,536 works looked up 2026-09-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Isni(u64);

/// Why a string is not an ISNI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Error {
    /// Not fifteen digits and a check character.
    Shape,
    /// The check character does not match the digits.
    Checksum,
    /// Not the expected URL prefix.
    BadPrefix,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Shape => "not fifteen digits and a check character",
            Self::Checksum => "check character does not match",
            Self::BadPrefix => "not https://www.isni.org/",
        })
    }
}

impl std::error::Error for Error {}

impl Isni {
    /// The prefix Crossref puts in front of an ISNI.
    pub const URL_PREFIX: &'static str = "https://www.isni.org/";

    /// Parses the bare sixteen characters, `0000000404370279`.
    ///
    /// # Errors
    ///
    /// [`Error::Shape`] or [`Error::Checksum`].
    pub fn from_bare(s: &str) -> Result<Self, Error> {
        let b = s.as_bytes();
        if b.len() != 16 || !b[..15].iter().all(u8::is_ascii_digit) {
            return Err(Error::Shape);
        }
        let check = match b[15] {
            b'X' => 10,
            c if c.is_ascii_digit() => c - b'0',
            _ => return Err(Error::Shape),
        };
        let base: u64 = s[..15].parse().map_err(|_| Error::Shape)?;
        if checksum(base) != check {
            return Err(Error::Checksum);
        }
        Ok(Self(base))
    }

    /// Parses `https://www.isni.org/0000000404370279`.
    ///
    /// # Errors
    ///
    /// [`Error::BadPrefix`], or as [`Isni::from_bare`].
    pub fn from_https_url(s: &str) -> Result<Self, Error> {
        s.strip_prefix(Self::URL_PREFIX)
            .ok_or(Error::BadPrefix)
            .and_then(Self::from_bare)
    }

    /// The fifteen-digit number without the check character.
    #[must_use]
    pub const fn base(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Isni {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:015}", self.0)?;
        match checksum(self.0) {
            10 => f.write_str("X"),
            check => write!(f, "{check}"),
        }
    }
}

impl Serialize for Isni {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::human::serialize(self, self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for Isni {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        crate::human::deserialize(
            deserializer,
            "ISNI",
            |n: u64| (n < 1_000_000_000_000_000).then_some(Self(n)),
            |s| Self::from_bare(s).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Isni};

    #[test]
    fn observed_forms_and_checksums() {
        let isni = Isni::from_https_url("https://www.isni.org/0000000404370279").unwrap();
        assert_eq!(isni.to_string(), "0000000404370279");
        assert_eq!(
            isni.base(),
            40_437_027,
            "the fifteen digits before the check"
        );
        for bare in ["0000000417903951", "0000000120973094"] {
            assert_eq!(Isni::from_bare(bare).unwrap().to_string(), bare);
        }
        assert_eq!(
            Isni::from_bare("0000000404370278"),
            Err(Error::Checksum),
            "one digit off"
        );
        assert_eq!(Isni::from_bare("000000040437027"), Err(Error::Shape));
        assert_eq!(
            Isni::from_https_url("https://isni.org/0000000404370279"),
            Err(Error::BadPrefix)
        );
        let json = serde_json::to_string(&isni).unwrap();
        assert_eq!(json, "\"0000000404370279\"");
        assert_eq!(serde_json::from_str::<Isni>(&json).unwrap(), isni);
        let bytes = postcard::to_allocvec(&isni).unwrap();
        assert_eq!(postcard::from_bytes::<Isni>(&bytes).unwrap(), isni);
    }
}
